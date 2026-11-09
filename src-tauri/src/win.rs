//! Panggilan Win32 tipis: jendela aktif, nama proses, idle, minimize/fokus.

use windows_sys::Win32::{
    Foundation::{CloseHandle, HWND},
    System::{
        SystemInformation::GetTickCount,
        Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION},
    },
    UI::{
        Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
        WindowsAndMessaging::{
            GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId, IsIconic, SetForegroundWindow, ShowWindow,
            SW_MINIMIZE, SW_RESTORE,
        },
    },
};

pub struct Foreground {
    pub hwnd: isize,
    pub title: String,
    /// nama exe huruf kecil tanpa ".exe"
    pub exe: String,
}

pub fn foreground() -> Option<Foreground> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let mut buf = [0u16; 512];
        let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        let title = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let exe = exe_name(pid).unwrap_or_default();
        Some(Foreground { hwnd: hwnd as isize, title, exe })
    }
}

fn exe_name(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut size);
        CloseHandle(handle);
        if ok == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..size as usize]);
        let file = path.rsplit('\\').next()?.to_lowercase();
        Some(file.strip_suffix(".exe").unwrap_or(&file).to_string())
    }
}

/// Detik sejak input keyboard/mouse terakhir.
pub fn idle_secs() -> u32 {
    unsafe {
        let mut info = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
        if GetLastInputInfo(&mut info) == 0 {
            return 0;
        }
        GetTickCount().wrapping_sub(info.dwTime) / 1000
    }
}

pub fn minimize(hwnd: isize) {
    unsafe {
        ShowWindow(hwnd as HWND, SW_MINIMIZE);
    }
}

pub fn focus(hwnd: isize) {
    unsafe {
        let h = hwnd as HWND;
        if IsIconic(h) != 0 {
            ShowWindow(h, SW_RESTORE);
        }
        SetForegroundWindow(h);
    }
}
