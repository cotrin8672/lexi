use crate::errors::{AppError, AppErrorCode};
use crate::llm::{self, SelectedTextState, TransformCaptureMetadata};
use crate::selection;
use crate::settings::SettingsState;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub const DEFAULT_SHORTCUT_LABEL: &str = "Ctrl+Shift+X";
const CAPTURE_EVENT: &str = "lexi:capture";

pub struct ShortcutRegistrationState {
    registration_error: Mutex<Option<AppError>>,
    current_shortcut: Mutex<String>,
}

impl Default for ShortcutRegistrationState {
    fn default() -> Self {
        Self {
            registration_error: Mutex::new(None),
            current_shortcut: Mutex::new(DEFAULT_SHORTCUT_LABEL.to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutStatus {
    pub shortcut: String,
    pub registered: bool,
    pub registration_error: Option<AppError>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CaptureEvent {
    Capturing {
        shortcut: String,
    },
    Captured {
        shortcut: String,
        capture_method: &'static str,
        source_process: Option<String>,
        source_window_title: Option<String>,
        character_count: usize,
        multiline: bool,
    },
    Failed {
        shortcut: String,
        error: AppError,
        selection_error_code: String,
        capture_method: Option<&'static str>,
        source_process: Option<String>,
        source_window_title: Option<String>,
    },
}

#[tauri::command]
pub fn get_shortcut_status(state: tauri::State<'_, ShortcutRegistrationState>) -> ShortcutStatus {
    let registration_error = state
        .registration_error
        .lock()
        .expect("shortcut registration state poisoned")
        .clone();
    let shortcut = state
        .current_shortcut
        .lock()
        .expect("shortcut registration state poisoned")
        .clone();

    ShortcutStatus {
        shortcut,
        registered: registration_error.is_none(),
        registration_error,
    }
}

pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    app.handle().plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, shortcut, event| {
                if event.state() == ShortcutState::Pressed {
                    let state = app.state::<ShortcutRegistrationState>();
                    let configured_shortcut = state
                        .current_shortcut
                        .lock()
                        .expect("shortcut registration state poisoned")
                        .clone();
                    if parse_shortcut(&configured_shortcut)
                        .map(|configured| &configured == shortcut)
                        .unwrap_or(false)
                    {
                        start_capture(app.clone(), configured_shortcut);
                    }
                }
            })
            .build(),
    )?;

    let settings_state = app.state::<SettingsState>();
    let shortcut_label = settings_state
        .load_settings(app.handle())
        .map(|settings| settings.shortcut)
        .unwrap_or_else(|_| DEFAULT_SHORTCUT_LABEL.to_string());

    match register_shortcut(app.handle(), &shortcut_label) {
        Ok(()) => Ok(()),
        Err(error) => {
            let state = app.state::<ShortcutRegistrationState>();
            *state
                .registration_error
                .lock()
                .expect("shortcut registration state poisoned") = Some(error.clone());

            show_popup(app.handle());
            let _ = app.handle().emit(
                CAPTURE_EVENT,
                CaptureEvent::Failed {
                    shortcut: shortcut_label,
                    error,
                    selection_error_code: "ShortcutRegistrationFailed".to_string(),
                    capture_method: None,
                    source_process: None,
                    source_window_title: None,
                },
            );

            Ok(())
        }
    }
}

pub fn unregister_all(app: &AppHandle) {
    let _ = app.global_shortcut().unregister_all();
}

pub fn update_registered_shortcut(
    app: &AppHandle,
    next_shortcut: &str,
) -> Result<String, AppError> {
    let state = app.state::<ShortcutRegistrationState>();
    let previous_shortcut = state
        .current_shortcut
        .lock()
        .expect("shortcut registration state poisoned")
        .clone();
    let normalized = normalize_shortcut_label(next_shortcut)?;

    if normalized == previous_shortcut {
        return Ok(normalized);
    }

    if let Err(error) = register_shortcut(app, &normalized) {
        let _ = register_shortcut(app, &previous_shortcut);
        return Err(error);
    }

    Ok(normalized)
}

pub fn normalize_shortcut_label(input: &str) -> Result<String, AppError> {
    let parsed = parse_shortcut_parts(input)?;
    Ok(parsed.label)
}

fn register_shortcut(app: &AppHandle, shortcut_label: &str) -> Result<(), AppError> {
    let shortcut = parse_shortcut(shortcut_label)?;
    app.global_shortcut().unregister_all().map_err(|error| {
        shortcut_registration_error(
            shortcut_label,
            format!("existing shortcut unregister failed: {error}"),
        )
    })?;

    app.global_shortcut()
        .register(shortcut)
        .map_err(|error| shortcut_registration_error(shortcut_label, error.to_string()))?;

    let state = app.state::<ShortcutRegistrationState>();
    *state
        .registration_error
        .lock()
        .expect("shortcut registration state poisoned") = None;
    *state
        .current_shortcut
        .lock()
        .expect("shortcut registration state poisoned") = normalize_shortcut_label(shortcut_label)?;

    Ok(())
}

fn parse_shortcut(input: &str) -> Result<Shortcut, AppError> {
    let parsed = parse_shortcut_parts(input)?;
    Ok(Shortcut::new(Some(parsed.modifiers), parsed.code))
}

struct ParsedShortcut {
    label: String,
    modifiers: Modifiers,
    code: Code,
}

fn parse_shortcut_parts(input: &str) -> Result<ParsedShortcut, AppError> {
    let mut modifiers = Modifiers::empty();
    let mut labels = Vec::new();
    let mut key: Option<KeySpec> = None;

    for part in input.split('+') {
        let token = part.trim();
        if token.is_empty() {
            return Err(invalid_shortcut(input, "empty shortcut segment"));
        }

        let normalized = token.to_ascii_lowercase();
        match normalized.as_str() {
            "ctrl" | "control" => {
                modifiers |= Modifiers::CONTROL;
                push_unique_label(&mut labels, "Ctrl");
            }
            "shift" => {
                modifiers |= Modifiers::SHIFT;
                push_unique_label(&mut labels, "Shift");
            }
            "alt" | "option" => {
                modifiers |= Modifiers::ALT;
                push_unique_label(&mut labels, "Alt");
            }
            "super" | "cmd" | "command" | "win" | "windows" => {
                modifiers |= Modifiers::SUPER;
                push_unique_label(&mut labels, "Super");
            }
            _ => {
                if key.is_some() {
                    return Err(invalid_shortcut(input, "shortcut has multiple keys"));
                }
                key = Some(code_for_key(token).ok_or_else(|| {
                    invalid_shortcut(input, format!("unsupported shortcut key: {token}"))
                })?);
            }
        }
    }

    let Some(key) = key else {
        return Err(invalid_shortcut(input, "shortcut must include a key"));
    };

    modifiers |= key.implied_modifiers;
    if modifiers.is_empty() {
        return Err(invalid_shortcut(input, "shortcut must include a modifier"));
    }
    labels.push(key.label);

    Ok(ParsedShortcut {
        label: labels.join("+"),
        modifiers,
        code: key.code,
    })
}

fn push_unique_label(labels: &mut Vec<String>, label: &'static str) {
    if !labels.iter().any(|existing| existing == label) {
        labels.push(label.to_string());
    }
}

struct KeySpec {
    label: String,
    code: Code,
    implied_modifiers: Modifiers,
}

fn code_for_key(token: &str) -> Option<KeySpec> {
    let upper = token.to_ascii_uppercase();
    let (label, code, implied_modifiers) = match upper.as_str() {
        "`" | "BACKQUOTE" => ("`".to_string(), Code::Backquote, Modifiers::empty()),
        "~" => ("~".to_string(), Code::Backquote, Modifiers::SHIFT),
        "A" => plain_key("A", Code::KeyA),
        "B" => plain_key("B", Code::KeyB),
        "C" => plain_key("C", Code::KeyC),
        "D" => plain_key("D", Code::KeyD),
        "E" => plain_key("E", Code::KeyE),
        "F" => plain_key("F", Code::KeyF),
        "G" => plain_key("G", Code::KeyG),
        "H" => plain_key("H", Code::KeyH),
        "I" => plain_key("I", Code::KeyI),
        "J" => plain_key("J", Code::KeyJ),
        "K" => plain_key("K", Code::KeyK),
        "L" => plain_key("L", Code::KeyL),
        "M" => plain_key("M", Code::KeyM),
        "N" => plain_key("N", Code::KeyN),
        "O" => plain_key("O", Code::KeyO),
        "P" => plain_key("P", Code::KeyP),
        "Q" => plain_key("Q", Code::KeyQ),
        "R" => plain_key("R", Code::KeyR),
        "S" => plain_key("S", Code::KeyS),
        "T" => plain_key("T", Code::KeyT),
        "U" => plain_key("U", Code::KeyU),
        "V" => plain_key("V", Code::KeyV),
        "W" => plain_key("W", Code::KeyW),
        "X" => plain_key("X", Code::KeyX),
        "Y" => plain_key("Y", Code::KeyY),
        "Z" => plain_key("Z", Code::KeyZ),
        "0" => plain_key("0", Code::Digit0),
        ")" => (")".to_string(), Code::Digit0, Modifiers::SHIFT),
        "1" => plain_key("1", Code::Digit1),
        "!" => ("!".to_string(), Code::Digit1, Modifiers::SHIFT),
        "2" => plain_key("2", Code::Digit2),
        "@" => ("@".to_string(), Code::Digit2, Modifiers::SHIFT),
        "3" => plain_key("3", Code::Digit3),
        "#" => ("#".to_string(), Code::Digit3, Modifiers::SHIFT),
        "4" => plain_key("4", Code::Digit4),
        "$" => ("$".to_string(), Code::Digit4, Modifiers::SHIFT),
        "5" => plain_key("5", Code::Digit5),
        "%" => ("%".to_string(), Code::Digit5, Modifiers::SHIFT),
        "6" => plain_key("6", Code::Digit6),
        "^" => ("^".to_string(), Code::Digit6, Modifiers::SHIFT),
        "7" => plain_key("7", Code::Digit7),
        "&" => ("&".to_string(), Code::Digit7, Modifiers::SHIFT),
        "8" => plain_key("8", Code::Digit8),
        "*" => ("*".to_string(), Code::Digit8, Modifiers::SHIFT),
        "9" => plain_key("9", Code::Digit9),
        "(" => ("(".to_string(), Code::Digit9, Modifiers::SHIFT),
        "-" | "MINUS" => ("-".to_string(), Code::Minus, Modifiers::empty()),
        "_" => ("_".to_string(), Code::Minus, Modifiers::SHIFT),
        "=" | "EQUAL" => ("=".to_string(), Code::Equal, Modifiers::empty()),
        "PLUS" => ("+".to_string(), Code::Equal, Modifiers::SHIFT),
        "[" | "BRACKETLEFT" => ("[".to_string(), Code::BracketLeft, Modifiers::empty()),
        "{" => ("{".to_string(), Code::BracketLeft, Modifiers::SHIFT),
        "]" | "BRACKETRIGHT" => ("]".to_string(), Code::BracketRight, Modifiers::empty()),
        "}" => ("}".to_string(), Code::BracketRight, Modifiers::SHIFT),
        "\\" | "BACKSLASH" => ("\\".to_string(), Code::Backslash, Modifiers::empty()),
        "|" => ("|".to_string(), Code::Backslash, Modifiers::SHIFT),
        ";" | "SEMICOLON" => (";".to_string(), Code::Semicolon, Modifiers::empty()),
        ":" => (":".to_string(), Code::Semicolon, Modifiers::SHIFT),
        "'" | "QUOTE" => ("'".to_string(), Code::Quote, Modifiers::empty()),
        "\"" => ("\"".to_string(), Code::Quote, Modifiers::SHIFT),
        "," | "COMMA" => (",".to_string(), Code::Comma, Modifiers::empty()),
        "<" => ("<".to_string(), Code::Comma, Modifiers::SHIFT),
        "." | "PERIOD" => (".".to_string(), Code::Period, Modifiers::empty()),
        ">" => (">".to_string(), Code::Period, Modifiers::SHIFT),
        "/" | "SLASH" => ("/".to_string(), Code::Slash, Modifiers::empty()),
        "?" => ("?".to_string(), Code::Slash, Modifiers::SHIFT),
        "SPACE" => ("Space".to_string(), Code::Space, Modifiers::empty()),
        "TAB" => ("Tab".to_string(), Code::Tab, Modifiers::empty()),
        "ENTER" => ("Enter".to_string(), Code::Enter, Modifiers::empty()),
        "BACKSPACE" => ("Backspace".to_string(), Code::Backspace, Modifiers::empty()),
        "F1" => plain_key("F1", Code::F1),
        "F2" => plain_key("F2", Code::F2),
        "F3" => plain_key("F3", Code::F3),
        "F4" => plain_key("F4", Code::F4),
        "F5" => plain_key("F5", Code::F5),
        "F6" => plain_key("F6", Code::F6),
        "F7" => plain_key("F7", Code::F7),
        "F8" => plain_key("F8", Code::F8),
        "F9" => plain_key("F9", Code::F9),
        "F10" => plain_key("F10", Code::F10),
        "F11" => plain_key("F11", Code::F11),
        "F12" => plain_key("F12", Code::F12),
        _ => return None,
    };

    Some(KeySpec {
        label,
        code,
        implied_modifiers,
    })
}

fn plain_key(label: &'static str, code: Code) -> (String, Code, Modifiers) {
    (label.to_string(), code, Modifiers::empty())
}

fn invalid_shortcut(input: &str, reason: impl Into<String>) -> AppError {
    shortcut_registration_error(input, reason)
}

fn shortcut_registration_error(shortcut_label: &str, reason: impl Into<String>) -> AppError {
    AppError::new(
        AppErrorCode::ShortcutRegistrationFailed,
        "Lexi could not register the shortcut.",
        format!("{} registration failed: {}", shortcut_label, reason.into()),
        false,
    )
}

fn start_capture(app: AppHandle, shortcut_label: String) {
    let clipboard_owner = clipboard_owner_hwnd(&app);

    std::thread::spawn(move || {
        let capture = clipboard_owner
            .map(selection::capture_selected_text_with_clipboard_owner)
            .unwrap_or_else(selection::capture_selected_text_with_failure);
        let event = match capture {
            Ok(selection) => {
                let character_count = selection.text.chars().count();
                let multiline = selection.text.contains('\n');
                let selected_text = selection.text.clone();
                app.state::<SelectedTextState>().replace(selection.text);
                let capture_method = selection.capture_method;
                let source_process = selection.source_process;
                let source_window_title = selection.source_window_title;

                let event = CaptureEvent::Captured {
                    shortcut: shortcut_label.clone(),
                    capture_method,
                    source_process: source_process.clone(),
                    source_window_title: source_window_title.clone(),
                    character_count,
                    multiline,
                };

                show_popup(&app);
                let _ = app.emit(CAPTURE_EVENT, event.clone());
                llm::start_transform_stream(
                    app.clone(),
                    selected_text,
                    TransformCaptureMetadata {
                        shortcut: shortcut_label,
                        capture_method,
                        source_process,
                        source_window_title,
                        character_count,
                        multiline,
                    },
                );

                return;
            }
            Err(failure) => {
                let selection_error_code = selection::error_code(&failure.error).to_string();
                CaptureEvent::Failed {
                    shortcut: shortcut_label,
                    error: AppError::from(failure.error),
                    selection_error_code,
                    capture_method: failure.capture_method,
                    source_process: failure.source_process,
                    source_window_title: failure.source_window_title,
                }
            }
        };

        show_popup(&app);
        let _ = app.emit(CAPTURE_EVENT, event);
    });
}

#[cfg(windows)]
fn clipboard_owner_hwnd(app: &AppHandle) -> Option<isize> {
    app.get_webview_window("main")
        .and_then(|window| window.hwnd().ok())
        .map(|hwnd| hwnd.0 as isize)
}

#[cfg(not(windows))]
fn clipboard_owner_hwnd(_app: &AppHandle) -> Option<isize> {
    None
}

fn show_popup(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_shortcut_label, CaptureEvent, DEFAULT_SHORTCUT_LABEL};
    use crate::errors::{AppError, AppErrorCode};
    use serde_json::json;

    #[test]
    fn serializes_captured_event_with_frontend_field_names() {
        let event = CaptureEvent::Captured {
            shortcut: DEFAULT_SHORTCUT_LABEL.to_string(),
            capture_method: "uia-foreground-window",
            source_process: Some("notepad.exe".to_string()),
            source_window_title: Some("note.txt - Notepad".to_string()),
            character_count: 42,
            multiline: true,
        };

        assert_eq!(
            serde_json::to_value(event).expect("capture event should serialize"),
            json!({
                "status": "captured",
                "shortcut": "Ctrl+Shift+X",
                "captureMethod": "uia-foreground-window",
                "sourceProcess": "notepad.exe",
                "sourceWindowTitle": "note.txt - Notepad",
                "characterCount": 42,
                "multiline": true
            })
        );
    }

    #[test]
    fn serializes_failed_event_with_frontend_error_field_names() {
        let event = CaptureEvent::Failed {
            shortcut: DEFAULT_SHORTCUT_LABEL.to_string(),
            error: AppError::new(
                AppErrorCode::SelectionUnavailable,
                "This app does not expose selected text to Lexi.",
                "The active control does not support a selected-text UI Automation pattern.",
                false,
            ),
            selection_error_code: "SelectionUnsupported".to_string(),
            capture_method: Some("uia-foreground-window"),
            source_process: Some("example.exe".to_string()),
            source_window_title: Some("Example".to_string()),
        };

        let value = serde_json::to_value(event).expect("capture event should serialize");

        assert_eq!(value["status"], "failed");
        assert_eq!(value["shortcut"], "Ctrl+Shift+X");
        assert_eq!(value["selectionErrorCode"], "SelectionUnsupported");
        assert_eq!(value["captureMethod"], "uia-foreground-window");
        assert_eq!(value["sourceProcess"], "example.exe");
        assert_eq!(value["sourceWindowTitle"], "Example");
        assert_eq!(
            value["error"]["userMessage"],
            "This app does not expose selected text to Lexi."
        );
        assert_eq!(value["error"]["retryable"], false);
    }

    #[test]
    fn normalizes_supported_shortcut_labels() {
        assert_eq!(
            normalize_shortcut_label("control + alt + f9").expect("shortcut should parse"),
            "Ctrl+Alt+F9"
        );
        assert_eq!(
            normalize_shortcut_label("ctrl+shift+(").expect("shortcut should parse"),
            "Ctrl+Shift+("
        );
    }

    #[test]
    fn rejects_shortcuts_without_modifier() {
        let error = normalize_shortcut_label("X").expect_err("shortcut should be rejected");

        assert_eq!(error.code, AppErrorCode::ShortcutRegistrationFailed);
        assert!(error.diagnostic_message.contains("modifier"));
    }
}
