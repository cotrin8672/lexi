use crate::{
    errors::{AppError, AppErrorCode},
    settings::SettingsState,
    sync_auth,
    vocabulary::{self, PendingMutation, PulledChange},
};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};

const PULL_BATCH_LIMIT: i64 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SyncLifecycle {
    #[default]
    Idle,
    Syncing,
    Synced,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub configured: bool,
    pub signed_in: bool,
    pub lifecycle: SyncLifecycle,
    pub pending_mutations: u32,
    pub last_server_revision: i64,
    pub last_sync_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SyncRuntimeState {
    lifecycle: SyncLifecycle,
    last_sync_at: Option<String>,
    last_error: Option<String>,
}

pub struct SyncRuntime {
    state: Mutex<SyncRuntimeState>,
    in_flight: AtomicBool,
    rerun_requested: AtomicBool,
}

impl Default for SyncRuntime {
    fn default() -> Self {
        Self {
            state: Mutex::new(SyncRuntimeState::default()),
            in_flight: AtomicBool::new(false),
            rerun_requested: AtomicBool::new(false),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MutationAck {
    operation_id: String,
    server_revision: i64,
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullResponse {
    changes: Vec<PulledChange>,
    last_revision: i64,
}

pub fn setup(app: &AppHandle) {
    schedule_sync(app.clone());
}

pub fn schedule_sync(app: AppHandle) {
    if app.try_state::<SyncRuntime>().is_none() {
        return;
    }

    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_sync_cycle(&app).await {
            let user_message = sync_failure_user_message(&error);
            update_runtime(&app, |state| {
                state.lifecycle = SyncLifecycle::Error;
                state.last_error = Some(user_message);
            });
            let _ = app.emit("lexi:sync-status", build_status(&app));
        }
    });
}

#[tauri::command]
pub fn get_sync_status(
    app: AppHandle,
    settings_state: tauri::State<'_, SettingsState>,
) -> Result<SyncStatus, AppError> {
    let settings = settings_state.load_settings(&app)?;
    let signed_in = sync_auth::read_session()?.is_some();
    let runtime = runtime_state(&app);

    Ok(SyncStatus {
        configured: settings.supabase_configured(),
        signed_in,
        lifecycle: runtime.lifecycle,
        pending_mutations: vocabulary::count_pending_mutations(&app).unwrap_or(0),
        last_server_revision: vocabulary::get_last_server_revision(&app).unwrap_or(0),
        last_sync_at: runtime.last_sync_at.clone(),
        last_error: runtime.last_error.clone(),
    })
}

#[tauri::command]
pub fn retry_sync(app: AppHandle) -> Result<(), AppError> {
    schedule_sync(app);
    Ok(())
}

pub fn reset_runtime(app: &AppHandle) {
    update_runtime(app, |state| {
        state.lifecycle = SyncLifecycle::Idle;
        state.last_sync_at = None;
        state.last_error = None;
    });
    let _ = app.emit("lexi:sync-status", build_status(app));
}

pub async fn run_sync_cycle(app: &AppHandle) -> Result<(), AppError> {
    let runtime = app.state::<SyncRuntime>();
    let runtime = runtime.inner();

    if !mark_sync_started_or_request_rerun(runtime) {
        return Ok(());
    }

    struct InFlightGuard<'a> {
        runtime: &'a SyncRuntime,
        released: bool,
    }

    impl Drop for InFlightGuard<'_> {
        fn drop(&mut self) {
            if !self.released {
                self.runtime.in_flight.store(false, Ordering::SeqCst);
            }
        }
    }

    impl InFlightGuard<'_> {
        fn release(&mut self) {
            self.runtime.in_flight.store(false, Ordering::SeqCst);
            self.released = true;
        }
    }

    let mut guard = InFlightGuard {
        runtime,
        released: false,
    };

    loop {
        runtime.rerun_requested.store(false, Ordering::SeqCst);

        let settings_state = app.state::<SettingsState>();
        let settings = settings_state.load_settings(app)?;
        let Some((supabase_url, supabase_anon_key)) = settings.supabase_connection() else {
            return Ok(());
        };

        let Some(session) = sync_auth::read_session()? else {
            return Ok(());
        };

        update_runtime(app, |state| {
            state.lifecycle = SyncLifecycle::Syncing;
            state.last_error = None;
        });
        let _ = app.emit("lexi:sync-status", build_status(app));

        let session =
            sync_auth::refresh_session_if_needed(&supabase_url, &supabase_anon_key, session)
                .await?;

        if let Some(user_id) = sync_auth::current_user_id() {
            vocabulary::migrate_local_rows_to_user(app, &user_id)?;
        }

        push_pending_mutations(
            app,
            &supabase_url,
            &supabase_anon_key,
            &session.access_token,
        )
        .await?;

        if !crate::vocabulary_bootstrap::is_bootstrap_complete(app)? {
            crate::vocabulary_bootstrap::bootstrap_from_supabase(
                app,
                &supabase_url,
                &supabase_anon_key,
                &session.access_token,
            )
            .await?;
        }

        pull_remote_changes(
            app,
            &supabase_url,
            &supabase_anon_key,
            &session.access_token,
        )
        .await?;

        update_runtime(app, |state| {
            state.lifecycle = SyncLifecycle::Synced;
            state.last_sync_at = Some(now_iso());
            state.last_error = None;
        });
        let _ = app.emit("lexi:sync-status", build_status(app));

        if runtime.rerun_requested.swap(false, Ordering::SeqCst) {
            continue;
        }

        guard.release();
        if runtime.rerun_requested.swap(false, Ordering::SeqCst)
            && mark_sync_started_or_request_rerun(runtime)
        {
            guard.released = false;
            continue;
        } else {
            break;
        }
    }

    Ok(())
}

async fn push_pending_mutations(
    app: &AppHandle,
    supabase_url: &str,
    supabase_anon_key: &str,
    access_token: &str,
) -> Result<(), AppError> {
    let pending = vocabulary::list_pending_mutations(app, 20)?;
    for mutation in pending {
        match push_mutation(supabase_url, supabase_anon_key, access_token, &mutation).await {
            Ok(ack) => {
                vocabulary::acknowledge_mutation(app, &ack.operation_id, ack.server_revision)?;
            }
            Err(error) => {
                if should_record_mutation_failure(&error) {
                    vocabulary::fail_mutation(
                        app,
                        &mutation.operation_id,
                        &error.diagnostic_message,
                        error.retryable,
                    )?;
                }
                return Err(error);
            }
        }
    }
    Ok(())
}

async fn push_mutation(
    supabase_url: &str,
    supabase_anon_key: &str,
    access_token: &str,
    mutation: &PendingMutation,
) -> Result<MutationAck, AppError> {
    let envelope = build_mutation_envelope(mutation)?;

    let endpoint = format!(
        "{}/rest/v1/rpc/apply_vocabulary_mutation",
        supabase_url.trim_end_matches('/')
    );
    let response = reqwest::Client::new()
        .post(endpoint)
        .header("apikey", supabase_anon_key)
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "envelope": envelope }))
        .send()
        .await
        .map_err(|error| {
            AppError::new(
                AppErrorCode::SyncPushFailed,
                "Vocabulary sync is temporarily unavailable.",
                format!("Supabase mutation push failed: {error}"),
                true,
            )
        })?;

    let status = response.status();
    let body = response.text().await.map_err(|error| {
        AppError::new(
            AppErrorCode::SyncPushFailed,
            "Vocabulary sync is temporarily unavailable.",
            format!("Supabase mutation response read failed: {error}"),
            true,
        )
    })?;

    if !status.is_success() {
        return Err(supabase_push_status_error(
            "mutation endpoint",
            status,
            &body,
        ));
    }

    parse_mutation_ack_body(&body, &mutation.operation_id)
}

async fn pull_remote_changes(
    app: &AppHandle,
    supabase_url: &str,
    supabase_anon_key: &str,
    access_token: &str,
) -> Result<(), AppError> {
    let mut since_revision = vocabulary::get_last_server_revision(app)?;
    loop {
        let pull = fetch_changes(
            supabase_url,
            supabase_anon_key,
            access_token,
            since_revision,
            PULL_BATCH_LIMIT,
        )
        .await?;

        if pull.changes.is_empty() {
            if pull.last_revision > since_revision {
                vocabulary::set_last_server_revision(app, pull.last_revision)?;
            }
            break;
        }

        for change in &pull.changes {
            vocabulary::apply_pulled_change(app, change)?;
            since_revision = change.server_revision;
        }

        vocabulary::set_last_server_revision(app, since_revision)?;

        if pull.changes.len() < PULL_BATCH_LIMIT as usize {
            break;
        }
    }

    Ok(())
}

async fn fetch_changes(
    supabase_url: &str,
    supabase_anon_key: &str,
    access_token: &str,
    since_revision: i64,
    batch_limit: i64,
) -> Result<PullResponse, AppError> {
    let endpoint = format!(
        "{}/rest/v1/rpc/pull_vocabulary_changes",
        supabase_url.trim_end_matches('/')
    );
    let response = reqwest::Client::new()
        .post(endpoint)
        .header("apikey", supabase_anon_key)
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "since_revision": since_revision,
            "batch_limit": batch_limit,
        }))
        .send()
        .await
        .map_err(|error| {
            AppError::new(
                AppErrorCode::SyncPullFailed,
                "Vocabulary sync is temporarily unavailable.",
                format!("Supabase pull request failed: {error}"),
                true,
            )
        })?;

    let status = response.status();
    let body = response.text().await.map_err(|error| {
        AppError::new(
            AppErrorCode::SyncPullFailed,
            "Vocabulary sync is temporarily unavailable.",
            format!("Supabase pull response read failed: {error}"),
            true,
        )
    })?;

    if !status.is_success() {
        return Err(supabase_pull_status_error("pull endpoint", status, &body));
    }

    parse_pull_response_body(&body)
}

fn build_status(app: &AppHandle) -> SyncStatus {
    let settings_state = app.state::<SettingsState>();
    let settings = settings_state.load_settings(app).ok();
    let auth = sync_auth::read_session()
        .ok()
        .flatten()
        .map(|_| true)
        .unwrap_or(false);
    let runtime = runtime_state(app);

    SyncStatus {
        configured: settings
            .as_ref()
            .map(|value| value.supabase_configured())
            .unwrap_or(false),
        signed_in: auth,
        lifecycle: runtime.lifecycle,
        pending_mutations: vocabulary::count_pending_mutations(app).unwrap_or(0),
        last_server_revision: vocabulary::get_last_server_revision(app).unwrap_or(0),
        last_sync_at: runtime.last_sync_at.clone(),
        last_error: runtime.last_error.clone(),
    }
}

fn runtime_state(app: &AppHandle) -> SyncRuntimeState {
    app.try_state::<SyncRuntime>()
        .and_then(|runtime| runtime.state.lock().ok().map(|state| state.clone()))
        .unwrap_or_default()
}

fn update_runtime(app: &AppHandle, update: impl FnOnce(&mut SyncRuntimeState)) {
    if let Some(runtime) = app.try_state::<SyncRuntime>() {
        if let Ok(mut state) = runtime.state.lock() {
            update(&mut state);
        }
    }
}

fn mark_sync_started_or_request_rerun(runtime: &SyncRuntime) -> bool {
    if runtime
        .in_flight
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        return true;
    }

    runtime.rerun_requested.store(true, Ordering::SeqCst);
    false
}

fn should_record_mutation_failure(error: &AppError) -> bool {
    error.code != AppErrorCode::SyncAuthRequired
}

fn sync_failure_user_message(error: &AppError) -> String {
    match error.code {
        AppErrorCode::SyncPushFailed => {
            if error.user_message.contains("unavailable") {
                "語彙の送信が一時的に利用できません".to_string()
            } else {
                "語彙の送信に失敗しました".to_string()
            }
        }
        AppErrorCode::SyncPullFailed => {
            if error.user_message.contains("unavailable") {
                "語彙の取得が一時的に利用できません".to_string()
            } else {
                "語彙の取得に失敗しました".to_string()
            }
        }
        AppErrorCode::SyncAuthRequired => {
            if error.user_message.contains("Sign in again")
                || error.user_message.contains("expired")
            {
                "セッションの有効期限が切れました。再ログインしてください".to_string()
            } else {
                "同期の認証に失敗しました".to_string()
            }
        }
        AppErrorCode::VocabularyStoreFailed | AppErrorCode::SettingsIoFailed => {
            "ローカル語彙データの処理に失敗しました".to_string()
        }
        AppErrorCode::CredentialStorageFailed => {
            "保存済み認証情報にアクセスできませんでした".to_string()
        }
        AppErrorCode::ProviderRequestFailed => "語彙の同期中にエラーが発生しました".to_string(),
        _ => error.user_message.clone(),
    }
}

fn now_iso() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("{seconds}")
}

fn build_mutation_envelope(mutation: &PendingMutation) -> Result<serde_json::Value, AppError> {
    let payload =
        serde_json::from_str::<serde_json::Value>(&mutation.payload_json).map_err(|error| {
            AppError::new(
                AppErrorCode::SyncPushFailed,
                "Local vocabulary mutation could not be synced.",
                format!("mutation payload parse failed: {error}"),
                false,
            )
        })?;

    Ok(serde_json::json!({
        "operationId": mutation.operation_id,
        "mutationType": mutation.mutation_type,
        "payload": payload,
    }))
}

fn supabase_response_diagnostic(
    endpoint_label: &str,
    status: reqwest::StatusCode,
    body: &str,
) -> String {
    format!(
        "Supabase {endpoint_label} returned {status} with {} response bytes",
        body.len()
    )
}

fn supabase_push_status_error(
    endpoint_label: &str,
    status: reqwest::StatusCode,
    body: &str,
) -> AppError {
    supabase_status_error(AppErrorCode::SyncPushFailed, endpoint_label, status, body)
}

fn supabase_pull_status_error(
    endpoint_label: &str,
    status: reqwest::StatusCode,
    body: &str,
) -> AppError {
    supabase_status_error(AppErrorCode::SyncPullFailed, endpoint_label, status, body)
}

fn supabase_status_error(
    fallback_code: AppErrorCode,
    endpoint_label: &str,
    status: reqwest::StatusCode,
    body: &str,
) -> AppError {
    let diagnostic = supabase_response_diagnostic(endpoint_label, status, body);
    match status {
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => AppError::new(
            AppErrorCode::SyncAuthRequired,
            "Supabase session expired. Sign in again.",
            diagnostic,
            false,
        ),
        reqwest::StatusCode::REQUEST_TIMEOUT
        | reqwest::StatusCode::CONFLICT
        | reqwest::StatusCode::TOO_EARLY
        | reqwest::StatusCode::TOO_MANY_REQUESTS => AppError::new(
            fallback_code,
            "Vocabulary sync is temporarily unavailable.",
            diagnostic,
            true,
        ),
        _ => AppError::new(
            fallback_code,
            "Vocabulary sync failed.",
            diagnostic,
            status.is_server_error(),
        ),
    }
}

fn parse_mutation_ack_body(
    body: &str,
    expected_operation_id: &str,
) -> Result<MutationAck, AppError> {
    let ack = serde_json::from_str::<MutationAck>(body).map_err(|error| {
        AppError::new(
            AppErrorCode::SyncPushFailed,
            "Vocabulary sync returned an invalid response.",
            format!("Supabase mutation ack parse failed: {error}"),
            false,
        )
    })?;

    if ack.operation_id.trim().is_empty() {
        return Err(AppError::new(
            AppErrorCode::SyncPushFailed,
            "Vocabulary sync returned an invalid response.",
            "Supabase mutation ack was missing operationId.",
            false,
        ));
    }

    if ack.operation_id != expected_operation_id {
        return Err(AppError::new(
            AppErrorCode::SyncPushFailed,
            "Vocabulary sync returned an invalid response.",
            format!(
                "Supabase mutation ack operationId mismatch: expected {}, got {}",
                expected_operation_id, ack.operation_id
            ),
            false,
        ));
    }

    if ack.server_revision < 1 {
        return Err(AppError::new(
            AppErrorCode::SyncPushFailed,
            "Vocabulary sync returned an invalid response.",
            format!(
                "Supabase mutation ack returned invalid serverRevision: {}",
                ack.server_revision
            ),
            false,
        ));
    }

    if ack.status != "accepted" {
        return Err(AppError::new(
            AppErrorCode::SyncPushFailed,
            "Vocabulary sync returned an invalid response.",
            format!(
                "Supabase mutation ack returned unexpected status: {}",
                ack.status
            ),
            false,
        ));
    }

    Ok(ack)
}

fn parse_pull_response_body(body: &str) -> Result<PullResponse, AppError> {
    let pull = serde_json::from_str::<PullResponse>(body).map_err(|error| {
        AppError::new(
            AppErrorCode::SyncPullFailed,
            "Vocabulary sync returned an invalid response.",
            format!("Supabase pull response parse failed: {error}"),
            false,
        )
    })?;

    let max_change_revision = pull
        .changes
        .iter()
        .map(|change| change.server_revision)
        .max()
        .unwrap_or(0);
    if pull.last_revision < max_change_revision {
        return Err(AppError::new(
            AppErrorCode::SyncPullFailed,
            "Vocabulary sync returned an invalid response.",
            format!(
                "Supabase pull lastRevision {} is behind max change revision {}",
                pull.last_revision, max_change_revision
            ),
            false,
        ));
    }

    Ok(pull)
}

#[cfg(test)]
mod tests {
    use super::{
        build_mutation_envelope, fetch_changes, mark_sync_started_or_request_rerun,
        parse_mutation_ack_body, parse_pull_response_body, push_mutation,
        should_record_mutation_failure, supabase_pull_status_error, supabase_push_status_error,
        sync_failure_user_message, SyncLifecycle, SyncRuntime,
    };
    use crate::errors::{AppError, AppErrorCode};
    use crate::vocabulary::PendingMutation;
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        sync::atomic::Ordering,
        thread,
        time::Duration,
    };

    fn pending_mutation(operation_id: &str) -> PendingMutation {
        PendingMutation {
            id: "local-row-1".to_string(),
            operation_id: operation_id.to_string(),
            mutation_type: "save_card_snapshot".to_string(),
            payload_json: serde_json::json!({
                "canonicalKey": "go",
                "canonicalText": "go",
                "resultLanguage": "ja",
                "schemaVersion": "lexi.result.v1",
                "content": { "headword": "go" }
            })
            .to_string(),
            attempts: 0,
        }
    }

    fn serve_once(status: &str, body: &'static str) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let url = format!("http://{}", listener.local_addr().expect("local addr"));
        let status = status.to_string();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept test request");
            read_http_request(&mut stream)
                .and_then(|request| {
                    let response = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    stream.write_all(response.as_bytes()).map(|_| request)
                })
                .expect("serve test response")
        });
        (url, handle)
    }

    fn read_http_request(stream: &mut TcpStream) -> std::io::Result<String> {
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 1024];
        loop {
            let read = stream.read(&mut chunk)?;
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);
            if let Some(header_end) = find_header_end(&buffer) {
                let headers = String::from_utf8_lossy(&buffer[..header_end]).to_string();
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                let body_start = header_end + 4;
                let expected = body_start + content_length;
                while buffer.len() < expected {
                    let read = stream.read(&mut chunk)?;
                    if read == 0 {
                        break;
                    }
                    buffer.extend_from_slice(&chunk[..read]);
                }
                break;
            }
        }
        Ok(String::from_utf8_lossy(&buffer).to_string())
    }

    fn find_header_end(buffer: &[u8]) -> Option<usize> {
        buffer.windows(4).position(|window| window == b"\r\n\r\n")
    }

    #[test]
    fn builds_mutation_envelope_for_rpc() {
        let mutation = pending_mutation("11111111-1111-4111-8111-111111111111");
        let envelope = build_mutation_envelope(&mutation).expect("envelope should build");

        assert_eq!(
            envelope.get("operationId").and_then(|value| value.as_str()),
            Some("11111111-1111-4111-8111-111111111111")
        );
        assert_eq!(
            envelope
                .get("mutationType")
                .and_then(|value| value.as_str()),
            Some("save_card_snapshot")
        );
    }

    #[test]
    fn rejects_invalid_mutation_payload_before_network_call() {
        let mutation = PendingMutation {
            id: "local-row-1".to_string(),
            operation_id: "11111111-1111-4111-8111-111111111111".to_string(),
            mutation_type: "save_card_snapshot".to_string(),
            payload_json: "{not-json".to_string(),
            attempts: 0,
        };

        let error = build_mutation_envelope(&mutation).expect_err("payload parse should fail");

        assert_eq!(error.code, crate::errors::AppErrorCode::SyncPushFailed);
        assert!(!error.retryable);
        assert!(error.diagnostic_message.contains("payload parse failed"));
    }

    #[test]
    fn push_mutation_posts_rpc_envelope_and_parses_ack() {
        let operation_id = "11111111-1111-4111-8111-111111111111";
        let mutation = pending_mutation(operation_id);
        let body = r#"{
            "operationId": "11111111-1111-4111-8111-111111111111",
            "serverRevision": 12,
            "status": "accepted"
        }"#;
        let (url, request) = serve_once("200 OK", body);

        let ack = tauri::async_runtime::block_on(push_mutation(
            &url,
            "publishable-key",
            "access-token",
            &mutation,
        ))
        .expect("push mutation");

        assert_eq!(ack.operation_id, operation_id);
        assert_eq!(ack.server_revision, 12);

        let request = request.join().expect("request thread");
        assert!(request.starts_with("POST /rest/v1/rpc/apply_vocabulary_mutation HTTP/1.1"));
        assert!(request.contains("apikey: publishable-key"));
        assert!(request.contains("authorization: Bearer access-token"));
        assert!(request.contains("\"envelope\""));
        assert!(request.contains("\"operationId\":\"11111111-1111-4111-8111-111111111111\""));
        assert!(request.contains("\"canonicalKey\":\"go\""));
    }

    #[test]
    fn fetch_changes_posts_since_revision_and_batch_limit() {
        let body = r#"{
            "changes": [{
                "serverRevision": 43,
                "operationId": "22222222-2222-4222-8222-222222222222",
                "entityType": "card_snapshot",
                "changeType": "upsert",
                "payload": {
                    "canonicalText": "go",
                    "canonicalKey": "go",
                    "resultLanguage": "ja",
                    "schemaVersion": "lexi.result.v1",
                    "content": { "headword": "go" }
                }
            }],
            "lastRevision": 43
        }"#;
        let (url, request) = serve_once("200 OK", body);

        let pull = tauri::async_runtime::block_on(fetch_changes(
            &url,
            "publishable-key",
            "access-token",
            42,
            20,
        ))
        .expect("fetch changes");

        assert_eq!(pull.changes.len(), 1);
        assert_eq!(pull.last_revision, 43);

        let request = request.join().expect("request thread");
        assert!(request.starts_with("POST /rest/v1/rpc/pull_vocabulary_changes HTTP/1.1"));
        assert!(request.contains("apikey: publishable-key"));
        assert!(request.contains("authorization: Bearer access-token"));
        assert!(request.contains("\"since_revision\":42"));
        assert!(request.contains("\"batch_limit\":20"));
    }

    #[test]
    fn parses_accepted_mutation_ack() {
        let ack = parse_mutation_ack_body(
            r#"{
                "operationId": "11111111-1111-4111-8111-111111111111",
                "serverRevision": 12,
                "status": "accepted"
            }"#,
            "11111111-1111-4111-8111-111111111111",
        )
        .expect("accepted ack should parse");

        assert_eq!(ack.operation_id, "11111111-1111-4111-8111-111111111111");
        assert_eq!(ack.server_revision, 12);
    }

    #[test]
    fn rejects_mutation_ack_with_unexpected_status() {
        let error = parse_mutation_ack_body(
            r#"{
                "operationId": "11111111-1111-4111-8111-111111111111",
                "serverRevision": 12,
                "status": "failed"
            }"#,
            "11111111-1111-4111-8111-111111111111",
        )
        .expect_err("failed ack status should be rejected");

        assert_eq!(error.code, crate::errors::AppErrorCode::SyncPushFailed);
        assert!(!error.retryable);
        assert!(error.diagnostic_message.contains("unexpected status"));
    }

    #[test]
    fn rejects_mutation_ack_for_different_operation() {
        let error = parse_mutation_ack_body(
            r#"{
                "operationId": "22222222-2222-4222-8222-222222222222",
                "serverRevision": 12,
                "status": "accepted"
            }"#,
            "11111111-1111-4111-8111-111111111111",
        )
        .expect_err("ack operation id mismatch should be rejected");

        assert_eq!(error.code, crate::errors::AppErrorCode::SyncPushFailed);
        assert!(!error.retryable);
        assert!(error.diagnostic_message.contains("operationId mismatch"));
    }

    #[test]
    fn parses_pull_response_with_changes() {
        let pull = parse_pull_response_body(
            r#"{
                "changes": [{
                    "serverRevision": 7,
                    "operationId": "11111111-1111-4111-8111-111111111111",
                    "entityType": "card_snapshot",
                    "changeType": "upsert",
                    "payload": {
                        "canonicalText": "go",
                        "canonicalKey": "go",
                        "resultLanguage": "ja",
                        "schemaVersion": "lexi.result.v1",
                        "content": { "headword": "go" }
                    }
                }],
                "lastRevision": 7
            }"#,
        )
        .expect("pull response should parse");

        assert_eq!(pull.changes.len(), 1);
        assert_eq!(pull.last_revision, 7);
    }

    #[test]
    fn rejects_pull_response_when_last_revision_moves_backwards() {
        let error = parse_pull_response_body(
            r#"{
                "changes": [{
                    "serverRevision": 8,
                    "operationId": "11111111-1111-4111-8111-111111111111",
                    "entityType": "card_snapshot",
                    "changeType": "upsert",
                    "payload": {}
                }],
                "lastRevision": 7
            }"#,
        )
        .expect_err("lastRevision behind change stream should fail");

        assert_eq!(error.code, crate::errors::AppErrorCode::SyncPullFailed);
        assert!(!error.retryable);
        assert!(error
            .diagnostic_message
            .contains("behind max change revision"));
    }

    #[test]
    fn sync_failure_user_message_maps_vocabulary_store_errors_to_japanese() {
        let error = AppError::vocabulary_store_failed("vocabulary store open failed", true);
        let message = sync_failure_user_message(&error);
        assert_eq!(message, "ローカル語彙データの処理に失敗しました");
    }

    #[test]
    fn sync_lifecycle_serializes_in_camel_case() {
        let lifecycle = SyncLifecycle::Syncing;
        let serialized = serde_json::to_string(&lifecycle).expect("serialize lifecycle");
        assert_eq!(serialized, "\"syncing\"");
    }

    #[test]
    fn schedule_during_in_flight_requests_follow_up_cycle() {
        let runtime = SyncRuntime::default();

        assert!(mark_sync_started_or_request_rerun(&runtime));
        assert!(!mark_sync_started_or_request_rerun(&runtime));
        assert!(runtime.rerun_requested.load(Ordering::SeqCst));
    }

    #[test]
    fn auth_errors_do_not_mark_mutations_failed() {
        let auth_error = AppError::new(
            AppErrorCode::SyncAuthRequired,
            "Supabase session expired. Sign in again.",
            "refresh failed",
            false,
        );
        let push_error = AppError::new(
            AppErrorCode::SyncPushFailed,
            "Vocabulary sync is temporarily unavailable.",
            "rate limited",
            true,
        );

        assert!(!should_record_mutation_failure(&auth_error));
        assert!(should_record_mutation_failure(&push_error));
    }

    #[test]
    fn supabase_status_errors_classify_auth_and_rate_limit_without_failing_outbox() {
        let auth_error =
            supabase_push_status_error("mutation endpoint", reqwest::StatusCode::UNAUTHORIZED, "");
        assert_eq!(auth_error.code, AppErrorCode::SyncAuthRequired);
        assert!(!auth_error.retryable);

        let rate_limit_error = supabase_push_status_error(
            "mutation endpoint",
            reqwest::StatusCode::TOO_MANY_REQUESTS,
            "",
        );
        assert_eq!(rate_limit_error.code, AppErrorCode::SyncPushFailed);
        assert!(rate_limit_error.retryable);

        let validation_error = supabase_push_status_error(
            "mutation endpoint",
            reqwest::StatusCode::UNPROCESSABLE_ENTITY,
            "",
        );
        assert_eq!(validation_error.code, AppErrorCode::SyncPushFailed);
        assert!(!validation_error.retryable);
    }

    #[test]
    fn pull_status_errors_share_auth_and_retry_classification() {
        let auth_error =
            supabase_pull_status_error("pull endpoint", reqwest::StatusCode::FORBIDDEN, "");
        assert_eq!(auth_error.code, AppErrorCode::SyncAuthRequired);
        assert!(!auth_error.retryable);

        let retryable_error =
            supabase_pull_status_error("pull endpoint", reqwest::StatusCode::REQUEST_TIMEOUT, "");
        assert_eq!(retryable_error.code, AppErrorCode::SyncPullFailed);
        assert!(retryable_error.retryable);
    }

    #[test]
    fn bootstrap_reads_revision_before_table_snapshot() {
        let source = include_str!("vocabulary_bootstrap.rs");
        let revision_position = source
            .find("let base_revision")
            .expect("bootstrap should read a base revision");
        let lexeme_fetch_position = source
            .find("let lexemes = fetch_all_rows")
            .expect("bootstrap should fetch lexemes");

        assert!(revision_position < lexeme_fetch_position);
    }

    #[test]
    fn sync_rpc_migration_records_each_lexeme_form_by_form_id() {
        let migration =
            include_str!("../../supabase/migrations/202605310002_vocabulary_sync_rpcs.sql");

        assert!(migration.contains("v_form_id uuid"));
        assert!(migration.contains("pg_advisory_xact_lock"));
        assert!(migration.contains("returning id into v_form_id"));
        assert!(migration.contains("'lexemeFormId', v_form_id::text"));
        assert!(migration.contains("'lexeme_form'"));
        assert!(migration.contains("v_form_id"));
        assert!(!migration.contains("v_lexeme_id,\n        'upsert'"));
    }

    #[test]
    fn supabase_user_tables_enable_rls_and_admin_owner_policies() {
        let schema =
            include_str!("../../supabase/migrations/202605310001_initial_vocabulary_schema.sql");
        let user_tables = [
            "user_lexemes",
            "lexeme_forms",
            "card_snapshots",
            "lookup_events",
            "vocabulary_mutations",
            "vocabulary_changes",
        ];

        for table in user_tables {
            assert!(
                schema.contains(&format!("alter table public.{table} enable row level security;")),
                "{table} should enable RLS"
            );
            assert!(
                schema.contains(&format!("on public.{table}")),
                "{table} should declare an RLS policy"
            );
        }

        let owner_policy = "using (public.lexi_is_admin() and user_id = auth.uid())";
        let owner_check = "with check (public.lexi_is_admin() and user_id = auth.uid())";
        assert_eq!(schema.matches(owner_policy).count(), user_tables.len());
        assert_eq!(schema.matches(owner_check).count(), user_tables.len());
        assert!(!schema.contains("service_role"));
    }

    #[test]
    fn supabase_rpcs_are_security_invoker_and_admin_gated() {
        let apply_and_pull =
            include_str!("../../supabase/migrations/202605310005_apply_mutation_ensure_lexeme_forms.sql");
        let original_pull =
            include_str!("../../supabase/migrations/202605310002_vocabulary_sync_rpcs.sql");
        let lookup =
            include_str!("../../supabase/migrations/202605310003_lookup_vocabulary_card.sql");

        for (name, migration) in [
            ("apply mutation", apply_and_pull),
            ("pull changes", original_pull),
            ("lookup", lookup),
        ] {
            assert!(
                migration.contains("security invoker"),
                "{name} RPC should not bypass RLS"
            );
            assert!(
                !migration.contains("security definer"),
                "{name} RPC should not be security definer"
            );
            assert!(
                migration.contains("v_user_id uuid := auth.uid()"),
                "{name} RPC should bind to auth.uid()"
            );
            assert!(
                migration.contains("if v_user_id is null then"),
                "{name} RPC should reject anonymous calls"
            );
            assert!(
                migration.contains("if not public.lexi_is_admin() then"),
                "{name} RPC should require admin app_metadata"
            );
        }

        assert!(original_pull.contains(
            "grant execute on function public.apply_vocabulary_mutation(jsonb) to authenticated"
        ));
        assert!(original_pull.contains(
            "grant execute on function public.pull_vocabulary_changes(bigint, integer) to authenticated"
        ));
        assert!(lookup.contains(
            "grant execute on function public.lookup_vocabulary_card(text, text, text) to authenticated"
        ));
    }

    #[test]
    fn apply_mutation_rpc_checks_idempotency_before_writes() {
        let migration = include_str!(
            "../../supabase/migrations/202605310005_apply_mutation_ensure_lexeme_forms.sql"
        );
        let existing_lookup = migration
            .find("select vm.server_revision, vm.status")
            .expect("existing mutation lookup");
        let first_user_write = migration
            .find("insert into public.user_lexemes")
            .expect("first lexeme write");
        let duplicate_return = migration
            .find("return jsonb_build_object(")
            .expect("duplicate return");

        assert!(existing_lookup < duplicate_return);
        assert!(duplicate_return < first_user_write);
        assert!(migration.contains("where vm.user_id = v_user_id"));
        assert!(migration.contains("and vm.operation_id = v_operation_id"));
    }

    #[test]
    fn pull_rpc_limits_owner_scoped_revision_stream() {
        let migration =
            include_str!("../../supabase/migrations/202605310002_vocabulary_sync_rpcs.sql");

        assert!(migration.contains("where vc.user_id = v_user_id"));
        assert!(migration.contains("and vc.server_revision > coalesce(since_revision, 0)"));
        assert!(migration.contains("order by vc.server_revision asc"));
        assert!(migration.contains("limit batch_limit"));
        assert!(migration.contains("if batch_limit > 500 then"));
    }

    #[test]
    fn lookup_rpc_migration_declares_form_and_canonical_matchers() {
        let migration =
            include_str!("../../supabase/migrations/202605310003_lookup_vocabulary_card.sql");

        assert!(migration.contains("lookup_vocabulary_card"));
        assert!(migration.contains("lf.form_key = lookup_key"));
        assert!(migration.contains("ul.canonical_key = lookup_key"));
    }

    #[test]
    fn backfill_migration_repairs_canonical_and_irregular_lexeme_forms() {
        let migration =
            include_str!("../../supabase/migrations/202605310004_backfill_lexeme_forms.sql");

        assert!(migration.contains("'canonical'"));
        assert!(migration.contains("'irregular'"));
        assert!(migration.contains("jsonb_array_elements(cs.content->'inflections')"));
        assert!(
            migration.contains("on conflict (user_id, language, form_key, lexeme_id, relation)")
        );
    }

    #[test]
    fn apply_mutation_migration_ensures_canonical_and_content_inflections() {
        let migration = include_str!(
            "../../supabase/migrations/202605310005_apply_mutation_ensure_lexeme_forms.sql"
        );

        assert!(migration.contains("'canonical'"));
        assert!(migration.contains("'irregular'"));
        assert!(migration.contains("jsonb_array_elements(v_content->'inflections')"));
        assert!(migration.contains("v_form->>'form'"));
    }

    #[test]
    fn supabase_schema_guards_one_active_snapshot_per_language() {
        let schema =
            include_str!("../../supabase/migrations/202605310001_initial_vocabulary_schema.sql");

        assert!(schema.contains("idx_card_snapshots_one_active"));
        assert!(schema.contains("on public.card_snapshots (user_id, lexeme_id, result_language)"));
        assert!(schema.contains("where active"));
    }
}
