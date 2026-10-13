//! Command-line actions. A second launch (or a jump-list task) passes them to the running instance.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliAction {
    PlayPause,
    Play,
    Pause,
    Next,
    Previous,
    Mini,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Cli {
    pub actions: Vec<CliAction>,
    /// Started by Windows at sign-in (so it may start in the tray).
    pub autostart: bool,
}

/// `args[0]` is the program. Unknown arguments are ignored so a newer shortcut never breaks an older build.
pub fn parse_args<S: AsRef<str>>(args: &[S]) -> Cli {
    let mut cli = Cli::default();
    for a in args.iter().skip(1) {
        match a.as_ref() {
            "--toggle" | "--play-pause" => cli.actions.push(CliAction::PlayPause),
            "--play" => cli.actions.push(CliAction::Play),
            "--pause" => cli.actions.push(CliAction::Pause),
            "--next" => cli.actions.push(CliAction::Next),
            "--prev" | "--previous" => cli.actions.push(CliAction::Previous),
            "--mini" => cli.actions.push(CliAction::Mini),
            "--autostart" => cli.autostart = true,
            _ => {}
        }
    }
    cli
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_are_read_in_order_and_the_program_name_is_skipped() {
        let c = parse_args(&["pyxee.exe", "--next", "--toggle", "--prev"]);
        assert_eq!(
            c.actions,
            vec![CliAction::Next, CliAction::PlayPause, CliAction::Previous]
        );
        assert!(!c.autostart);
    }

    #[test]
    fn the_autostart_flag_is_recognised_and_unknown_arguments_ignored() {
        let c = parse_args(&[
            "pyxee.exe",
            "--autostart",
            "--future-thing",
            "C:\\music\\a.mp3",
            "--mini",
        ]);
        assert!(c.autostart);
        assert_eq!(c.actions, vec![CliAction::Mini]);
        assert_eq!(parse_args::<&str>(&[]), Cli::default());
        assert_eq!(parse_args(&["pyxee.exe"]), Cli::default());
    }

    #[test]
    fn the_program_name_is_never_taken_for_a_flag() {
        assert!(parse_args(&["--next"]).actions.is_empty());
    }
}
