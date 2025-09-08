#[cfg(feature = "tray-icon")]
use gtk::prelude::*;
#[cfg(feature = "tray-icon")]
use gtk::{AboutDialog, Menu, MenuItem, SeparatorMenuItem, CheckMenuItem};
#[cfg(feature = "tray-icon")]
use libappindicator::{AppIndicator, AppIndicatorStatus};
#[cfg(feature = "tray-icon")]
use gtk::glib::{self, Priority, ControlFlow};
#[cfg(feature = "tray-icon")]
use std::collections::HashMap;
#[cfg(feature = "tray-icon")]
use std::cell::RefCell;
#[cfg(feature = "tray-icon")]
use std::path::Path;
#[cfg(feature = "tray-icon")]
use std::rc::Rc;
#[cfg(feature = "tray-icon")]
use std::sync::{Mutex, mpsc::Sender};

#[cfg(feature = "tray-icon")]
use lazy_static::lazy_static;

#[cfg(feature = "tray-icon")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayStatus {
    Ready,
    Recording,
    Processing,
}

#[cfg(feature = "tray-icon")]
#[derive(Debug, Clone)]
pub struct ModelProgress {
    pub percent: u8,
    pub eta_secs: u64,
}

#[cfg(feature = "tray-icon")]
#[derive(Debug, Clone)]
pub struct AppView {
    pub active_model: String,
    pub status: TrayStatus,
    pub loading: HashMap<String, ModelProgress>,
    pub translate_enabled: bool,
}

// Intents from tray UI to app thread
#[cfg(feature = "tray-icon")]
#[derive(Debug, Clone)]
pub enum UiIntent {
    SelectModel(String),
    ToggleTranslate(bool),
    QuitRequested,
}

#[cfg(feature = "tray-icon")]
lazy_static! {
    // Channel for app -> tray snapshots
    static ref TRAY_UI_TX: Mutex<Option<glib::Sender<AppView>>> = Mutex::new(None);
}

#[cfg(feature = "tray-icon")]
fn icon_name_for_status(status: TrayStatus, translate: bool) -> &'static str {
    match (status, translate) {
        (TrayStatus::Ready, false) => "voice-input-white",
        (TrayStatus::Recording, false) => "voice-input-red",
        (TrayStatus::Processing, false) => "voice-input-blue",
        (TrayStatus::Ready, true) => "voice-input-translate-white",
        (TrayStatus::Recording, true) => "voice-input-translate-red",
        (TrayStatus::Processing, true) => "voice-input-translate-blue",
    }
}

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
pub fn init_tray_icon(intents_tx: Sender<UiIntent>, initial_model: String, initial_translate: bool) -> Result<(), String> {
    gtk::init().map_err(|e| format!("Failed to initialize GTK: {}", e))?;

    let indicator = Rc::new(RefCell::new(AppIndicator::new("voice_input", "indicator-messages")));
    indicator.borrow_mut().set_status(AppIndicatorStatus::Active);

    // Prefer icons from assets/icons/hicolor/48x48/apps
    let mut theme_set = false;
    let preferred_subpath = Path::new("assets").join("icons").join("hicolor").join("48x48").join("apps");
    if let Ok(cwd) = std::env::current_dir() {
        let candidate = cwd.join(&preferred_subpath);
        if candidate.exists() {
            if let Some(dir_str) = candidate.to_str() {
                indicator.borrow_mut().set_icon_theme_path(dir_str);
                theme_set = true;
            }
        }
    }
    if !theme_set {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let candidate = exe_dir.join(&preferred_subpath);
                if candidate.exists() {
                    if let Some(dir_str) = candidate.to_str() {
                        indicator.borrow_mut().set_icon_theme_path(dir_str);
                    }
                }
            }
        }
    }

    // Default icon: white (ready), respect initial_translate
    indicator.borrow_mut().set_icon(if initial_translate { "voice-input-translate-white" } else { "voice-input-white" });

    let mut menu = Menu::new();

    // Model submenu
    let model_menu_item = MenuItem::with_label(&format!("Model: {}", initial_model));
    let model_menu = Menu::new();
    let model_options = vec!["tiny", "base", "small", "medium", "large"];
    let mut model_items: Vec<CheckMenuItem> = Vec::new();

    // Channel for AppView snapshots
    let (tx, rx) = glib::MainContext::channel::<AppView>(Priority::DEFAULT);
    *TRAY_UI_TX.lock().unwrap() = Some(tx);

    for model in &model_options {
        let item = CheckMenuItem::with_label(model);
        item.set_active(*model == initial_model);

        let model_clone = model.to_string();
        let intents_tx_clone = intents_tx.clone();
        item.connect_activate(move |check_item| {
            if check_item.is_active() {
                let _ = intents_tx_clone.send(UiIntent::SelectModel(model_clone.clone()));
            }
        });

        model_menu.append(&item);
        model_items.push(item);
    }

    model_menu_item.set_submenu(Some(&model_menu));
    menu.append(&model_menu_item);

    // Separator
    menu.append(&SeparatorMenuItem::new());

    // Translate to English checkbox
    let translate_item = CheckMenuItem::with_label("Translate to English");
    translate_item.set_active(initial_translate);
    {
        let intents_tx_clone = intents_tx.clone();
        translate_item.connect_toggled(move |item| {
            let _ = intents_tx_clone.send(UiIntent::ToggleTranslate(item.is_active()));
        });
    }
    menu.append(&translate_item);

    let about = MenuItem::with_label("About");
    about.connect_activate(|_| {
        let dialog = AboutDialog::new();
        dialog.set_program_name("Voice Input");
        dialog.set_comments(Some("A simple application for recording voice input using the microphone.\n\n\
                                 • Press Ctrl+CAPSLOCK to start and finish recording\n\
                                 • Press Alt+CAPSLOCK to toggle translation mode\n\
                                 • Transcribed text will be inserted into the current application\n\
                                 • Transcription language is determined by your current keyboard layout"));
        dialog.run();
    });
    menu.append(&about);

    let quit = MenuItem::with_label("Quit");
    {
        let intents_tx_clone = intents_tx.clone();
        quit.connect_activate(move |_| {
            let _ = intents_tx_clone.send(UiIntent::QuitRequested);
        });
    }
    menu.append(&quit);

    // Apply AppView updates
    {
        let mut items_map: HashMap<String, CheckMenuItem> = HashMap::new();
        for (i, name) in model_options.iter().enumerate() {
            if let Some(item) = model_items.get(i) {
                items_map.insert((*name).to_string(), item.clone());
            }
        }
        let indicator_for_rx = indicator.clone();
        let model_menu_item_for_rx = model_menu_item.clone();
        let translate_item_for_rx = translate_item.clone();

        rx.attach(None, move |view: AppView| {
            // Update icon based on status and translate mode
            indicator_for_rx.borrow_mut().set_icon(icon_name_for_status(view.status, view.translate_enabled));

            // Build top label and update items (show progress where available)
            let mut top_label = format!("Model: {}", view.active_model);

            for (name, item) in items_map.iter() {
                let is_active = *name == view.active_model;
                item.set_active(is_active);

                if let Some(p) = view.loading.get(name) {
                    let eta = format_eta(p.eta_secs);
                    item.set_label(&format!("{} ({}% - {} left)", name, p.percent, eta));
                    if is_active {
                        top_label = format!("Model: {} ({}% - {} left)", name, p.percent, eta);
                    }
                } else {
                    item.set_label(name);
                }
            }

            // Reflect translate toggle state in the checkbox
            translate_item_for_rx.set_active(view.translate_enabled);

            model_menu_item_for_rx.set_label(&top_label);
            ControlFlow::Continue
        });
    }

    menu.show_all();
    indicator.borrow_mut().set_menu(&mut menu);
    Ok(())
}

// Single snapshot update from app thread
#[cfg(feature = "tray-icon")]
pub fn tray_post_view(view: AppView) {
    if let Some(ref tx) = *TRAY_UI_TX.lock().unwrap() {
        let _ = tx.send(view);
    }
}

// Stubs for non-tray builds
#[cfg(not(feature = "tray-icon"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayStatus { Ready, Recording, Processing }
#[cfg(not(feature = "tray-icon"))]
#[derive(Debug, Clone)]
pub struct ModelProgress { pub percent: u8, pub eta_secs: u64 }
#[cfg(not(feature = "tray-icon"))]
#[derive(Debug, Clone)]
pub struct AppView { pub active_model: String, pub status: TrayStatus, pub loading: std::collections::HashMap<String, ModelProgress>, pub translate_enabled: bool }
#[cfg(not(feature = "tray-icon"))]
#[derive(Debug, Clone)]
pub enum UiIntent { SelectModel(String), ToggleTranslate(bool), QuitRequested }
#[cfg(not(feature = "tray-icon"))]
pub fn init_tray_icon(_: std::sync::mpsc::Sender<UiIntent>, _: String, _: bool) -> Result<(), String> { Ok(()) }
#[cfg(not(feature = "tray-icon"))]
pub fn tray_post_view(_: AppView) {}
