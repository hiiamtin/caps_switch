#![windows_subsystem = "windows"]

use std::cell::Cell;
use std::time::{Duration, Instant};
use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const CAPS_LOCK_VK: u32 = 0x14;
const THRESHOLD_MS: u64 = 180;
const LLKHF_INJECTED: u32 = 0x00000010;

const TRAY_MSG: u32 = WM_APP;
const IDM_EXIT: u32 = 1001;

thread_local! {
    static PRESS_START: Cell<Option<Instant>> = Cell::new(None);
    static HOOK: Cell<Option<HHOOK>> = Cell::new(None);
    static TRAY_ADDED: Cell<bool> = Cell::new(false);
}

const ICO_BYTES: &[u8] = include_bytes!("../caps_switch.ico");

fn create_tray_icon() -> HICON {
    // ICO file: 6-byte header + 16-byte directory entry = 22 bytes before image data
    let icon_data = &ICO_BYTES[22..];
    unsafe {
        CreateIconFromResourceEx(
            icon_data,
            true,
            0x00030000,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            LR_DEFAULTCOLOR,
        )
        .unwrap_or_default()
    }
}

fn make_input(vk: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn switch_language() {
    let inputs = [
        make_input(VK_LWIN.0 as u16, false),
        make_input(VK_SPACE.0 as u16, false),
        make_input(VK_SPACE.0 as u16, true),
        make_input(VK_LWIN.0 as u16, true),
    ];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

fn inject_caps_lock() {
    let inputs = [
        make_input(CAPS_LOCK_VK as u16, false),
        make_input(CAPS_LOCK_VK as u16, true),
    ];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

unsafe extern "system" fn keyboard_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let kbd = unsafe { &*(l_param.0 as *const KBDLLHOOKSTRUCT) };

        if (kbd.flags & KBDLLHOOKSTRUCT_FLAGS(LLKHF_INJECTED)) != KBDLLHOOKSTRUCT_FLAGS(0) {
            return unsafe { CallNextHookEx(HHOOK::default(), n_code, w_param, l_param) };
        }

        if kbd.vkCode == CAPS_LOCK_VK {
            if w_param == WPARAM(WM_KEYDOWN as usize) {
                PRESS_START.with(|cell| {
                    if cell.get().is_none() {
                        cell.set(Some(Instant::now()));
                    }
                });
                return LRESULT(1);
            } else if w_param == WPARAM(WM_KEYUP as usize) {
                let duration = PRESS_START.with(|cell| {
                    cell.take().map(|t| t.elapsed())
                });

                if let Some(dur) = duration {
                    if dur < Duration::from_millis(THRESHOLD_MS) {
                        switch_language();
                    } else {
                        inject_caps_lock();
                    }
                }
                return LRESULT(1);
            }
        }
    }

    unsafe { CallNextHookEx(HHOOK::default(), n_code, w_param, l_param) }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    match msg {
        TRAY_MSG => match l_param.0 as u32 {
            WM_RBUTTONUP => {
                let mut pt = POINT { x: 0, y: 0 };
                let _ = unsafe { GetCursorPos(&mut pt) };
                let hmenu = unsafe { CreatePopupMenu() }.unwrap_or_default();
                unsafe {
                    let _ = AppendMenuW(
                        hmenu,
                        MF_STRING,
                        IDM_EXIT as usize,
                        windows::core::w!("E&xit"),
                    );
                    SetForegroundWindow(hwnd);
                    let _ = TrackPopupMenu(
                        hmenu,
                        TPM_BOTTOMALIGN | TPM_LEFTALIGN,
                        pt.x,
                        pt.y,
                        0,
                        hwnd,
                        None,
                    );
                    let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
                    let _ = DestroyMenu(hmenu);
                }
            }
            _ => {}
        },
        WM_COMMAND => {
            if w_param.0 as u32 == IDM_EXIT {
                HOOK.with(|cell| {
                    if let Some(hook) = cell.take() {
                        let _ = unsafe { UnhookWindowsHookEx(hook) };
                    }
                });
                TRAY_ADDED.with(|cell| {
                    if cell.get() {
                        let mut nid = NOTIFYICONDATAW::default();
                        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
                        nid.hWnd = hwnd;
                        nid.uID = 1;
                        let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
                    }
                });
                unsafe { PostQuitMessage(0) };
            }
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
        }
        _ => {}
    }
    unsafe { DefWindowProcW(hwnd, msg, w_param, l_param) }
}

fn main() {
    unsafe {
        let h_instance = GetModuleHandleW(None).unwrap_or_default();

        let class_name = windows::core::w!("CapsSwitchWnd");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: h_instance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        let _ = RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            PCWSTR(class_name.as_ptr()),
            windows::core::w!("CapsSwitch"),
            WINDOW_STYLE::default(),
            0, 0, 0, 0,
            HWND_MESSAGE,
            None,
            h_instance,
            None,
        );

        let tip_text: Vec<u16> = "Caps Switch v0.2\0".encode_utf16().collect();
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            uFlags: NIF_ICON | NIF_TIP | NIF_MESSAGE,
            uCallbackMessage: TRAY_MSG,
            hIcon: create_tray_icon(),
            ..Default::default()
        };
        for (i, &c) in tip_text.iter().enumerate() {
            if i >= nid.szTip.len() {
                break;
            }
            nid.szTip[i] = c;
        }
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
        TRAY_ADDED.with(|cell| cell.set(true));

        let hook = match SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(keyboard_proc),
            h_instance,
            0,
        ) {
            Ok(h) => h,
            Err(_) => std::process::exit(1),
        };
        HOOK.with(|cell| cell.set(Some(hook)));

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND::default(), 0, 0).into() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
