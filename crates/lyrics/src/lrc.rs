use crate::Line;

/// Parses LRC text. Understands `[mm:ss.xx]`, `[mm:ss.xxx]`, `[mm:ss]` and `[m:ss:xx]`, several timestamps in front
/// of one line, the `[offset:±ms]` tag (positive = lyrics come earlier, per the LRC convention), and ignores other
/// tags (`[ar:..]`, `[ti:..]`) and enhanced per-word `<mm:ss.xx>` marks. The result is sorted by time.
pub fn parse_lrc(text: &str) -> Vec<Line> {
    let mut offset: i64 = 0;
    let mut out: Vec<Line> = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim_start_matches('\u{feff}').trim();
        let mut stamps: Vec<u64> = Vec::new();
        while let Some(stripped) = rest.strip_prefix('[') {
            let Some(end) = stripped.find(']') else { break };
            let tag = &stripped[..end];
            if let Some(ms) = timestamp_ms(tag) {
                stamps.push(ms);
            } else if let Some(v) = tag.strip_prefix("offset:") {
                offset = v.trim().parse().unwrap_or(0);
            } else if !is_tag(tag) {
                break; // "[Chorus]" style text, not a tag: it belongs to the line
            }
            rest = stripped[end + 1..].trim_start();
        }
        if stamps.is_empty() {
            continue;
        }
        let words = strip_word_marks(rest);
        for ms in stamps {
            // Saturating: the text comes from a web service, and a huge timestamp or offset must not wrap around.
            let t = i64::try_from(ms)
                .unwrap_or(i64::MAX)
                .saturating_sub(offset)
                .max(0) as u64;
            out.push(Line {
                time_ms: t,
                text: words.clone(),
            });
        }
    }
    out.sort_by_key(|l| l.time_ms); // stable: equal times keep file order
    out
}

fn is_tag(tag: &str) -> bool {
    matches!(tag.split_once(':'), Some((k, _)) if k.len() <= 8 && k.chars().all(|c| c.is_ascii_alphabetic()))
}

/// `mm:ss`, `mm:ss.f`, `mm:ss.ff`, `mm:ss.fff` or `mm:ss:ff` (minutes may exceed 59).
fn timestamp_ms(tag: &str) -> Option<u64> {
    let (min, rest) = tag.split_once(':')?;
    let min: u64 = min.trim().parse().ok().filter(|_| !min.trim().is_empty())?;
    let (sec, frac) = match rest.find(['.', ':']) {
        Some(i) => (&rest[..i], Some(&rest[i + 1..])),
        None => (rest, None),
    };
    let sec: u64 = sec.trim().parse().ok().filter(|s| *s < 60)?;
    let frac_ms = match frac {
        None => 0,
        Some(f) => {
            if f.is_empty() || f.len() > 3 || !f.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let n: u64 = f.parse().ok()?;
            match f.len() {
                1 => n * 100,
                2 => n * 10,
                _ => n,
            }
        }
    };
    // Checked: a timestamp too large to be a time is not a timestamp (it is then treated as text, like any other tag).
    min.checked_mul(60_000)?.checked_add(sec * 1000 + frac_ms)
}

/// Removes enhanced-LRC word marks such as `<00:12.34>` and squeezes the spaces they leave.
fn strip_word_marks(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<' {
            let mut mark = String::new();
            let mut closed = false;
            for n in chars.by_ref() {
                if n == '>' {
                    closed = true;
                    break;
                }
                mark.push(n);
            }
            if closed && timestamp_ms(&mark).is_some() {
                continue;
            }
            out.push('<');
            out.push_str(&mark);
            if closed {
                out.push('>');
            }
        } else {
            out.push(c);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Index of the line being sung at `pos_ms`: the last line whose time has passed; `None` before the first line.
pub fn current_index(lines: &[Line], pos_ms: u64) -> Option<usize> {
    lines
        .partition_point(|l| l.time_ms <= pos_ms)
        .checked_sub(1)
}

/// A title as lyric services want it: no "(Official Video)", "[Lyrics]", "(feat. X)" or trailing " - Topic".
pub fn clean_title(title: &str) -> String {
    let mut s = title.trim().to_string();
    // Drop bracketed noise, but keep brackets that carry the name ("(Interlude)" stays).
    const NOISE: [&str; 12] = [
        "official",
        "lyric",
        "lyrics",
        "video",
        "audio",
        "visualizer",
        "hd",
        "4k",
        "remaster",
        "feat",
        "ft.",
        "with ",
    ];
    loop {
        let Some(open) = s.rfind(['(', '[']) else {
            break;
        };
        let close = if s[open..].starts_with('(') { ')' } else { ']' };
        let Some(end_rel) = s[open..].find(close) else {
            break;
        };
        let inner = s[open + 1..open + end_rel].to_lowercase();
        if end_rel + open + 1 == s.len() && NOISE.iter().any(|n| inner.contains(n)) {
            s.truncate(open);
            s = s.trim_end().to_string();
        } else {
            break;
        }
    }
    s = s.trim_end_matches(" - Topic").trim().to_string();
    s
}

/// The first artist of "A, B" / "A & B" / "A feat. B", without " - Topic" and " VEVO".
pub fn clean_artist(artist: &str) -> String {
    let a = artist.trim().trim_end_matches(" - Topic");
    let a = a.strip_suffix("VEVO").map_or(a, str::trim_end);
    let lower = a.to_lowercase();
    let cut = [", ", " & ", " feat. ", " feat ", " ft. ", " x ", " and "]
        .iter()
        .filter_map(|sep| lower.find(sep))
        .min()
        .unwrap_or(a.len());
    a[..cut].trim().to_string()
}

#[cfg(test)]
mod hostile_input {
    use super::*;

    #[test]
    fn absurd_minutes_do_not_overflow() {
        let lines =
            parse_lrc("[99999999999999999999:00.00]x\n[307445734561825861:00.00]y\n[00:01.00]ok");
        assert!(lines.iter().any(|l| l.text == "ok"));
    }

    #[test]
    fn an_extreme_offset_does_not_overflow() {
        let lines = parse_lrc(
            "[offset:-9223372036854775808]\n[00:01.00]a\n[offset:9223372036854775807]\n[00:02.00]b",
        );
        assert_eq!(lines.len(), 2);
    }
}
