use lazy_static::lazy_static;
use rdev::{Event, EventType, Key};
use std::sync::mpsc::Sender;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy)]
pub enum KeyboardEvent {
    CtrlCapsLockPressed,
    CtrlCapsLockReleased,
    AltCapsToggleTranslate,
}

lazy_static! {
    pub static ref KEYBOARD_EVENT_SENDER: Mutex<Option<Sender<KeyboardEvent>>> = Mutex::new(None);
    static ref CTRL_PRESSED: Mutex<bool> = Mutex::new(false);
    static ref ALT_PRESSED: Mutex<bool> = Mutex::new(false);
}

pub fn handle_keyboard_event(event: Event) {
    // We're interested in Ctrl+CAPSLOCK for recording and Alt+CAPSLOCK for toggling translate mode
    match event.event_type {
        EventType::KeyPress(Key::ControlLeft) | EventType::KeyPress(Key::ControlRight) => {
            *CTRL_PRESSED.lock().unwrap() = true;
        }
        EventType::KeyRelease(Key::ControlLeft) | EventType::KeyRelease(Key::ControlRight) => {
            *CTRL_PRESSED.lock().unwrap() = false;
            // Send CtrlCapsLockReleased event when Ctrl is released
            if let Some(sender) = &*KEYBOARD_EVENT_SENDER.lock().unwrap() {
                let _ = sender.send(KeyboardEvent::CtrlCapsLockReleased);
            }
        }
        EventType::KeyPress(Key::Alt) | EventType::KeyPress(Key::AltGr) => {
            *ALT_PRESSED.lock().unwrap() = true;
        }
        EventType::KeyRelease(Key::Alt) | EventType::KeyRelease(Key::AltGr) => {
            *ALT_PRESSED.lock().unwrap() = false;
        }
        EventType::KeyPress(Key::CapsLock) => {
            let ctrl = *CTRL_PRESSED.lock().unwrap();
            let alt = *ALT_PRESSED.lock().unwrap();
            if alt {
                if let Some(sender) = &*KEYBOARD_EVENT_SENDER.lock().unwrap() {
                    let _ = sender.send(KeyboardEvent::AltCapsToggleTranslate);
                }
            } else if ctrl {
                if let Some(sender) = &*KEYBOARD_EVENT_SENDER.lock().unwrap() {
                    let _ = sender.send(KeyboardEvent::CtrlCapsLockPressed);
                }
            }
        }
        EventType::KeyRelease(Key::CapsLock) => {
            // Send CtrlCapsLockReleased event when CAPSLOCK is released, regardless of Ctrl state
            if let Some(sender) = &*KEYBOARD_EVENT_SENDER.lock().unwrap() {
                let _ = sender.send(KeyboardEvent::CtrlCapsLockReleased);
            }
        }
        _ => {}
    }
}
