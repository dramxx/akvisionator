//! Switches Windows Terminal to fullscreen (F11) on start and back on exit.

use std::sync::atomic::{AtomicBool, Ordering};

/// Set only if we pressed F11, so exit (or a panic) toggles back exactly once.
static TOGGLED: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
pub fn enter() {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow};
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowRect};

    // F11 = fullscreen is a Windows Terminal binding; other consoles use it differently.
    if std::env::var_os("WT_SESSION").is_none() {
        return;
    }
    // Skip if the terminal window already covers its whole monitor, i.e. is fullscreen.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return;
        }
        let mut win: RECT = std::mem::zeroed();
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        if GetWindowRect(hwnd, &mut win) == 0 || GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut info) == 0 {
            return;
        }
        let m = info.rcMonitor;
        if (win.left, win.top, win.right, win.bottom) == (m.left, m.top, m.right, m.bottom) {
            return;
        }
    }
    press_f11();
    TOGGLED.store(true, Ordering::SeqCst);
}

#[cfg(not(windows))]
pub fn enter() {}

pub fn leave() {
    if TOGGLED.swap(false, Ordering::SeqCst) {
        press_f11();
    }
}

#[cfg(windows)]
fn press_f11() {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{KEYEVENTF_KEYUP, VK_F11, keybd_event};
    unsafe {
        keybd_event(VK_F11 as u8, 0, 0, 0);
        keybd_event(VK_F11 as u8, 0, KEYEVENTF_KEYUP, 0);
    }
}

#[cfg(not(windows))]
fn press_f11() {}
