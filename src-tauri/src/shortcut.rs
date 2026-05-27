use crate::errors::{AppError, AppErrorCode};
use crate::llm::{self, SelectedTextState, TransformCaptureMetadata};
use crate::selection;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

const DEFAULT_SHORTCUT_LABEL: &str = "Ctrl+Shift+X";
const CAPTURE_EVENT: &str = "lexi:capture";

#[derive(Default)]
pub struct ShortcutRegistrationState {
    registration_error: Mutex<Option<AppError>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutStatus {
    pub shortcut: &'static str,
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
        shortcut: &'static str,
    },
    Captured {
        shortcut: &'static str,
        capture_method: &'static str,
        source_process: Option<String>,
        source_window_title: Option<String>,
        character_count: usize,
        multiline: bool,
    },
    Failed {
        shortcut: &'static str,
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

    ShortcutStatus {
        shortcut: DEFAULT_SHORTCUT_LABEL,
        registered: registration_error.is_none(),
        registration_error,
    }
}

pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let default_shortcut = default_shortcut();
    let handler_shortcut = default_shortcut;

    app.handle().plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, shortcut, event| {
                if shortcut == &handler_shortcut && event.state() == ShortcutState::Pressed {
                    start_capture(app.clone());
                }
            })
            .build(),
    )?;

    match app.global_shortcut().register(default_shortcut) {
        Ok(()) => Ok(()),
        Err(error) => {
            let app_error = AppError::new(
                AppErrorCode::ShortcutRegistrationFailed,
                "Lexi could not register the shortcut.",
                format!("{DEFAULT_SHORTCUT_LABEL} registration failed: {error}"),
                false,
            );

            let state = app.state::<ShortcutRegistrationState>();
            *state
                .registration_error
                .lock()
                .expect("shortcut registration state poisoned") = Some(app_error.clone());

            show_popup(app.handle());
            let _ = app.handle().emit(
                CAPTURE_EVENT,
                CaptureEvent::Failed {
                    shortcut: DEFAULT_SHORTCUT_LABEL,
                    error: app_error,
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

fn default_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyX)
}

fn start_capture(app: AppHandle) {
    let _ = app.emit(
        CAPTURE_EVENT,
        CaptureEvent::Capturing {
            shortcut: DEFAULT_SHORTCUT_LABEL,
        },
    );

    std::thread::spawn(move || {
        let event = match selection::capture_selected_text_with_failure() {
            Ok(selection) => {
                let character_count = selection.text.chars().count();
                let multiline = selection.text.contains('\n');
                let selected_text = selection.text.clone();
                app.state::<SelectedTextState>().replace(selection.text);
                let capture_method = selection.capture_method;
                let source_process = selection.source_process;
                let source_window_title = selection.source_window_title;

                let event = CaptureEvent::Captured {
                    shortcut: DEFAULT_SHORTCUT_LABEL,
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
                        shortcut: DEFAULT_SHORTCUT_LABEL.to_string(),
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
                    shortcut: DEFAULT_SHORTCUT_LABEL,
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

fn show_popup(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::{CaptureEvent, DEFAULT_SHORTCUT_LABEL};
    use crate::errors::{AppError, AppErrorCode};
    use serde_json::json;

    #[test]
    fn serializes_captured_event_with_frontend_field_names() {
        let event = CaptureEvent::Captured {
            shortcut: DEFAULT_SHORTCUT_LABEL,
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
            shortcut: DEFAULT_SHORTCUT_LABEL,
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
}
