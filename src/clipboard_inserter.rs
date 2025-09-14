use arboard::Clipboard;
use rdev::{simulate, EventType, Key};
use std::thread;
use std::time::Duration;

/// Inserts text at the current cursor position by using the clipboard
///
/// This function:
/// 1. Stores the current clipboard content
/// 2. Puts the provided text into the clipboard
/// 3. Simulates Ctrl+V to paste the text
/// 4. Restores the original clipboard content
pub fn insert_text(text: &str) {
    println!("Inserting text via clipboard: {}", text);

    // Create clipboard instance
    let mut clipboard = Clipboard::new().expect("Failed to initialize clipboard");

    // Store current clipboard content (if any)
    let original_content = clipboard.get_text().ok();

    // Set the new text to clipboard
    clipboard
        .set_text(text)
        .expect("Failed to set clipboard content");

    // Give the system a moment to register new clipboard contents
    thread::sleep(Duration::from_millis(120));

    // Best-effort: ensure common modifiers aren't left logically pressed
    let _ = simulate(&EventType::KeyRelease(Key::ControlLeft));
    let _ = simulate(&EventType::KeyRelease(Key::ControlRight));
    let _ = simulate(&EventType::KeyRelease(Key::ShiftLeft));
    let _ = simulate(&EventType::KeyRelease(Key::ShiftRight));
    let _ = simulate(&EventType::KeyRelease(Key::Alt));

    // Paste via Ctrl+V with small inter-event delays for reliability
    paste_ctrl_v();

    // Some apps (e.g., certain IDEs) evaluate the clipboard on key release; wait
    thread::sleep(Duration::from_millis(350));

    // Restore original clipboard content if there was any
    if let Some(content) = original_content {
        // A tiny delay to de-couple from paste completion in slow apps
        thread::sleep(Duration::from_millis(50));
        let _ = clipboard.set_text(content);
    }
}

fn paste_ctrl_v() {
    let _ = simulate(&EventType::KeyPress(Key::ControlLeft));
    thread::sleep(Duration::from_millis(20));
    let _ = simulate(&EventType::KeyPress(Key::KeyV));
    thread::sleep(Duration::from_millis(30));
    let _ = simulate(&EventType::KeyRelease(Key::KeyV));
    thread::sleep(Duration::from_millis(20));
    let _ = simulate(&EventType::KeyRelease(Key::ControlLeft));
}
