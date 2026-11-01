#[cfg(windows)]
pub fn foreground_title_contains(title: &str) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW};

    let mut buf = [0u16; 256];
    // SAFETY: GetForegroundWindow has no preconditions, and GetWindowTextW writes at most
    // `buf.len()` UTF-16 units into a buffer we own.
    let n = unsafe { GetWindowTextW(GetForegroundWindow(), buf.as_mut_ptr(), 256) };
    let n = usize::try_from(n).unwrap_or(0);
    String::from_utf16_lossy(&buf[..n]).contains(title)
}

#[cfg(not(windows))]
pub fn foreground_title_contains(_: &str) -> bool {
    true
}
