#[cfg(feature = "tray-icon")]
use gtk::prelude::*;
#[cfg(feature = "tray-icon")]
use gtk::{AboutDialog, Menu, MenuItem, SeparatorMenuItem, CheckMenuItem};
#[cfg(feature = "tray-icon")]
use libappindicator::{AppIndicator, AppIndicatorStatus};
#[cfg(feature = "tray-icon")]
use glib;
#[cfg(feature = "tray-icon")]
use glib::ControlFlow;
#[cfg(feature = "tray-icon")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "tray-icon")]
use std::path::Path;
#[cfg(feature = "tray-icon")]
use std::thread;

#[cfg(feature = "tray-icon")]
use crate::{SELECTED_MODEL, MODEL_LOADING};
#[cfg(feature = "tray-icon")]
use crate::whisper::WhisperTranscriber;

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

    // Model selection submenu
    // Get the current selected model
    let current_model = SELECTED_MODEL.lock().unwrap().clone();
    let model_menu_item = MenuItem::with_label(&format!("Model: {}", current_model));
    let model_menu = Menu::new();

    // Create model options
    let model_options = vec!["base", "small", "medium", "large"];
    let mut model_items = Vec::new();

    // Add model options to the menu
    for model in &model_options {
        let item = CheckMenuItem::with_label(model);

        // Set the base model as selected by default
        if *model == "base" {
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

                // Update the model menu item label
                model_menu_item_clone.set_label(&format!("Model: {}", model_clone));

                // Get both English and multilingual model filenames
                let (en_model_file, multi_model_file) = get_both_model_filenames(&model_clone);

                // Check if either model file doesn't exist
                let en_exists = Path::new(&en_model_file).exists();
                let multi_exists = Path::new(&multi_model_file).exists();

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

                    // We'll use a simpler approach without capturing UI elements
                    // The download thread will update MODEL_LOADING when it's done

                    // Download the models in a separate thread
                    thread::spawn(move || {
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

                        // Reset loading flag
                        *MODEL_LOADING.lock().unwrap() = false;
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

#[cfg(feature = "tray-icon")]
fn get_model_filename(model: &str) -> String {
    match model {
        "base" | "small" | "medium" => {
            // For base, small, medium: both English and multilingual models
            // Check if we need English or multilingual model based on the detected language
            let language = crate::CURRENT_LANGUAGE.with(|lang| lang.borrow().clone());
            if language == "en" {
                format!("ggml-{}.en.bin", model)
            } else {
                format!("ggml-{}.bin", model)
            }
        },
        "large" => {
            // For large: only multilingual model
            format!("ggml-{}-v3-turbo.bin", model)
        },
        _ => {
            // Default to base model
            "ggml-base.bin".to_string()
        }
    }
}

#[cfg(feature = "tray-icon")]
fn get_both_model_filenames(model: &str) -> (String, String) {
    match model {
        "base" | "small" | "medium" => {
            // For base, small, medium: both English and multilingual models
            (
                format!("ggml-{}.en.bin", model),
                format!("ggml-{}.bin", model)
            )
        },
        "large" => {
            // For large: only multilingual model
            (
                format!("ggml-{}-v3-turbo.bin", model), // No English-specific version for large
                format!("ggml-{}-v3-turbo.bin", model)
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
