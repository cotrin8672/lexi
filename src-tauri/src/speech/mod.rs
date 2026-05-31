use crate::errors::{AppError, AppErrorCode};

#[cfg(windows)]
mod windows;

#[tauri::command]
pub fn speak_headword(text: String) -> Result<(), AppError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    #[cfg(windows)]
    {
        windows::speak(trimmed).map_err(speech_error)
    }

    #[cfg(not(windows))]
    {
        let _ = trimmed;
        Err(speech_error(
            "Speech synthesis is only available on Windows in this build.".to_string(),
        ))
    }
}

#[tauri::command]
pub fn stop_headword_speech() -> Result<(), AppError> {
    #[cfg(windows)]
    {
        windows::stop().map_err(speech_error)
    }

    #[cfg(not(windows))]
    {
        Ok(())
    }
}

fn speech_error(diagnostic_message: String) -> AppError {
    AppError::new(
        AppErrorCode::ProviderRequestFailed,
        "Could not play pronunciation. Add English (United States) in Settings > Time & language > Language & region, then download the speech pack.",
        diagnostic_message,
        false,
    )
}
