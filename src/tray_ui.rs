#[cfg(feature = "tray-icon")]
use gtk::glib::{self, ControlFlow, Priority};
#[cfg(feature = "tray-icon")]
use gtk::prelude::*;
#[cfg(feature = "tray-icon")]
use gtk::{AboutDialog, CheckMenuItem, Menu, MenuItem, SeparatorMenuItem, RadioMenuItem, Window, Label, WindowType, Box as GtkBox, Orientation, RadioButton, Entry};
#[cfg(feature = "tray-icon")]
use gtk::gdk::{self, ModifierType};
#[cfg(feature = "tray-icon")]
use libappindicator::{AppIndicator, AppIndicatorStatus};
#[cfg(feature = "tray-icon")]
use std::cell::RefCell;
#[cfg(feature = "tray-icon")]
use std::collections::HashMap;
#[cfg(feature = "tray-icon")]
use std::path::Path;
#[cfg(feature = "tray-icon")]
use std::rc::Rc;
#[cfg(feature = "tray-icon")]
use std::sync::{mpsc::Sender, Mutex};

#[cfg(feature = "tray-icon")]
use lazy_static::lazy_static;

#[cfg(feature = "tray-icon")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayStatus {
    Priming,
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
        (TrayStatus::Priming, false) => "voice-input-yellow",
        (TrayStatus::Ready, false) => "voice-input-white",
        (TrayStatus::Recording, false) => "voice-input-red",
        (TrayStatus::Processing, false) => "voice-input-blue",
        (TrayStatus::Priming, true) => "voice-input-translate-yellow",
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
fn is_modifier_keyval(keyval: gtk::gdk::keys::Key) -> bool {
    use gtk::gdk::keys::constants as key;
    matches!(
        keyval,
        key::Shift_L
            | key::Shift_R
            | key::Control_L
            | key::Control_R
            | key::Alt_L
            | key::Alt_R
            | key::Meta_L
            | key::Meta_R
            | key::Super_L
            | key::Super_R
            | key::Hyper_L
            | key::Hyper_R
            | key::ISO_Level3_Shift
            | key::ISO_Level5_Shift
    )
}

#[cfg(feature = "tray-icon")]
fn keyval_to_pretty(keyval: gtk::gdk::keys::Key) -> Option<String> {
    // Letters: make uppercase single letter
    if let Some(ch) = keyval.to_unicode() {
        if ch.is_ascii_alphabetic() {
            return Some(ch.to_ascii_uppercase().to_string());
        }
        if ch.is_ascii_digit() {
            return Some(ch.to_string());
        }
    }
    // F1..F24 and named keys via the Key's name
    if let Some(name) = keyval.name() {
        // Some environments map CapsLock to ISO_Next_Group (layout switch). Normalize to CapsLock.
        if name == "ISO_Next_Group" {
            return Some("CapsLock".to_string());
        }
        let mut s = name.replace('_', "");
        // Normalize casing for some common keys
        // Keep existing casing if it already contains uppercase letters
        if s.chars().all(|c| c.is_lowercase()) {
            // Capitalize first
            if let Some(first) = s.get(..1) {
                s = first.to_uppercase() + s.get(1..).unwrap_or("");
            }
        }
        // A few aliases
        match s.as_str() {
            "Return" => s = "Enter".to_string(),
            "Escape" => s = "Esc".to_string(),
            _ => {}
        }
        return Some(s);
    }
    None
}

#[cfg(feature = "tray-icon")]
fn format_shortcut_from_event(event: &gdk::EventKey) -> Option<String> {
    let keyval = event.keyval();
    if is_modifier_keyval(keyval) {
        return None;
    }
    let state = event.state();
    let mut parts: Vec<&'static str> = Vec::new();
    if state.contains(ModifierType::CONTROL_MASK) {
        parts.push("Ctrl");
    }
    if state.contains(ModifierType::MOD1_MASK) {
        parts.push("Alt");
    }
    if state.contains(ModifierType::SUPER_MASK) {
        parts.push("Super");
    }
    if state.contains(ModifierType::SHIFT_MASK) {
        parts.push("Shift");
    }
    let key_str = keyval_to_pretty(keyval)?;
    // Avoid duplicate Shift for characters that inherently require Shift (like '!' etc.)
    // We keep it simple: always include Shift if pressed.
    let mut out = parts.join("+");
    if !out.is_empty() {
        out.push('+');
    }
    out.push_str(&key_str);
    Some(out)
}

#[cfg(feature = "tray-icon")]
pub fn init_tray_icon(
    intents_tx: Sender<UiIntent>,
    initial_model: String,
    initial_translate: bool,
) -> Result<(), String> {
    gtk::init().map_err(|e| format!("Failed to initialize GTK: {}", e))?;

    let indicator = Rc::new(RefCell::new(AppIndicator::new(
        "voice_input",
        "indicator-messages",
    )));
    indicator
        .borrow_mut()
        .set_status(AppIndicatorStatus::Active);

    // Prefer icons from assets/icons/hicolor/48x48/apps
    let mut theme_set = false;
    let preferred_subpath = Path::new("assets")
        .join("icons")
        .join("hicolor")
        .join("48x48")
        .join("apps");
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
    indicator.borrow_mut().set_icon(if initial_translate {
        "voice-input-translate-white"
    } else {
        "voice-input-white"
    });

    let mut menu = Menu::new();

    // Settings window holder (singleton)
    let settings_window: Rc<RefCell<Option<Window>>> = Rc::new(RefCell::new(None));

    // Model submenu
    let model_menu_item = MenuItem::with_label(&format!("Model: {}", initial_model));
    let model_menu = Menu::new();
    let model_options = vec!["small", "medium", "large"];
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

    // Settings item
    let settings_item = MenuItem::with_label("Settings");
    {
        let settings_window_rc = settings_window.clone();
        settings_item.connect_activate(move |_| {
            // If already created, just present it
            if let Some(ref win) = *settings_window_rc.borrow() {
                win.present();
                return;
            }
            // Create settings window
            let win = Window::new(WindowType::Toplevel);
            win.set_title("Voice Input Settings");
            win.set_default_size(420, 200);

            // Build content
            let vbox = GtkBox::new(Orientation::Vertical, 8);
            vbox.set_margin_top(12);
            vbox.set_margin_bottom(12);
            vbox.set_margin_start(12);
            vbox.set_margin_end(12);

            // Title/description
            let title = Label::new(Some("Whisper compute device (CPU/GPU)"));
            title.set_halign(gtk::Align::Start);
            vbox.pack_start(&title, false, false, 0);

            let subtitle = Label::new(Some("Select which device runs Whisper transcription. GPU requires CUDA build."));
            subtitle.set_halign(gtk::Align::Start);
            vbox.pack_start(&subtitle, false, false, 0);

            // Radio buttons for CPU/GPU
            let rb_cpu = RadioButton::with_label("CPU");
            let rb_gpu = RadioButton::with_label_from_widget(&rb_cpu, "GPU (CUDA)");

            // Initial state from config
            let use_gpu_now = crate::config::use_gpu();
            rb_gpu.set_active(use_gpu_now);
            rb_cpu.set_active(!use_gpu_now);

            // Disable GPU option if not built with CUDA
            #[cfg(not(feature = "cuda"))]
            {
                rb_gpu.set_sensitive(false);
                let note = Label::new(Some("Built without CUDA; GPU unavailable (CPU will be used)."));
                note.set_halign(gtk::Align::Start);
                vbox.pack_start(&note, false, false, 0);
            }

            // Save handlers
            {
                let rb_cpu_clone = rb_cpu.clone();
                rb_cpu.connect_toggled(move |btn| {
                    if btn.is_active() {
                        let _ = crate::config::save_device("cpu");
                        // Ensure mutual exclusivity visually
                        rb_cpu_clone.set_active(true);
                    }
                });
            }
            rb_gpu.connect_toggled(move |btn| {
                if btn.is_active() {
                    let _ = crate::config::save_device("gpu");
                }
            });

            vbox.pack_start(&rb_cpu, false, false, 0);
            vbox.pack_start(&rb_gpu, false, false, 0);

            // Shortcuts section (UI only; not yet used by app logic)
            let shortcuts_title = Label::new(Some("Shortcuts"));
            shortcuts_title.set_halign(gtk::Align::Start);
            vbox.pack_start(&shortcuts_title, false, false, 6);

            // Change mode shortcut
            let change_label = Label::new(Some("Toggle translate/transcribe:"));
            change_label.set_halign(gtk::Align::Start);
            let change_entry = Entry::new();
            change_entry.set_text(&crate::config::get_change_mode_shortcut());
            {
                // Save when text manually edited
                change_entry.connect_changed(|e| {
                    let text = e.text().to_string();
                    let _ = crate::config::save_change_mode_shortcut(&text);
                    // Refresh hotkeys in the listener immediately
                    crate::hotkeys::init_hotkeys_from_config(
                        crate::config::get_record_shortcut(),
                        crate::config::get_change_mode_shortcut(),
                    );
                });
                change_entry.connect_activate(|e| {
                    let text = e.text().to_string();
                    let _ = crate::config::save_change_mode_shortcut(&text);
                    crate::hotkeys::init_hotkeys_from_config(
                        crate::config::get_record_shortcut(),
                        crate::config::get_change_mode_shortcut(),
                    );
                });
                // Capture actual key presses to set shortcut
                change_entry.connect_key_press_event(|e, ev| {
                    if let Some(accel) = format_shortcut_from_event(ev) {
                        e.set_text(&accel);
                        let _ = crate::config::save_change_mode_shortcut(&accel);
                        crate::hotkeys::init_hotkeys_from_config(
                            crate::config::get_record_shortcut(),
                            crate::config::get_change_mode_shortcut(),
                        );
                    }
                    true.into()
                });
            }
            vbox.pack_start(&change_label, false, false, 0);
            vbox.pack_start(&change_entry, false, false, 0);

            // Record shortcut
            let record_label = Label::new(Some("Start/stop recording:"));
            record_label.set_halign(gtk::Align::Start);
            let record_entry = Entry::new();
            record_entry.set_text(&crate::config::get_record_shortcut());
            {
                record_entry.connect_changed(|e| {
                    let text = e.text().to_string();
                    let _ = crate::config::save_record_shortcut(&text);
                    crate::hotkeys::init_hotkeys_from_config(
                        crate::config::get_record_shortcut(),
                        crate::config::get_change_mode_shortcut(),
                    );
                });
                record_entry.connect_activate(|e| {
                    let text = e.text().to_string();
                    let _ = crate::config::save_record_shortcut(&text);
                    crate::hotkeys::init_hotkeys_from_config(
                        crate::config::get_record_shortcut(),
                        crate::config::get_change_mode_shortcut(),
                    );
                });
                // Capture actual key presses to set shortcut
                record_entry.connect_key_press_event(|e, ev| {
                    if let Some(accel) = format_shortcut_from_event(ev) {
                        e.set_text(&accel);
                        let _ = crate::config::save_record_shortcut(&accel);
                        crate::hotkeys::init_hotkeys_from_config(
                            crate::config::get_record_shortcut(),
                            crate::config::get_change_mode_shortcut(),
                        );
                    }
                    true.into()
                });
            }
            vbox.pack_start(&record_label, false, false, 0);
            vbox.pack_start(&record_entry, false, false, 0);

            win.add(&vbox);

            // Keep singleton reference; clear it on destroy
            let settings_window_rc2 = settings_window_rc.clone();
            win.connect_destroy(move |_| {
                *settings_window_rc2.borrow_mut() = None;
            });

            *settings_window_rc.borrow_mut() = Some(win.clone());
            win.show_all();
            win.present();
        });
    }
    menu.append(&settings_item);

    // Separator
    menu.append(&SeparatorMenuItem::new());

    // Transcription mode radio group
    let transcribe_item = RadioMenuItem::with_label("Transcribe");
    let translate_item = RadioMenuItem::with_label_from_widget(&transcribe_item, Some("Translate to English"));

    // Set initial selection
    if initial_translate {
        translate_item.set_active(true);
    } else {
        transcribe_item.set_active(true);
    }

    {
        let intents_tx_clone = intents_tx.clone();
        // Only send when this item becomes active
        transcribe_item.connect_toggled(move |item| {
            if item.is_active() {
                let _ = intents_tx_clone.send(UiIntent::ToggleTranslate(false));
            }
        });
    }
    {
        let intents_tx_clone = intents_tx.clone();
        translate_item.connect_toggled(move |item| {
            if item.is_active() {
                let _ = intents_tx_clone.send(UiIntent::ToggleTranslate(true));
            }
        });
    }

    menu.append(&transcribe_item);
    menu.append(&translate_item);

    // Separator before language preference
    menu.append(&SeparatorMenuItem::new());

    // Language preference radio group (UI-only; not used during transcription)
    let lang_default = RadioMenuItem::with_label("Default language (detected from keyboard layout)");
    let lang_ru = RadioMenuItem::with_label_from_widget(&lang_default, Some("Russian language"));
    let lang_en = RadioMenuItem::with_label_from_widget(&lang_default, Some("English language"));

    // Initial selection from config
    match crate::config::get_language_preference().as_str() {
        "ru" => lang_ru.set_active(true),
        "en" => lang_en.set_active(true),
        _ => lang_default.set_active(true),
    }

    // Save on change (only when item becomes active)
    lang_default.connect_toggled(|item| {
        if item.is_active() {
            let _ = crate::config::save_language_preference("default");
        }
    });
    lang_ru.connect_toggled(|item| {
        if item.is_active() {
            let _ = crate::config::save_language_preference("ru");
        }
    });
    lang_en.connect_toggled(|item| {
        if item.is_active() {
            let _ = crate::config::save_language_preference("en");
        }
    });

    menu.append(&lang_default);
    menu.append(&lang_ru);
    menu.append(&lang_en);

    // Separator after language preference
    menu.append(&SeparatorMenuItem::new());

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
        let transcribe_item_for_rx = transcribe_item.clone();

        rx.attach(None, move |view: AppView| {
            // Update icon based on status and translate mode
            indicator_for_rx
                .borrow_mut()
                .set_icon(icon_name_for_status(view.status, view.translate_enabled));

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

            // Reflect translate mode in radio items
            if view.translate_enabled {
                translate_item_for_rx.set_active(true);
            } else {
                transcribe_item_for_rx.set_active(true);
            }

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
pub enum TrayStatus {
    Priming,
    Ready,
    Recording,
    Processing,
}
#[cfg(not(feature = "tray-icon"))]
#[derive(Debug, Clone)]
pub struct ModelProgress {
    pub percent: u8,
    pub eta_secs: u64,
}
#[cfg(not(feature = "tray-icon"))]
#[derive(Debug, Clone)]
pub struct AppView {
    pub active_model: String,
    pub status: TrayStatus,
    pub loading: std::collections::HashMap<String, ModelProgress>,
    pub translate_enabled: bool,
}
#[cfg(not(feature = "tray-icon"))]
#[derive(Debug, Clone)]
pub enum UiIntent {
    SelectModel(String),
    ToggleTranslate(bool),
    QuitRequested,
}
#[cfg(not(feature = "tray-icon"))]
pub fn init_tray_icon(
    _: std::sync::mpsc::Sender<UiIntent>,
    _: String,
    _: bool,
) -> Result<(), String> {
    Ok(())
}
#[cfg(not(feature = "tray-icon"))]
pub fn tray_post_view(_: AppView) {}
