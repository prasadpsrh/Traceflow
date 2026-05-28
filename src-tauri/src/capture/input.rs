// Low-level input event producer.
//
// Installs OS-level hooks that emit MouseClick and KeyboardInput events to the
// session log without blocking the capture loop. The hooks run on a dedicated
// OS thread (required by Windows hook architecture).
//
// Safety rules:
//   - Keyboard hook NEVER logs raw key values for password fields; it logs only
//     the virtual key name (e.g. "VK_RETURN") so workflow context is captured
//     without credential exposure.
//   - Mouse hook logs position and button only — no drag paths.

use crate::commands::SharedState;
use crate::events::EventKind;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Spawn the input-hook thread. Returns a stop handle.
/// Dropping the handle (or calling `stop()`) unhooks and joins the thread.
pub struct InputHookHandle {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl InputHookHandle {
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // The hook thread wakes on the next OS message; give it a moment.
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

pub fn start_input_hooks(state: SharedState) -> InputHookHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();
    // Capture the current Tokio runtime handle BEFORE leaving the async context.
    // Hook callbacks run on a plain OS thread and cannot call tokio::spawn directly;
    // they use this handle instead.
    let rt = tokio::runtime::Handle::current();

    let thread = std::thread::spawn(move || {
        platform::run_hook_loop(state, stop_clone, rt);
    });

    InputHookHandle {
        stop,
        thread: Some(thread),
    }
}

// ── Windows implementation ────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::UI::Accessibility::{
        SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, GetWindowTextW,
        GetWindowThreadProcessId, PostQuitMessage, SetWindowsHookExW,
        TranslateMessage, UnhookWindowsHookEx,
        EVENT_SYSTEM_FOREGROUND, WINEVENT_OUTOFCONTEXT,
        HC_ACTION, KBDLLHOOKSTRUCT, MSLLHOOKSTRUCT,
        WH_KEYBOARD_LL, WH_MOUSE_LL,
        WM_KEYDOWN, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_MOUSEMOVE, WM_RBUTTONDOWN,
        WM_SYSKEYDOWN, MSG,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW,
        PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::Foundation::{CloseHandle, FALSE};

    // Thread-local used to pass shared context into the hook callbacks.
    thread_local! {
        static HOOK_STATE: std::cell::RefCell<Option<Arc<SharedStateInner>>> =
            std::cell::RefCell::new(None);
    }

    struct SharedStateInner {
        state: SharedState,
        stop: Arc<AtomicBool>,
        /// Tokio runtime handle captured before the OS thread was spawned.
        /// Callbacks use `rt.spawn()` instead of `tokio::spawn()` because they
        /// run on a plain OS thread with no Tokio reactor.
        rt: tokio::runtime::Handle,
    }

    pub fn run_hook_loop(state: SharedState, stop: Arc<AtomicBool>, rt: tokio::runtime::Handle) {
        let inner = Arc::new(SharedStateInner {
            state: state.clone(),
            stop: stop.clone(),
            rt,
        });

        HOOK_STATE.with(|s| {
            *s.borrow_mut() = Some(inner);
        });

        unsafe {
            let mouse_hook =
                SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), std::ptr::null_mut(), 0);
            let kbd_hook =
                SetWindowsHookExW(WH_KEYBOARD_LL, Some(kbd_proc), std::ptr::null_mut(), 0);
            // Foreground-window change events (no HMODULE needed for out-of-process).
            let focus_hook = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                std::ptr::null_mut(),
                Some(focus_proc),
                0, 0,
                WINEVENT_OUTOFCONTEXT,
            );

            let mut msg: MSG = std::mem::zeroed();
            loop {
                if stop.load(Ordering::SeqCst) {
                    PostQuitMessage(0);
                }
                // GetMessage blocks until a message arrives — hooks fire via callbacks.
                if GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) == 0 {
                    break;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            if !mouse_hook.is_null() {
                UnhookWindowsHookEx(mouse_hook);
            }
            if !kbd_hook.is_null() {
                UnhookWindowsHookEx(kbd_hook);
            }
            if !focus_hook.is_null() {
                UnhookWinEvent(focus_hook);
            }
        }

        HOOK_STATE.with(|s| {
            *s.borrow_mut() = None;
        });
    }

    unsafe extern "system" fn mouse_proc(
        code: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if code == HC_ACTION as i32 {
            let button = match wparam as u32 {
                WM_LBUTTONDOWN => Some("left"),
                WM_RBUTTONDOWN => Some("right"),
                WM_MBUTTONDOWN => Some("middle"),
                WM_MOUSEMOVE => None, // too noisy — skip
                _ => None,
            };

            if let Some(btn) = button {
                let info = &*(lparam as *const MSLLHOOKSTRUCT);
                let (x, y) = (info.pt.x, info.pt.y);
                let btn = btn.to_string();

                HOOK_STATE.with(|s| {
                    if let Some(inner) = s.borrow().as_ref() {
                        if !inner.stop.load(Ordering::SeqCst) {
                            let state = inner.state.clone();
                            inner.rt.spawn(async move {
                                let guard = state.lock().await;
                                if let Some(active) = &guard.active {
                                    let _ = active.log.append(EventKind::MouseClick {
                                        x,
                                        y,
                                        button: btn,
                                    });
                                }
                            });
                        }
                    }
                });
            }
        }
        CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
    }

    /// WinEvent callback fired when the foreground window changes.
    unsafe extern "system" fn focus_proc(
        _hook: HWINEVENTHOOK,
        _event: u32,
        hwnd: HWND,
        _id_object: i32,
        _id_child: i32,
        _id_thread: u32,
        _event_time: u32,
    ) {
        if hwnd == std::ptr::null_mut() {
            return;
        }

        // Get window title
        let mut title_buf = [0u16; 512];
        let title_len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), 512);
        let window_title = if title_len > 0 {
            Some(String::from_utf16_lossy(&title_buf[..title_len as usize]))
        } else {
            None
        };

        // Get app exe name
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let app_name = if pid != 0 {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
            if !handle.is_null() {
                let mut exe_buf = [0u16; 512];
                let mut size: u32 = 512;
                let ok = QueryFullProcessImageNameW(
                    handle, PROCESS_NAME_WIN32, exe_buf.as_mut_ptr(), &mut size,
                );
                CloseHandle(handle);
                if ok != 0 && size > 0 {
                    let path = String::from_utf16_lossy(&exe_buf[..size as usize]);
                    std::path::Path::new(&path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string())
                } else { None }
            } else { None }
        } else { None };

        HOOK_STATE.with(|s| {
            if let Some(inner) = s.borrow().as_ref() {
                if !inner.stop.load(Ordering::SeqCst) {
                    let state = inner.state.clone();
                    let title = window_title.clone();
                    let app = app_name.clone();
                    inner.rt.spawn(async move {
                        let guard = state.lock().await;
                        if guard.is_recording {
                            if let Some(active) = &guard.active {
                                let _ = active.log.append(EventKind::WindowFocusChanged {
                                    window_title: title,
                                    window_class: None,
                                    app_name: app,
                                });
                            }
                        }
                    });
                }
            }
        });
    }

    unsafe extern "system" fn kbd_proc(
        code: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if code == HC_ACTION as i32 {
            let is_keydown = matches!(wparam as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
            if is_keydown {
                let info = &*(lparam as *const KBDLLHOOKSTRUCT);
                let vk = vk_name(info.vkCode);

                HOOK_STATE.with(|s| {
                    if let Some(inner) = s.borrow().as_ref() {
                        if !inner.stop.load(Ordering::SeqCst) {
                            let state = inner.state.clone();
                            let vk = vk.to_string();
                            inner.rt.spawn(async move {
                                let guard = state.lock().await;
                                if let Some(active) = &guard.active {
                                    let _ = active.log.append(EventKind::KeyboardInput {
                                        virtual_key: vk,
                                    });
                                }
                            });
                        }
                    }
                });
            }
        }
        CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
    }

    /// Map a Windows virtual-key code to a human-readable name.
    /// Returns the VK_ constant name — never the character value, so password
    /// fields can't be reconstructed from the log.
    fn vk_name(vk: u32) -> &'static str {
        match vk {
            0x08 => "VK_BACK",
            0x09 => "VK_TAB",
            0x0D => "VK_RETURN",
            0x1B => "VK_ESCAPE",
            0x20 => "VK_SPACE",
            0x21 => "VK_PRIOR",
            0x22 => "VK_NEXT",
            0x23 => "VK_END",
            0x24 => "VK_HOME",
            0x25 => "VK_LEFT",
            0x26 => "VK_UP",
            0x27 => "VK_RIGHT",
            0x28 => "VK_DOWN",
            0x2E => "VK_DELETE",
            0x70..=0x7B => "VK_F(n)",
            _ => "VK_OTHER",
        }
    }
}

// ── Non-Windows stub ──────────────────────────────────────────────────────────

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::*;

    pub fn run_hook_loop(_state: SharedState, stop: Arc<AtomicBool>, _rt: tokio::runtime::Handle) {
        // Poll stop flag on other platforms until OS hooks are wired in P3.
        while !stop.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}
