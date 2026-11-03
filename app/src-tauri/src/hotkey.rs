//! A system-wide hotkey, Ctrl+Alt+M, that brings the main window up. The app has no tray icon,
//! so once its window is hidden this and launching it again are the ways back.

use tauri::{AppHandle, Runtime};
use tracing::warn;

/// Starts listening for the hotkey; `show` runs on each press.
pub fn listen<R: Runtime>(app: &AppHandle<R>, show: fn(&AppHandle<R>)) {
    #[cfg(windows)]
    {
        let app = app.clone();
        let spawned = std::thread::Builder::new()
            .name("hotkey".into())
            .spawn(move || windows::run(|| show(&app)));
        if let Err(error) = spawned {
            warn!(%error, "Hotkey thread failed to start");
        }
    }
    #[cfg(not(windows))]
    let _ = (app, show);
}

#[cfg(windows)]
mod windows {
    use std::ffi::c_void;
    use std::ptr;

    use tracing::{info, warn};

    const MOD_ALT: u32 = 0x0001;
    const MOD_CONTROL: u32 = 0x0002;
    const MOD_NOREPEAT: u32 = 0x4000;
    const VK_M: u32 = 0x4D;
    const WM_HOTKEY: u32 = 0x0312;
    const ID: i32 = 1;

    #[repr(C)]
    struct Msg {
        window: *mut c_void,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        point: [i32; 2],
        private: u32,
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn RegisterHotKey(window: *mut c_void, id: i32, modifiers: u32, key: u32) -> i32;
        fn GetMessageW(msg: *mut Msg, window: *mut c_void, min: u32, max: u32) -> i32;
    }

    /// Registers the hotkey to this thread and waits on its messages for good. A hotkey
    /// belongs to the thread that registers it, so this thread has to be the one waiting.
    pub(super) fn run(on_press: impl Fn()) {
        // SAFETY: a null window ties the hotkey to this thread's message queue.
        let registered = unsafe {
            RegisterHotKey(
                ptr::null_mut(),
                ID,
                MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
                VK_M,
            )
        };
        if registered == 0 {
            warn!(error = %std::io::Error::last_os_error(), "Ctrl+Alt+M is taken; no hotkey");
            return;
        }
        info!("Ctrl+Alt+M shows the main window");
        let mut msg = Msg {
            window: ptr::null_mut(),
            message: 0,
            wparam: 0,
            lparam: 0,
            time: 0,
            point: [0; 2],
            private: 0,
        };
        // SAFETY: `msg` is a valid MSG for the call to fill; 0 or -1 ends the loop.
        while unsafe { GetMessageW(&raw mut msg, ptr::null_mut(), 0, 0) } > 0 {
            if msg.message == WM_HOTKEY {
                on_press();
            }
        }
    }
}
