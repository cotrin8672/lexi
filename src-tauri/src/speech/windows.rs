use rand::seq::SliceRandom;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::sync::Mutex;
use windows::core::PCWSTR;
use windows::Win32::Foundation::BOOL;
use windows::Win32::Media::Speech::{
    IEnumSpObjectTokens, ISpObjectToken, ISpObjectTokenCategory, ISpVoice, SpObjectTokenCategory,
    SpVoice, SPCAT_VOICES, SPF_PURGEBEFORESPEAK,
};
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER};
use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};

static SPEECH_MUTEX: Mutex<()> = Mutex::new(());

struct ComApartment;

impl ComApartment {
    fn init() -> Result<Self, String> {
        unsafe {
            OleInitialize(None).map_err(|error| format!("OleInitialize failed: {error}"))?;
        }
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe {
            OleUninitialize();
        }
    }
}

pub fn speak(text: &str) -> Result<(), String> {
    let text = text.to_owned();
    std::thread::spawn(move || {
        let _guard = match SPEECH_MUTEX.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        let _ = speak_on_thread(&text);
    });
    Ok(())
}

pub fn stop() -> Result<(), String> {
    std::thread::spawn(|| {
        let _guard = match SPEECH_MUTEX.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        let _ = stop_on_thread();
    });
    Ok(())
}

fn speak_on_thread(text: &str) -> Result<(), String> {
    let _com = ComApartment::init()?;
    unsafe {
        let voice: ISpVoice =
            CoCreateInstance(&SpVoice, None, CLSCTX_INPROC_SERVER).map_err(map_windows_error)?;

        if let Ok(token) = find_english_voice_token() {
            voice.SetVoice(&token).map_err(map_windows_error)?;
        }

        let wide = encode_wide(text);
        voice
            .Speak(
                PCWSTR::from_raw(wide.as_ptr()),
                SPF_PURGEBEFORESPEAK.0 as u32,
                None,
            )
            .map_err(map_windows_error)?;
    }
    Ok(())
}

fn stop_on_thread() -> Result<(), String> {
    let _com = ComApartment::init()?;
    unsafe {
        let voice: ISpVoice =
            CoCreateInstance(&SpVoice, None, CLSCTX_INPROC_SERVER).map_err(map_windows_error)?;
        voice
            .Speak(None, SPF_PURGEBEFORESPEAK.0 as u32, None)
            .map_err(map_windows_error)?;
    }
    Ok(())
}

fn find_english_voice_token() -> Result<ISpObjectToken, String> {
    unsafe {
        let category: ISpObjectTokenCategory =
            CoCreateInstance(&SpObjectTokenCategory, None, CLSCTX_INPROC_SERVER)
                .map_err(map_windows_error)?;
        category
            .SetId(SPCAT_VOICES, BOOL(0))
            .map_err(map_windows_error)?;

        for required in [
            windows::core::w!("Language=409"),
            windows::core::w!("Language=809"),
            windows::core::w!(""),
        ] {
            let enum_tokens = category
                .EnumTokens(required, None)
                .map_err(map_windows_error)?;
            if let Some(token) = pick_best_voice_token(&enum_tokens)? {
                return Ok(token);
            }
        }
    }

    Err(
        "No English speech voice is installed. Add English (United States) in Windows language settings and download speech.".to_string(),
    )
}

fn pick_best_voice_token(
    enum_tokens: &IEnumSpObjectTokens,
) -> Result<Option<ISpObjectToken>, String> {
    unsafe {
        let mut count = 0u32;
        enum_tokens
            .GetCount(&mut count)
            .map_err(map_windows_error)?;
        if count == 0 {
            return Ok(None);
        }

        let mut candidates = Vec::new();

        for index in 0..count {
            let token = enum_tokens.Item(index).map_err(map_windows_error)?;
            let id = token.GetId().map_err(map_windows_error)?;
            let id_text = pwstr_to_string(id.0);
            CoTaskMemFree(Some(id.0.cast()));
            let score = score_voice_id(&id_text);
            if score > 0 {
                candidates.push(token);
            }
        }

        Ok(candidates.choose(&mut rand::thread_rng()).cloned())
    }
}

fn score_voice_id(id: &str) -> i32 {
    let lower = id.to_ascii_lowercase();
    let mut score = 0;

    if lower.contains("409") || lower.contains("en-us") || lower.contains("enus") {
        score += 100;
    } else if lower.contains("809") || lower.contains("en-gb") || lower.contains("engb") {
        score += 80;
    } else if lower.contains("english") || lower.contains("\\en") {
        score += 60;
    }

    if lower.contains("zira") || lower.contains("david") || lower.contains("mark") {
        score += 40;
    } else if lower.contains("jenny") || lower.contains("aria") || lower.contains("guy") {
        score += 30;
    }

    if lower.contains("haruka")
        || lower.contains("ayumi")
        || lower.contains("ichiro")
        || lower.contains("nanami")
        || lower.contains("keita")
        || lower.contains("japanese")
    {
        score -= 200;
    }

    score
}

fn pwstr_to_string(value: *mut u16) -> String {
    if value.is_null() {
        return String::new();
    }

    unsafe {
        let mut length = 0usize;
        while *value.add(length) != 0 {
            length += 1;
        }
        OsString::from_wide(std::slice::from_raw_parts(value, length))
            .to_string_lossy()
            .into_owned()
    }
}

fn encode_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn map_windows_error(error: windows::core::Error) -> String {
    format!("Windows speech API failed: {error}")
}

#[cfg(test)]
mod tests {
    use super::score_voice_id;

    #[test]
    fn prefers_us_english_voice_ids() {
        assert!(
            score_voice_id("HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Speech\\Voices\\Tokens\\TTS_MS_EN-US_ZIRA_11.0")
                > score_voice_id("HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Speech\\Voices\\Tokens\\TTS_MS_JA-JP_HARUKA_11.0")
        );
    }
}
