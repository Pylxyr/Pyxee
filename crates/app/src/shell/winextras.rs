//! Taskbar thumbnail buttons (previous / play-pause / next) and the jump list (right-click on the taskbar icon).
//!
//! Windows only, behind the `win-extras` feature. NOT compiled where it was written (no Windows toolchain there):
//! if `cargo check -p pyxee-app --features win-extras` complains, the fix is local to this file. What the buttons and
//! tasks are, and how a click maps to an action, is decided in `pyxee_shell::winui` and tested there.

use pyxee_shell::cli::CliAction;
use pyxee_shell::progress::Phase;
use pyxee_shell::winui::{jump_tasks, thumb_specs, Glyph, ThumbButton, ThumbSpec};
use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};
use windows::core::{w, Interface, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Storage::EnhancedStorage::PKEY_Title;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::Common::{IObjectArray, IObjectCollection};
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, InitPropVariantFromStringW};
use windows::Win32::UI::Shell::{
    DefSubclassProc, DestinationList, EnumerableObjectCollection, ICustomDestinationList,
    IShellLinkW, ITaskbarList3, SetWindowSubclass, ShellLink, TaskbarList, THBF_DISABLED,
    THBF_ENABLED, THB_FLAGS, THB_ICON, THB_TOOLTIP, THUMBBUTTON,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconFromResourceEx, RegisterWindowMessageW, HICON, LR_DEFAULTCOLOR, LR_DEFAULTSIZE,
    WM_COMMAND,
};

/// The notification code Windows puts in the high word of `wParam` for a thumbnail button click.
const THBN_CLICKED: usize = 0x1800;
const SUBCLASS_ID: usize = 0x5059_5845; // "PYXE"

static APP: OnceLock<AppHandle> = OnceLock::new();
static BUTTON_CREATED_MSG: AtomicU32 = AtomicU32::new(0);

thread_local! {
    /// Only ever touched on the main (UI) thread, where the COM object was created.
    static TASKBAR: RefCell<Option<Taskbar>> = const { RefCell::new(None) };
}

struct Taskbar {
    list: ITaskbarList3,
    hwnd: HWND,
    added: bool,
}

pub fn init(app: &AppHandle) {
    let _ = APP.set(app.clone());
    let Some(window) = app.get_webview_window(super::MAIN) else {
        return;
    };
    let hwnd = match window.hwnd() {
        // The pointer is the same whichever `windows` version Tauri was built with.
        #[allow(clippy::unnecessary_cast)]
        Ok(h) => HWND(h.0 as *mut c_void),
        Err(e) => {
            log::warn!("taskbar extras: no window handle: {e}");
            return;
        }
    };
    let _ = window.run_on_main_thread(move || {
        // SAFETY: plain COM and window-procedure calls on the thread that owns the window.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            BUTTON_CREATED_MSG.store(
                RegisterWindowMessageW(w!("TaskbarButtonCreated")),
                Ordering::Relaxed,
            );
            if !SetWindowSubclass(hwnd, Some(subclass), SUBCLASS_ID, 0).as_bool() {
                log::warn!("taskbar extras: could not watch the window for button clicks");
            }
            match CoCreateInstance::<_, ITaskbarList3>(&TaskbarList, None, CLSCTX_INPROC_SERVER) {
                Ok(list) if list.HrInit().is_ok() => {
                    TASKBAR.with(|t| {
                        *t.borrow_mut() = Some(Taskbar {
                            list,
                            hwnd,
                            added: false,
                        })
                    });
                    apply(Phase::Idle, false);
                }
                Ok(_) => log::warn!("taskbar extras: the taskbar list would not start"),
                Err(e) => log::warn!("taskbar extras: no taskbar list: {e}"),
            }
        }
        if let Err(e) = build_jump_list() {
            log::warn!("jump list not created: {e}");
        }
    });
}

/// The play state changed: refresh the three buttons.
pub fn update(app: &AppHandle, phase: Phase, has_track: bool) {
    if let Some(w) = app.get_webview_window(super::MAIN) {
        let _ = w.run_on_main_thread(move || apply(phase, has_track));
    }
}

fn apply(phase: Phase, has_track: bool) {
    TASKBAR.with(|cell| {
        let mut guard = cell.borrow_mut();
        let Some(tb) = guard.as_mut() else {
            return;
        };
        let buttons = thumb_specs(phase, has_track).map(|s| button(&s));
        // SAFETY: `buttons` outlives the call; the window handle is ours.
        let r = unsafe {
            if tb.added {
                tb.list.ThumbBarUpdateButtons(tb.hwnd, &buttons)
            } else {
                let r = tb.list.ThumbBarAddButtons(tb.hwnd, &buttons);
                tb.added = r.is_ok();
                r
            }
        };
        if let Err(e) = r {
            // Before the taskbar button exists Windows says no; the `TaskbarButtonCreated` message retries.
            log::debug!("thumbnail buttons not applied yet: {e}");
        }
    });
}

fn button(s: &ThumbSpec) -> THUMBBUTTON {
    let mut tip = [0u16; 260];
    for (slot, c) in tip.iter_mut().zip(s.tooltip.encode_utf16().take(259)) {
        *slot = c;
    }
    THUMBBUTTON {
        dwMask: THB_ICON | THB_TOOLTIP | THB_FLAGS,
        iId: s.button.id(),
        iBitmap: 0,
        hIcon: glyph_icon(s.glyph),
        szTip: tip,
        dwFlags: if s.enabled {
            THBF_ENABLED
        } else {
            THBF_DISABLED
        },
    }
}

fn glyph_icon(g: Glyph) -> HICON {
    let png: &[u8] = match g {
        Glyph::Previous => include_bytes!("../../icons/thumb/prev.png"),
        Glyph::Play => include_bytes!("../../icons/thumb/play.png"),
        Glyph::Pause => include_bytes!("../../icons/thumb/pause.png"),
        Glyph::Next => include_bytes!("../../icons/thumb/next.png"),
    };
    // SAFETY: the PNG bytes are static; the icon is owned by Windows for the life of the app.
    unsafe {
        CreateIconFromResourceEx(
            png,
            true,
            0x0003_0000,
            0,
            0,
            LR_DEFAULTCOLOR | LR_DEFAULTSIZE,
        )
    }
    .unwrap_or_default()
}

/// Watches the main window for thumbnail button clicks and for the taskbar button being (re)created.
unsafe extern "system" fn subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if msg == WM_COMMAND && (wparam.0 >> 16) & 0xFFFF == THBN_CLICKED {
        if let (Some(button), Some(app)) =
            (ThumbButton::from_id((wparam.0 & 0xFFFF) as u32), APP.get())
        {
            run(app, button.action());
        }
        return LRESULT(0);
    }
    let created = BUTTON_CREATED_MSG.load(Ordering::Relaxed);
    if created != 0 && msg == created {
        // The taskbar button exists now (or Explorer restarted): add the buttons afresh.
        TASKBAR.with(|t| {
            if let Some(tb) = t.borrow_mut().as_mut() {
                tb.added = false;
            }
        });
        if let Some(app) = APP.get() {
            let (phase, has_track) = super::current_phase(app);
            apply(phase, has_track);
        }
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

fn run(app: &AppHandle, action: CliAction) {
    super::run_actions(app, &[action]);
}

/// The jump list: four tasks that start Pyxee with an argument; the running instance does the work.
fn build_jump_list() -> windows::core::Result<()> {
    let exe = std::env::current_exe().map_err(|_| windows::core::Error::from_win32())?;
    let exe_w: Vec<u16> = exe
        .to_string_lossy()
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(Some(0)).collect() };
    // SAFETY: COM calls with buffers that live until the end of this function.
    unsafe {
        let list: ICustomDestinationList =
            CoCreateInstance(&DestinationList, None, CLSCTX_INPROC_SERVER)?;
        let mut min_slots = 0u32;
        let _removed: IObjectArray = list.BeginList(&mut min_slots)?;
        let tasks: IObjectCollection =
            CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER)?;
        for t in jump_tasks() {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            link.SetPath(PCWSTR(exe_w.as_ptr()))?;
            link.SetArguments(PCWSTR(wide(t.args).as_ptr()))?;
            link.SetDescription(PCWSTR(wide(t.description).as_ptr()))?;
            link.SetIconLocation(PCWSTR(exe_w.as_ptr()), 0)?;
            let store: IPropertyStore = link.cast()?;
            let title = InitPropVariantFromStringW(PCWSTR(wide(t.title).as_ptr()))?;
            store.SetValue(&PKEY_Title, &title)?;
            store.Commit()?;
            tasks.AddObject(&link)?;
        }
        list.AddUserTasks(&tasks.cast::<IObjectArray>()?)?;
        list.CommitList()?;
    }
    Ok(())
}
