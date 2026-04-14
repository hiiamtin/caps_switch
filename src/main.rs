use std::cell::Cell;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const CAPS_LOCK_VK: u32 = 0x14;
const THRESHOLD_MS: u64 = 180;
const LLKHF_INJECTED: u32 = 0x00000010;

thread_local! {
    static PRESS_START: Cell<Option<Instant>> = Cell::new(None);
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

        // If injected by our own program, let it pass through
        if (kbd.flags & KBDLLHOOKSTRUCT_FLAGS(LLKHF_INJECTED)) != KBDLLHOOKSTRUCT_FLAGS(0) {
            return unsafe { CallNextHookEx(HHOOK::default(), n_code, w_param, l_param) };
        }

        if kbd.vkCode == CAPS_LOCK_VK {
            if w_param == WPARAM(WM_KEYDOWN as usize) {
                // Only record time on FIRST press, ignore auto-repeat
                PRESS_START.with(|cell| {
                    if cell.get().is_none() {
                        cell.set(Some(Instant::now()));
                    }
                });
                // Block immediately so Windows doesn't toggle Caps Lock
                return LRESULT(1);
            } else if w_param == WPARAM(WM_KEYUP as usize) {
                let duration = PRESS_START.with(|cell| {
                    cell.take().map(|t| t.elapsed())
                });

                if let Some(dur) = duration {
                    if dur < Duration::from_millis(THRESHOLD_MS) {
                        // Short press: switch language
                        switch_language();
                    } else {
                        // Long press: inject real Caps Lock back to system
                        inject_caps_lock();
                    }
                }
                return LRESULT(1);
            }
        }
    }

    unsafe { CallNextHookEx(HHOOK::default(), n_code, w_param, l_param) }
}

fn main() {
    println!("Caps Switch v0.2.0 running");
    println!("- Short press Caps Lock = switch language");
    println!("- Hold > {}ms = normal Caps Lock", THRESHOLD_MS);

    unsafe {
        let hook = match SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(keyboard_proc),
            HINSTANCE::default(),
            0,
        ) {
            Ok(h) => h,
            Err(_) => {
                eprintln!("Failed to install keyboard hook");
                std::process::exit(1);
            }
        };

        if hook.is_invalid() {
            eprintln!("Failed to install keyboard hook");
            std::process::exit(1);
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND::default(), 0, 0).into() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = UnhookWindowsHookEx(hook);
    }
}
