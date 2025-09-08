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

    // Small delay to ensure clipboard is updated
    thread::sleep(Duration::from_millis(50));

    // Simulate Ctrl+V to paste using rdev
    // Press Ctrl
    let _ = simulate(&EventType::KeyPress(Key::ControlLeft));
    // Press V
    let _ = simulate(&EventType::KeyPress(Key::KeyV));
    // Release V
    let _ = simulate(&EventType::KeyRelease(Key::KeyV));
    // Release Ctrl
    let _ = simulate(&EventType::KeyRelease(Key::ControlLeft));

    // Small delay to ensure paste operation completes
    thread::sleep(Duration::from_millis(50));

    // Restore original clipboard content if there was any
    if let Some(content) = original_content {
        clipboard
            .set_text(content)
            .expect("Failed to restore clipboard content");
    }
}
