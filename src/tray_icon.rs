#[cfg(feature = "tray-icon")]
use gtk::prelude::*;
#[cfg(feature = "tray-icon")]
use gtk::{AboutDialog, Menu, MenuItem, SeparatorMenuItem, CheckMenuItem};
#[cfg(feature = "tray-icon")]
use libappindicator::{AppIndicator, AppIndicatorStatus};
#[cfg(feature = "tray-icon")]
use std::path::Path;
#[cfg(feature = "tray-icon")]
use std::process;
#[cfg(feature = "tray-icon")]
use std::thread;

#[cfg(feature = "tray-icon")]
use crate::{SELECTED_MODEL, MODEL_LOADING};
#[cfg(feature = "tray-icon")]
use crate::whisper::WhisperTranscriber;
#[cfg(feature = "tray-icon")]
use crate::config;
#[cfg(feature = "tray-icon")]
use gtk::glib::{self, Priority, ControlFlow};
#[cfg(feature = "tray-icon")]
use lazy_static::lazy_static;
#[cfg(feature = "tray-icon")]
use std::sync::Mutex;
#[cfg(feature = "tray-icon")]
use std::collections::HashMap;

#[cfg(feature = "tray-icon")]
lazy_static! {
    static ref TRAY_UI_TX: Mutex<Option<glib::Sender<String>>> = Mutex::new(None);
}

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
fn format_eta(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}

#[cfg(feature = "tray-icon")]
pub fn init_tray_icon() -> Result<(), String> {
    gtk::init().map_err(|e| format!("Failed to initialize GTK: {}", e))?;

    let mut indicator = AppIndicator::new("voice_input", "indicator-messages");
    indicator.set_status(AppIndicatorStatus::Active);

    let mut menu = Menu::new();

    // Model selection submenu
    // Get the current selected model
    let current_model = SELECTED_MODEL.lock().unwrap().clone();
    let model_menu_item = MenuItem::with_label(&format!("Model: {}", current_model));
    let model_menu = Menu::new();

    // Create model options
    let model_options = vec!["tiny", "base", "small", "medium", "large"];
    let mut model_items: Vec<CheckMenuItem> = Vec::new();

    // Channel to update UI from background threads
    let (tx, rx) = glib::MainContext::channel::<String>(Priority::DEFAULT);
    *TRAY_UI_TX.lock().unwrap() = Some(tx);

    // Add model options to the menu
    for model in &model_options {
        let item = CheckMenuItem::with_label(model);

        // Set the model as active if it matches the current selected model
        if *model == current_model {
            item.set_active(true);
        }

        // Clone model for the closure
        let model_clone = model.to_string();

        // Clone model_menu_item for the closure
        let model_menu_item_clone = model_menu_item.clone();

        // Connect the activate signal
        item.connect_activate(move |check_item| {
            // Only process if the item is being activated (not deactivated)
            if check_item.is_active() {
                // Update the selected model
                let mut selected_model = SELECTED_MODEL.lock().unwrap();
                *selected_model = model_clone.clone();

                // Save the selected model to the config file
                if let Err(e) = config::save_selected_model(&model_clone) {
                    eprintln!("Failed to save selected model to config file: {}", e);
                } else {
                    println!("Saved selected model '{}' to config file", model_clone);
                }

                // Update the model menu item label
                model_menu_item_clone.set_label(&format!("Model: {}", model_clone));

                // Get both English and multilingual model filenames
                let (en_model_file, multi_model_file) = get_both_model_filenames(&model_clone);

                // Check if either model file doesn't exist in XDG data directory or current directory
                let en_exists = config::get_model_path(&en_model_file).is_some();
                let multi_exists = config::get_model_path(&multi_model_file).is_some();

                if !en_exists || !multi_exists {
                    // Set loading flag
                    *MODEL_LOADING.lock().unwrap() = true;

                    // Update menu items to show loading status
                    check_item.set_label(&format!("{} (loading...)", model_clone));
                    model_menu_item_clone.set_label(&format!("Model: {} (loading...)", model_clone));

                    // Clone for the thread
                    let en_model_file_clone = en_model_file.clone();
                    let multi_model_file_clone = multi_model_file.clone();
                    let model_clone_thread = model_clone.clone();

                    // We'll use a simpler approach where the download thread sends a message
                    // to the GTK thread to update the UI when done

                    // Download the models in a separate thread
                    thread::spawn(move || {
                        // Register progress callback to forward updates to GTK thread
                        WhisperTranscriber::set_download_progress_callback(Some(Box::new({
                            let model_for_cb = model_clone_thread.clone();
                            move |percent, eta_secs| {
                                if let Some(ref tx) = *TRAY_UI_TX.lock().unwrap() {
                                    // Send compact progress message: P|model|percent|eta_secs
                                    let _ = tx.send(format!("P|{}|{:.0}|{}", model_for_cb, percent, eta_secs));
                                }
                            }
                        })));

                        // Download English model if it doesn't exist
                        if !en_exists && model_clone_thread != "large" {
                            println!("Downloading English model: {}", en_model_file_clone);
                            if let Err(e) = WhisperTranscriber::download_model(&en_model_file_clone) {
                                eprintln!("Failed to download English model {}: {}", model_clone_thread, e);
                            }
                        }

                        // Download multilingual model if it doesn't exist
                        if !multi_exists {
                            println!("Downloading multilingual model: {}", multi_model_file_clone);
                            if let Err(e) = WhisperTranscriber::download_model(&multi_model_file_clone) {
                                eprintln!("Failed to download multilingual model {}: {}", model_clone_thread, e);
                            }
                        }

                        // Clear the progress callback
                        WhisperTranscriber::set_download_progress_callback(None);

                        // Reset loading flag
                        *MODEL_LOADING.lock().unwrap() = false;

                        // Notify GTK thread to update the tray menu (if channel is available)
                        if let Some(ref tx) = *TRAY_UI_TX.lock().unwrap() {
                            let _ = tx.send(model_clone_thread.clone());
                        }
                    });
                }
            }
        });

        model_menu.append(&item);
        model_items.push(item);
    }

    model_menu_item.set_submenu(Some(&model_menu));
    menu.append(&model_menu_item);

    // Add a separator
    let separator = SeparatorMenuItem::new();
    menu.append(&separator);

    let about = MenuItem::with_label("About");
    about.connect_activate(|_| {
        let dialog = AboutDialog::new();
        dialog.set_program_name("Voice Input");
        dialog.set_comments(Some("A simple application for recording voice input using the microphone.\n\n\
                                 • Press Ctrl+CAPSLOCK to start and finish recording\n\
                                 • Transcribed text will be inserted into the current application\n\
                                 • Transcription language is determined by your current keyboard layout"));
        dialog.run();
        // dialog.destroy();
    });
    menu.append(&about);

    let quit = MenuItem::with_label("Quit");
    quit.connect_activate(|_| {
        // Use std::process::exit instead of gtk::main_quit to ensure the application terminates
        // regardless of how it's installed or run
        std::process::exit(0);
    });
    menu.append(&quit);

    // Attach the receiver to update UI when model finishes loading
    {
        // Build a map from model name to its menu item for easy updates
        let mut items_map: HashMap<String, CheckMenuItem> = HashMap::new();
        for (i, name) in model_options.iter().enumerate() {
            if let Some(item) = model_items.get(i) {
                items_map.insert((*name).to_string(), item.clone());
            }
        }
        let model_menu_item_for_rx = model_menu_item.clone();
        rx.attach(None, move |msg: String| {
            // Handle progress messages: "P|model|percent|eta_secs"
            if let Some(rest) = msg.strip_prefix("P|") {
                let parts: Vec<&str> = rest.split('|').collect();
                if parts.len() == 3 {
                    let model = parts[0];
                    let percent = parts[1];
                    let eta_secs: u64 = parts[2].parse().unwrap_or(0);
                    let eta_formatted = format_eta(eta_secs);
                    model_menu_item_for_rx.set_label(&format!("Model: {} ({}% - {} left)", model, percent, eta_formatted));
                    if let Some(item) = items_map.get(model) {
                        item.set_label(&format!("{} ({}% - {} left)", model, percent, eta_formatted));
                    }
                }
            } else {
                let model_name = msg;
                // Update the top label
                model_menu_item_for_rx.set_label(&format!("Model: {}", model_name));
                // Update each check item label and active state
                for (name, item) in items_map.iter() {
                    item.set_label(name);
                    item.set_active(name == &model_name);
                }
            }
            ControlFlow::Continue
        });
    }

    menu.show_all();
    indicator.set_menu(&mut menu);

    // Return success - we'll process GTK events in the main loop
    Ok(())
}


#[cfg(feature = "tray-icon")]
fn get_both_model_filenames(model: &str) -> (String, String) {
    match model {
        "base" | "tiny" | "small" | "medium" => {
            // For base, tiny, small, medium: both English and multilingual models
            (
                format!("ggml-{}.en.bin", model),
                format!("ggml-{}.bin", model)
            )
        },
        "large" => {
            // For large: only multilingual model
            (
                format!("ggml-{}-v2.bin", model), // No English-specific version for large
                format!("ggml-{}-v2.bin", model)
            )
        },
        _ => {
            // Default to base model
            (
                "ggml-base.en.bin".to_string(),
                "ggml-base.bin".to_string()
            )
        }
    }
}

/// Dummy implementation for when the tray-icon feature is disabled
#[cfg(not(feature = "tray-icon"))]
pub fn init_tray_icon() -> Result<(), String> {
    // Do nothing when the tray-icon feature is disabled
    Ok(())
}
