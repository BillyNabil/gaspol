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

/// Pastikan jendela tembus pandang per-piksel. Di sebagian build Windows 11
/// (24H2+) flag `transparent` dari tao tidak terpasang, sehingga area kosong
/// WebView2 tampil hitam. Blur-behind dengan region kosong mengaktifkan alpha
/// tanpa efek blur apa pun.
pub fn ensure_transparent(hwnd: isize) {
    use windows_sys::Win32::Graphics::{
        Dwm::{DwmEnableBlurBehindWindow, DWM_BB_BLURREGION, DWM_BB_ENABLE, DWM_BLURBEHIND},
        Gdi::{CreateRectRgn, DeleteObject},
    };
    unsafe {
        let rgn = CreateRectRgn(0, 0, -1, -1);
        let bb = DWM_BLURBEHIND {
            dwFlags: DWM_BB_ENABLE | DWM_BB_BLURREGION,
            fEnable: 1,
            hRgnBlur: rgn,
            fTransitionOnMaximized: 0,
        };
        DwmEnableBlurBehindWindow(hwnd as HWND, &bb);
        if !rgn.is_null() {
            DeleteObject(rgn);
        }
    }
}

// ------------------------------------------------------------------ util tambahan

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Buka URL / file / folder / nama app lewat shell Windows (sama seperti Win+R).
pub fn shell_open(target: &str) -> bool {
    shell_exec("open", target, "")
}

/// Jalankan program dengan hak admin (memunculkan dialog UAC).
pub fn shell_runas(file: &str, params: &str) -> bool {
    shell_exec("runas", file, params)
}

fn shell_exec(verb: &str, file: &str, params: &str) -> bool {
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
    let (v, f, p) = (wide(verb), wide(file), wide(params));
    unsafe {
        let r = ShellExecuteW(
            std::ptr::null_mut(),
            v.as_ptr(),
            f.as_ptr(),
            if params.is_empty() { std::ptr::null() } else { p.as_ptr() },
            std::ptr::null(),
            SW_SHOWNORMAL,
        );
        r as isize > 32
    }
}

/// `Command` tanpa jendela konsol yang berkedip.
pub fn command(program: &str) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    let mut c = std::process::Command::new(program);
    c.creation_flags(0x0800_0000);
    c
}

fn tap(vk: u8) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP};
    unsafe {
        keybd_event(vk, 0, 0, 0);
        keybd_event(vk, 0, KEYEVENTF_KEYUP, 0);
    }
}

/// Tombol media Play/Pause.
pub fn media_play_pause() {
    tap(0xB3);
}

fn key_down(vk: i32) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
}

pub fn clipboard_seq() -> u32 {
    unsafe { windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber() }
}

pub fn clipboard_text() -> Option<String> {
    use windows_sys::Win32::System::{
        DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard},
        Memory::{GlobalLock, GlobalUnlock},
    };
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }
        let h = GetClipboardData(13); // CF_UNICODETEXT
        let mut out = None;
        if !h.is_null() {
            let p = GlobalLock(h) as *const u16;
            if !p.is_null() {
                let mut len = 0usize;
                while *p.add(len) != 0 && len < 200_000 {
                    len += 1;
                }
                out = Some(String::from_utf16_lossy(std::slice::from_raw_parts(p, len)));
                GlobalUnlock(h);
            }
        }
        CloseClipboard();
        out
    }
}

/// Salin teks yang sedang diblok di app aktif: tunggu tombol hotkey dilepas, kirim Ctrl+C,
/// lalu baca clipboard kalau isinya berubah.
pub fn copy_selection() -> Option<String> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP};
    let start = std::time::Instant::now();
    // Shift/Ctrl/Alt/Win/Space masih ditekan → Ctrl+C bisa jadi Ctrl+Shift+C (DevTools).
    while [0x10, 0x11, 0x12, 0x5B, 0x5C, 0x20].iter().any(|k| key_down(*k)) {
        if start.elapsed().as_millis() > 1500 {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    let before = clipboard_seq();
    unsafe {
        keybd_event(0x11, 0, 0, 0);
        keybd_event(0x43, 0, 0, 0);
        keybd_event(0x43, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(0x11, 0, KEYEVENTF_KEYUP, 0);
    }
    for _ in 0..25 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        if clipboard_seq() != before {
            std::thread::sleep(std::time::Duration::from_millis(30));
            return clipboard_text().filter(|t| !t.trim().is_empty());
        }
    }
    None
}

pub struct WinInfo {
    pub hwnd: isize,
    pub title: String,
    pub exe: String,
}

/// Semua jendela top-level yang terlihat dan punya judul.
pub fn list_windows() -> Vec<WinInfo> {
    use windows_sys::Win32::{
        Foundation::{BOOL, LPARAM},
        UI::WindowsAndMessaging::{EnumWindows, IsWindowVisible},
    };
    unsafe extern "system" fn cb(h: HWND, l: LPARAM) -> BOOL {
        let out = &mut *(l as *mut Vec<WinInfo>);
        if IsWindowVisible(h) != 0 {
            let mut buf = [0u16; 512];
            let n = GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32);
            if n > 0 {
                let mut pid = 0u32;
                GetWindowThreadProcessId(h, &mut pid);
                out.push(WinInfo {
                    hwnd: h as isize,
                    title: String::from_utf16_lossy(&buf[..n as usize]),
                    exe: exe_name(pid).unwrap_or_default(),
                });
            }
        }
        1
    }
    let mut out: Vec<WinInfo> = Vec::new();
    unsafe {
        EnumWindows(Some(cb), &mut out as *mut _ as LPARAM);
    }
    out
}

pub fn close_window(hwnd: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
    unsafe {
        PostMessageW(hwnd as HWND, WM_CLOSE, 0, 0);
    }
}

/// Nama exe (huruf kecil, tanpa .exe) semua proses yang berjalan.
pub fn processes() -> Vec<String> {
    use windows_sys::Win32::{
        Foundation::INVALID_HANDLE_VALUE,
        System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS},
    };
    let mut out = Vec::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE || snap.is_null() {
            return out;
        }
        let mut e: PROCESSENTRY32W = std::mem::zeroed();
        e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut e);
        while ok != 0 {
            let len = e.szExeFile.iter().position(|c| *c == 0).unwrap_or(e.szExeFile.len());
            let name = String::from_utf16_lossy(&e.szExeFile[..len]).to_lowercase();
            out.push(name.strip_suffix(".exe").unwrap_or(&name).to_string());
            ok = Process32NextW(snap, &mut e);
        }
        CloseHandle(snap);
    }
    out
}
