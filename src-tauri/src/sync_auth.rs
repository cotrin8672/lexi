use crate::{
    errors::{AppError, AppErrorCode},
    secrets,
    settings::SettingsState,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter};

const CALLBACK_PATH: &str = "/auth/callback";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncAuthStatus {
    pub configured: bool,
    pub signed_in: bool,
    pub user_id: Option<String>,
    pub user_email: Option<String>,
    pub callback_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleSignInStart {
    pub auth_url: String,
    pub callback_url: String,
}

#[derive(Debug, Deserialize)]
struct SupabaseTokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: Option<u64>,
    token_type: String,
    user: SupabaseUser,
}

#[derive(Debug, Deserialize, Serialize)]
struct SupabaseUser {
    id: String,
    email: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredSupabaseSession {
    access_token: String,
    refresh_token: String,
    token_type: String,
    expires_at: Option<u64>,
    user: SupabaseUser,
}

#[tauri::command]
pub fn get_sync_auth_status(
    app: AppHandle,
    settings_state: tauri::State<'_, SettingsState>,
) -> Result<SyncAuthStatus, AppError> {
    let settings = settings_state.load_settings(&app)?;
    Ok(status_from_session(
        settings.supabase_configured(),
        settings.supabase_callback_url(),
        read_stored_session()?,
    ))
}

#[tauri::command]
pub fn start_google_sign_in(
    app: AppHandle,
    settings_state: tauri::State<'_, SettingsState>,
) -> Result<GoogleSignInStart, AppError> {
    let settings = settings_state.load_settings(&app)?;
    let Some((supabase_url, supabase_anon_key)) = settings.supabase_connection() else {
        return Err(AppError::new(
            AppErrorCode::ProviderNotConfigured,
            "Supabase is not configured.",
            "Supabase URL and anon key are required before Google sign-in.",
            false,
        ));
    };

    let callback_url = settings.supabase_callback_url();
    let callback_port = settings.supabase_callback_port;
    let verifier = random_url_token(64);
    let challenge = pkce_challenge(&verifier);
    let state = random_url_token(32);
    let auth_url = build_google_auth_url(&supabase_url, &callback_url, &challenge, &state);

    start_callback_listener(
        app,
        callback_port,
        supabase_url,
        supabase_anon_key,
        callback_url.clone(),
        verifier,
        state,
    )?;

    Ok(GoogleSignInStart {
        auth_url,
        callback_url,
    })
}

#[tauri::command]
pub fn sign_out_sync() -> Result<(), AppError> {
    secrets::delete_supabase_session()
}

fn start_callback_listener(
    app: AppHandle,
    callback_port: u16,
    supabase_url: String,
    supabase_anon_key: String,
    callback_url: String,
    verifier: String,
    expected_state: String,
) -> Result<(), AppError> {
    let listener = TcpListener::bind(("127.0.0.1", callback_port)).map_err(|error| {
        AppError::new(
            AppErrorCode::ProviderRequestFailed,
            "Could not start the Google sign-in callback.",
            format!("Supabase callback listener bind failed: {error}"),
            true,
        )
    })?;

    std::thread::spawn(move || {
        let result = accept_callback(
            listener,
            &supabase_url,
            &supabase_anon_key,
            &verifier,
            &expected_state,
        );
        let payload = match result {
            Ok(session) => {
                let email = session.user.email.clone();
                let user_id = session.user.id.clone();
                match store_session(&session) {
                    Ok(()) => SyncAuthStatus {
                        configured: true,
                        signed_in: true,
                        user_id: Some(user_id),
                        user_email: email,
                        callback_url,
                    },
                    Err(error) => {
                        let _ = app.emit("lexi:sync-auth-error", error);
                        return;
                    }
                }
            }
            Err(error) => {
                let _ = app.emit("lexi:sync-auth-error", error);
                return;
            }
        };
        let _ = app.emit("lexi:sync-auth", payload);
    });

    Ok(())
}

fn accept_callback(
    listener: TcpListener,
    supabase_url: &str,
    supabase_anon_key: &str,
    verifier: &str,
    expected_state: &str,
) -> Result<StoredSupabaseSession, AppError> {
    let (mut stream, _) = listener.accept().map_err(|error| {
        AppError::new(
            AppErrorCode::ProviderRequestFailed,
            "Google sign-in callback failed.",
            format!("Supabase callback accept failed: {error}"),
            true,
        )
    })?;

    let request = read_request(&mut stream)?;
    let query = parse_callback_query(&request)?;
    let state = query
        .get("lexi_state")
        .or_else(|| query.get("state"))
        .map(String::as_str)
        .unwrap_or_default();
    if state != expected_state {
        write_callback_response(&mut stream, false)?;
        return Err(AppError::new(
            AppErrorCode::ProviderRequestFailed,
            "Google sign-in callback was rejected.",
            "Supabase callback state did not match the pending sign-in.",
            false,
        ));
    }

    let Some(code) = query.get("code") else {
        write_callback_response(&mut stream, false)?;
        return Err(AppError::new(
            AppErrorCode::ProviderRequestFailed,
            "Google sign-in did not return an auth code.",
            "Supabase callback was missing the code query parameter.",
            false,
        ));
    };

    let session = match tauri::async_runtime::block_on(exchange_code_for_session(
        supabase_url,
        supabase_anon_key,
        code,
        verifier,
    )) {
        Ok(session) => session,
        Err(error) => {
            write_callback_response(&mut stream, false)?;
            return Err(error);
        }
    };
    write_callback_response(&mut stream, true)?;
    Ok(session)
}

async fn exchange_code_for_session(
    supabase_url: &str,
    supabase_anon_key: &str,
    code: &str,
    verifier: &str,
) -> Result<StoredSupabaseSession, AppError> {
    let endpoint = format!("{supabase_url}/auth/v1/token?grant_type=pkce");
    let response = reqwest::Client::new()
        .post(endpoint)
        .header("apikey", supabase_anon_key)
        .header("Authorization", format!("Bearer {supabase_anon_key}"))
        .json(&serde_json::json!({
            "auth_code": code,
            "code_verifier": verifier,
        }))
        .send()
        .await
        .map_err(|error| {
            AppError::new(
                AppErrorCode::ProviderRequestFailed,
                "Google sign-in token exchange failed.",
                format!("Supabase token request failed: {error}"),
                true,
            )
        })?;

    let status = response.status();
    let body = response.text().await.map_err(|error| {
        AppError::new(
            AppErrorCode::ProviderRequestFailed,
            "Google sign-in token exchange failed.",
            format!("Supabase token response read failed: {error}"),
            true,
        )
    })?;

    if !status.is_success() {
        return Err(AppError::new(
            AppErrorCode::ProviderRequestFailed,
            "Google sign-in token exchange failed.",
            format!("Supabase token endpoint returned {status}: {body}"),
            false,
        ));
    }

    let token = serde_json::from_str::<SupabaseTokenResponse>(&body).map_err(|error| {
        AppError::new(
            AppErrorCode::ProviderRequestFailed,
            "Google sign-in token response was invalid.",
            format!("Supabase token response parse failed: {error}"),
            false,
        )
    })?;

    Ok(StoredSupabaseSession {
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        token_type: token.token_type,
        expires_at: token
            .expires_in
            .and_then(|seconds| now_unix_seconds().checked_add(seconds)),
        user: token.user,
    })
}

fn build_google_auth_url(
    supabase_url: &str,
    callback_url: &str,
    challenge: &str,
    state: &str,
) -> String {
    let callback_url = format!("{}?lexi_state={}", callback_url, percent_encode(state));
    format!(
        "{}/auth/v1/authorize?provider=google&redirect_to={}&code_challenge={}&code_challenge_method=s256",
        supabase_url.trim_end_matches('/'),
        percent_encode(&callback_url),
        percent_encode(challenge)
    )
}

fn read_request(stream: &mut TcpStream) -> Result<String, AppError> {
    let mut buffer = [0_u8; 4096];
    let size = stream.read(&mut buffer).map_err(|error| {
        AppError::new(
            AppErrorCode::ProviderRequestFailed,
            "Google sign-in callback could not be read.",
            format!("Supabase callback request read failed: {error}"),
            true,
        )
    })?;

    String::from_utf8(buffer[..size].to_vec()).map_err(|error| {
        AppError::new(
            AppErrorCode::ProviderRequestFailed,
            "Google sign-in callback was invalid.",
            format!("Supabase callback request was not UTF-8: {error}"),
            false,
        )
    })
}

fn parse_callback_query(request: &str) -> Result<BTreeMap<String, String>, AppError> {
    let first_line = request.lines().next().ok_or_else(|| {
        AppError::new(
            AppErrorCode::ProviderRequestFailed,
            "Google sign-in callback was invalid.",
            "Supabase callback request was empty.",
            false,
        )
    })?;
    let path = first_line.split_whitespace().nth(1).ok_or_else(|| {
        AppError::new(
            AppErrorCode::ProviderRequestFailed,
            "Google sign-in callback was invalid.",
            "Supabase callback request did not contain a path.",
            false,
        )
    })?;

    let (route, query) = path.split_once('?').unwrap_or((path, ""));
    if route != CALLBACK_PATH {
        return Err(AppError::new(
            AppErrorCode::ProviderRequestFailed,
            "Google sign-in callback path was invalid.",
            format!("unexpected Supabase callback path: {route}"),
            false,
        ));
    }

    let mut values = BTreeMap::new();
    for pair in query.split('&').filter(|item| !item.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        values.insert(percent_decode(key), percent_decode(value));
    }
    Ok(values)
}

fn write_callback_response(stream: &mut TcpStream, success: bool) -> Result<(), AppError> {
    let title = if success {
        "Lexi sign-in complete"
    } else {
        "Lexi sign-in failed"
    };
    let message = if success {
        "Google sign-in completed. You can close this browser tab."
    } else {
        "Google sign-in failed. Return to Lexi and try again."
    };
    let body = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>{title}</title><body><h1>{title}</h1><p>{message}</p></body>"
    );
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(response.as_bytes()).map_err(|error| {
        AppError::new(
            AppErrorCode::ProviderRequestFailed,
            "Google sign-in callback response failed.",
            format!("Supabase callback response write failed: {error}"),
            true,
        )
    })
}

fn read_stored_session() -> Result<Option<StoredSupabaseSession>, AppError> {
    secrets::read_supabase_session()?
        .map(|raw| {
            serde_json::from_str(&raw).map_err(|error| {
                AppError::new(
                    AppErrorCode::ProviderRequestFailed,
                    "Stored Supabase session is invalid.",
                    format!("Supabase session parse failed: {error}"),
                    false,
                )
            })
        })
        .transpose()
}

fn store_session(session: &StoredSupabaseSession) -> Result<(), AppError> {
    let raw = serde_json::to_string(session).map_err(|error| {
        AppError::new(
            AppErrorCode::ProviderRequestFailed,
            "Supabase session could not be stored.",
            format!("Supabase session serialize failed: {error}"),
            false,
        )
    })?;
    secrets::write_supabase_session(&raw)
}

fn status_from_session(
    configured: bool,
    callback_url: String,
    session: Option<StoredSupabaseSession>,
) -> SyncAuthStatus {
    match session {
        Some(session) => SyncAuthStatus {
            configured,
            signed_in: true,
            user_id: Some(session.user.id),
            user_email: session.user.email,
            callback_url,
        },
        None => SyncAuthStatus {
            configured,
            signed_in: false,
            user_id: None,
            user_email: None,
            callback_url,
        },
    }
}

fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn random_url_token(byte_len: usize) -> String {
    let mut bytes = vec![0_u8; byte_len];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(hex) = u8::from_str_radix(&value[index + 1..index + 3], 16) {
                output.push(hex);
                index += 3;
                continue;
            }
        }
        output.push(if bytes[index] == b'+' {
            b' '
        } else {
            bytes[index]
        });
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{build_google_auth_url, parse_callback_query, percent_decode, percent_encode};

    #[test]
    fn encodes_callback_url_for_query_param() {
        assert_eq!(
            percent_encode("http://localhost:38271/auth/callback"),
            "http%3A%2F%2Flocalhost%3A38271%2Fauth%2Fcallback"
        );
    }

    #[test]
    fn decodes_callback_query_values() {
        let query =
            "GET /auth/callback?code=abc%20123&state=state_value HTTP/1.1\r\nHost: localhost\r\n";
        let parsed = parse_callback_query(query).expect("valid callback query");

        assert_eq!(parsed.get("code").map(String::as_str), Some("abc 123"));
        assert_eq!(parsed.get("state").map(String::as_str), Some("state_value"));
    }

    #[test]
    fn includes_lexi_state_in_redirect_to_url() {
        let url = build_google_auth_url(
            "https://project-ref.supabase.co",
            "http://localhost:38271/auth/callback",
            "challenge_value",
            "state_value",
        );

        assert!(url.contains("redirect_to=http%3A%2F%2Flocalhost%3A38271%2Fauth%2Fcallback%3Flexi_state%3Dstate_value"));
        assert!(!url.contains("&state=state_value"));
    }

    #[test]
    fn decodes_plus_as_space() {
        assert_eq!(percent_decode("hello+world"), "hello world");
    }
}
