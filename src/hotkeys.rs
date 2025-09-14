use lazy_static::lazy_static;
use rdev::{Event, EventType, Key};
use std::sync::mpsc::Sender;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy)]
pub enum KeyboardEvent {
    CtrlCapsLockPressed,   // Start recording (kept name for backward compatibility)
    CtrlCapsLockReleased,  // Stop recording (kept name for backward compatibility)
    AltCapsToggleTranslate, // Toggle translate mode (kept name for backward compatibility)
}

#[derive(Debug, Clone, Copy)]
struct Hotkey {
    ctrl: bool,
    alt: bool,
    shift: bool,
    super_: bool,
    key: Key,
}

lazy_static! {
    pub static ref KEYBOARD_EVENT_SENDER: Mutex<Option<Sender<KeyboardEvent>>> = Mutex::new(None);
    static ref CTRL_PRESSED: Mutex<bool> = Mutex::new(false);
    static ref ALT_PRESSED: Mutex<bool> = Mutex::new(false);
    static ref SHIFT_PRESSED: Mutex<bool> = Mutex::new(false);
    static ref SUPER_PRESSED: Mutex<bool> = Mutex::new(false);
    static ref RECORD_HOTKEY: Mutex<Option<Hotkey>> = Mutex::new(None);
    static ref MODE_HOTKEY: Mutex<Option<Hotkey>> = Mutex::new(None);
    static ref RECORD_ACTIVE: Mutex<bool> = Mutex::new(false);
}

fn parse_key_name(name: &str) -> Option<Key> {
    match name {
        "CapsLock" => Some(Key::CapsLock),
        "Esc" | "Escape" => Some(Key::Escape),
        "Enter" | "Return" => Some(Key::Return),
        // Letters A..Z
        s if s.len() == 1 && s.chars().all(|c| c.is_ascii_alphabetic()) => {
            let c = s.chars().next().unwrap().to_ascii_uppercase();
            match c {
                'A' => Some(Key::KeyA), 'B' => Some(Key::KeyB), 'C' => Some(Key::KeyC),
                'D' => Some(Key::KeyD), 'E' => Some(Key::KeyE), 'F' => Some(Key::KeyF),
                'G' => Some(Key::KeyG), 'H' => Some(Key::KeyH), 'I' => Some(Key::KeyI),
                'J' => Some(Key::KeyJ), 'K' => Some(Key::KeyK), 'L' => Some(Key::KeyL),
                'M' => Some(Key::KeyM), 'N' => Some(Key::KeyN), 'O' => Some(Key::KeyO),
                'P' => Some(Key::KeyP), 'Q' => Some(Key::KeyQ), 'R' => Some(Key::KeyR),
                'S' => Some(Key::KeyS), 'T' => Some(Key::KeyT), 'U' => Some(Key::KeyU),
                'V' => Some(Key::KeyV), 'W' => Some(Key::KeyW), 'X' => Some(Key::KeyX),
                'Y' => Some(Key::KeyY), 'Z' => Some(Key::KeyZ),
                _ => None,
            }
        }
        // Function keys F1..F12
        s if s.starts_with('F') && s[1..].chars().all(|c| c.is_ascii_digit()) => {
            match &s[1..] {
                "1" => Some(Key::F1), "2" => Some(Key::F2), "3" => Some(Key::F3), "4" => Some(Key::F4),
                "5" => Some(Key::F5), "6" => Some(Key::F6), "7" => Some(Key::F7), "8" => Some(Key::F8),
                "9" => Some(Key::F9), "10" => Some(Key::F10), "11" => Some(Key::F11), "12" => Some(Key::F12),
                _ => None,
            }
        }
        _ => None,
    }
}

fn parse_shortcut(s: &str) -> Option<Hotkey> {
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut super_ = false;
    let mut key_opt: Option<Key> = None;

    for part in s.split('+') {
        let p = part.trim();
        match p {
            "Ctrl" | "Control" => ctrl = true,
            "Alt" | "AltGr" => alt = true,
            "Shift" => shift = true,
            "Super" | "Meta" | "Win" => super_ = true,
            other => {
                key_opt = parse_key_name(other);
            }
        }
    }

    if let Some(key) = key_opt {
        Some(Hotkey { ctrl, alt, shift, super_, key })
    } else {
        None
    }
}

fn mods_match(h: Hotkey) -> bool {
    let ctrl = *CTRL_PRESSED.lock().unwrap();
    let alt = *ALT_PRESSED.lock().unwrap();
    let shift = *SHIFT_PRESSED.lock().unwrap();
    let super_ = *SUPER_PRESSED.lock().unwrap();
    (!h.ctrl || ctrl) && (!h.alt || alt) && (!h.shift || shift) && (!h.super_ || super_)
}

fn is_modifier_key(k: Key) -> bool {
    matches!(k,
        Key::ControlLeft | Key::ControlRight |
        Key::ShiftLeft | Key::ShiftRight |
        Key::Alt | Key::AltGr |
        Key::MetaLeft | Key::MetaRight)
}

pub fn init_hotkeys_from_config(record: String, change_mode: String) {
    let rec = parse_shortcut(&record);
    let mode = parse_shortcut(&change_mode);
    *RECORD_HOTKEY.lock().unwrap() = rec;
    *MODE_HOTKEY.lock().unwrap() = mode;
    println!("Using shortcuts: record='{}', toggle='{}'", record, change_mode);
}

pub fn handle_keyboard_event(event: Event) {
    // Update modifier states
    match event.event_type {
        EventType::KeyPress(Key::ControlLeft) | EventType::KeyPress(Key::ControlRight) => {
            *CTRL_PRESSED.lock().unwrap() = true;
            return;
        }
        EventType::KeyRelease(Key::ControlLeft) | EventType::KeyRelease(Key::ControlRight) => {
            *CTRL_PRESSED.lock().unwrap() = false;
        }
        EventType::KeyPress(Key::ShiftLeft) | EventType::KeyPress(Key::ShiftRight) => {
            *SHIFT_PRESSED.lock().unwrap() = true;
            return;
        }
        EventType::KeyRelease(Key::ShiftLeft) | EventType::KeyRelease(Key::ShiftRight) => {
            *SHIFT_PRESSED.lock().unwrap() = false;
        }
        EventType::KeyPress(Key::Alt) | EventType::KeyPress(Key::AltGr) => {
            *ALT_PRESSED.lock().unwrap() = true;
            return;
        }
        EventType::KeyRelease(Key::Alt) | EventType::KeyRelease(Key::AltGr) => {
            *ALT_PRESSED.lock().unwrap() = false;
        }
        EventType::KeyPress(Key::MetaLeft) | EventType::KeyPress(Key::MetaRight) => {
            *SUPER_PRESSED.lock().unwrap() = true;
            return;
        }
        EventType::KeyRelease(Key::MetaLeft) | EventType::KeyRelease(Key::MetaRight) => {
            *SUPER_PRESSED.lock().unwrap() = false;
        }
        _ => {}
    }

    let sender_opt = KEYBOARD_EVENT_SENDER.lock().unwrap().clone();
    if sender_opt.is_none() { return; }
    let sender = sender_opt.unwrap();

    // Current configured hotkeys
    let rec_opt = *RECORD_HOTKEY.lock().unwrap();
    let mode_opt = *MODE_HOTKEY.lock().unwrap();

    match event.event_type {
        EventType::KeyPress(k) => {
            if is_modifier_key(k) {
                return;
            }
            if let Some(h) = rec_opt {
                if k == h.key && mods_match(h) {
                    *RECORD_ACTIVE.lock().unwrap() = true;
                    let _ = sender.send(KeyboardEvent::CtrlCapsLockPressed);
                    return;
                }
            }
            if let Some(h) = mode_opt {
                if k == h.key && mods_match(h) {
                    let _ = sender.send(KeyboardEvent::AltCapsToggleTranslate);
                    return;
                }
            }
        }
        EventType::KeyRelease(k) => {
            // Stop recording when main key of record is released, or when a required modifier is released while active
            let active = *RECORD_ACTIVE.lock().unwrap();
            if active {
                if let Some(h) = rec_opt {
                    if k == h.key {
                        *RECORD_ACTIVE.lock().unwrap() = false;
                        let _ = sender.send(KeyboardEvent::CtrlCapsLockReleased);
                        return;
                    }
                    // If a required modifier is released, also stop
                    let modifier_released = (h.ctrl && matches!(k, Key::ControlLeft | Key::ControlRight))
                        || (h.alt && matches!(k, Key::Alt | Key::AltGr))
                        || (h.shift && matches!(k, Key::ShiftLeft | Key::ShiftRight))
                        || (h.super_ && matches!(k, Key::MetaLeft | Key::MetaRight));
                    if modifier_released {
                        *RECORD_ACTIVE.lock().unwrap() = false;
                        let _ = sender.send(KeyboardEvent::CtrlCapsLockReleased);
                        return;
                    }
                }
            }
        }
        _ => {}
    }
}
