//! Hands memory back to Windows the moment the page has gone to sleep.
//!
//! `webview_power` tells WebView2 the window is hidden and suspends the page. WebView2 then gives its memory back on
//! its own schedule: measured on one PC, the page's renderer fell within a second, but the GPU process (about 60 MB)
//! and the manager (about 24 MB) held theirs for about 30 s. This empties the *working set* of Pyxee's own process
//! and of its WebView2 processes (`msedgewebview2.exe` children of this process) as soon as the suspend is done,
//! and once more a moment later for what was allocated while it settled.
//!
//! What it does and does not do: emptying a working set moves the pages out of the process's resident set into
//! Windows' standby list. Task Manager's Memory column (the private working set) drops at once, and Windows can
//! reuse that memory for anything else straight away. Nothing is freed or lost: if the window is shown again soon the
//! pages are taken back from the standby list (a quick soft fault, no disk), so the first redraw can be a little
//! slower. It is only done for a hidden window, never while one is in view.

use std::time::Duration;

/// One running process, as much of it as picking the family needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Proc {
    pub pid: u32,
    pub parent: u32,
    pub exe: String,
}

fn is_webview_exe(exe: &str) -> bool {
    exe.eq_ignore_ascii_case("msedgewebview2.exe")
}

/// `own` and every `msedgewebview2.exe` process below it (the manager is a child of this process, the renderer, GPU
/// and utility processes are children of the manager). Other processes are never touched.
pub(crate) fn family(own: u32, all: &[Proc]) -> Vec<u32> {
    let mut out = vec![own];
    let mut i = 0;
    while i < out.len() {
        let parent = out[i];
        for p in all {
            if p.parent == parent
                && p.pid != parent
                && !out.contains(&p.pid)
                && is_webview_exe(&p.exe)
            {
                out.push(p.pid);
            }
        }
        i += 1;
    }
    out
}

/// How long after the first trim the second one runs.
const SECOND_PASS: Duration = Duration::from_millis(1500);

/// Trims now, then once more after a moment, on a thread of its own. `still_wanted` says whether the page is still
/// asleep (a window shown meanwhile is left alone).
pub fn trim_after_sleep(label: String, still_wanted: Box<dyn Fn() -> bool + Send>) {
    let spawned = std::thread::Builder::new()
        .name("trim-working-set".into())
        .spawn(move || {
            if !still_wanted() {
                return;
            }
            let first = trim_family();
            std::thread::sleep(SECOND_PASS);
            let second = if still_wanted() { trim_family() } else { 0 };
            let done = format!(
                "{label}: trimmed the working set of {first} processes, then of {second} more"
            );
            log::debug!("{done}");
            crate::commands::perf_note(&done);
        });
    if let Err(e) = spawned {
        log::debug!("could not start the working-set trim: {e}");
    }
}

#[cfg(windows)]
fn trim_family() -> usize {
    sys::trim_family()
}

#[cfg(not(windows))]
fn trim_family() -> usize {
    0
}

#[cfg(windows)]
mod sys {
    use super::{family, Proc};
    use std::ffi::c_void;

    type Handle = *mut c_void;

    const TH32CS_SNAPPROCESS: u32 = 0x2;
    const PROCESS_SET_QUOTA: u32 = 0x0100;
    const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
    const INVALID_HANDLE_VALUE: isize = -1;

    /// `PROCESSENTRY32W` from tlhelp32.h.
    #[repr(C)]
    struct ProcessEntry32W {
        size: u32,
        usage: u32,
        process_id: u32,
        default_heap_id: usize,
        module_id: u32,
        threads: u32,
        parent_process_id: u32,
        pri_class_base: i32,
        flags: u32,
        exe_file: [u16; 260],
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> Handle;
        fn Process32FirstW(snapshot: Handle, entry: *mut ProcessEntry32W) -> i32;
        fn Process32NextW(snapshot: Handle, entry: *mut ProcessEntry32W) -> i32;
        fn OpenProcess(access: u32, inherit: i32, process_id: u32) -> Handle;
        fn CloseHandle(handle: Handle) -> i32;
        fn K32EmptyWorkingSet(process: Handle) -> i32;
    }

    /// Every running process (pid, parent and exe name).
    fn processes() -> Vec<Proc> {
        let mut found = Vec::new();
        // SAFETY: plain Win32 calls; the snapshot handle is closed below, the entry is a correctly sized, zeroed
        // PROCESSENTRY32W whose `size` is set as the API requires.
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot as isize == INVALID_HANDLE_VALUE || snapshot.is_null() {
                return found;
            }
            let mut entry: ProcessEntry32W = std::mem::zeroed();
            entry.size = std::mem::size_of::<ProcessEntry32W>() as u32;
            let mut more = Process32FirstW(snapshot, &mut entry) != 0;
            while more {
                let len = entry
                    .exe_file
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.exe_file.len());
                found.push(Proc {
                    pid: entry.process_id,
                    parent: entry.parent_process_id,
                    exe: String::from_utf16_lossy(&entry.exe_file[..len]),
                });
                more = Process32NextW(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
        }
        found
    }

    /// Empties the working set of every process of the family; returns how many it worked on.
    pub fn trim_family() -> usize {
        let own = std::process::id();
        let mut done = 0;
        for pid in family(own, &processes()) {
            // SAFETY: the handle is opened for this call only and closed again; a process that cannot be opened (it
            // has just exited, or belongs to someone else) is skipped.
            unsafe {
                let handle = OpenProcess(PROCESS_SET_QUOTA | PROCESS_QUERY_INFORMATION, 0, pid);
                if handle.is_null() {
                    continue;
                }
                if K32EmptyWorkingSet(handle) != 0 {
                    done += 1;
                }
                CloseHandle(handle);
            }
        }
        done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pid: u32, parent: u32, exe: &str) -> Proc {
        Proc {
            pid,
            parent,
            exe: exe.to_string(),
        }
    }

    #[test]
    fn takes_this_process_and_the_webview_tree_below_it() {
        let all = [
            p(1, 0, "System"),
            p(100, 1, "explorer.exe"),
            p(200, 100, "pyxee-app.exe"),
            p(210, 200, "msedgewebview2.exe"), // the manager
            p(211, 210, "msedgewebview2.exe"), // its renderer
            p(212, 210, "MSEdgeWebView2.exe"), // its GPU process, in another case
            p(213, 211, "msedgewebview2.exe"), // a child of the renderer
        ];
        assert_eq!(family(200, &all), vec![200, 210, 211, 212, 213]);
    }

    #[test]
    fn leaves_everything_else_alone() {
        let all = [
            p(200, 100, "pyxee-app.exe"),
            p(210, 200, "msedgewebview2.exe"),
            p(300, 100, "msedgewebview2.exe"), // another app's WebView2
            p(301, 300, "msedgewebview2.exe"),
            p(220, 200, "yt-dlp.exe"), // ours, but not a WebView2 process
            p(221, 220, "msedgewebview2.exe"), // below a process that is not one
        ];
        assert_eq!(family(200, &all), vec![200, 210]);
    }

    #[test]
    fn a_process_that_is_its_own_parent_does_not_loop() {
        let all = [
            p(210, 210, "msedgewebview2.exe"),
            p(200, 100, "pyxee-app.exe"),
        ];
        assert_eq!(family(200, &all), vec![200]);
        assert_eq!(family(210, &all), vec![210]);
    }
}
