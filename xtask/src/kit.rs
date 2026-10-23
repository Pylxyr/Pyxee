//! The FFmpeg build kit: the release asset that lets anyone relink Pyxee with a different or modified FFmpeg
//! (LGPL v2.1+ asks for this because libav is linked statically). `cargo xtask ffmpeg-kit` writes it after the
//! installer build; its contents and the relink steps are described in crates/app/licenses/THIRD_PARTY.txt and
//! docs/windows-checklist.md.

use std::path::{Path, PathBuf};

type Res<T> = Result<T, String>;

/// The libraries the audio crate links, as named in FFmpeg's `lib/` (`avformat.lib` with MSVC, `libavformat.a`
/// elsewhere).
pub const LIBRARIES: [&str; 4] = ["avformat", "avcodec", "swresample", "avutil"];

/// The pinned FFmpeg, read from scripts/build-ffmpeg.sh so there is one place that names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    pub tag: String,
    pub commit: String,
    pub url: String,
}

fn assigned(script: &str, name: &str) -> Res<String> {
    let prefix = format!("{name}=\"");
    script
        .lines()
        .find_map(|l| l.trim().strip_prefix(&prefix)?.strip_suffix('"'))
        .map(str::to_string)
        .ok_or_else(|| format!("{name} is not set in scripts/build-ffmpeg.sh"))
}

pub fn pin(script: &str) -> Res<Pin> {
    let pin = Pin {
        tag: assigned(script, "FFMPEG_TAG")?,
        commit: assigned(script, "FFMPEG_COMMIT")?,
        url: assigned(script, "FFMPEG_URL")?,
    };
    if pin.commit.len() != 40 || !pin.commit.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "FFMPEG_COMMIT is not a full commit hash: {}",
            pin.commit
        ));
    }
    Ok(pin)
}

pub fn kit_folder_name(version: &str) -> String {
    format!("Pyxee_{version}_ffmpeg-build-kit")
}

pub fn kit_zip_name(version: &str) -> String {
    format!("{}.zip", kit_folder_name(version))
}

pub fn source_archive_name(pin: &Pin) -> String {
    format!("FFmpeg-{}.tar.gz", pin.commit)
}

/// The kit's README: what is in it and how to relink.
pub fn readme(version: &str, pin: &Pin) -> String {
    let archive = source_archive_name(pin);
    format!(
        "Pyxee {version}: FFmpeg build kit (material for relinking)\r\n\
\r\n\
Pyxee links FFmpeg's libavformat, libavcodec, libswresample and libavutil statically. They are licensed under the\r\n\
LGPL v2.1 or later (FFmpeg-LGPL-2.1.txt). This kit lets you build Pyxee with a modified or different FFmpeg, or check\r\n\
that the libraries in the installer are the ones built from the pinned, unmodified source.\r\n\
\r\n\
Contents\r\n\
  prefix/lib/                      the exact static libraries linked into Pyxee {version} (Windows x64, MSVC)\r\n\
  prefix/include/                  their headers\r\n\
  prefix/pyxee-ffmpeg.stamp        hash of the commit and configure options that built them\r\n\
  ffmpeg-source/{archive}\r\n\
                                   FFmpeg source, unmodified: tag {tag}, commit {commit}\r\n\
  build-inputs/build-ffmpeg.sh     the pinned commit, the configure options and the build steps\r\n\
  build-inputs/ffmpeg-action.yml   how the release workflow runs that script (MSYS2 packages, MSVC environment)\r\n\
  FFmpeg-LGPL-2.1.txt              the licence\r\n\
\r\n\
To relink Pyxee with your own FFmpeg\r\n\
  1. Get Pyxee's source for this version (MIT licensed): https://github.com/Pylxyr/Pyxee, tag v{version}.\r\n\
  2. Provide your libraries in the same layout as prefix/ (a lib folder with the four libraries under the same names,\r\n\
     and an include folder). Either copy your own builds over prefix/lib, or build them from modified source: unpack\r\n\
     the source above, change it, and build it with the configure options in build-inputs/build-ffmpeg.sh\r\n\
     (`cargo xtask ffmpeg` in the Pyxee tree does this for the pinned commit).\r\n\
  3. Set the environment variable PYXEE_FFMPEG_DIR to the full path of that prefix folder, then run\r\n\
     `cargo xtask installer` (or `cargo xtask portable`) in the Pyxee tree. That links Pyxee against your libraries.\r\n\
     It needs Windows, Rust, Node and the Visual Studio C++ build tools (docs/development.md in the source).\r\n\
\r\n\
Source of FFmpeg: {url} (tag {tag}, commit {commit}).\r\n",
        tag = pin.tag,
        commit = pin.commit,
        url = pin.url,
    )
}

/// The files of an FFmpeg install prefix that belong in the kit, as (name inside `prefix/`, path on disk): the four
/// static libraries, every header and the stamp. Errors when a library is missing, so a broken build is not shipped.
pub fn prefix_files(prefix: &Path) -> Res<Vec<(String, PathBuf)>> {
    fn walk(
        dir: &Path,
        rel: &str,
        keep: &dyn Fn(&Path) -> bool,
        out: &mut Vec<(String, PathBuf)>,
    ) -> Res<()> {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| format!("cannot read {}: {e}", dir.display()))?
            .filter_map(Result::ok)
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let name = format!("{rel}/{}", e.file_name().to_string_lossy());
            let path = e.path();
            if path.is_dir() {
                walk(&path, &name, keep, out)?;
            } else if keep(&path) {
                out.push((name, path));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    let is_static_lib =
        |p: &Path| matches!(p.extension().and_then(|e| e.to_str()), Some("lib" | "a"));
    walk(&prefix.join("lib"), "lib", &is_static_lib, &mut files)?;
    for lib in LIBRARIES {
        let found = files.iter().any(|(n, _)| {
            let file = n.rsplit('/').next().unwrap_or(n);
            file == format!("{lib}.lib") || file == format!("lib{lib}.a")
        });
        if !found {
            return Err(format!(
                "{lib} (as {lib}.lib or lib{lib}.a) is not in {}; run `cargo xtask ffmpeg` first",
                prefix.join("lib").display()
            ));
        }
    }
    if prefix.join("include").is_dir() {
        walk(&prefix.join("include"), "include", &|_| true, &mut files)?;
    }
    if !files
        .iter()
        .any(|(n, _)| n == "include/libavformat/avformat.h")
    {
        return Err(format!(
            "{} has no libavformat headers",
            prefix.join("include").display()
        ));
    }
    let stamp = prefix.join("pyxee-ffmpeg.stamp");
    if stamp.is_file() {
        files.push(("pyxee-ffmpeg.stamp".into(), stamp));
    }
    Ok(files)
}

/// Where each kit file goes inside the zip's top folder. `fixed` are the files that are not part of the prefix:
/// `(name in the zip, bytes)`.
pub fn kit_names(prefix_names: &[String], pin: &Pin) -> Vec<String> {
    let mut names = vec!["README.txt".to_string(), "FFmpeg-LGPL-2.1.txt".into()];
    names.push(format!("ffmpeg-source/{}", source_archive_name(pin)));
    names.push("build-inputs/build-ffmpeg.sh".into());
    names.push("build-inputs/ffmpeg-action.yml".into());
    names.extend(prefix_names.iter().map(|n| format!("prefix/{n}")));
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCRIPT: &str = "set -e\nFFMPEG_TAG=\"n8.1.3\"\nFFMPEG_COMMIT=\"1041abdc962f4cc4f394aa8de9dc5236c0c3b9e7\"\nFFMPEG_URL=\"https://github.com/FFmpeg/FFmpeg.git\"\n";

    fn the_pin() -> Pin {
        pin(SCRIPT).unwrap()
    }

    #[test]
    fn the_pin_is_read_from_the_build_script() {
        let p = the_pin();
        assert_eq!(p.tag, "n8.1.3");
        assert_eq!(p.commit, "1041abdc962f4cc4f394aa8de9dc5236c0c3b9e7");
        assert_eq!(p.url, "https://github.com/FFmpeg/FFmpeg.git");
        let real = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../scripts/build-ffmpeg.sh"
        ))
        .unwrap();
        assert!(pin(&real).is_ok(), "the real script must parse");
    }

    #[test]
    fn a_missing_or_short_pin_is_an_error() {
        assert!(pin("FFMPEG_TAG=\"x\"").is_err());
        assert!(
            pin(&SCRIPT.replace("1041abdc962f4cc4f394aa8de9dc5236c0c3b9e7", "1041abd")).is_err()
        );
    }

    #[test]
    fn names_carry_the_version_and_the_commit() {
        assert_eq!(kit_zip_name("0.3.10"), "Pyxee_0.3.10_ffmpeg-build-kit.zip");
        assert_eq!(
            source_archive_name(&the_pin()),
            "FFmpeg-1041abdc962f4cc4f394aa8de9dc5236c0c3b9e7.tar.gz"
        );
    }

    #[test]
    fn the_readme_says_what_the_kit_holds_and_how_to_relink() {
        let r = readme("0.3.10", &the_pin());
        for needed in [
            "0.3.10",
            "n8.1.3",
            "1041abdc962f4cc4f394aa8de9dc5236c0c3b9e7",
            "prefix/lib/",
            "build-inputs/build-ffmpeg.sh",
            "PYXEE_FFMPEG_DIR",
            "cargo xtask installer",
            "LGPL",
            "tag v0.3.10",
        ] {
            assert!(r.contains(needed), "{needed}");
        }
        assert!(r.contains("\r\n"), "Windows line endings for Notepad");
    }

    fn fake_prefix(tag: &str, libs: &[&str], with_headers: bool) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pyxee-kit-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("lib/pkgconfig")).unwrap();
        for l in libs {
            std::fs::write(dir.join("lib").join(l), b"lib").unwrap();
        }
        std::fs::write(dir.join("lib/pkgconfig/libavutil.pc"), b"pc").unwrap();
        if with_headers {
            std::fs::create_dir_all(dir.join("include/libavformat")).unwrap();
            std::fs::write(dir.join("include/libavformat/avformat.h"), b"h").unwrap();
        }
        std::fs::write(dir.join("pyxee-ffmpeg.stamp"), b"s").unwrap();
        dir
    }

    #[test]
    fn the_four_libraries_the_headers_and_the_stamp_are_collected() {
        let dir = fake_prefix(
            "collect",
            &[
                "avformat.lib",
                "avcodec.lib",
                "swresample.lib",
                "avutil.lib",
            ],
            true,
        );
        let names: Vec<String> = prefix_files(&dir)
            .unwrap()
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        assert_eq!(
            names,
            [
                "lib/avcodec.lib",
                "lib/avformat.lib",
                "lib/avutil.lib",
                "lib/swresample.lib",
                "include/libavformat/avformat.h",
                "pyxee-ffmpeg.stamp"
            ]
        );
        assert!(
            !names.iter().any(|n| n.contains("pkgconfig")),
            "only static libraries from lib/"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unix_style_names_work_too() {
        let dir = fake_prefix(
            "unix",
            &[
                "libavformat.a",
                "libavcodec.a",
                "libswresample.a",
                "libavutil.a",
            ],
            true,
        );
        assert!(prefix_files(&dir).is_ok());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_missing_library_or_header_stops_the_kit() {
        let dir = fake_prefix(
            "nolib",
            &["avformat.lib", "avcodec.lib", "avutil.lib"],
            true,
        );
        assert!(prefix_files(&dir).unwrap_err().contains("swresample"));
        let _ = std::fs::remove_dir_all(dir);
        let dir = fake_prefix(
            "nohdr",
            &[
                "avformat.lib",
                "avcodec.lib",
                "swresample.lib",
                "avutil.lib",
            ],
            false,
        );
        assert!(prefix_files(&dir).unwrap_err().contains("headers"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_kit_lists_everything_the_readme_promises() {
        let names = kit_names(&["lib/avutil.lib".into()], &the_pin());
        for n in [
            "README.txt",
            "FFmpeg-LGPL-2.1.txt",
            "build-inputs/build-ffmpeg.sh",
            "build-inputs/ffmpeg-action.yml",
            "ffmpeg-source/FFmpeg-1041abdc962f4cc4f394aa8de9dc5236c0c3b9e7.tar.gz",
            "prefix/lib/avutil.lib",
        ] {
            assert!(names.iter().any(|x| x == n), "{n}");
        }
    }
}
