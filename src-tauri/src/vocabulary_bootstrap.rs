use crate::{
    errors::{AppError, AppErrorCode},
    vocabulary::{
        effective_user_id, open_store, repair_lexeme_forms_for_active_cards,
        set_last_server_revision,
    },
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use tauri::AppHandle;

const BOOTSTRAP_PAGE_SIZE: i64 = 500;
const SYNC_HTTP_TIMEOUT_SECS: u64 = 60;

const BOOTSTRAP_SCOPE_PREFIX: &str = "vocabulary_bootstrap";

#[derive(Debug, Deserialize)]
struct BootstrapLexemeRow {
    id: String,
    language: String,
    canonical_text: String,
    canonical_key: String,
    part_of_speech: Option<String>,
    favorite: Option<bool>,
    user_note: Option<String>,
    deleted_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BootstrapFormRow {
    id: String,
    lexeme_id: String,
    language: String,
    form_text: String,
    form_key: String,
    relation: String,
    source: String,
    confidence: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct BootstrapCardRow {
    id: String,
    lexeme_id: String,
    schema_version: String,
    provider: Option<String>,
    model: Option<String>,
    result_language: String,
    content: serde_json::Value,
    active: bool,
}

#[derive(Debug, Deserialize)]
struct MaxRevisionRow {
    server_revision: i64,
}

pub fn is_bootstrap_complete(app: &AppHandle) -> Result<bool, AppError> {
    let connection = open_store(app)?;
    Ok(read_bootstrap_flag(&connection)? >= 1)
}

pub async fn bootstrap_from_supabase(
    app: &AppHandle,
    supabase_url: &str,
    supabase_anon_key: &str,
    access_token: &str,
) -> Result<(), AppError> {
    if is_bootstrap_complete(app)? {
        return Ok(());
    }

    let client = build_http_client()?;
    let base_url = supabase_url.trim_end_matches('/');

    let lexemes = fetch_all_rows::<BootstrapLexemeRow>(
        &client,
        base_url,
        supabase_anon_key,
        access_token,
        "user_lexemes",
        "id,language,canonical_text,canonical_key,part_of_speech,favorite,user_note,deleted_at",
        &[("deleted_at", "is.null")],
    )
    .await?;

    let forms = fetch_all_rows::<BootstrapFormRow>(
        &client,
        base_url,
        supabase_anon_key,
        access_token,
        "lexeme_forms",
        "id,lexeme_id,language,form_text,form_key,relation,source,confidence",
        &[],
    )
    .await?;

    let cards = fetch_all_rows::<BootstrapCardRow>(
        &client,
        base_url,
        supabase_anon_key,
        access_token,
        "card_snapshots",
        "id,lexeme_id,schema_version,provider,model,result_language,content,active",
        &[("active", "eq.true")],
    )
    .await?;

    let max_revision = fetch_max_server_revision(&client, base_url, supabase_anon_key, access_token)
        .await
        .unwrap_or(0);

    let mut connection = open_store(app)?;
    let user_id = effective_user_id();
    let tx = connection
        .transaction()
        .map_err(|error| bootstrap_store_error(format!("bootstrap transaction failed: {error}")))?;

    for lexeme in &lexemes {
        upsert_bootstrap_lexeme(&tx, &user_id, lexeme)?;
    }

    for form in &forms {
        upsert_bootstrap_form(&tx, &user_id, form)?;
    }

    for card in &cards {
        if card.active {
            upsert_bootstrap_card(&tx, &user_id, card)?;
        }
    }

    repair_lexeme_forms_for_active_cards(&tx)?;

    mark_bootstrap_complete_in_tx(&tx)?;
    tx.commit()
        .map_err(|error| bootstrap_store_error(format!("bootstrap commit failed: {error}")))?;

    if max_revision > 0 {
        set_last_server_revision(app, max_revision)?;
    }

    Ok(())
}

fn bootstrap_scope_key() -> String {
    format!("{BOOTSTRAP_SCOPE_PREFIX}:{}", effective_user_id())
}

fn read_bootstrap_flag(connection: &Connection) -> Result<i64, AppError> {
    let scope = bootstrap_scope_key();
    Ok(connection
        .query_row(
            "select last_server_revision from sync_state where scope = ?1",
            params![scope],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| bootstrap_store_error(format!("bootstrap state read failed: {error}")))?
        .unwrap_or(0))
}

fn mark_bootstrap_complete_in_tx(connection: &Connection) -> Result<(), AppError> {
    let scope = bootstrap_scope_key();
    connection
        .execute(
            r#"
            insert into sync_state (scope, last_server_revision, updated_at)
            values (?1, 1, datetime('now'))
            on conflict(scope) do update set
              last_server_revision = 1,
              updated_at = excluded.updated_at
            "#,
            params![scope],
        )
        .map_err(|error| bootstrap_store_error(format!("bootstrap state write failed: {error}")))?;
    Ok(())
}

fn upsert_bootstrap_lexeme(
    connection: &Connection,
    user_id: &str,
    row: &BootstrapLexemeRow,
) -> Result<(), AppError> {
    connection
        .execute(
            r#"
            delete from lexeme_forms
            where lexeme_id in (
              select id from user_lexemes
              where user_id = ?1 and language = ?2 and canonical_key = ?3 and id != ?4
            )
            "#,
            params![user_id, row.language, row.canonical_key, row.id],
        )
        .map_err(|error| bootstrap_store_error(format!("bootstrap stale form cleanup failed: {error}")))?;

    connection
        .execute(
            r#"
            delete from card_snapshots
            where lexeme_id in (
              select id from user_lexemes
              where user_id = ?1 and language = ?2 and canonical_key = ?3 and id != ?4
            )
            "#,
            params![user_id, row.language, row.canonical_key, row.id],
        )
        .map_err(|error| bootstrap_store_error(format!("bootstrap stale card cleanup failed: {error}")))?;

    connection
        .execute(
            r#"
            delete from user_lexemes
            where user_id = ?1 and language = ?2 and canonical_key = ?3 and id != ?4
            "#,
            params![user_id, row.language, row.canonical_key, row.id],
        )
        .map_err(|error| bootstrap_store_error(format!("bootstrap stale lexeme cleanup failed: {error}")))?;

    let favorite = if row.favorite.unwrap_or(false) { 1 } else { 0 };
    connection
        .execute(
            r#"
            insert into user_lexemes (
              id, user_id, language, canonical_text, canonical_key,
              part_of_speech, favorite, user_note, deleted_at, updated_at
            ) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'))
            on conflict(id) do update set
              user_id = excluded.user_id,
              language = excluded.language,
              canonical_text = excluded.canonical_text,
              canonical_key = excluded.canonical_key,
              part_of_speech = excluded.part_of_speech,
              favorite = excluded.favorite,
              user_note = excluded.user_note,
              deleted_at = excluded.deleted_at,
              updated_at = excluded.updated_at
            "#,
            params![
                row.id,
                user_id,
                row.language,
                row.canonical_text,
                row.canonical_key,
                row.part_of_speech,
                favorite,
                row.user_note,
                row.deleted_at
            ],
        )
        .map_err(|error| bootstrap_store_error(format!("bootstrap lexeme upsert failed: {error}")))?;
    Ok(())
}

fn upsert_bootstrap_form(
    connection: &Connection,
    user_id: &str,
    row: &BootstrapFormRow,
) -> Result<(), AppError> {
    connection
        .execute(
            r#"
            insert into lexeme_forms (
              id, user_id, lexeme_id, language, form_text, form_key, relation, source, confidence
            ) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            on conflict(id) do update set
              user_id = excluded.user_id,
              lexeme_id = excluded.lexeme_id,
              language = excluded.language,
              form_text = excluded.form_text,
              form_key = excluded.form_key,
              relation = excluded.relation,
              source = excluded.source,
              confidence = excluded.confidence
            "#,
            params![
                row.id,
                user_id,
                row.lexeme_id,
                row.language,
                row.form_text,
                row.form_key,
                row.relation,
                row.source,
                row.confidence.unwrap_or(1.0)
            ],
        )
        .map_err(|error| bootstrap_store_error(format!("bootstrap form upsert failed: {error}")))?;
    Ok(())
}

fn upsert_bootstrap_card(
    connection: &Connection,
    user_id: &str,
    row: &BootstrapCardRow,
) -> Result<(), AppError> {
    let content_json = serde_json::to_string(&row.content).map_err(|error| {
        bootstrap_store_error(format!("bootstrap card serialize failed: {error}"))
    })?;

    connection
        .execute(
            "update card_snapshots set active = 0 where user_id = ?1 and lexeme_id = ?2 and result_language = ?3",
            params![user_id, row.lexeme_id, row.result_language],
        )
        .map_err(|error| bootstrap_store_error(format!("bootstrap card deactivate failed: {error}")))?;

    connection
        .execute(
            r#"
            insert into card_snapshots (
              id, user_id, lexeme_id, schema_version, provider, model,
              result_language, content_json, active
            ) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1)
            on conflict(id) do update set
              user_id = excluded.user_id,
              lexeme_id = excluded.lexeme_id,
              schema_version = excluded.schema_version,
              provider = excluded.provider,
              model = excluded.model,
              result_language = excluded.result_language,
              content_json = excluded.content_json,
              active = 1
            "#,
            params![
                row.id,
                user_id,
                row.lexeme_id,
                row.schema_version,
                row.provider,
                row.model,
                row.result_language,
                content_json
            ],
        )
        .map_err(|error| bootstrap_store_error(format!("bootstrap card upsert failed: {error}")))?;
    Ok(())
}

async fn fetch_max_server_revision(
    client: &reqwest::Client,
    base_url: &str,
    supabase_anon_key: &str,
    access_token: &str,
) -> Result<i64, AppError> {
    let endpoint = format!("{base_url}/rest/v1/vocabulary_changes");
    let response = client
        .get(endpoint)
        .header("apikey", supabase_anon_key)
        .header("Authorization", format!("Bearer {access_token}"))
        .query(&[
            ("select", "server_revision"),
            ("order", "server_revision.desc"),
            ("limit", "1"),
        ])
        .send()
        .await
        .map_err(|error| bootstrap_pull_error(format!("bootstrap max revision request failed: {error}")))?;

    let body = read_success_body(response, "vocabulary_changes max revision").await?;
    let rows = serde_json::from_str::<Vec<MaxRevisionRow>>(&body)
        .map_err(|error| bootstrap_store_error(format!("bootstrap max revision parse failed: {error}")))?;
    Ok(rows.first().map(|row| row.server_revision).unwrap_or(0))
}

async fn fetch_all_rows<T>(
    client: &reqwest::Client,
    base_url: &str,
    supabase_anon_key: &str,
    access_token: &str,
    table: &str,
    select: &str,
    filters: &[(&str, &str)],
) -> Result<Vec<T>, AppError>
where
    T: for<'de> Deserialize<'de>,
{
    let mut offset = 0_i64;
    let mut rows = Vec::new();

    loop {
        let endpoint = format!("{base_url}/rest/v1/{table}");
        let mut request = client
            .get(&endpoint)
            .header("apikey", supabase_anon_key)
            .header("Authorization", format!("Bearer {access_token}"))
            .query(&[
                ("select", select),
                ("order", "id.asc"),
                ("limit", &BOOTSTRAP_PAGE_SIZE.to_string()),
                ("offset", &offset.to_string()),
            ]);

        for (key, value) in filters {
            request = request.query(&[(key, value)]);
        }

        let response = request
            .send()
            .await
            .map_err(|error| bootstrap_pull_error(format!("bootstrap {table} request failed: {error}")))?;

        let body = read_success_body(response, table).await?;
        let page = serde_json::from_str::<Vec<T>>(&body).map_err(|error| {
            bootstrap_store_error(format!("bootstrap {table} parse failed: {error}"))
        })?;

        let count = page.len();
        rows.extend(page);
        if count < BOOTSTRAP_PAGE_SIZE as usize {
            break;
        }
        offset += BOOTSTRAP_PAGE_SIZE;
    }

    Ok(rows)
}

async fn read_success_body(
    response: reqwest::Response,
    endpoint_label: &str,
) -> Result<String, AppError> {
    let status = response.status();
    let body = response.text().await.map_err(|error| {
        bootstrap_pull_error(format!("bootstrap {endpoint_label} response read failed: {error}"))
    })?;

    if !status.is_success() {
        return Err(bootstrap_pull_error(format!(
            "bootstrap {endpoint_label} returned {status} with {} response bytes",
            body.len()
        )));
    }

    Ok(body)
}

fn build_http_client() -> Result<reqwest::Client, AppError> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(SYNC_HTTP_TIMEOUT_SECS))
        .build()
        .map_err(|error| bootstrap_pull_error(format!("bootstrap http client build failed: {error}")))
}

fn bootstrap_store_error(message: String) -> AppError {
    AppError::vocabulary_store_failed(message, true)
}

fn bootstrap_pull_error(message: String) -> AppError {
    AppError::new(
        AppErrorCode::SyncPullFailed,
        "Vocabulary sync is temporarily unavailable.",
        message,
        true,
    )
}
