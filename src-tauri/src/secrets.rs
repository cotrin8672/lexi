use crate::{errors::AppError, settings::ProviderKind};

const SERVICE_NAME: &str = "io.github.cotrin8672.lexi";
const SUPABASE_SESSION_USER: &str = "supabase-session";

pub fn write_api_key(provider: ProviderKind, api_key: &str) -> Result<(), AppError> {
    platform::write_secret(&secret_name(provider), provider.secret_user(), api_key)
}

pub fn read_api_key(provider: ProviderKind) -> Result<Option<String>, AppError> {
    if let Some(secret) = read_api_key_from_env(provider) {
        return Ok(Some(secret));
    }

    match platform::read_secret(&secret_name(provider)) {
        Ok(Some(secret)) => Ok(Some(secret)),
        Ok(None) => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn has_api_key(provider: ProviderKind) -> bool {
    read_api_key(provider)
        .map(|key| key.is_some())
        .unwrap_or(false)
}

pub fn write_supabase_session(session_json: &str) -> Result<(), AppError> {
    platform::write_secret(
        &named_secret_name(SUPABASE_SESSION_USER),
        SUPABASE_SESSION_USER,
        session_json,
    )
}

pub fn read_supabase_session() -> Result<Option<String>, AppError> {
    platform::read_secret(&named_secret_name(SUPABASE_SESSION_USER))
}

pub fn delete_supabase_session() -> Result<(), AppError> {
    platform::delete_secret(&named_secret_name(SUPABASE_SESSION_USER))
}

fn secret_name(provider: ProviderKind) -> String {
    named_secret_name(provider.secret_user())
}

fn named_secret_name(name: &str) -> String {
    format!("{SERVICE_NAME}:{name}")
}

fn read_api_key_from_env(provider: ProviderKind) -> Option<String> {
    api_key_from_lookup(provider, |name| std::env::var(name).ok())
}

fn api_key_from_lookup(
    provider: ProviderKind,
    lookup: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    env_var_names(provider)
        .iter()
        .filter_map(|name| lookup(name))
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
}

fn env_var_names(provider: ProviderKind) -> &'static [&'static str] {
    match provider {
        ProviderKind::Mock => &[],
        ProviderKind::Gemini => &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
        ProviderKind::OpenAi => &["OPENAI_API_KEY"],
        ProviderKind::DeepL => &["DEEPL_API_KEY", "DEEPL_AUTH_KEY"],
    }
}

#[cfg(test)]
mod tests {
    use super::api_key_from_lookup;
    use crate::settings::ProviderKind;
    use std::collections::BTreeMap;

    #[test]
    fn reads_gemini_key_from_preferred_env_name() {
        let values = BTreeMap::from([
            ("GEMINI_API_KEY", "gemini-secret"),
            ("GOOGLE_API_KEY", "google-secret"),
        ]);

        assert_eq!(
            api_key_from_lookup(ProviderKind::Gemini, |name| values
                .get(name)
                .map(|value| value.to_string())),
            Some("gemini-secret".to_string())
        );
    }

    #[test]
    fn falls_back_to_google_key_for_gemini() {
        let values = BTreeMap::from([("GOOGLE_API_KEY", "google-secret")]);

        assert_eq!(
            api_key_from_lookup(ProviderKind::Gemini, |name| values
                .get(name)
                .map(|value| value.to_string())),
            Some("google-secret".to_string())
        );
    }

    #[test]
    fn ignores_empty_env_values() {
        let values = BTreeMap::from([("OPENAI_API_KEY", "   ")]);

        assert_eq!(
            api_key_from_lookup(ProviderKind::OpenAi, |name| values
                .get(name)
                .map(|value| value.to_string())),
            None
        );
    }
}

#[cfg(windows)]
mod platform {
    use crate::errors::AppError;
    use std::ptr;
    use windows::{
        core::{PCWSTR, PWSTR},
        Win32::Security::Credentials::{
            CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
            CRED_TYPE_GENERIC,
        },
    };

    pub fn write_secret(target_name: &str, user_name: &str, secret: &str) -> Result<(), AppError> {
        let mut target_name_wide = wide_null(target_name);
        let mut user_name_wide = wide_null(user_name);
        let mut blob = secret.as_bytes().to_vec();

        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target_name_wide.as_mut_ptr()),
            CredentialBlobSize: blob.len() as u32,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: PWSTR(user_name_wide.as_mut_ptr()),
            ..Default::default()
        };

        unsafe { CredWriteW(&credential, 0) }.map_err(|error| {
            AppError::credential_storage_failed(
                format!("Windows Credential Manager write failed: {error}"),
                true,
            )
        })
    }

    pub fn read_secret(target_name: &str) -> Result<Option<String>, AppError> {
        let target_name_wide = wide_null(target_name);
        let mut credential_ptr = ptr::null_mut();

        let result = unsafe {
            CredReadW(
                PCWSTR(target_name_wide.as_ptr()),
                CRED_TYPE_GENERIC,
                0,
                &mut credential_ptr,
            )
        };

        if let Err(error) = result {
            let code = error.code().0 as u32;
            if code == windows::Win32::Foundation::ERROR_NOT_FOUND.0 {
                return Ok(None);
            }

            return Err(AppError::credential_storage_failed(
                format!("Windows Credential Manager read failed: {error}"),
                true,
            ));
        }

        if credential_ptr.is_null() {
            return Ok(None);
        }

        let credential = unsafe { &*credential_ptr };
        let bytes = unsafe {
            std::slice::from_raw_parts(
                credential.CredentialBlob,
                credential.CredentialBlobSize as usize,
            )
        };
        let secret = String::from_utf8(bytes.to_vec()).map_err(|error| {
            AppError::credential_storage_failed(
                format!("Windows Credential Manager secret decode failed: {error}"),
                false,
            )
        });

        unsafe {
            CredFree(credential_ptr.cast());
        }

        secret.map(Some)
    }

    pub fn delete_secret(target_name: &str) -> Result<(), AppError> {
        let target_name_wide = wide_null(target_name);
        let result =
            unsafe { CredDeleteW(PCWSTR(target_name_wide.as_ptr()), CRED_TYPE_GENERIC, 0) };

        if let Err(error) = result {
            let code = error.code().0 as u32;
            if code == windows::Win32::Foundation::ERROR_NOT_FOUND.0 {
                return Ok(());
            }

            return Err(AppError::credential_storage_failed(
                format!("Windows Credential Manager delete failed: {error}"),
                true,
            ));
        }

        Ok(())
    }

    fn wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }
}

#[cfg(not(windows))]
mod platform {
    use crate::errors::AppError;

    pub fn write_secret(
        _target_name: &str,
        _user_name: &str,
        _secret: &str,
    ) -> Result<(), AppError> {
        Err(AppError::credential_storage_failed(
            "OS credential storage is not implemented for this platform",
            false,
        ))
    }

    pub fn read_secret(_target_name: &str) -> Result<Option<String>, AppError> {
        Err(AppError::credential_storage_failed(
            "OS credential storage is not implemented for this platform",
            false,
        ))
    }

    pub fn delete_secret(_target_name: &str) -> Result<(), AppError> {
        Err(AppError::credential_storage_failed(
            "OS credential storage is not implemented for this platform",
            false,
        ))
    }
}
