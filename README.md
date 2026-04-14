# Caps Switch

A lightweight Windows utility that turns your **Caps Lock** key into a language switcher — just like macOS.

- **Short press** (< 180ms) → Switch input language (`Win + Space`)
- **Long press** (≥ 180ms) → Normal Caps Lock behavior

Written in Rust. Zero dependencies. No installer needed. Just run the exe.

## Features

- Short press Caps Lock → switch language, Long press → normal Caps Lock

## Download

Grab the latest release from the [Releases](../../releases) page, or build from source.

## Build from Source

**Prerequisites:** [Rust](https://rustup.rs/) (MSVC toolchain)

```bash
git clone https://github.com/hiiamtin/caps_switch.git
cd caps_switch
cargo build --release
```

The exe will be at `target/release/caps_switch.exe`.

## Usage

1. Run `caps_switch.exe`
2. A **CS** icon appears in the system tray
3. Press Caps Lock briefly to switch language
4. Hold Caps Lock to toggle caps (normal behavior)
5. Right-click tray icon → Exit to quit

### Run at Startup

1. Press `Win + R`, type `shell:startup`, press Enter
2. Place a shortcut to `caps_switch.exe` in that folder

## How It Works

```
Caps Lock pressed (WM_KEYDOWN)
  → Block immediately (return 1, Windows never sees it)
  → Record timestamp (first press only, ignore auto-repeat)
  → Wait...

Caps Lock released (WM_KEYUP)
  → Calculate hold duration
  → < 180ms? → SendInput(Win + Space) — switch language
  → ≥ 180ms? → SendInput(Caps Lock down + up) — normal caps toggle
  → Block (return 1)
```

## Why This Exists

Windows has a well-known bug where switching language with the `~` (tilde) key has a noticeable delay (200–800ms). The native Caps Lock key doesn't have this issue, but Windows doesn't let you bind it to language switching natively.

This tool solves both problems: use Caps Lock to switch languages instantly, just like macOS.

## Technical Details

| Aspect | Implementation |
|--------|---------------|
| Keyboard hook | `WH_KEYBOARD_LL` via `SetWindowsHookExW` |
| Input simulation | `SendInput` (atomic batch, not deprecated `keybd_event`) |
| State management | `thread_local! { Cell }` (no `static mut`) |
| Language switch | `Win + Space` (Windows 10/11 default) |
| Process model | Message-only window (`HWND_MESSAGE`) with system tray |
| Icon | Embedded `.ico` loaded via `CreateIconFromResourceEx` |
| No console | `#![windows_subsystem = "windows"]` |

## License

MIT
