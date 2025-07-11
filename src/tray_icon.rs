#[cfg(feature = "tray-icon")]
use gtk::prelude::*;
#[cfg(feature = "tray-icon")]
use gtk::{AboutDialog, Menu, MenuItem};
#[cfg(feature = "tray-icon")]
use libappindicator::{AppIndicator, AppIndicatorStatus};

/// Initialize the system tray icon
/// 
/// This function initializes a system tray icon with a menu that includes
/// "About" and "Quit" options. It should be called once from the main function.
/// 
/// # Returns
/// 
/// Returns `Ok(())` if the tray icon was successfully initialized, or an
/// `Err` with a description of the error otherwise.
#[cfg(feature = "tray-icon")]
pub fn init_tray_icon() -> Result<(), String> {
    gtk::init().map_err(|e| format!("Failed to initialize GTK: {}", e))?;

    let mut indicator = AppIndicator::new("voice_input", "indicator-messages");
    indicator.set_status(AppIndicatorStatus::Active);

    let mut menu = Menu::new();

    let about = MenuItem::with_label("About");
    about.connect_activate(|_| {
        let dialog = AboutDialog::new();
        dialog.set_program_name("Voice Input");
        dialog.set_comments(Some("A simple application for recording voice input using the microphone."));
        dialog.run();
        // dialog.destroy();
    });
    menu.append(&about);

    let quit = MenuItem::with_label("Quit");
    quit.connect_activate(|_| {
        gtk::main_quit();
    });
    menu.append(&quit);

    menu.show_all();
    indicator.set_menu(&mut menu);

    // Return success - we'll process GTK events in the main loop
    Ok(())
}

/// Dummy implementation for when the tray-icon feature is disabled
#[cfg(not(feature = "tray-icon"))]
pub fn init_tray_icon() -> Result<(), String> {
    // Do nothing when the tray-icon feature is disabled
    Ok(())
}
