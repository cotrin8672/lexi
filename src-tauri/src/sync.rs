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
}

impl Default for SyncRuntime {
    fn default() -> Self {
        Self {
            state: Mutex::new(SyncRuntimeState::default()),
            in_flight: AtomicBool::new(false),
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

    if runtime
        .in_flight
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(());
    }

    struct InFlightGuard<'a> {
        runtime: &'a SyncRuntime,
    }

    impl Drop for InFlightGuard<'_> {
        fn drop(&mut self) {
            self.runtime.in_flight.store(false, Ordering::SeqCst);
        }
    }

    let _guard = InFlightGuard { runtime };

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
        sync_auth::refresh_session_if_needed(&supabase_url, &supabase_anon_key, session).await?;

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
                vocabulary::fail_mutation(
                    app,
                    &mutation.operation_id,
                    &error.diagnostic_message,
                    error.retryable,
                )?;
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
        return Err(AppError::new(
            AppErrorCode::SyncPushFailed,
            "Vocabulary sync failed.",
            supabase_response_diagnostic("mutation endpoint", status, &body),
            status.is_server_error(),
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
        return Err(AppError::new(
            AppErrorCode::SyncPullFailed,
            "Vocabulary sync failed.",
            supabase_response_diagnostic("pull endpoint", status, &body),
            status.is_server_error(),
        ));
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
        build_mutation_envelope, parse_mutation_ack_body, parse_pull_response_body,
        sync_failure_user_message, SyncLifecycle,
    };
    use crate::errors::AppError;
    use crate::vocabulary::PendingMutation;

    #[test]
    fn builds_mutation_envelope_for_rpc() {
        let mutation = PendingMutation {
            id: "local-row-1".to_string(),
            operation_id: "11111111-1111-4111-8111-111111111111".to_string(),
            mutation_type: "save_card_snapshot".to_string(),
            payload_json: serde_json::json!({
                "canonicalKey": "go",
                "canonicalText": "go",
                "resultLanguage": "ja",
                "schemaVersion": "lexi.word-study.v1",
                "content": { "headword": "go" }
            })
            .to_string(),
            attempts: 0,
        };
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
        assert!(migration.contains("on conflict (user_id, language, form_key, lexeme_id, relation)"));
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
