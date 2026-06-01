use crate::{errors::AppError, secrets, shortcut};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, sync::Mutex};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    Mock,
    Gemini,
    OpenAi,
    DeepL,
}

impl ProviderKind {
    pub fn default_model(self) -> &'static str {
        match self {
            Self::Mock => "mock-word-study",
            Self::Gemini => "gemini-2.5-flash-lite",
            Self::OpenAi => "gpt-5.4-nano",
            Self::DeepL => "deepl-translate",
        }
    }

    pub fn secret_user(self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Gemini => "gemini",
            Self::OpenAi => "openai",
            Self::DeepL => "deepl",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettings {
    #[serde(default = "default_shortcut_setting")]
    pub shortcut: String,
    #[serde(default = "default_close_shortcut_setting")]
    pub close_shortcut: String,
    #[serde(default = "default_pronunciation_shortcut_setting")]
    pub pronunciation_shortcut: String,
    #[serde(default = "default_background_opacity")]
    pub background_opacity: f64,
    #[serde(default = "default_theme")]
    pub theme: ThemeMode,
    pub provider: ProviderKind,
    pub model: String,
    pub result_language: String,
    pub prompt_mode: String,
    #[serde(default)]
    pub supabase_url: String,
    #[serde(default)]
    pub supabase_anon_key: String,
    #[serde(default = "default_supabase_callback_port")]
    pub supabase_callback_port: u16,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            shortcut: shortcut::DEFAULT_SHORTCUT_LABEL.to_string(),
            close_shortcut: shortcut::DEFAULT_CLOSE_SHORTCUT_LABEL.to_string(),
            pronunciation_shortcut: shortcut::DEFAULT_PRONUNCIATION_SHORTCUT_LABEL.to_string(),
            background_opacity: default_background_opacity(),
            theme: default_theme(),
            provider: ProviderKind::Gemini,
            model: ProviderKind::Gemini.default_model().to_string(),
            result_language: "ja".to_string(),
            prompt_mode: "word-study".to_string(),
            supabase_url: String::new(),
            supabase_anon_key: String::new(),
            supabase_callback_port: default_supabase_callback_port(),
        }
    }
}

impl ProviderSettings {
    pub fn supabase_configured(&self) -> bool {
        self.supabase_connection().is_some()
    }

    pub fn supabase_connection(&self) -> Option<(String, String)> {
        let url = env_setting("SUPABASE_URL")
            .or_else(|| env_setting("LEXI_SUPABASE_URL"))
            .unwrap_or_else(|| self.supabase_url.trim().to_string());
        let anon_key = env_setting("SUPABASE_ANON_KEY")
            .or_else(|| env_setting("SUPABASE_PUBLISHABLE_KEY"))
            .or_else(|| env_setting("LEXI_SUPABASE_ANON_KEY"))
            .unwrap_or_else(|| self.supabase_anon_key.trim().to_string());

        if url.is_empty() || anon_key.is_empty() {
            return None;
        }

        Some((url.trim_end_matches('/').to_string(), anon_key))
    }

    pub fn supabase_callback_url(&self) -> String {
        format!(
            "http://localhost:{}/auth/callback",
            self.supabase_callback_port
        )
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettingsView {
    pub shortcut: String,
    pub close_shortcut: String,
    pub pronunciation_shortcut: String,
    pub background_opacity: f64,
    pub theme: ThemeMode,
    pub provider: ProviderKind,
    pub model: String,
    pub result_language: String,
    pub prompt_mode: String,
    pub api_key_configured: bool,
    pub deepl_api_key_configured: bool,
    pub supabase_anon_key_configured: bool,
    pub supabase_callback_url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettingsUpdate {
    pub shortcut: String,
    pub close_shortcut: String,
    pub pronunciation_shortcut: String,
    pub background_opacity: f64,
    pub theme: ThemeMode,
    pub provider: ProviderKind,
    pub model: String,
    pub result_language: String,
    pub prompt_mode: String,
    pub api_key: Option<String>,
    pub deepl_api_key: Option<String>,
    #[serde(default)]
    pub supabase_url: Option<String>,
    pub supabase_anon_key: Option<String>,
}

#[derive(Debug, Default)]
pub struct SettingsState {
    settings: Mutex<Option<ProviderSettings>>,
}

impl SettingsState {
    pub fn load_view(&self, app: &AppHandle) -> Result<ProviderSettingsView, AppError> {
        let settings = self.load_settings(app)?;
        Ok(settings_view(app, settings))
    }

    pub fn load_settings(&self, app: &AppHandle) -> Result<ProviderSettings, AppError> {
        let mut guard = self.settings.lock().expect("settings state poisoned");

        if let Some(settings) = guard.clone() {
            return Ok(settings);
        }

        let settings = read_settings(app).unwrap_or_default();
        *guard = Some(settings.clone());
        Ok(settings)
    }

    pub fn save_settings(
        &self,
        app: &AppHandle,
        update: ProviderSettingsUpdate,
    ) -> Result<ProviderSettingsView, AppError> {
        let model = update.model.trim();
        if model.is_empty() {
            return Err(AppError::new(
                crate::errors::AppErrorCode::ProviderNotConfigured,
                "Model name is required.",
                "provider settings rejected an empty model name",
                false,
            ));
        }

        let result_language = update.result_language.trim();
        if result_language.is_empty() {
            return Err(AppError::new(
                crate::errors::AppErrorCode::ProviderNotConfigured,
                "Result language is required.",
                "provider settings rejected an empty result language",
                false,
            ));
        }

        if update.prompt_mode.trim() != "word-study" {
            return Err(AppError::new(
                crate::errors::AppErrorCode::ProviderNotConfigured,
                "Prompt mode is not supported yet.",
                "only the word-study prompt mode is supported",
                false,
            ));
        }

        if update.provider == ProviderKind::DeepL {
            return Err(AppError::new(
                crate::errors::AppErrorCode::ProviderNotConfigured,
                "DeepL is used automatically for sentence translation.",
                "DeepL cannot be selected as the word-study provider",
                false,
            ));
        }

        let normalized_shortcut = shortcut::normalize_shortcut_label(&update.shortcut)?;
        let normalized_close_shortcut =
            shortcut::normalize_close_shortcut_label(&update.close_shortcut)?;
        let normalized_pronunciation_shortcut =
            shortcut::normalize_pronunciation_shortcut_label(&update.pronunciation_shortcut)?;
        ensure_distinct_shortcuts(
            &normalized_shortcut,
            &normalized_close_shortcut,
            &normalized_pronunciation_shortcut,
        )?;
        let background_opacity = validate_background_opacity(update.background_opacity)?;
        let previous_settings = self.load_settings(app)?;
        let supabase_url = match update.supabase_url.as_deref() {
            Some(value) => normalize_optional_url(value)?,
            None => previous_settings.supabase_url,
        };
        let supabase_anon_key = update
            .supabase_anon_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or(previous_settings.supabase_anon_key);

        let settings = ProviderSettings {
            shortcut: normalized_shortcut,
            close_shortcut: normalized_close_shortcut,
            pronunciation_shortcut: normalized_pronunciation_shortcut,
            background_opacity,
            theme: update.theme,
            provider: update.provider,
            model: model.to_string(),
            result_language: result_language.to_string(),
            prompt_mode: "word-study".to_string(),
            supabase_url,
            supabase_anon_key,
            supabase_callback_port: previous_settings.supabase_callback_port,
        };

        if let Some(api_key) = update.api_key {
            let trimmed = api_key.trim();
            if !trimmed.is_empty() {
                secrets::write_api_key(update.provider, trimmed)?;
            }
        }

        if let Some(api_key) = update.deepl_api_key {
            let trimmed = api_key.trim();
            if !trimmed.is_empty() {
                secrets::write_api_key(ProviderKind::DeepL, trimmed)?;
            }
        }

        let registered_shortcut = shortcut::update_registered_shortcut(app, &settings.shortcut)?;
        let settings = ProviderSettings {
            shortcut: registered_shortcut,
            ..settings
        };

        write_settings(app, &settings)?;
        *self.settings.lock().expect("settings state poisoned") = Some(settings.clone());

        Ok(settings_view(app, settings))
    }

    pub fn api_key(
        &self,
        _app: &AppHandle,
        provider: ProviderKind,
    ) -> Result<Option<String>, AppError> {
        secrets::read_api_key(provider)
    }
}

#[tauri::command]
pub fn get_provider_settings(
    app: AppHandle,
    state: tauri::State<'_, SettingsState>,
) -> Result<ProviderSettingsView, AppError> {
    state.load_view(&app)
}

#[tauri::command]
pub fn update_provider_settings(
    app: AppHandle,
    state: tauri::State<'_, SettingsState>,
    update: ProviderSettingsUpdate,
) -> Result<ProviderSettingsView, AppError> {
    state.save_settings(&app, update)
}

fn settings_view(app: &AppHandle, settings: ProviderSettings) -> ProviderSettingsView {
    let api_key_configured = has_api_key(app, settings.provider);
    let deepl_api_key_configured = has_api_key(app, ProviderKind::DeepL);
    let supabase_anon_key_configured = settings.supabase_connection().is_some();
    let supabase_callback_url = settings.supabase_callback_url();
    ProviderSettingsView {
        shortcut: settings.shortcut,
        close_shortcut: settings.close_shortcut,
        pronunciation_shortcut: settings.pronunciation_shortcut,
        background_opacity: settings.background_opacity,
        theme: settings.theme,
        provider: settings.provider,
        model: settings.model,
        result_language: settings.result_language,
        prompt_mode: settings.prompt_mode,
        api_key_configured,
        deepl_api_key_configured,
        supabase_anon_key_configured,
        supabase_callback_url,
    }
}

fn env_setting(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn read_settings(app: &AppHandle) -> Result<ProviderSettings, AppError> {
    let path = settings_path(app)?;
    if !path.exists() {
        return Ok(ProviderSettings::default());
    }

    let raw = fs::read_to_string(&path).map_err(|error| {
        AppError::settings_io_failed(format!("provider settings read failed: {error}"), true)
    })?;

    let mut settings = serde_json::from_str::<ProviderSettings>(&raw).map_err(|error| {
        AppError::settings_io_failed(format!("provider settings parse failed: {error}"), false)
    })?;

    if settings.provider == ProviderKind::DeepL {
        settings.provider = ProviderKind::Gemini;
        settings.model = ProviderKind::Gemini.default_model().to_string();
    }

    Ok(settings)
}

fn write_settings(app: &AppHandle, settings: &ProviderSettings) -> Result<(), AppError> {
    let path = settings_path(app)?;
    ensure_parent(&path)?;
    let raw = serde_json::to_string_pretty(settings).map_err(|error| {
        AppError::settings_io_failed(
            format!("provider settings serialize failed: {error}"),
            false,
        )
    })?;

    fs::write(path, raw).map_err(|error| {
        AppError::settings_io_failed(format!("provider settings write failed: {error}"), true)
    })
}

fn has_api_key(app: &AppHandle, provider: ProviderKind) -> bool {
    let _ = app;
    secrets::has_api_key(provider)
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    app.path()
        .app_config_dir()
        .map(|dir| dir.join("provider-settings.json"))
        .map_err(|error| {
            AppError::settings_io_failed(format!("app config dir unavailable: {error}"), true)
        })
}

fn ensure_parent(path: &PathBuf) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            AppError::settings_io_failed(format!("app config dir create failed: {error}"), true)
        })?;
    }

    Ok(())
}

fn default_shortcut_setting() -> String {
    shortcut::DEFAULT_SHORTCUT_LABEL.to_string()
}

fn default_close_shortcut_setting() -> String {
    shortcut::DEFAULT_CLOSE_SHORTCUT_LABEL.to_string()
}

fn default_pronunciation_shortcut_setting() -> String {
    shortcut::DEFAULT_PRONUNCIATION_SHORTCUT_LABEL.to_string()
}

fn ensure_distinct_shortcuts(
    capture: &str,
    close: &str,
    pronunciation: &str,
) -> Result<(), AppError> {
    if capture == close {
        return Err(AppError::new(
            crate::errors::AppErrorCode::ShortcutRegistrationFailed,
            "Close shortcut must be different from the capture shortcut.",
            "provider settings rejected matching capture and close shortcuts",
            false,
        ));
    }
    if capture == pronunciation {
        return Err(AppError::new(
            crate::errors::AppErrorCode::ShortcutRegistrationFailed,
            "Pronunciation shortcut must be different from the capture shortcut.",
            "provider settings rejected matching capture and pronunciation shortcuts",
            false,
        ));
    }
    if close == pronunciation {
        return Err(AppError::new(
            crate::errors::AppErrorCode::ShortcutRegistrationFailed,
            "Pronunciation shortcut must be different from the close shortcut.",
            "provider settings rejected matching close and pronunciation shortcuts",
            false,
        ));
    }

    Ok(())
}

fn default_background_opacity() -> f64 {
    0.94
}

fn default_theme() -> ThemeMode {
    ThemeMode::Light
}

fn default_supabase_callback_port() -> u16 {
    38271
}

fn validate_background_opacity(value: f64) -> Result<f64, AppError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(AppError::new(
            crate::errors::AppErrorCode::ProviderNotConfigured,
            "Background opacity is invalid.",
            format!("background opacity must be between 0 and 1: {value}"),
            false,
        ));
    }

    Ok((value * 100.0).round() / 100.0)
}

fn normalize_optional_url(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim().trim_end_matches('/').to_string();
    if trimmed.is_empty() {
        return Ok(trimmed);
    }

    if !(trimmed.starts_with("https://") || trimmed.starts_with("http://localhost")) {
        return Err(AppError::new(
            crate::errors::AppErrorCode::ProviderNotConfigured,
            "Supabase URL is invalid.",
            "Supabase URL must start with https:// or http://localhost.",
            false,
        ));
    }

    Ok(trimmed)
}

#[cfg(test)]
mod tests {
    use super::{normalize_optional_url, validate_background_opacity};
    use crate::errors::AppErrorCode;

    #[test]
    fn validates_background_opacity_within_range() {
        assert_eq!(validate_background_opacity(0.0).expect("min opacity"), 0.0);
        assert_eq!(validate_background_opacity(1.0).expect("max opacity"), 1.0);
        assert_eq!(
            validate_background_opacity(0.456).expect("rounded opacity"),
            0.46
        );
    }

    #[test]
    fn rejects_non_finite_background_opacity() {
        let error = validate_background_opacity(f64::NAN).expect_err("nan should fail");

        assert_eq!(error.code, AppErrorCode::ProviderNotConfigured);
        assert!(error.diagnostic_message.contains("background opacity"));
    }

    #[test]
    fn rejects_background_opacity_outside_range() {
        let error = validate_background_opacity(1.5).expect_err("too high should fail");

        assert_eq!(error.code, AppErrorCode::ProviderNotConfigured);
    }

    #[test]
    fn normalizes_optional_supabase_url() {
        assert_eq!(
            normalize_optional_url("  https://project.supabase.co/  ").expect("https url"),
            "https://project.supabase.co"
        );
        assert_eq!(normalize_optional_url("").expect("empty url"), "");
    }

    #[test]
    fn rejects_invalid_supabase_url_scheme() {
        let error =
            normalize_optional_url("http://example.com").expect_err("http should be rejected");

        assert_eq!(error.code, AppErrorCode::ProviderNotConfigured);
        assert!(error.diagnostic_message.contains("https://"));
    }
}
