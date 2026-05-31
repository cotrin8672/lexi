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
    CredentialStorageFailed,
    SettingsIoFailed,
    VocabularyStoreFailed,
    SyncAuthRequired,
    SyncPushFailed,
    SyncPullFailed,
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

    pub fn credential_storage_failed(
        diagnostic_message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self::new(
            AppErrorCode::CredentialStorageFailed,
            "Could not access stored credentials.",
            diagnostic_message,
            retryable,
        )
    }

    pub fn settings_io_failed(diagnostic_message: impl Into<String>, retryable: bool) -> Self {
        Self::new(
            AppErrorCode::SettingsIoFailed,
            "Could not read or save settings.",
            diagnostic_message,
            retryable,
        )
    }

    pub fn vocabulary_store_failed(diagnostic_message: impl Into<String>, retryable: bool) -> Self {
        Self::new(
            AppErrorCode::VocabularyStoreFailed,
            "Local vocabulary data could not be accessed.",
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

    fn assert_selection_error(
        source: SelectionCaptureError,
        expected_code: AppErrorCode,
        expected_retryable: bool,
    ) {
        let error = AppError::from(source);
        assert_eq!(error.code, expected_code);
        assert_eq!(error.retryable, expected_retryable);
        assert!(!error.user_message.is_empty());
        assert!(!error.diagnostic_message.is_empty());
    }

    #[test]
    fn maps_empty_selection_to_non_retryable_app_error() {
        assert_selection_error(
            SelectionCaptureError::EmptySelection,
            AppErrorCode::SelectionEmpty,
            false,
        );
    }

    #[test]
    fn maps_access_denied_to_selection_permission_denied() {
        assert_selection_error(
            SelectionCaptureError::AccessDenied,
            AppErrorCode::SelectionPermissionDenied,
            false,
        );
    }

    #[test]
    fn maps_no_foreground_window_to_retryable_selection_unavailable() {
        assert_selection_error(
            SelectionCaptureError::NoForegroundWindow,
            AppErrorCode::SelectionUnavailable,
            true,
        );
    }

    #[test]
    fn maps_focused_element_unavailable_to_retryable_selection_unavailable() {
        assert_selection_error(
            SelectionCaptureError::FocusedElementUnavailable,
            AppErrorCode::SelectionUnavailable,
            true,
        );
    }

    #[test]
    fn maps_text_pattern_unavailable_to_non_retryable_selection_unavailable() {
        assert_selection_error(
            SelectionCaptureError::TextPatternUnavailable,
            AppErrorCode::SelectionUnavailable,
            false,
        );
    }

    #[test]
    fn maps_selection_unsupported_to_non_retryable_selection_unavailable() {
        assert_selection_error(
            SelectionCaptureError::SelectionUnsupported,
            AppErrorCode::SelectionUnavailable,
            false,
        );
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

    #[test]
    fn provider_not_configured_is_not_retryable() {
        let error = AppError::provider_not_configured("DeepL API key is not configured");

        assert_eq!(error.code, AppErrorCode::ProviderNotConfigured);
        assert!(!error.retryable);
        assert!(error
            .diagnostic_message
            .contains("DeepL API key is not configured"));
    }

    #[test]
    fn provider_request_failed_respects_retryable_flag() {
        let retryable = AppError::provider_request_failed("network timeout", true);
        let permanent = AppError::provider_request_failed("invalid api key", false);

        assert_eq!(retryable.code, AppErrorCode::ProviderRequestFailed);
        assert!(retryable.retryable);
        assert_eq!(permanent.code, AppErrorCode::ProviderRequestFailed);
        assert!(!permanent.retryable);
    }

    #[test]
    fn serializes_app_error_with_frontend_field_names() {
        let error = AppError::from(SelectionCaptureError::EmptySelection);
        let value = serde_json::to_value(&error).expect("app error should serialize");

        assert_eq!(value["code"], "SelectionEmpty");
        assert_eq!(value["retryable"], false);
        assert!(value.get("userMessage").is_some());
        assert!(value.get("diagnosticMessage").is_some());
    }
}
