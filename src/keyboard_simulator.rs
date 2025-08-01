use enigo::{Enigo, Keyboard};

// Function to simulate typing text at the current cursor position
pub fn simulate_typing(text: &str) {
    println!("Simulating typing: {}", text);

    // Create a new Enigo instance
    let enigo = Enigo::new(&enigo::Settings::default());
    enigo.unwrap().fast_text(text).unwrap();

    // Type the text character by character
    // for c in text.chars() {
    //     // Type the character
    //     enigo.key_sequence(&c.to_string());
    //
    //     // Add a small delay between keystrokes
    //     thread::sleep(Duration::from_millis(5));
    // }
}