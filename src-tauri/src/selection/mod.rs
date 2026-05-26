use serde::Serialize;

#[derive(Debug, Clone)]
pub struct CapturedSelection {
    pub text: String,
    pub source_process: Option<String>,
    pub source_window_title: Option<String>,
    pub capture_method: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum SelectionCaptureError {
    NoForegroundWindow,
    FocusedElementUnavailable,
    TextPatternUnavailable,
    SelectionUnsupported,
    EmptySelection,
    AccessDenied,
    WindowsApiFailure(String),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionDiagnostics {
    pub ok: bool,
    pub code: String,
    pub capture_method: Option<&'static str>,
    pub source_process: Option<String>,
    pub source_window_title: Option<String>,
    pub character_count: usize,
    pub multiline: bool,
}

pub fn capture_selected_text() -> Result<CapturedSelection, SelectionCaptureError> {
    platform::capture_selected_text()
}

pub fn capture_selection_diagnostics() -> SelectionDiagnostics {
    platform::capture_selection_diagnostics()
}

fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn error_code(error: &SelectionCaptureError) -> &'static str {
    match error {
        SelectionCaptureError::NoForegroundWindow => "NoForegroundWindow",
        SelectionCaptureError::FocusedElementUnavailable => "FocusedElementUnavailable",
        SelectionCaptureError::TextPatternUnavailable => "TextPatternUnavailable",
        SelectionCaptureError::SelectionUnsupported => "SelectionUnsupported",
        SelectionCaptureError::EmptySelection => "SelectionEmpty",
        SelectionCaptureError::AccessDenied => "SelectionPermissionDenied",
        SelectionCaptureError::WindowsApiFailure(_) => "WindowsApiFailure",
    }
}

#[cfg(windows)]
mod windows;

#[cfg(windows)]
use windows as platform;

#[cfg(not(windows))]
mod platform {
    use super::{error_code, CapturedSelection, SelectionCaptureError, SelectionDiagnostics};

    pub fn capture_selected_text() -> Result<CapturedSelection, SelectionCaptureError> {
        Err(SelectionCaptureError::SelectionUnsupported)
    }

    pub fn capture_selection_diagnostics() -> SelectionDiagnostics {
        let error = SelectionCaptureError::SelectionUnsupported;
        SelectionDiagnostics {
            ok: false,
            code: error_code(&error).to_string(),
            capture_method: None,
            source_process: None,
            source_window_title: None,
            character_count: 0,
            multiline: false,
        }
    }
}
