use sys_locale::get_locale;

pub struct KeyboardLayoutDetector;

impl KeyboardLayoutDetector {
    /// Detect the current keyboard layout and return its language code
    pub fn detect_language() -> Result<String, String> {
        let locale = get_locale().unwrap_or_else(|| String::from("en-US"));

        // Try to detect the active keyboard layout using xkb-switch
        if let Some(lang) = Self::try_xkb_switch() {
            println!("Detected keyboard layout language from xkb-switch: {}", lang);
            return Ok(lang);
        }

        // Fall back to /etc/default/keyboard if xkb-switch failed
        println!("Falling back to /etc/default/keyboard");
        Self::try_keyboard_config(&locale)
    }

    fn try_xkb_switch() -> Option<String> {
        let output = std::process::Command::new("xkb-switch").output().ok()?;
        
        if !output.status.success() {
            println!("xkb-switch command failed, falling back to /etc/default/keyboard");
            return None;
        }

        let layout_code = String::from_utf8_lossy(&output.stdout).trim().to_string();
        println!("xkb-switch output: {}", layout_code);

        // Map layout codes to language codes
        let lang = match layout_code.as_str() {
            "us" | "gb" => "en".to_string(),
            "de" => "de".to_string(),
            "fr" => "fr".to_string(),
            "es" => "es".to_string(),
            "it" => "it".to_string(),
            "ru" => "ru".to_string(),
            _ => {
                println!("Unknown keyboard layout: {}, falling back to /etc/default/keyboard", layout_code);
                return None;
            }
        };

        Some(lang)
    }

    fn try_keyboard_config(locale: &str) -> Result<String, String> {
        let content = std::fs::read_to_string("/etc/default/keyboard")
            .map_err(|e| format!("Could not read keyboard configuration: {}, falling back to locale", e))?;

        // Look for XKBLAYOUT=xx pattern
        if let Some(layout_line) = content.lines().find(|line| line.starts_with("XKBLAYOUT=")) {
            let layout_code = layout_line.trim_start_matches("XKBLAYOUT=").trim_matches('"');
            println!("Found layout code in /etc/default/keyboard: {}", layout_code);

            let lang = match layout_code {
                "us" | "gb" => "en".to_string(),
                "de" => "de".to_string(),
                "fr" => "fr".to_string(),
                "es" => "es".to_string(),
                "it" => "it".to_string(),
                "ru" => "ru".to_string(),
                _ => {
                    println!("Unknown keyboard layout: {}, falling back to locale", layout_code);
                    Self::fallback_to_locale(locale)
                }
            };

            return Ok(lang);
        }

        println!("Could not find XKBLAYOUT in keyboard configuration, falling back to locale");
        Ok(Self::fallback_to_locale(locale))
    }

    fn fallback_to_locale(locale: &str) -> String {
        if locale.len() >= 2 {
            locale[0..2].to_string()
        } else {
            "en".to_string()
        }
    }
}