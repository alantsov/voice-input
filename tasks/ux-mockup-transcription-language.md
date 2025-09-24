# UX Mockup: Transcription Language Setting (Override and Fallback)

Date: 2025-09-19
Owner: UX/PM
Status: Draft for implementation planning
Scope: Documentation only (no code changes in this task)

## Summary
Add a user-facing setting to control the language used for speech transcription. This setting should:
- Allow users to explicitly select a transcription language that overrides the keyboard layout language.
- Provide an Auto mode that uses the current keyboard layout when detectable.
- When keyboard layout cannot be detected, the app falls back to English (US); this fallback is fixed and not user-configurable.
- Persist across sessions in the existing config directory.

This feature ensures stable behavior when keyboard detection fails and offers predictability for multilingual users.

## Goals
- Users can set a fixed transcription language (Manual/Forced mode) that always applies, regardless of keyboard layout.
- Users can keep Auto mode; when detection fails, the app automatically uses English (US) as a fixed fallback.
- Quick access via tray menu (when tray-icon feature is enabled) and a more detailed Settings dialog.
- Non-tray environments have guidance to set via config file until a CLI flag is considered in future.

## Non-Goals
- Implementing model download logic for every language in this task (existing model selection mechanisms apply).
- Automatic language detection from audio (out of scope).

## Key Concepts and Precedence
- Mode:
  - Auto (default): Use keyboard layout if available; otherwise fall back to English (US).
  - Manual (Forced): Always use the selected language; ignore keyboard layout entirely.
- Precedence order:
  1) Manual (Forced) language if Manual mode is selected.
  2) If Auto mode: keyboard layout language if detectable.
  3) If Auto mode and keyboard not detectable: English (US) fallback.

## Entry Points
- Tray menu (feature `tray-icon`): quick toggles.
- Settings dialog (if `tray-icon`): full controls (mode + language selectors).
- Headless/non-tray: documented config.json keys and values.

## Tray Menu (Quick Controls)
When the tray icon feature is enabled:

Tray menu structure:
- Voice Input
  - Transcription Language ▶
    - Auto (Keyboard)  [ ]  (radio)
    - Manual (Forced)  [x]  (radio)
    - —
    - Current: English (US)  (disabled label)
    - Choose Language…      (opens Settings dialog in Manual mode)
  - Settings…
  - Quit

Behavior notes:
- Auto/Manual are mutually exclusive radio items.
- Current shows the effective language based on precedence.
- Choose Language… opens Settings dialog anchored to Manual language field.

### Tray Menu Wireframe (text)
[Transcription Language >]
  ( ) Auto (Keyboard)
  (•) Manual (Forced)
  ————————————————
  Current: English (US)
  Choose Language…

Legend: (•) selected radio, ( ) unselected.

## Settings Dialog
Accessible from tray “Settings…” or quick entries above.

Dialog sections:
- Transcription Language
  - Mode: [ Auto (Keyboard) | Manual (Forced) ]  (segmented control or radio group)
  - When Auto is selected:
    - Keyboard layout: <detected name or “Not detected”>
    - When keyboard is not detected, the app will use English (US).
  - When Manual is selected:
    - Language: [ Searchable dropdown ]
- Apply | Cancel

### Settings Dialog Wireframe
+———————————————————————————————+
| Settings — Transcription Language         |
|                                           |
| Mode:  [ Auto (Keyboard) ] [ Manual ]     |
|                                           |
| Keyboard layout: English (US)             |
| When keyboard is not detected: English (US) will be used |
|                                           |
|                     [Cancel] [Apply]      |
+———————————————————————————————+

Manual mode variant:

+———————————————————————————————+
| Settings — Transcription Language         |
|                                           |
| Mode:  [ Auto (Keyboard) ] [ Manual ]     |
|                                           |
| Language: [ German (DE)  ▾ ]              |
|                                           |
|                     [Cancel] [Apply]      |
+———————————————————————————————+

### Dropdown Behavior
- Searchable: type-ahead filter by language name and code (e.g., “English”, “EN”, “en-US”).
- Show language name and region code. Example: “English (US) — en-US”.
- List is sorted alphabetically.

## States and Edge Cases
- First run: Mode defaults to Auto. Fallback is always English (US).
- Keyboard layout unavailable: Dialog shows “Not detected” and indicates that English (US) will be used.
- Missing model for chosen language: Show non-blocking note in dialog: “Model may download on first use or fall back to base model.” Do not block selection.
- Headless/no tray: Only config file is used; document keys below.
- Hotkey recording flow is unchanged by this feature.

## Persistence (Config)
Config path: ~/.config/voice_input/config.json

Proposed keys (backward-compatible; implementation can map to existing structures):
- language_mode: "auto" | "manual"
- language_manual: "en-US"   (IETF BCP 47 code; examples: en, en-US, de, es, fr-FR)

Notes:
- If language_mode == "manual", the app uses language_manual.
- If language_mode == "auto" and keyboard detection fails, English (US) is used as a fixed fallback.
- If language_mode == "auto" and keyboard is detected, use keyboard language (existing behavior).

This task does not change code; keys are defined for implementation reference.

## Accessibility
- Keyboard navigation fully supported: focus order, arrow keys for radios, type-ahead dropdown.
- High-contrast icons/text; labels include language codes for clarity.
- Screen reader: include descriptive labels, e.g., “Mode: Auto (use keyboard layout)” and “When keyboard not detected, English (US) will be used.”

## Localization of UI
- UI text can be in English initially; language names are localized if available in system locale in future work.

## Error Handling
- If saving config fails: surface a non-blocking toast/message: “Could not save settings. Changes will not persist.” with retry guidance.
- If language list fails to load: show minimal set (English) and a retry control.

## Telemetry (optional, future)
- No telemetry

## Acceptance Criteria
- User can select Manual (Forced) language; selection overrides keyboard layout.
- In Auto mode, if keyboard not detected, English (US) is used as the fallback.
- Tray menu reflects mode and current effective language.
- Settings dialog shows correct fields for each mode and preserves choices on reopen.
- Choices persist across app restarts via config.
- No change to behavior when left in Auto and keyboard is detectable.

## Test Checklist (Manual QA)
- Default: Start app with no prior config; verify Auto mode and an informational English (US) fallback note is visible.
- Auto + detected keyboard: confirm transcription language matches keyboard.
- Auto + no keyboard detection: confirm transcription uses English (US) as fallback.
- Manual mode set to German: regardless of keyboard, transcription uses German.
- Restart app; verify selections persist.

## Open Questions
- What is the canonical list of supported languages and mapping to available models? (Leverage existing transcriber_utils/select_model_file.)
- Should we expose per-session override separate from persistent setting? (e.g., quick switch resets on restart.)
- CLI flag for headless environments? (e.g., --lang-mode manual --lang de). Not in scope for this task.
