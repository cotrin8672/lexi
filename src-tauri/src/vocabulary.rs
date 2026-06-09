use crate::{
    errors::AppError,
    schema::{
        Inflection, JapaneseWordCandidatesResultV1, LexiResultV1,
        LEXI_JP_WORD_CANDIDATES_V1_SCHEMA_VERSION, LEXI_RESULT_V1_SCHEMA_VERSION,
    },
    settings::ProviderKind,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use tauri::{AppHandle, Manager};

const LOCAL_USER_ID: &str = "local";
const SYNC_SCOPE: &str = "vocabulary";
const MAX_SYNC_ATTEMPTS: i64 = 8;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingMutation {
    pub id: String,
    pub operation_id: String,
    pub mutation_type: String,
    pub payload_json: String,
    pub attempts: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MutationForm {
    form_text: String,
    form_key: String,
    relation: String,
    source: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PulledChange {
    pub server_revision: i64,
    pub operation_id: String,
    pub entity_type: String,
    pub change_type: String,
    pub payload: serde_json::Value,
}

pub fn effective_user_id() -> String {
    crate::sync_auth::current_user_id().unwrap_or_else(|| LOCAL_USER_ID.to_string())
}

pub fn load_cached_word_study(
    app: &AppHandle,
    selected_text: &str,
    result_language: &str,
) -> Result<Option<LexiResultV1>, AppError> {
    let lookup_key = normalize_lookup_key(selected_text);
    if lookup_key.is_empty() {
        return Ok(None);
    }

    let connection = open_store(app)?;
    load_cached_word_study_from_connection(&connection, &lookup_key, result_language)
}

/// Resolve a word-study card from the local SQLite replica.
pub async fn load_word_study(
    app: &AppHandle,
    selected_text: &str,
    result_language: &str,
) -> Result<Option<LexiResultV1>, AppError> {
    load_cached_word_study(app, selected_text, result_language)
}

pub fn save_word_study_result(
    app: &AppHandle,
    result: &LexiResultV1,
    provider: ProviderKind,
    model: &str,
    selected_text: &str,
) -> Result<(), AppError> {
    let mut connection = open_store(app)?;
    save_word_study_result_to_connection(&mut connection, result, provider, model, selected_text)
}

/// Resolve a Japanese word-candidates card from the local SQLite replica.
pub async fn load_japanese_word_candidates(
    app: &AppHandle,
    selected_text: &str,
    result_language: &str,
) -> Result<Option<JapaneseWordCandidatesResultV1>, AppError> {
    let lookup_key = normalize_lookup_key(selected_text);
    if lookup_key.is_empty() {
        return Ok(None);
    }

    let connection = open_store(app)?;
    load_cached_japanese_word_candidates_from_connection(&connection, &lookup_key, result_language)
}

pub fn save_japanese_word_candidates_result(
    app: &AppHandle,
    result: &JapaneseWordCandidatesResultV1,
    provider: ProviderKind,
    model: &str,
    selected_text: &str,
) -> Result<(), AppError> {
    let mut connection = open_store(app)?;
    save_japanese_word_candidates_result_to_connection(
        &mut connection,
        result,
        provider,
        model,
        selected_text,
    )
}

pub fn open_store(app: &AppHandle) -> Result<Connection, AppError> {
    let path = store_path(app)?;
    ensure_parent(&path)?;
    let connection = Connection::open(path).map_err(|error| {
        AppError::vocabulary_store_failed(format!("vocabulary store open failed: {error}"), true)
    })?;
    initialize_schema(&connection)?;
    Ok(connection)
}

fn store_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join("lexi-vocabulary.sqlite3"))
        .map_err(|error| {
            AppError::vocabulary_store_failed(format!("app data dir unavailable: {error}"), true)
        })
}

fn ensure_parent(path: &PathBuf) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("vocabulary store dir create failed: {error}"),
                true,
            )
        })?;
    }
    Ok(())
}

fn initialize_schema(connection: &Connection) -> Result<(), AppError> {
    connection
        .execute_batch(
            r#"
            pragma foreign_keys = on;

            create table if not exists schema_migrations (
              version integer primary key,
              applied_at text not null default (datetime('now'))
            );

            create table if not exists dictionary_entries (
              id text primary key default (lower(hex(randomblob(16)))),
              source_key text not null,
              language text not null,
              headword text not null,
              normalized_key text not null,
              reading text,
              part_of_speech text,
              definitions_json text not null default '[]',
              metadata_json text not null default '{}',
              created_at text not null default (datetime('now')),
              updated_at text not null default (datetime('now')),
              unique (source_key, language, normalized_key)
            );

            create table if not exists user_lexemes (
              id text primary key default (lower(hex(randomblob(16)))),
              user_id text not null default 'local',
              language text not null,
              canonical_text text not null,
              canonical_key text not null,
              part_of_speech text,
              dictionary_entry_id text references dictionary_entries(id) on delete set null,
              favorite integer not null default 0,
              user_note text,
              created_at text not null default (datetime('now')),
              updated_at text not null default (datetime('now')),
              deleted_at text,
              unique (user_id, language, canonical_key)
            );

            create table if not exists lexeme_forms (
              id text primary key default (lower(hex(randomblob(16)))),
              user_id text not null default 'local',
              lexeme_id text not null references user_lexemes(id) on delete cascade,
              language text not null,
              form_text text not null,
              form_key text not null,
              relation text not null,
              source text not null,
              confidence real,
              created_at text not null default (datetime('now')),
              unique (user_id, language, form_key, lexeme_id, relation)
            );

            create table if not exists card_snapshots (
              id text primary key default (lower(hex(randomblob(16)))),
              user_id text not null default 'local',
              lexeme_id text not null references user_lexemes(id) on delete cascade,
              schema_version text not null,
              provider text,
              model text,
              result_language text not null,
              content_json text not null,
              active integer not null default 1,
              remote_operation_id text,
              remote_server_revision integer,
              created_at text not null default (datetime('now'))
            );

            create table if not exists lookup_events (
              id text primary key default (lower(hex(randomblob(16)))),
              user_id text not null default 'local',
              operation_id text not null,
              lexeme_id text references user_lexemes(id) on delete set null,
              language text not null,
              lookup_key text not null,
              result_mode text not null,
              capture_method text,
              created_at text not null default (datetime('now')),
              unique (user_id, operation_id)
            );

            create table if not exists mutation_outbox (
              id text primary key default (lower(hex(randomblob(16)))),
              user_id text not null default 'local',
              operation_id text not null,
              mutation_type text not null,
              payload_json text not null,
              status text not null default 'pending',
              attempts integer not null default 0,
              last_error text,
              server_revision integer,
              created_at text not null default (datetime('now')),
              updated_at text not null default (datetime('now')),
              unique (user_id, operation_id)
            );

            create table if not exists sync_state (
              scope text primary key,
              last_server_revision integer not null default 0,
              updated_at text not null default (datetime('now'))
            );

            create index if not exists idx_user_lexemes_lookup
              on user_lexemes (user_id, language, canonical_key)
              where deleted_at is null;

            create index if not exists idx_lexeme_forms_lookup
              on lexeme_forms (user_id, language, form_key);

            create index if not exists idx_card_snapshots_active
              on card_snapshots (user_id, lexeme_id, created_at desc)
              where active = 1;

            create index if not exists idx_mutation_outbox_pending
              on mutation_outbox (status, created_at);

            insert or ignore into schema_migrations (version) values (1);
            insert or ignore into sync_state (scope) values ('vocabulary');
            "#,
        )
        .map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("vocabulary store schema init failed: {error}"),
                false,
            )
        })?;
    ensure_column(connection, "card_snapshots", "remote_operation_id", "text")?;
    ensure_column(
        connection,
        "card_snapshots",
        "remote_server_revision",
        "integer",
    )?;
    connection
        .execute(
            r#"
            create unique index if not exists idx_card_snapshots_remote_change
              on card_snapshots (user_id, remote_operation_id, remote_server_revision)
              where remote_operation_id is not null
            "#,
            [],
        )
        .map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("vocabulary remote change index failed: {error}"),
                false,
            )
        })?;
    Ok(())
}

fn ensure_column(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), AppError> {
    let mut statement = connection
        .prepare(&format!("pragma table_info({table})"))
        .map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("vocabulary schema inspect failed for {table}: {error}"),
                false,
            )
        })?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("vocabulary schema inspect query failed for {table}: {error}"),
                false,
            )
        })?;
    let columns = rows.collect::<Result<Vec<_>, _>>().map_err(|error| {
        AppError::vocabulary_store_failed(
            format!("vocabulary schema inspect row failed for {table}: {error}"),
            false,
        )
    })?;
    if columns.iter().any(|existing| existing == column) {
        return Ok(());
    }
    connection
        .execute(
            &format!("alter table {table} add column {column} {definition}"),
            [],
        )
        .map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("vocabulary schema alter failed for {table}.{column}: {error}"),
                false,
            )
        })?;
    Ok(())
}

const FORM_RELATION_PRIORITY: &[&str] = &["canonical", "observed", "irregular", "regular"];

#[derive(Debug, Clone)]
struct LexemeFormMatch {
    lexeme_id: String,
    relation: String,
    form_text: String,
    content_json: Option<String>,
}

fn form_relation_rank(relation: &str) -> Option<usize> {
    FORM_RELATION_PRIORITY
        .iter()
        .position(|candidate| *candidate == relation)
}

fn resolve_unique_lexeme_for_form_key(
    lookup_key: &str,
    matches: &[LexemeFormMatch],
) -> Option<String> {
    let mut best_by_lexeme: std::collections::HashMap<String, (usize, Option<String>)> =
        std::collections::HashMap::new();

    for form_match in matches {
        let Some(rank) = form_relation_rank(&form_match.relation) else {
            continue;
        };
        if form_match.relation == "canonical"
            && normalize_lookup_key(&form_match.form_text) != lookup_key
        {
            continue;
        }
        best_by_lexeme
            .entry(form_match.lexeme_id.clone())
            .and_modify(|(best_rank, content)| {
                if rank < *best_rank {
                    *best_rank = rank;
                    *content = form_match.content_json.clone();
                }
            })
            .or_insert((rank, form_match.content_json.clone()));
    }

    if best_by_lexeme.is_empty() {
        return None;
    }

    let min_rank = best_by_lexeme
        .values()
        .map(|(rank, _)| *rank)
        .min()
        .expect("non-empty lexeme matches");
    let winners: Vec<_> = best_by_lexeme
        .into_iter()
        .filter(|(_, (rank, _))| *rank == min_rank)
        .collect();
    if winners.len() != 1 {
        return None;
    }

    Some(winners[0].0.clone())
}

fn query_lexeme_form_matches(
    connection: &Connection,
    lookup_key: &str,
    lexeme_language: &str,
    result_language: Option<&str>,
    user_id: &str,
) -> Result<Vec<LexemeFormMatch>, AppError> {
    if let Some(result_language) = result_language {
        let mut statement = connection
            .prepare(
                r#"
                select lf.lexeme_id, lf.relation, lf.form_text, cs.content_json
                from lexeme_forms lf
                join user_lexemes ul on ul.id = lf.lexeme_id
                join card_snapshots cs on cs.lexeme_id = lf.lexeme_id
                where lf.user_id = ?1
                  and lf.language = ?4
                  and lf.form_key = ?2
                  and cs.result_language = ?3
                  and cs.active = 1
                  and ul.deleted_at is null
                  and cs.created_at = (
                    select max(cs2.created_at)
                    from card_snapshots cs2
                    where cs2.lexeme_id = lf.lexeme_id
                      and cs2.user_id = ?1
                      and cs2.result_language = ?3
                      and cs2.active = 1
                  )

                union all

                select ul.id, 'canonical', ul.canonical_text, cs.content_json
                from user_lexemes ul
                join card_snapshots cs on cs.lexeme_id = ul.id
                where ul.user_id = ?1
                  and ul.language = ?4
                  and ul.canonical_key = ?2
                  and cs.result_language = ?3
                  and cs.active = 1
                  and ul.deleted_at is null
                  and cs.created_at = (
                    select max(cs2.created_at)
                    from card_snapshots cs2
                    where cs2.lexeme_id = ul.id
                      and cs2.user_id = ?1
                      and cs2.result_language = ?3
                      and cs2.active = 1
                  )
                "#,
            )
            .map_err(|error| {
                AppError::vocabulary_store_failed(
                    format!("vocabulary form match prepare failed: {error}"),
                    true,
                )
            })?;

        let rows = statement
            .query_map(
                params![user_id, lookup_key, result_language, lexeme_language],
                |row| {
                    Ok(LexemeFormMatch {
                        lexeme_id: row.get(0)?,
                        relation: row.get(1)?,
                        form_text: row.get(2)?,
                        content_json: Some(row.get(3)?),
                    })
                },
            )
            .map_err(|error| {
                AppError::vocabulary_store_failed(
                    format!("vocabulary form match query failed: {error}"),
                    true,
                )
            })?;

        return rows.collect::<Result<Vec<_>, _>>().map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("vocabulary form match row failed: {error}"),
                true,
            )
        });
    }

    let mut statement = connection
        .prepare(
            r#"
            select lf.lexeme_id, lf.relation, lf.form_text
            from lexeme_forms lf
            join user_lexemes ul on ul.id = lf.lexeme_id
            where lf.user_id = ?1
              and lf.language = ?3
              and lf.form_key = ?2
              and ul.deleted_at is null

            union all

            select ul.id, 'canonical', ul.canonical_text
            from user_lexemes ul
            where ul.user_id = ?1
              and ul.language = ?3
              and ul.canonical_key = ?2
              and ul.deleted_at is null
            "#,
        )
        .map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("vocabulary form match prepare failed: {error}"),
                true,
            )
        })?;

    let rows = statement
        .query_map(params![user_id, lookup_key, lexeme_language], |row| {
            Ok(LexemeFormMatch {
                lexeme_id: row.get(0)?,
                relation: row.get(1)?,
                form_text: row.get(2)?,
                content_json: None,
            })
        })
        .map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("vocabulary form match query failed: {error}"),
                true,
            )
        })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|error| {
        AppError::vocabulary_store_failed(
            format!("vocabulary form match row failed: {error}"),
            true,
        )
    })
}

fn resolve_lexeme_for_save(
    connection: &Connection,
    selected_text: &str,
    proposed_headword: &str,
    user_id: &str,
) -> Result<Option<(String, String)>, AppError> {
    let lookup_key = normalize_lookup_key(selected_text);
    let proposed_key = normalize_lookup_key(proposed_headword);
    if lookup_key.is_empty() || proposed_key.is_empty() {
        return Ok(None);
    }

    let matches = query_lexeme_form_matches(connection, &lookup_key, "en", None, user_id)?;
    let Some(lexeme_id) = resolve_unique_lexeme_for_form_key(&lookup_key, &matches) else {
        return Ok(None);
    };

    let Some((canonical_text, canonical_key)) = connection
        .query_row(
            r#"
            select canonical_text, canonical_key
            from user_lexemes
            where id = ?1 and user_id = ?2 and language = 'en' and deleted_at is null
            "#,
            params![lexeme_id, user_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("vocabulary save redirect lookup failed: {error}"),
                true,
            )
        })?
    else {
        return Ok(None);
    };

    if proposed_key == canonical_key {
        return Ok(None);
    }

    if proposed_key != lookup_key {
        return Ok(None);
    }

    // Same surface as selection (e.g. playing/playing) but a different canonical lexeme
    // already owns this form as a regular or observed alias — redirect to that lexeme.
    let has_soft_alias = matches.iter().any(|form_match| {
        form_match.lexeme_id == lexeme_id
            && matches!(form_match.relation.as_str(), "observed" | "regular")
    });
    if has_soft_alias {
        return Ok(Some((canonical_text, canonical_key)));
    }

    // Irregular-only link (e.g. saw on see) while saving a standalone homograph card.
    Ok(None)
}

fn align_result_to_canonical(result: &LexiResultV1, canonical_text: &str) -> LexiResultV1 {
    let mut aligned = result.clone();
    aligned.headword = canonical_text.to_string();
    aligned
}

fn load_cached_word_study_from_connection(
    connection: &Connection,
    lookup_key: &str,
    result_language: &str,
) -> Result<Option<LexiResultV1>, AppError> {
    if let Some(result) = load_cached_word_study_from_connection_with_language(
        connection,
        lookup_key,
        result_language,
    )? {
        return Ok(Some(result));
    }

    // Cards saved before the result_language cache-key fix were stored as "en"
    // while settings used "ja".
    if result_language == "ja" {
        return load_cached_word_study_from_connection_with_language(connection, lookup_key, "en");
    }

    Ok(None)
}

fn load_cached_word_study_from_connection_with_language(
    connection: &Connection,
    lookup_key: &str,
    result_language: &str,
) -> Result<Option<LexiResultV1>, AppError> {
    let user_id = effective_user_id();
    let matches = query_lexeme_form_matches(
        connection,
        lookup_key,
        "en",
        Some(result_language),
        &user_id,
    )?;
    word_study_from_form_matches(lookup_key, &matches)
}

fn word_study_from_form_matches(
    lookup_key: &str,
    matches: &[LexemeFormMatch],
) -> Result<Option<LexiResultV1>, AppError> {
    let Some(lexeme_id) = resolve_unique_lexeme_for_form_key(lookup_key, matches) else {
        return Ok(None);
    };

    let content_json = matches
        .iter()
        .find(|form_match| form_match.lexeme_id == lexeme_id)
        .and_then(|form_match| form_match.content_json.as_ref())
        .ok_or_else(|| {
            AppError::vocabulary_store_failed(
                "resolved vocabulary match missing card content",
                false,
            )
        })?;

    let parsed = serde_json::from_str::<LexiResultV1>(content_json).map_err(|error| {
        AppError::invalid_model_output(format!("cached card parse failed: {error}"))
    })?;
    Ok(Some(parsed.validate()?))
}

fn load_cached_japanese_word_candidates_from_connection(
    connection: &Connection,
    lookup_key: &str,
    result_language: &str,
) -> Result<Option<JapaneseWordCandidatesResultV1>, AppError> {
    let user_id = effective_user_id();
    let matches = query_lexeme_form_matches(
        connection,
        lookup_key,
        "ja",
        Some(result_language),
        &user_id,
    )?;
    japanese_word_candidates_from_form_matches(lookup_key, &matches)
}

fn japanese_word_candidates_from_form_matches(
    lookup_key: &str,
    matches: &[LexemeFormMatch],
) -> Result<Option<JapaneseWordCandidatesResultV1>, AppError> {
    let Some(lexeme_id) = resolve_unique_lexeme_for_form_key(lookup_key, matches) else {
        return Ok(None);
    };

    let content_json = matches
        .iter()
        .find(|form_match| form_match.lexeme_id == lexeme_id)
        .and_then(|form_match| form_match.content_json.as_ref())
        .ok_or_else(|| {
            AppError::vocabulary_store_failed(
                "resolved vocabulary match missing card content",
                false,
            )
        })?;

    let parsed =
        serde_json::from_str::<JapaneseWordCandidatesResultV1>(content_json).map_err(|error| {
            AppError::invalid_model_output(format!("cached ja2en card parse failed: {error}"))
        })?;
    Ok(Some(parsed.validate()?))
}

fn save_word_study_result_to_connection(
    connection: &mut Connection,
    result: &LexiResultV1,
    provider: ProviderKind,
    model: &str,
    selected_text: &str,
) -> Result<(), AppError> {
    let user_id = effective_user_id();
    let (canonical_text, canonical_key) = if let Some((canonical_text, canonical_key)) =
        resolve_lexeme_for_save(connection, selected_text, &result.headword, &user_id)?
    {
        (canonical_text, canonical_key)
    } else {
        let canonical_key = normalize_lookup_key(&result.headword);
        if canonical_key.is_empty() {
            return Ok(());
        }
        (result.headword.clone(), canonical_key)
    };

    let result = align_result_to_canonical(result, &canonical_text);

    let content_json = serde_json::to_string(&result).map_err(|error| {
        AppError::vocabulary_store_failed(
            format!("vocabulary card serialize failed: {error}"),
            false,
        )
    })?;
    let provider_json = serde_json::to_string(&provider).map_err(|error| {
        AppError::vocabulary_store_failed(format!("provider serialize failed: {error}"), false)
    })?;
    let operation_id = new_operation_id();

    let tx = connection.transaction().map_err(|error| {
        AppError::vocabulary_store_failed(format!("vocabulary transaction failed: {error}"), true)
    })?;

    tx.execute(
        r#"
        insert or ignore into user_lexemes (
          user_id, language, canonical_text, canonical_key, part_of_speech
        ) values (?1, 'en', ?2, ?3, ?4)
        "#,
        params![
            user_id,
            canonical_text,
            canonical_key,
            result
                .translations
                .first()
                .and_then(|translation| translation.note.as_deref())
        ],
    )
    .map_err(|error| {
        AppError::vocabulary_store_failed(format!("vocabulary lexeme insert failed: {error}"), true)
    })?;

    tx.execute(
        r#"
        update user_lexemes
        set canonical_text = ?3,
            part_of_speech = coalesce(?4, part_of_speech),
            updated_at = datetime('now'),
            deleted_at = null
        where user_id = ?1 and language = 'en' and canonical_key = ?2
        "#,
        params![
            user_id,
            canonical_key,
            canonical_text,
            result
                .translations
                .first()
                .and_then(|translation| translation.note.as_deref())
        ],
    )
    .map_err(|error| {
        AppError::vocabulary_store_failed(format!("vocabulary lexeme update failed: {error}"), true)
    })?;

    let lexeme_id = tx
        .query_row(
            "select id from user_lexemes where user_id = ?1 and language = 'en' and canonical_key = ?2",
            params![user_id, canonical_key],
            |row| row.get::<_, String>(0),
        )
        .map_err(|error| {
            AppError::vocabulary_store_failed(format!("vocabulary lexeme lookup failed: {error}"), true)
        })?;

    ensure_lexeme_forms(
        &tx,
        &user_id,
        &lexeme_id,
        &canonical_text,
        &canonical_key,
        &result,
        EnsureLexemeFormsOptions {
            observed_text: Some(selected_text),
        },
    )?;

    tx.execute(
        "update card_snapshots set active = 0 where user_id = ?1 and lexeme_id = ?2 and result_language = ?3",
        params![user_id, lexeme_id, result.result_language],
    )
    .map_err(|error| {
        AppError::vocabulary_store_failed(format!("vocabulary snapshot deactivate failed: {error}"), true)
    })?;

    tx.execute(
        r#"
        insert into card_snapshots (
          user_id, lexeme_id, schema_version, provider, model, result_language, content_json, active
        ) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)
        "#,
        params![
            user_id,
            lexeme_id,
            result.schema_version,
            provider_json.trim_matches('"'),
            model,
            result.result_language,
            content_json
        ],
    )
    .map_err(|error| {
        AppError::vocabulary_store_failed(
            format!("vocabulary snapshot insert failed: {error}"),
            true,
        )
    })?;

    let forms: Vec<MutationForm> = collect_forms_for_lexeme(&tx, &user_id, &lexeme_id)?
        .into_iter()
        .filter(|form| form.source != "capture")
        .collect();
    let mutation_payload = serde_json::json!({
        "schemaVersion": result.schema_version,
        "language": result.source_language,
        "resultLanguage": result.result_language,
        "canonicalText": canonical_text,
        "canonicalKey": canonical_key,
        "provider": provider,
        "model": model,
        "content": &result,
        "forms": forms,
    });
    let mutation_payload_json = serde_json::to_string(&mutation_payload).map_err(|error| {
        AppError::vocabulary_store_failed(
            format!("vocabulary mutation serialize failed: {error}"),
            false,
        )
    })?;

    tx.execute(
        r#"
        insert or ignore into mutation_outbox (
          user_id, operation_id, mutation_type, payload_json
        ) values (?1, ?2, 'save_card_snapshot', ?3)
        "#,
        params![user_id, operation_id, mutation_payload_json],
    )
    .map_err(|error| {
        AppError::vocabulary_store_failed(
            format!("vocabulary mutation enqueue failed: {error}"),
            true,
        )
    })?;

    tx.commit().map_err(|error| {
        AppError::vocabulary_store_failed(
            format!("vocabulary transaction commit failed: {error}"),
            true,
        )
    })
}

fn save_japanese_word_candidates_result_to_connection(
    connection: &mut Connection,
    result: &JapaneseWordCandidatesResultV1,
    provider: ProviderKind,
    model: &str,
    selected_text: &str,
) -> Result<(), AppError> {
    let user_id = effective_user_id();
    let canonical_key = normalize_lookup_key(&result.query);
    if canonical_key.is_empty() {
        return Ok(());
    }
    let canonical_text = result.query.clone();

    let content_json = serde_json::to_string(result).map_err(|error| {
        AppError::vocabulary_store_failed(
            format!("vocabulary card serialize failed: {error}"),
            false,
        )
    })?;
    let provider_json = serde_json::to_string(&provider).map_err(|error| {
        AppError::vocabulary_store_failed(format!("provider serialize failed: {error}"), false)
    })?;
    let operation_id = new_operation_id();

    let tx = connection.transaction().map_err(|error| {
        AppError::vocabulary_store_failed(format!("vocabulary transaction failed: {error}"), true)
    })?;

    tx.execute(
        r#"
        insert or ignore into user_lexemes (
          user_id, language, canonical_text, canonical_key, part_of_speech
        ) values (?1, 'ja', ?2, ?3, null)
        "#,
        params![user_id, canonical_text, canonical_key],
    )
    .map_err(|error| {
        AppError::vocabulary_store_failed(format!("vocabulary lexeme insert failed: {error}"), true)
    })?;

    tx.execute(
        r#"
        update user_lexemes
        set canonical_text = ?3,
            updated_at = datetime('now'),
            deleted_at = null
        where user_id = ?1 and language = 'ja' and canonical_key = ?2
        "#,
        params![user_id, canonical_key, canonical_text],
    )
    .map_err(|error| {
        AppError::vocabulary_store_failed(format!("vocabulary lexeme update failed: {error}"), true)
    })?;

    let lexeme_id = tx
        .query_row(
            "select id from user_lexemes where user_id = ?1 and language = 'ja' and canonical_key = ?2",
            params![user_id, canonical_key],
            |row| row.get::<_, String>(0),
        )
        .map_err(|error| {
            AppError::vocabulary_store_failed(format!("vocabulary lexeme lookup failed: {error}"), true)
        })?;

    ensure_japanese_lexeme_forms(
        &tx,
        &user_id,
        &lexeme_id,
        &canonical_text,
        &canonical_key,
        selected_text,
    )?;

    tx.execute(
        "update card_snapshots set active = 0 where user_id = ?1 and lexeme_id = ?2 and result_language = ?3",
        params![user_id, lexeme_id, result.result_language],
    )
    .map_err(|error| {
        AppError::vocabulary_store_failed(format!("vocabulary snapshot deactivate failed: {error}"), true)
    })?;

    tx.execute(
        r#"
        insert into card_snapshots (
          user_id, lexeme_id, schema_version, provider, model, result_language, content_json, active
        ) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)
        "#,
        params![
            user_id,
            lexeme_id,
            result.schema_version,
            provider_json.trim_matches('"'),
            model,
            result.result_language,
            content_json
        ],
    )
    .map_err(|error| {
        AppError::vocabulary_store_failed(
            format!("vocabulary snapshot insert failed: {error}"),
            true,
        )
    })?;

    let forms: Vec<MutationForm> = collect_forms_for_lexeme(&tx, &user_id, &lexeme_id)?
        .into_iter()
        .filter(|form| form.source != "capture")
        .collect();
    let mutation_payload = serde_json::json!({
        "schemaVersion": result.schema_version,
        "language": "ja",
        "resultLanguage": result.result_language,
        "canonicalText": canonical_text,
        "canonicalKey": canonical_key,
        "provider": provider,
        "model": model,
        "content": &result,
        "forms": forms,
    });
    let mutation_payload_json = serde_json::to_string(&mutation_payload).map_err(|error| {
        AppError::vocabulary_store_failed(
            format!("vocabulary mutation serialize failed: {error}"),
            false,
        )
    })?;

    tx.execute(
        r#"
        insert or ignore into mutation_outbox (
          user_id, operation_id, mutation_type, payload_json
        ) values (?1, ?2, 'save_card_snapshot', ?3)
        "#,
        params![user_id, operation_id, mutation_payload_json],
    )
    .map_err(|error| {
        AppError::vocabulary_store_failed(
            format!("vocabulary mutation enqueue failed: {error}"),
            true,
        )
    })?;

    tx.commit().map_err(|error| {
        AppError::vocabulary_store_failed(
            format!("vocabulary transaction commit failed: {error}"),
            true,
        )
    })
}

fn ensure_japanese_lexeme_forms(
    connection: &Connection,
    user_id: &str,
    lexeme_id: &str,
    canonical_text: &str,
    canonical_key: &str,
    observed_text: &str,
) -> Result<(), AppError> {
    insert_form(
        connection,
        user_id,
        lexeme_id,
        canonical_text,
        "canonical",
        "provider",
    )?;

    let observed_key = normalize_lookup_key(observed_text);
    if !observed_key.is_empty() && observed_key != canonical_key {
        insert_form(
            connection,
            user_id,
            lexeme_id,
            observed_text,
            "observed",
            "capture",
        )?;
    }

    Ok(())
}

fn insert_inflection_form(
    connection: &Connection,
    user_id: &str,
    lexeme_id: &str,
    inflection: &Inflection,
) -> Result<(), AppError> {
    let relation = match inflection.kind.trim() {
        "" => "irregular",
        "past" | "pastParticiple" | "plural" => "irregular",
        _ => "irregular",
    };
    insert_form(
        connection,
        user_id,
        lexeme_id,
        &inflection.form,
        relation,
        "provider",
    )
}

fn is_verb_lexeme(result: &LexiResultV1) -> bool {
    result
        .translations
        .first()
        .and_then(|translation| translation.note.as_deref())
        == Some("動詞")
}

fn regular_verb_surface_forms(headword: &str) -> Vec<String> {
    let base = headword.trim();
    if base.is_empty() || base.split_whitespace().count() != 1 {
        return Vec::new();
    }

    let lower = base.to_lowercase();
    let third_person = if lower.ends_with('y')
        && base.len() > 1
        && !lower.ends_with("ay")
        && !lower.ends_with("ey")
        && !lower.ends_with("oy")
        && !lower.ends_with("uy")
    {
        format!("{}ies", &base[..base.len() - 1])
    } else if lower.ends_with("s")
        || lower.ends_with("sh")
        || lower.ends_with("ch")
        || lower.ends_with('x')
        || lower.ends_with('z')
    {
        format!("{base}es")
    } else {
        format!("{base}s")
    };

    if lower.ends_with('e') && base.len() > 1 {
        return vec![third_person, format!("{base}d"), format!("{base}ing")];
    }

    vec![third_person, format!("{base}ed"), format!("{base}ing")]
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct EnsureLexemeFormsOptions<'a> {
    pub observed_text: Option<&'a str>,
}

/// Ensures lookup aliases exist for a lexeme: canonical, optional observed capture,
/// irregular forms from card inflections, and generated regular verb surfaces.
pub(crate) fn ensure_lexeme_forms(
    connection: &Connection,
    user_id: &str,
    lexeme_id: &str,
    canonical_text: &str,
    canonical_key: &str,
    result: &LexiResultV1,
    options: EnsureLexemeFormsOptions<'_>,
) -> Result<(), AppError> {
    insert_form(
        connection,
        user_id,
        lexeme_id,
        canonical_text,
        "canonical",
        "provider",
    )?;

    if let Some(observed_text) = options.observed_text {
        let observed_key = normalize_lookup_key(observed_text);
        let observed_matches_inflection = result
            .inflections
            .iter()
            .any(|inflection| normalize_lookup_key(&inflection.form) == observed_key);
        if !observed_key.is_empty() && observed_key != canonical_key && !observed_matches_inflection
        {
            insert_form(
                connection,
                user_id,
                lexeme_id,
                observed_text,
                "observed",
                "capture",
            )?;
        }
    }

    for inflection in &result.inflections {
        insert_inflection_form(connection, user_id, lexeme_id, inflection)?;
    }

    if is_verb_lexeme(result) {
        for form_text in regular_verb_surface_forms(canonical_text) {
            insert_form(
                connection,
                user_id,
                lexeme_id,
                &form_text,
                "regular",
                "generated",
            )?;
        }
    }

    Ok(())
}

pub(crate) fn ensure_lexeme_forms_from_content_json(
    connection: &Connection,
    user_id: &str,
    lexeme_id: &str,
    canonical_text: &str,
    canonical_key: &str,
    content_json: &str,
    options: EnsureLexemeFormsOptions<'_>,
) -> Result<(), AppError> {
    let result = serde_json::from_str::<LexiResultV1>(content_json).map_err(|error| {
        AppError::vocabulary_store_failed(format!("card content parse failed: {error}"), false)
    })?;
    ensure_lexeme_forms(
        connection,
        user_id,
        lexeme_id,
        canonical_text,
        canonical_key,
        &result,
        options,
    )
}

/// Backfills missing `lexeme_forms` rows for every active card in the local replica.
pub(crate) fn repair_lexeme_forms_for_active_cards(
    connection: &Connection,
) -> Result<(), AppError> {
    let mut statement = connection
        .prepare(
            r#"
            select distinct cs.user_id, cs.lexeme_id, ul.canonical_text, ul.canonical_key, cs.content_json
            from card_snapshots cs
            inner join user_lexemes ul on ul.id = cs.lexeme_id
            where cs.active = 1
              and ul.deleted_at is null
            "#,
        )
        .map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("lexeme form repair prepare failed: {error}"),
                true,
            )
        })?;

    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("lexeme form repair query failed: {error}"),
                true,
            )
        })?;

    for row in rows {
        let (user_id, lexeme_id, canonical_text, canonical_key, content_json) =
            row.map_err(|error| {
                AppError::vocabulary_store_failed(
                    format!("lexeme form repair row failed: {error}"),
                    true,
                )
            })?;
        if content_json.contains(LEXI_JP_WORD_CANDIDATES_V1_SCHEMA_VERSION) {
            ensure_japanese_lexeme_forms(
                connection,
                &user_id,
                &lexeme_id,
                &canonical_text,
                &canonical_key,
                &canonical_text,
            )?;
        } else {
            ensure_lexeme_forms_from_content_json(
                connection,
                &user_id,
                &lexeme_id,
                &canonical_text,
                &canonical_key,
                &content_json,
                EnsureLexemeFormsOptions::default(),
            )?;
        }
    }

    Ok(())
}

fn insert_form(
    connection: &Connection,
    user_id: &str,
    lexeme_id: &str,
    form_text: &str,
    relation: &str,
    source: &str,
) -> Result<(), AppError> {
    let form_key = normalize_lookup_key(form_text);
    if form_key.is_empty() {
        return Ok(());
    }

    let lexeme_language = connection
        .query_row(
            "select language from user_lexemes where id = ?1",
            params![lexeme_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("vocabulary lexeme language lookup failed: {error}"),
                true,
            )
        })?;

    connection
        .execute(
            r#"
            insert or ignore into lexeme_forms (
              user_id, lexeme_id, language, form_text, form_key, relation, source, confidence
            ) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1.0)
            "#,
            params![
                user_id,
                lexeme_id,
                lexeme_language,
                form_text,
                form_key,
                relation,
                source
            ],
        )
        .map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("vocabulary form insert failed: {error}"),
                true,
            )
        })?;
    Ok(())
}

fn normalize_lookup_key(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_lowercase()
}

fn new_operation_id() -> String {
    use rand::RngCore;
    let mut bytes = [0_u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

fn collect_forms_for_lexeme(
    connection: &Connection,
    user_id: &str,
    lexeme_id: &str,
) -> Result<Vec<MutationForm>, AppError> {
    let mut statement = connection
        .prepare(
            r#"
            select form_text, form_key, relation, source
            from lexeme_forms
            where user_id = ?1 and lexeme_id = ?2
            order by relation, form_key
            "#,
        )
        .map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("vocabulary form collect prepare failed: {error}"),
                true,
            )
        })?;

    let rows = statement
        .query_map(params![user_id, lexeme_id], |row| {
            Ok(MutationForm {
                form_text: row.get(0)?,
                form_key: row.get(1)?,
                relation: row.get(2)?,
                source: row.get(3)?,
            })
        })
        .map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("vocabulary form collect query failed: {error}"),
                true,
            )
        })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|error| {
        AppError::vocabulary_store_failed(
            format!("vocabulary form collect row failed: {error}"),
            true,
        )
    })
}

pub fn list_pending_mutations(
    app: &AppHandle,
    limit: usize,
) -> Result<Vec<PendingMutation>, AppError> {
    let connection = open_store(app)?;
    let user_id = effective_user_id();
    let mut statement = connection
        .prepare(
            r#"
            select id, operation_id, mutation_type, payload_json, attempts
            from mutation_outbox
            where user_id = ?1
              and status = 'pending'
              and attempts < ?2
            order by created_at asc
            limit ?3
            "#,
        )
        .map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("vocabulary outbox prepare failed: {error}"),
                true,
            )
        })?;

    let rows = statement
        .query_map(params![user_id, MAX_SYNC_ATTEMPTS, limit as i64], |row| {
            Ok(PendingMutation {
                id: row.get(0)?,
                operation_id: row.get(1)?,
                mutation_type: row.get(2)?,
                payload_json: row.get(3)?,
                attempts: row.get(4)?,
            })
        })
        .map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("vocabulary outbox query failed: {error}"),
                true,
            )
        })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|error| {
        AppError::vocabulary_store_failed(format!("vocabulary outbox row failed: {error}"), true)
    })
}

pub fn count_pending_mutations(app: &AppHandle) -> Result<u32, AppError> {
    let connection = open_store(app)?;
    let user_id = effective_user_id();
    connection
        .query_row(
            r#"
            select count(*)
            from mutation_outbox
            where user_id = ?1 and status = 'pending' and attempts < ?2
            "#,
            params![user_id, MAX_SYNC_ATTEMPTS],
            |row| row.get::<_, i64>(0),
        )
        .map(|count| count.max(0) as u32)
        .map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("vocabulary outbox count failed: {error}"),
                true,
            )
        })
}

pub fn acknowledge_mutation(
    app: &AppHandle,
    operation_id: &str,
    server_revision: i64,
) -> Result<(), AppError> {
    let connection = open_store(app)?;
    let user_id = effective_user_id();
    let changed = connection
        .execute(
            r#"
            update mutation_outbox
            set status = 'acknowledged',
                server_revision = ?3,
                last_error = null,
                updated_at = datetime('now')
            where user_id = ?1 and operation_id = ?2
            "#,
            params![user_id, operation_id, server_revision],
        )
        .map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("vocabulary outbox ack failed: {error}"),
                true,
            )
        })?;
    if changed == 0 {
        return Err(AppError::vocabulary_store_failed(
            format!("vocabulary outbox ack did not match operation {operation_id}"),
            false,
        ));
    }
    Ok(())
}

pub fn fail_mutation(
    app: &AppHandle,
    operation_id: &str,
    error_message: &str,
    retryable: bool,
) -> Result<(), AppError> {
    let connection = open_store(app)?;
    let user_id = effective_user_id();
    let retryable_flag = if retryable { 1 } else { 0 };
    let changed = connection
        .execute(
            r#"
            update mutation_outbox
            set status = case
                    when ?4 = 1 and attempts + 1 < ?5 then 'pending'
                    else 'failed'
                end,
                attempts = attempts + 1,
                last_error = ?3,
                updated_at = datetime('now')
            where user_id = ?1 and operation_id = ?2
            "#,
            params![
                user_id,
                operation_id,
                error_message,
                retryable_flag,
                MAX_SYNC_ATTEMPTS
            ],
        )
        .map_err(|err| {
            AppError::vocabulary_store_failed(
                format!("vocabulary outbox fail update failed: {err}"),
                true,
            )
        })?;
    if changed == 0 {
        return Err(AppError::vocabulary_store_failed(
            format!("vocabulary outbox failure did not match operation {operation_id}"),
            false,
        ));
    }
    Ok(())
}

pub fn get_last_server_revision(app: &AppHandle) -> Result<i64, AppError> {
    let connection = open_store(app)?;
    let scope = sync_scope_key();
    let revision = connection
        .query_row(
            "select last_server_revision from sync_state where scope = ?1",
            params![scope],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("vocabulary sync state read failed: {error}"),
                true,
            )
        })?
        .unwrap_or(0);
    Ok(revision)
}

pub fn set_last_server_revision(app: &AppHandle, revision: i64) -> Result<(), AppError> {
    let connection = open_store(app)?;
    let scope = sync_scope_key();
    connection
        .execute(
            r#"
            insert into sync_state (scope, last_server_revision, updated_at)
            values (?1, ?2, datetime('now'))
            on conflict(scope) do update set
              last_server_revision = excluded.last_server_revision,
              updated_at = excluded.updated_at
            "#,
            params![scope, revision],
        )
        .map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("vocabulary sync state write failed: {error}"),
                true,
            )
        })?;
    Ok(())
}

fn sync_scope_key() -> String {
    format!("{SYNC_SCOPE}:{}", effective_user_id())
}

pub fn migrate_local_rows_to_user(app: &AppHandle, user_id: &str) -> Result<(), AppError> {
    let connection = open_store(app)?;
    let tables = [
        "user_lexemes",
        "lexeme_forms",
        "card_snapshots",
        "lookup_events",
        "mutation_outbox",
    ];
    for table in tables {
        connection
            .execute(
                &format!("update {table} set user_id = ?1 where user_id = ?2"),
                params![user_id, LOCAL_USER_ID],
            )
            .map_err(|error| {
                AppError::vocabulary_store_failed(
                    format!("vocabulary user migration failed for {table}: {error}"),
                    true,
                )
            })?;
    }
    Ok(())
}

pub fn apply_pulled_change(app: &AppHandle, change: &PulledChange) -> Result<(), AppError> {
    if change.entity_type != "card_snapshot" || change.change_type != "upsert" {
        return Ok(());
    }

    let user_id = effective_user_id();
    let mut connection = open_store(app)?;
    if pulled_change_already_applied(&connection, &user_id, change)? {
        return Ok(());
    }
    if local_mutation_already_acknowledged(&connection, &user_id, &change.operation_id)? {
        return Ok(());
    }

    apply_pulled_card_snapshot_to_connection(&mut connection, &user_id, change)
}

pub(crate) fn apply_pulled_card_snapshot_to_connection(
    connection: &mut Connection,
    user_id: &str,
    change: &PulledChange,
) -> Result<(), AppError> {
    let payload = &change.payload;
    let language = payload
        .get("language")
        .and_then(|value| value.as_str())
        .unwrap_or("en");
    let canonical_text = payload
        .get("canonicalText")
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            AppError::vocabulary_store_failed("pull payload missing canonicalText", false)
        })?;
    let canonical_key = payload
        .get("canonicalKey")
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            AppError::vocabulary_store_failed("pull payload missing canonicalKey", false)
        })?;
    let result_language = payload
        .get("resultLanguage")
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            AppError::vocabulary_store_failed("pull payload missing resultLanguage", false)
        })?;
    let schema_version = payload
        .get("schemaVersion")
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            AppError::vocabulary_store_failed("pull payload missing schemaVersion", false)
        })?;
    let provider = payload
        .get("provider")
        .and_then(|value| value.as_str())
        .unwrap_or("mock");
    let model = payload
        .get("model")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let content = payload
        .get("content")
        .ok_or_else(|| AppError::vocabulary_store_failed("pull payload missing content", false))?;
    let content_json = serde_json::to_string(content).map_err(|error| {
        AppError::vocabulary_store_failed(format!("pull content serialize failed: {error}"), false)
    })?;
    let part_of_speech = if schema_version == LEXI_RESULT_V1_SCHEMA_VERSION {
        content
            .get("translations")
            .and_then(|value| value.as_array())
            .and_then(|items| items.first())
            .and_then(|item| item.get("note"))
            .and_then(|value| value.as_str())
    } else {
        None
    };

    let tx = connection.transaction().map_err(|error| {
        AppError::vocabulary_store_failed(format!("pull apply transaction failed: {error}"), true)
    })?;

    tx.execute(
        r#"
        insert or ignore into user_lexemes (
          user_id, language, canonical_text, canonical_key, part_of_speech
        ) values (?1, ?2, ?3, ?4, ?5)
        "#,
        params![
            user_id,
            language,
            canonical_text,
            canonical_key,
            part_of_speech
        ],
    )
    .map_err(|error| {
        AppError::vocabulary_store_failed(format!("pull lexeme insert failed: {error}"), true)
    })?;

    tx.execute(
        r#"
        update user_lexemes
        set canonical_text = ?3,
            part_of_speech = coalesce(?5, part_of_speech),
            updated_at = datetime('now'),
            deleted_at = null
        where user_id = ?1 and language = ?2 and canonical_key = ?4
        "#,
        params![
            user_id,
            language,
            canonical_text,
            canonical_key,
            part_of_speech
        ],
    )
    .map_err(|error| {
        AppError::vocabulary_store_failed(format!("pull lexeme update failed: {error}"), true)
    })?;

    let lexeme_id = tx
        .query_row(
            "select id from user_lexemes where user_id = ?1 and language = ?2 and canonical_key = ?3",
            params![user_id, language, canonical_key],
            |row| row.get::<_, String>(0),
        )
        .map_err(|error| {
            AppError::vocabulary_store_failed(format!("pull lexeme lookup failed: {error}"), true)
        })?;

    if let Some(forms) = payload.get("forms").and_then(|value| value.as_array()) {
        for form in forms {
            let form_text = form
                .get("formText")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let relation = form
                .get("relation")
                .and_then(|value| value.as_str())
                .unwrap_or("observed");
            let source = form
                .get("source")
                .and_then(|value| value.as_str())
                .unwrap_or("sync");
            insert_form(&tx, &user_id, &lexeme_id, form_text, relation, source)?;
        }
    }

    tx.execute(
        "update card_snapshots set active = 0 where user_id = ?1 and lexeme_id = ?2 and result_language = ?3",
        params![user_id, lexeme_id, result_language],
    )
    .map_err(|error| {
        AppError::vocabulary_store_failed(format!("pull snapshot deactivate failed: {error}"), true)
    })?;

    tx.execute(
        r#"
        insert into card_snapshots (
          user_id, lexeme_id, schema_version, provider, model, result_language, content_json,
          active, remote_operation_id, remote_server_revision
        ) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?9)
        "#,
        params![
            user_id,
            lexeme_id,
            schema_version,
            provider,
            model,
            result_language,
            content_json,
            change.operation_id,
            change.server_revision
        ],
    )
    .map_err(|error| {
        AppError::vocabulary_store_failed(format!("pull snapshot insert failed: {error}"), true)
    })?;

    if schema_version == LEXI_RESULT_V1_SCHEMA_VERSION {
        ensure_lexeme_forms_from_content_json(
            &tx,
            user_id,
            &lexeme_id,
            canonical_text,
            canonical_key,
            &content_json,
            EnsureLexemeFormsOptions::default(),
        )?;
    }

    tx.commit().map_err(|error| {
        AppError::vocabulary_store_failed(format!("pull apply commit failed: {error}"), true)
    })
}

fn pulled_change_already_applied(
    connection: &Connection,
    user_id: &str,
    change: &PulledChange,
) -> Result<bool, AppError> {
    connection
        .query_row(
            r#"
            select exists(
              select 1
              from card_snapshots
              where user_id = ?1
                and remote_operation_id = ?2
                and remote_server_revision = ?3
            )
            "#,
            params![user_id, change.operation_id, change.server_revision],
            |row| row.get::<_, i64>(0),
        )
        .map(|exists| exists != 0)
        .map_err(|error| {
            AppError::vocabulary_store_failed(format!("pull duplicate check failed: {error}"), true)
        })
}

fn local_mutation_already_acknowledged(
    connection: &Connection,
    user_id: &str,
    operation_id: &str,
) -> Result<bool, AppError> {
    connection
        .query_row(
            r#"
            select exists(
              select 1
              from mutation_outbox
              where user_id = ?1
                and operation_id = ?2
                and status = 'acknowledged'
            )
            "#,
            params![user_id, operation_id],
            |row| row.get::<_, i64>(0),
        )
        .map(|exists| exists != 0)
        .map_err(|error| {
            AppError::vocabulary_store_failed(
                format!("pull own-mutation check failed: {error}"),
                true,
            )
        })
}

#[cfg(test)]
mod tests {
    use super::{
        apply_pulled_card_snapshot_to_connection, effective_user_id, ensure_lexeme_forms,
        ensure_lexeme_forms_from_content_json, initialize_schema,
        load_cached_japanese_word_candidates_from_connection,
        load_cached_word_study_from_connection,
        load_cached_word_study_from_connection_with_language, local_mutation_already_acknowledged,
        normalize_lookup_key, pulled_change_already_applied, repair_lexeme_forms_for_active_cards,
        save_japanese_word_candidates_result_to_connection, save_word_study_result_to_connection,
        sync_scope_key, EnsureLexemeFormsOptions, PulledChange,
    };
    use crate::{
        schema::{
            CandidateConfidence, CandidateExample, EnglishCandidate, ExampleSentence, Inflection,
            JapaneseWordCandidatesResultV1, LexiResultV1, Translation,
            LEXI_JP_WORD_CANDIDATES_V1_SCHEMA_VERSION, LEXI_RESULT_V1_SCHEMA_VERSION,
        },
        settings::ProviderKind,
    };
    use rusqlite::{params, Connection};

    fn word_study_result(
        headword: &str,
        selected_surface: Option<&str>,
        inflections: Vec<Inflection>,
        note: Option<&str>,
    ) -> (LexiResultV1, String) {
        let selected_text = selected_surface.unwrap_or(headword).to_string();
        (
            LexiResultV1 {
                schema_version: LEXI_RESULT_V1_SCHEMA_VERSION.to_string(),
                mode: "word-study".to_string(),
                source_language: "en".to_string(),
                result_language: "ja".to_string(),
                headword: headword.to_string(),
                inflections,
                translations: vec![Translation {
                    text: "意味".to_string(),
                    note: note.map(str::to_string),
                    example: ExampleSentence {
                        sentence: format!("I {headword}."),
                        japanese: "例文。".to_string(),
                    },
                    sense_kind: None,
                    base_word: None,
                }],
                nuance: "Usage nuance.".to_string(),
                synonyms: vec![],
                idioms: vec![],
                warnings: vec![],
            },
            selected_text,
        )
    }

    fn go_with_went_inflection() -> (LexiResultV1, String) {
        word_study_result(
            "go",
            Some("went"),
            vec![Inflection {
                kind: "past".to_string(),
                form: "went".to_string(),
            }],
            Some("動詞"),
        )
    }

    fn save(connection: &mut Connection, result: &LexiResultV1, selected_text: &str) {
        save_word_study_result_to_connection(
            connection,
            result,
            ProviderKind::Mock,
            "mock-word-study",
            selected_text,
        )
        .expect("save result");
    }

    fn japanese_word_candidates_result(query: &str) -> JapaneseWordCandidatesResultV1 {
        JapaneseWordCandidatesResultV1 {
            schema_version: LEXI_JP_WORD_CANDIDATES_V1_SCHEMA_VERSION.to_string(),
            mode: "jp-word-candidates".to_string(),
            source_language: "ja".to_string(),
            result_language: "en".to_string(),
            query: query.to_string(),
            candidates: vec![EnglishCandidate {
                term: "adopt".to_string(),
                part_of_speech: "動詞".to_string(),
                japanese_nuance: "取り入れる".to_string(),
                usage_note: "制度を採用する文脈で使う。".to_string(),
                example: CandidateExample {
                    sentence: "The team adopted a new policy.".to_string(),
                    japanese: "チームは新しい方針を採用した。".to_string(),
                },
                confidence: CandidateConfidence::High,
            }],
            warnings: vec![],
        }
    }

    fn save_japanese(
        connection: &mut Connection,
        result: &JapaneseWordCandidatesResultV1,
        selected_text: &str,
    ) {
        save_japanese_word_candidates_result_to_connection(
            connection,
            result,
            ProviderKind::Gemini,
            "gemini-2.5-flash-lite",
            selected_text,
        )
        .expect("save ja2en result");
    }

    fn form_count_for_lexeme(
        connection: &Connection,
        lexeme_id: &str,
        relation: Option<&str>,
    ) -> i64 {
        match relation {
            Some(relation) => connection
                .query_row(
                    "select count(*) from lexeme_forms where lexeme_id = ?1 and relation = ?2",
                    params![lexeme_id, relation],
                    |row| row.get(0),
                )
                .expect("form count"),
            None => connection
                .query_row(
                    "select count(*) from lexeme_forms where lexeme_id = ?1",
                    params![lexeme_id],
                    |row| row.get(0),
                )
                .expect("form count"),
        }
    }

    fn go_pull_payload(include_forms: bool) -> serde_json::Value {
        let mut payload = serde_json::json!({
            "schemaVersion": LEXI_RESULT_V1_SCHEMA_VERSION,
            "language": "en",
            "resultLanguage": "ja",
            "canonicalText": "go",
            "canonicalKey": "go",
            "provider": "mock",
            "model": "mock-word-study",
            "content": {
                "schemaVersion": LEXI_RESULT_V1_SCHEMA_VERSION,
                "mode": "word-study",
                "sourceLanguage": "en",
                "resultLanguage": "ja",
                "headword": "go",
                "inflections": [{ "kind": "past", "form": "went" }],
                "translations": [{
                    "text": "行く",
                    "note": "動詞",
                    "example": { "sentence": "I go.", "japanese": "行く。" }
                }],
                "nuance": "Usage nuance.",
                "synonyms": [],
                "idioms": [],
                "warnings": []
            }
        });
        if include_forms {
            payload["forms"] = serde_json::json!([
                { "formText": "go", "relation": "canonical", "source": "provider" },
                { "formText": "went", "relation": "irregular", "source": "provider" }
            ]);
        }
        payload
    }

    fn relations_for_form_key(connection: &Connection, form_key: &str) -> Vec<(String, String)> {
        let mut statement = connection
            .prepare(
                r#"
                select ul.canonical_text, lf.relation
                from lexeme_forms lf
                join user_lexemes ul on ul.id = lf.lexeme_id
                where lf.form_key = ?1
                order by ul.canonical_text, lf.relation
                "#,
            )
            .expect("prepare relations query");
        statement
            .query_map(params![form_key], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .expect("relations query")
            .map(|row| row.expect("relation row"))
            .collect()
    }

    #[test]
    fn normalizes_lookup_key() {
        assert_eq!(normalize_lookup_key("  Went\nHome  "), "went home");
    }

    #[test]
    fn save_japanese_word_candidates_inserts_ja_lexeme_and_schema() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let result = japanese_word_candidates_result("採用");
        save_japanese(&mut connection, &result, "採用");

        let language: String = connection
            .query_row(
                "select language from user_lexemes where canonical_key = '採用'",
                [],
                |row| row.get(0),
            )
            .expect("lexeme language");
        assert_eq!(language, "ja");

        let schema_version: String = connection
            .query_row(
                "select schema_version from card_snapshots where active = 1",
                [],
                |row| row.get(0),
            )
            .expect("snapshot schema");
        assert_eq!(schema_version, LEXI_JP_WORD_CANDIDATES_V1_SCHEMA_VERSION);

        let result_language: String = connection
            .query_row(
                "select result_language from card_snapshots where active = 1",
                [],
                |row| row.get(0),
            )
            .expect("result language");
        assert_eq!(result_language, "en");
    }

    #[test]
    fn loads_cached_japanese_word_candidates_by_query() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let result = japanese_word_candidates_result("採用");
        save_japanese(&mut connection, &result, "採用");

        let cached =
            load_cached_japanese_word_candidates_from_connection(&connection, "採用", "en")
                .expect("cache lookup")
                .expect("cached ja2en result");

        assert_eq!(cached.query, "採用");
        assert_eq!(cached.candidates[0].term, "adopt");
        assert_eq!(
            cached.schema_version,
            LEXI_JP_WORD_CANDIDATES_V1_SCHEMA_VERSION
        );
    }

    #[test]
    fn save_japanese_word_candidates_mutation_payload_has_ja_language_no_prompt() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let result = japanese_word_candidates_result("微妙");
        save_japanese(&mut connection, &result, "微妙");

        let payload: String = connection
            .query_row("select payload_json from mutation_outbox", [], |row| {
                row.get(0)
            })
            .expect("mutation payload");

        assert!(payload.contains("\"language\":\"ja\""));
        assert!(payload.contains(LEXI_JP_WORD_CANDIDATES_V1_SCHEMA_VERSION));
        assert!(payload.contains("\"canonicalText\":\"微妙\""));
        assert!(!payload.to_ascii_lowercase().contains("prompt"));
        assert!(!payload.contains("selectedText"));
    }

    #[test]
    fn loads_cached_card_by_irregular_alias() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let (result, selected_text) = go_with_went_inflection();
        save(&mut connection, &result, &selected_text);

        let cached = load_cached_word_study_from_connection(&connection, "went", "ja")
            .expect("cache lookup")
            .expect("cached result");

        assert_eq!(cached.headword, "go");
        assert_eq!(
            relations_for_form_key(&connection, "went"),
            vec![("go".to_string(), "irregular".to_string())]
        );
    }

    #[test]
    fn loads_legacy_word_study_cache_saved_with_en_result_language() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let (mut result, selected_text) =
            word_study_result("play", Some("playing"), vec![], Some("動詞"));
        result.result_language = "en".to_string();
        save(&mut connection, &result, &selected_text);

        let cached = load_cached_word_study_from_connection(&connection, "playing", "ja")
            .expect("cache lookup")
            .expect("legacy cache hit");

        assert_eq!(cached.headword, "play");
    }

    #[test]
    fn word_study_save_persists_result_language_for_cache_key() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let (result, selected_text) = word_study_result("play", Some("play"), vec![], Some("動詞"));
        save(&mut connection, &result, &selected_text);

        let result_language: String = connection
            .query_row(
                "select result_language from card_snapshots where active = 1",
                [],
                |row| row.get(0),
            )
            .expect("snapshot result language");
        assert_eq!(result_language, "ja");
    }

    #[test]
    fn word_study_cache_misses_for_unmatched_result_language() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let (result, selected_text) = word_study_result("play", Some("play"), vec![], Some("動詞"));
        save(&mut connection, &result, &selected_text);

        assert!(
            load_cached_word_study_from_connection(&connection, "play", "ko")
                .expect("cache lookup")
                .is_none()
        );
    }

    #[test]
    fn word_study_cache_keeps_separate_snapshots_per_result_language() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");

        let (mut ja_result, selected_text) =
            word_study_result("play", Some("play"), vec![], Some("動詞"));
        ja_result.translations[0].text = "日本語".to_string();
        save(&mut connection, &ja_result, &selected_text);

        let (mut en_result, _) = word_study_result("play", Some("play"), vec![], Some("動詞"));
        en_result.result_language = "en".to_string();
        en_result.translations[0].text = "English gloss".to_string();
        save(&mut connection, &en_result, &selected_text);

        let ja_cached = load_cached_word_study_from_connection(&connection, "play", "ja")
            .expect("ja lookup")
            .expect("ja cache");
        let en_cached = load_cached_word_study_from_connection(&connection, "play", "en")
            .expect("en lookup")
            .expect("en cache");

        assert_eq!(ja_cached.translations[0].text, "日本語");
        assert_eq!(en_cached.translations[0].text, "English gloss");
    }

    #[test]
    fn legacy_word_study_cache_fallback_only_applies_from_ja_to_en() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let (mut result, selected_text) =
            word_study_result("play", Some("play"), vec![], Some("動詞"));
        result.result_language = "en".to_string();
        save(&mut connection, &result, &selected_text);

        assert!(
            load_cached_word_study_from_connection_with_language(&connection, "play", "ja")
                .expect("direct ja lookup")
                .is_none()
        );
        assert!(
            load_cached_word_study_from_connection_with_language(&connection, "play", "ko")
                .expect("direct ko lookup")
                .is_none()
        );
        assert!(
            load_cached_word_study_from_connection(&connection, "play", "ja")
                .expect("fallback ja lookup")
                .is_some()
        );
        assert!(
            load_cached_word_study_from_connection(&connection, "play", "en")
                .expect("direct en lookup")
                .is_some()
        );
    }

    #[test]
    fn repeated_word_study_lookup_returns_same_cached_card() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let (result, selected_text) =
            word_study_result("play", Some("playing"), vec![], Some("動詞"));
        save(&mut connection, &result, &selected_text);

        let first = load_cached_word_study_from_connection(&connection, "playing", "ja")
            .expect("first lookup")
            .expect("first cache hit");
        let second = load_cached_word_study_from_connection(&connection, "playing", "ja")
            .expect("second lookup")
            .expect("second cache hit");

        assert_eq!(first.headword, second.headword);
        assert_eq!(first.translations, second.translations);
        assert_eq!(first.nuance, second.nuance);
    }

    /// User flow: search "playing" once (LLM returns headword "play"), then reuse cache
    /// without calling the provider again for "playing" or "play".
    #[test]
    fn playing_to_play_cache_scenario() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let (result, selected_text) =
            word_study_result("play", Some("playing"), vec![], Some("動詞"));
        save(&mut connection, &result, &selected_text);

        for lookup in ["playing", "play", "played", "plays"] {
            let cached = load_cached_word_study_from_connection(&connection, lookup, "ja")
                .expect("cache lookup")
                .unwrap_or_else(|| panic!("expected cache hit for '{lookup}'"));
            assert_eq!(
                cached.headword, "play",
                "lookup '{lookup}' should resolve to play lexeme"
            );
        }
    }

    /// Regular aliases alone are enough when the user first searched the base form.
    #[test]
    fn playing_hits_cache_after_play_lookup_without_observed_alias() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let (result, selected_text) = word_study_result("play", Some("play"), vec![], Some("動詞"));
        save(&mut connection, &result, &selected_text);

        assert!(
            !relations_for_form_key(&connection, "playing")
                .iter()
                .any(|(_, relation)| relation == "observed"),
            "playing should not be observed when user only searched play"
        );

        let cached = load_cached_word_study_from_connection(&connection, "playing", "ja")
            .expect("cache lookup")
            .expect("cached result via regular alias");

        assert_eq!(cached.headword, "play");
    }

    #[test]
    fn observed_alias_save_loads_surface_form() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let (result, selected_text) =
            word_study_result("play", Some("playing"), vec![], Some("動詞"));
        save(&mut connection, &result, &selected_text);

        let cached = load_cached_word_study_from_connection(&connection, "playing", "ja")
            .expect("cache lookup")
            .expect("cached result");

        assert_eq!(cached.headword, "play");
        assert!(relations_for_form_key(&connection, "playing")
            .iter()
            .any(|(_, relation)| relation == "observed"));
    }

    #[test]
    fn canonical_lookup_after_observed_save() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let (result, selected_text) =
            word_study_result("play", Some("playing"), vec![], Some("動詞"));
        save(&mut connection, &result, &selected_text);

        let cached = load_cached_word_study_from_connection(&connection, "play", "ja")
            .expect("cache lookup")
            .expect("cached result");

        assert_eq!(cached.headword, "play");
    }

    #[test]
    fn irregular_relation_stored_as_irregular_not_past() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let (result, selected_text) = go_with_went_inflection();
        save(&mut connection, &result, &selected_text);

        let relations = relations_for_form_key(&connection, "went");
        assert_eq!(relations, vec![("go".to_string(), "irregular".to_string())]);
        assert!(!relations
            .iter()
            .any(|(_, relation)| relation == "past" || relation == "pastParticiple"));
    }

    #[test]
    fn ambiguous_saw_prefers_canonical_saw_lexeme() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");

        let (see_result, _) = word_study_result(
            "see",
            Some("see"),
            vec![Inflection {
                kind: "past".to_string(),
                form: "saw".to_string(),
            }],
            Some("動詞"),
        );
        save(&mut connection, &see_result, "see");

        let (saw_result, _) = word_study_result("saw", Some("saw"), vec![], Some("名詞"));
        save(&mut connection, &saw_result, "saw");

        let cached = load_cached_word_study_from_connection(&connection, "saw", "ja")
            .expect("cache lookup")
            .expect("cached result");

        assert_eq!(cached.headword, "saw");
    }

    #[test]
    fn ambiguous_form_without_canonical_returns_none() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");

        let (river_result, _) = word_study_result("river", Some("bank"), vec![], Some("名詞"));
        save(&mut connection, &river_result, "bank");

        let (finance_result, _) = word_study_result("finance", Some("bank"), vec![], Some("名詞"));
        save(&mut connection, &finance_result, "bank");

        let cached = load_cached_word_study_from_connection(&connection, "bank", "ja")
            .expect("cache lookup");

        assert!(cached.is_none());
        assert_eq!(
            relations_for_form_key(&connection, "bank"),
            vec![
                ("finance".to_string(), "observed".to_string()),
                ("river".to_string(), "observed".to_string()),
            ]
        );
        assert!(relations_for_form_key(&connection, "bank")
            .iter()
            .all(|(_, relation)| relation != "canonical"));
    }

    #[test]
    fn regular_aliases_generated_for_verb_headword() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let (result, selected_text) = word_study_result("play", Some("play"), vec![], Some("動詞"));
        save(&mut connection, &result, &selected_text);

        let relations = relations_for_form_key(&connection, "plays")
            .into_iter()
            .map(|(_, relation)| relation)
            .collect::<Vec<_>>();
        assert_eq!(relations, vec!["regular".to_string()]);

        for form in ["played", "playing"] {
            assert!(
                relations_for_form_key(&connection, form)
                    .iter()
                    .any(|(_, relation)| relation == "regular"),
                "expected regular alias for {form}"
            );
        }

        let cached = load_cached_word_study_from_connection(&connection, "playing", "ja")
            .expect("cache lookup")
            .expect("cached result");
        assert_eq!(cached.headword, "play");
    }

    #[test]
    fn save_redirects_to_existing_lexeme_when_selected_surface_matches_alias() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");

        let (play_result, _) = word_study_result("play", Some("play"), vec![], Some("動詞"));
        save(&mut connection, &play_result, "play");

        let (wrong_headword, selected_text) =
            word_study_result("playing", Some("playing"), vec![], Some("動詞"));
        save(&mut connection, &wrong_headword, &selected_text);

        let lexeme_count: i64 = connection
            .query_row(
                "select count(*) from user_lexemes where deleted_at is null",
                [],
                |row| row.get(0),
            )
            .expect("lexeme count");
        assert_eq!(lexeme_count, 1);

        let canonical: String = connection
            .query_row(
                "select canonical_text from user_lexemes where canonical_key = 'play'",
                [],
                |row| row.get(0),
            )
            .expect("canonical text");
        assert_eq!(canonical, "play");

        let snapshot_headword: String = connection
            .query_row(
                "select json_extract(content_json, '$.headword') from card_snapshots where active = 1 order by created_at desc limit 1",
                [],
                |row| row.get(0),
            )
            .expect("snapshot headword");
        assert_eq!(snapshot_headword, "play");
    }

    #[test]
    fn save_enqueues_pending_mutation_without_raw_selected_text() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let (result, _) = go_with_went_inflection();
        save(&mut connection, &result, "went home");

        let payload: String = connection
            .query_row("select payload_json from mutation_outbox", [], |row| {
                row.get(0)
            })
            .expect("mutation payload");

        assert!(payload.contains("\"canonicalText\":\"go\""));
        assert!(!payload.contains("went home"));
        assert!(payload.contains("\"forms\""));
        assert!(payload.contains("\"relation\":\"irregular\""));
    }

    #[test]
    fn outbox_records_uuid_operation_id_and_can_be_acknowledged() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let (result, selected_text) = go_with_went_inflection();
        save(&mut connection, &result, &selected_text);

        let operation_id: String = connection
            .query_row(
                "select operation_id from mutation_outbox where status = 'pending'",
                [],
                |row| row.get(0),
            )
            .expect("operation id");
        assert!(operation_id.contains('-'));

        connection
            .execute(
                r#"
                update mutation_outbox
                set status = 'acknowledged', server_revision = 42
                where operation_id = ?1
                "#,
                params![operation_id],
            )
            .expect("ack mutation");

        let status: String = connection
            .query_row(
                "select status from mutation_outbox where operation_id = ?1",
                params![operation_id],
                |row| row.get(0),
            )
            .expect("status");
        assert_eq!(status, "acknowledged");
    }

    #[test]
    fn sync_scope_is_user_scoped() {
        let scope = sync_scope_key();
        assert!(scope.starts_with("vocabulary:"));
        assert!(scope.len() > "vocabulary:".len());
    }

    #[test]
    fn detects_acknowledged_local_mutation_for_pull_skip() {
        let connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let user_id = effective_user_id();

        connection
            .execute(
                r#"
                insert into mutation_outbox (
                  user_id, operation_id, mutation_type, payload_json, status, server_revision
                ) values (?1, ?2, 'save_card_snapshot', '{}', 'acknowledged', 9)
                "#,
                params![user_id, "11111111-1111-4111-8111-111111111111"],
            )
            .expect("insert acknowledged mutation");

        assert!(local_mutation_already_acknowledged(
            &connection,
            &user_id,
            "11111111-1111-4111-8111-111111111111",
        )
        .expect("ack lookup"));
        assert!(!local_mutation_already_acknowledged(
            &connection,
            &user_id,
            "22222222-2222-4222-8222-222222222222",
        )
        .expect("missing ack lookup"));
    }

    #[test]
    fn detects_already_applied_remote_change() {
        let connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let user_id = effective_user_id();

        connection
            .execute(
                r#"
                insert into user_lexemes (
                  user_id, language, canonical_text, canonical_key
                ) values (?1, 'en', 'go', 'go')
                "#,
                params![user_id],
            )
            .expect("insert lexeme");
        let lexeme_id: String = connection
            .query_row(
                "select id from user_lexemes where user_id = ?1 and canonical_key = 'go'",
                params![user_id],
                |row| row.get(0),
            )
            .expect("lexeme id");
        connection
            .execute(
                r#"
                insert into card_snapshots (
                  user_id, lexeme_id, schema_version, result_language, content_json,
                  remote_operation_id, remote_server_revision
                ) values (?1, ?2, ?3, 'ja', '{}', ?4, 12)
                "#,
                params![
                    user_id,
                    lexeme_id,
                    LEXI_RESULT_V1_SCHEMA_VERSION,
                    "11111111-1111-4111-8111-111111111111"
                ],
            )
            .expect("insert remote snapshot");

        let change = PulledChange {
            server_revision: 12,
            operation_id: "11111111-1111-4111-8111-111111111111".to_string(),
            entity_type: "card_snapshot".to_string(),
            change_type: "upsert".to_string(),
            payload: serde_json::json!({}),
        };

        assert!(
            pulled_change_already_applied(&connection, &user_id, &change)
                .expect("duplicate lookup")
        );
    }

    #[test]
    fn pulled_card_snapshot_projection_resolves_irregular_alias() {
        let connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let user_id = effective_user_id();

        connection
            .execute(
                r#"
                insert into user_lexemes (
                  user_id, language, canonical_text, canonical_key
                ) values (?1, 'en', 'go', 'go')
                "#,
                params![user_id],
            )
            .expect("insert lexeme");

        let lexeme_id: String = connection
            .query_row(
                "select id from user_lexemes where canonical_key = 'go'",
                [],
                |row| row.get(0),
            )
            .expect("lexeme id");

        super::insert_form(
            &connection,
            &user_id,
            &lexeme_id,
            "went",
            "irregular",
            "provider",
        )
        .expect("insert form");

        let content = serde_json::json!({
            "schemaVersion": LEXI_RESULT_V1_SCHEMA_VERSION,
            "mode": "word-study",
            "sourceLanguage": "en",
            "resultLanguage": "ja",
            "headword": "go",
            "inflections": [{ "kind": "past", "form": "went" }],
            "translations": [{
                "text": "行く",
                "note": "動詞",
                "example": { "sentence": "I go.", "japanese": "行く。" }
            }],
            "nuance": "Usage nuance.",
            "synonyms": [],
            "idioms": [],
            "warnings": []
        });
        connection
            .execute(
                r#"
                insert into card_snapshots (
                  user_id, lexeme_id, schema_version, provider, model, result_language, content_json, active
                ) values (?1, ?2, ?3, 'mock', 'mock-word-study', 'ja', ?4, 1)
                "#,
                params![user_id, lexeme_id, LEXI_RESULT_V1_SCHEMA_VERSION, content.to_string()],
            )
            .expect("insert snapshot");

        let cached = load_cached_word_study_from_connection(&connection, "went", "ja")
            .expect("cache lookup")
            .expect("cached card");
        assert_eq!(cached.headword, "go");
    }

    #[test]
    fn pull_lexeme_update_sql_uses_canonical_key_in_where_clause() {
        let connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let user_id = effective_user_id();

        connection
            .execute(
                r#"
                insert into user_lexemes (
                  user_id, language, canonical_text, canonical_key
                ) values (?1, 'en', 'old-text', 'gain')
                "#,
                params![user_id],
            )
            .expect("insert lexeme");

        connection
            .execute(
                r#"
                update user_lexemes
                set canonical_text = ?3,
                    part_of_speech = coalesce(?5, part_of_speech),
                    updated_at = datetime('now'),
                    deleted_at = null
                where user_id = ?1 and language = ?2 and canonical_key = ?4
                "#,
                params![user_id, "en", "gain", "gain", "動詞"],
            )
            .expect("update lexeme");

        let canonical_text: String = connection
            .query_row(
                "select canonical_text from user_lexemes where canonical_key = 'gain'",
                [],
                |row| row.get(0),
            )
            .expect("canonical text");
        assert_eq!(canonical_text, "gain");
    }

    #[test]
    fn ensure_lexeme_forms_adds_canonical_irregular_and_regular() {
        let connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let user_id = effective_user_id();
        let (result, _) = go_with_went_inflection();

        connection
            .execute(
                r#"
                insert into user_lexemes (
                  id, user_id, language, canonical_text, canonical_key
                ) values ('lexeme-go', ?1, 'en', 'go', 'go')
                "#,
                params![user_id],
            )
            .expect("insert lexeme");

        ensure_lexeme_forms(
            &connection,
            &user_id,
            "lexeme-go",
            "go",
            "go",
            &result,
            EnsureLexemeFormsOptions::default(),
        )
        .expect("ensure forms");

        assert!(relations_for_form_key(&connection, "go")
            .iter()
            .any(|(_, relation)| relation == "canonical"));
        assert_eq!(
            relations_for_form_key(&connection, "went"),
            vec![("go".to_string(), "irregular".to_string())]
        );
        assert!(relations_for_form_key(&connection, "going")
            .iter()
            .any(|(_, relation)| relation == "regular"));
    }

    #[test]
    fn ensure_lexeme_forms_is_idempotent() {
        let connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let user_id = effective_user_id();
        let (result, _) = go_with_went_inflection();

        connection
            .execute(
                r#"
                insert into user_lexemes (
                  id, user_id, language, canonical_text, canonical_key
                ) values ('lexeme-go', ?1, 'en', 'go', 'go')
                "#,
                params![user_id],
            )
            .expect("insert lexeme");

        let options = EnsureLexemeFormsOptions::default();
        for _ in 0..2 {
            ensure_lexeme_forms(
                &connection,
                &user_id,
                "lexeme-go",
                "go",
                "go",
                &result,
                options,
            )
            .expect("ensure forms");
        }

        let total = form_count_for_lexeme(&connection, "lexeme-go", None);
        assert!(total >= 4);
        let second_pass = form_count_for_lexeme(&connection, "lexeme-go", None);
        ensure_lexeme_forms(
            &connection,
            &user_id,
            "lexeme-go",
            "go",
            "go",
            &result,
            options,
        )
        .expect("ensure forms again");
        assert_eq!(
            form_count_for_lexeme(&connection, "lexeme-go", None),
            second_pass
        );
    }

    #[test]
    fn apply_pulled_change_ensures_forms_when_payload_forms_omitted() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let user_id = effective_user_id();

        let change = PulledChange {
            server_revision: 7,
            operation_id: "33333333-3333-4333-8333-333333333333".to_string(),
            entity_type: "card_snapshot".to_string(),
            change_type: "upsert".to_string(),
            payload: go_pull_payload(false),
        };

        apply_pulled_card_snapshot_to_connection(&mut connection, &user_id, &change)
            .expect("apply pull");

        assert!(relations_for_form_key(&connection, "go")
            .iter()
            .any(|(_, relation)| relation == "canonical"));
        assert_eq!(
            relations_for_form_key(&connection, "went"),
            vec![("go".to_string(), "irregular".to_string())]
        );
        assert!(relations_for_form_key(&connection, "going")
            .iter()
            .any(|(_, relation)| relation == "regular"));
    }

    #[test]
    fn apply_pulled_change_went_resolves_without_forms_array() {
        let mut connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let user_id = effective_user_id();

        let change = PulledChange {
            server_revision: 8,
            operation_id: "44444444-4444-4444-8444-444444444444".to_string(),
            entity_type: "card_snapshot".to_string(),
            change_type: "upsert".to_string(),
            payload: go_pull_payload(false),
        };
        apply_pulled_card_snapshot_to_connection(&mut connection, &user_id, &change)
            .expect("apply pull");

        let cached = load_cached_word_study_from_connection(&connection, "went", "ja")
            .expect("cache lookup")
            .expect("cached card");
        assert_eq!(cached.headword, "go");
    }

    #[test]
    fn repair_lexeme_forms_backfills_lexeme_and_card_only() {
        let connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let user_id = effective_user_id();

        connection
            .execute(
                r#"
                insert into user_lexemes (
                  id, user_id, language, canonical_text, canonical_key
                ) values ('lexeme-go', ?1, 'en', 'go', 'go')
                "#,
                params![user_id],
            )
            .expect("insert lexeme");

        let content = go_pull_payload(false)["content"].clone();
        connection
            .execute(
                r#"
                insert into card_snapshots (
                  user_id, lexeme_id, schema_version, result_language, content_json, active
                ) values (?1, 'lexeme-go', ?2, 'ja', ?3, 1)
                "#,
                params![user_id, LEXI_RESULT_V1_SCHEMA_VERSION, content.to_string()],
            )
            .expect("insert snapshot");

        assert_eq!(form_count_for_lexeme(&connection, "lexeme-go", None), 0);

        repair_lexeme_forms_for_active_cards(&connection).expect("repair forms");

        assert_eq!(
            form_count_for_lexeme(&connection, "lexeme-go", Some("canonical")),
            1
        );
        assert_eq!(
            form_count_for_lexeme(&connection, "lexeme-go", Some("irregular")),
            1
        );
        assert!(form_count_for_lexeme(&connection, "lexeme-go", Some("regular")) >= 3);

        let cached = load_cached_word_study_from_connection(&connection, "went", "ja")
            .expect("cache lookup")
            .expect("cached card");
        assert_eq!(cached.headword, "go");
    }

    #[test]
    fn ensure_lexeme_forms_from_content_json_matches_struct_path() {
        let connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let user_id = effective_user_id();
        let (result, _) = go_with_went_inflection();
        let content_json = serde_json::to_string(&result).expect("serialize");

        connection
            .execute(
                r#"
                insert into user_lexemes (
                  id, user_id, language, canonical_text, canonical_key
                ) values ('lexeme-go', ?1, 'en', 'go', 'go')
                "#,
                params![user_id],
            )
            .expect("insert lexeme");

        ensure_lexeme_forms_from_content_json(
            &connection,
            &user_id,
            "lexeme-go",
            "go",
            "go",
            &content_json,
            EnsureLexemeFormsOptions::default(),
        )
        .expect("ensure from json");

        assert_eq!(
            relations_for_form_key(&connection, "went"),
            vec![("go".to_string(), "irregular".to_string())]
        );
    }

    #[test]
    fn loads_cached_card_by_canonical_key_without_lexeme_form_row() {
        let connection = Connection::open_in_memory().expect("memory sqlite");
        initialize_schema(&connection).expect("schema");
        let user_id = effective_user_id();

        connection
            .execute(
                r#"
                insert into user_lexemes (
                  id, user_id, language, canonical_text, canonical_key
                ) values ('lexeme-go', ?1, 'en', 'go', 'go')
                "#,
                params![user_id],
            )
            .expect("insert lexeme");

        let content = serde_json::json!({
            "schemaVersion": LEXI_RESULT_V1_SCHEMA_VERSION,
            "mode": "word-study",
            "sourceLanguage": "en",
            "resultLanguage": "ja",
            "headword": "go",
            "inflections": [],
            "translations": [{
                "text": "行く",
                "note": "動詞",
                "example": { "sentence": "I go.", "japanese": "行く。" }
            }],
            "nuance": "Usage nuance.",
            "synonyms": [],
            "idioms": [],
            "warnings": []
        });
        connection
            .execute(
                r#"
                insert into card_snapshots (
                  user_id, lexeme_id, schema_version, result_language, content_json, active
                ) values (?1, 'lexeme-go', ?2, 'ja', ?3, 1)
                "#,
                params![user_id, LEXI_RESULT_V1_SCHEMA_VERSION, content.to_string()],
            )
            .expect("insert snapshot");

        let form_count: i64 = connection
            .query_row(
                "select count(*) from lexeme_forms where form_key = 'go'",
                [],
                |row| row.get(0),
            )
            .expect("form count");
        assert_eq!(form_count, 0);

        let cached = load_cached_word_study_from_connection(&connection, "go", "ja")
            .expect("cache lookup")
            .expect("cached card");
        assert_eq!(cached.headword, "go");
    }
}
