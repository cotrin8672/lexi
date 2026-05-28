use crate::selection::SelectionCaptureError;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AppErrorCode {
    ShortcutRegistrationFailed,
    SelectionUnavailable,
    SelectionEmpty,
    SelectionPermissionDenied,
    ProviderNotConfigured,
    ProviderRequestFailed,
    InvalidModelOutput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: AppErrorCode,
    pub user_message: String,
    pub diagnostic_message: String,
    pub retryable: bool,
}

impl AppError {
    pub fn new(
        code: AppErrorCode,
        user_message: impl Into<String>,
        diagnostic_message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            code,
            user_message: user_message.into(),
            diagnostic_message: diagnostic_message.into(),
            retryable,
        }
    }

    pub fn invalid_model_output(diagnostic_message: impl Into<String>) -> Self {
        Self::new(
            AppErrorCode::InvalidModelOutput,
            "AI response could not be displayed.",
            diagnostic_message,
            true,
        )
    }

    pub fn provider_not_configured(diagnostic_message: impl Into<String>) -> Self {
        Self::new(
            AppErrorCode::ProviderNotConfigured,
            "LLM provider is not configured.",
            diagnostic_message,
            false,
        )
    }

    pub fn provider_request_failed(diagnostic_message: impl Into<String>, retryable: bool) -> Self {
        Self::new(
            AppErrorCode::ProviderRequestFailed,
            "LLM request failed.",
            diagnostic_message,
            retryable,
        )
    }
}

impl From<SelectionCaptureError> for AppError {
    fn from(error: SelectionCaptureError) -> Self {
        match error {
            SelectionCaptureError::EmptySelection => Self::new(
                AppErrorCode::SelectionEmpty,
                "Select text before running Lexi.",
                "No selected text was available from the active control.",
                false,
            ),
            SelectionCaptureError::AccessDenied => Self::new(
                AppErrorCode::SelectionPermissionDenied,
                "Lexi cannot access the selected text in this app.",
                "Windows denied UI Automation access to the selected text.",
                false,
            ),
            SelectionCaptureError::NoForegroundWindow => Self::new(
                AppErrorCode::SelectionUnavailable,
                "Lexi could not find the active window.",
                "No foreground window was available during selected-text capture.",
                true,
            ),
            SelectionCaptureError::FocusedElementUnavailable => Self::new(
                AppErrorCode::SelectionUnavailable,
                "Lexi could not read from the focused control.",
                "UI Automation did not expose a focused element.",
                true,
            ),
            SelectionCaptureError::TextPatternUnavailable
            | SelectionCaptureError::SelectionUnsupported => Self::new(
                AppErrorCode::SelectionUnavailable,
                "This app does not expose selected text to Lexi.",
                "The active control does not support a selected-text UI Automation pattern.",
                false,
            ),
            SelectionCaptureError::WindowsApiFailure(message) => Self::new(
                AppErrorCode::SelectionUnavailable,
                "Lexi could not read the selected text.",
                format!("Windows selected-text capture failure: {message}"),
                true,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AppError, AppErrorCode};
    use crate::selection::SelectionCaptureError;

    #[test]
    fn maps_empty_selection_to_non_retryable_app_error() {
        let error = AppError::from(SelectionCaptureError::EmptySelection);

        assert_eq!(error.code, AppErrorCode::SelectionEmpty);
        assert!(!error.retryable);
        assert!(!error.user_message.is_empty());
        assert!(!error.diagnostic_message.is_empty());
    }

    #[test]
    fn maps_windows_api_failure_without_exposing_sensitive_payloads() {
        let error = AppError::from(SelectionCaptureError::WindowsApiFailure(
            "HRESULT 0x80004005".to_string(),
        ));

        assert_eq!(error.code, AppErrorCode::SelectionUnavailable);
        assert!(error.retryable);
        assert!(error.diagnostic_message.contains("HRESULT 0x80004005"));
    }

    #[test]
    fn invalid_model_output_is_retryable() {
        let error = AppError::invalid_model_output("missing title");

        assert_eq!(error.code, AppErrorCode::InvalidModelOutput);
        assert!(error.retryable);
        assert_eq!(error.diagnostic_message, "missing title");
    }
}
