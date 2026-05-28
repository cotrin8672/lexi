use crate::{errors::AppError, secrets, shortcut};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, sync::Mutex};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    Mock,
    Gemini,
    OpenAi,
}

impl ProviderKind {
    pub fn default_model(self) -> &'static str {
        match self {
            Self::Mock => "mock-word-study",
            Self::Gemini => "gemini-2.5-flash-lite",
            Self::OpenAi => "gpt-5.4-nano",
        }
    }

    pub fn secret_user(self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Gemini => "gemini",
            Self::OpenAi => "openai",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettings {
    #[serde(default = "default_shortcut_setting")]
    pub shortcut: String,
    #[serde(default = "default_background_opacity")]
    pub background_opacity: f64,
    pub provider: ProviderKind,
    pub model: String,
    pub result_language: String,
    pub prompt_mode: String,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            shortcut: shortcut::DEFAULT_SHORTCUT_LABEL.to_string(),
            background_opacity: default_background_opacity(),
            provider: ProviderKind::Gemini,
            model: ProviderKind::Gemini.default_model().to_string(),
            result_language: "ja".to_string(),
            prompt_mode: "word-study".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettingsView {
    pub shortcut: String,
    pub background_opacity: f64,
    pub provider: ProviderKind,
    pub model: String,
    pub result_language: String,
    pub prompt_mode: String,
    pub api_key_configured: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettingsUpdate {
    pub shortcut: String,
    pub background_opacity: f64,
    pub provider: ProviderKind,
    pub model: String,
    pub result_language: String,
    pub prompt_mode: String,
    pub api_key: Option<String>,
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

        let normalized_shortcut = shortcut::normalize_shortcut_label(&update.shortcut)?;
        let background_opacity = validate_background_opacity(update.background_opacity)?;

        let settings = ProviderSettings {
            shortcut: normalized_shortcut,
            background_opacity,
            provider: update.provider,
            model: model.to_string(),
            result_language: result_language.to_string(),
            prompt_mode: "word-study".to_string(),
        };

        if let Some(api_key) = update.api_key {
            let trimmed = api_key.trim();
            if !trimmed.is_empty() {
                secrets::write_api_key(update.provider, trimmed)?;
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
    ProviderSettingsView {
        shortcut: settings.shortcut,
        background_opacity: settings.background_opacity,
        provider: settings.provider,
        model: settings.model,
        result_language: settings.result_language,
        prompt_mode: settings.prompt_mode,
        api_key_configured,
    }
}

fn read_settings(app: &AppHandle) -> Result<ProviderSettings, AppError> {
    let path = settings_path(app)?;
    if !path.exists() {
        return Ok(ProviderSettings::default());
    }

    let raw = fs::read_to_string(&path).map_err(|error| {
        AppError::provider_request_failed(format!("provider settings read failed: {error}"), true)
    })?;

    serde_json::from_str(&raw).map_err(|error| {
        AppError::provider_request_failed(format!("provider settings parse failed: {error}"), false)
    })
}

fn write_settings(app: &AppHandle, settings: &ProviderSettings) -> Result<(), AppError> {
    let path = settings_path(app)?;
    ensure_parent(&path)?;
    let raw = serde_json::to_string_pretty(settings).map_err(|error| {
        AppError::provider_request_failed(
            format!("provider settings serialize failed: {error}"),
            false,
        )
    })?;

    fs::write(path, raw).map_err(|error| {
        AppError::provider_request_failed(format!("provider settings write failed: {error}"), true)
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
            AppError::provider_request_failed(format!("app config dir unavailable: {error}"), true)
        })
}

fn ensure_parent(path: &PathBuf) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            AppError::provider_request_failed(
                format!("app config dir create failed: {error}"),
                true,
            )
        })?;
    }

    Ok(())
}

fn default_shortcut_setting() -> String {
    shortcut::DEFAULT_SHORTCUT_LABEL.to_string()
}

fn default_background_opacity() -> f64 {
    0.94
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
