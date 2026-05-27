use crate::{
    errors::AppError,
    schema::{
        parse_lexi_result_v1, ExampleSentence, LexiResultV1, RelatedWord, Translation,
        LEXI_RESULT_V1_SCHEMA_VERSION, TRANSLATION_NOTE_VALUES,
    },
    settings::{ProviderKind, SettingsState},
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};
use tauri::{AppHandle, Emitter, Manager};

const REQUEST_TIMEOUT_MS: u64 = 60_000;
const MAX_OUTPUT_TOKENS: u32 = 2048;
const TRANSFORM_EVENT: &str = "lexi:transform";
static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Default)]
pub struct SelectedTextState {
    selected_text: Mutex<Option<String>>,
}

impl SelectedTextState {
    pub fn replace(&self, selected_text: String) {
        *self
            .selected_text
            .lock()
            .expect("selected text state poisoned") = Some(selected_text);
    }

    fn current(&self) -> Result<String, AppError> {
        self.selected_text
            .lock()
            .expect("selected text state poisoned")
            .clone()
            .filter(|text| !text.trim().is_empty())
            .ok_or_else(|| {
                AppError::new(
                    crate::errors::AppErrorCode::SelectionEmpty,
                    "Select text before running Lexi.",
                    "transform requested without selected text in backend state",
                    false,
                )
            })
    }
}

#[derive(Debug, Clone)]
pub struct TransformRequest {
    pub selected_text: String,
    pub result_language: String,
    pub prompt_mode: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformResult {
    pub result: LexiResultV1,
    pub provider: ProviderKind,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct TransformCaptureMetadata {
    pub shortcut: String,
    pub capture_method: &'static str,
    pub source_process: Option<String>,
    pub source_window_title: Option<String>,
    pub character_count: usize,
    pub multiline: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformCaptureInput {
    pub shortcut: String,
    pub capture_method: String,
    pub source_process: Option<String>,
    pub source_window_title: Option<String>,
    pub character_count: usize,
    pub multiline: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LexiPartialResult {
    pub headword: Option<String>,
    pub translations: Vec<Translation>,
    pub nuance: Option<String>,
    pub synonyms: Vec<RelatedWord>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct StreamTextDelta {
    text: Option<String>,
    finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TransformEvent {
    Started {
        request_id: u64,
        selected_text_preview: String,
        shortcut: String,
        capture_method: &'static str,
        source_process: Option<String>,
        source_window_title: Option<String>,
        character_count: usize,
        multiline: bool,
        provider: ProviderKind,
        model: String,
    },
    Streaming {
        request_id: u64,
        partial: LexiPartialResult,
    },
    Validating {
        request_id: u64,
        partial: LexiPartialResult,
    },
    Ready {
        request_id: u64,
        result: LexiResultV1,
        provider: ProviderKind,
        model: String,
    },
    Failed {
        request_id: u64,
        error: AppError,
    },
}

impl LexiPartialResult {
    fn from_result(result: &LexiResultV1) -> Self {
        Self {
            headword: Some(result.headword.clone()),
            translations: result.translations.clone(),
            nuance: Some(result.nuance.clone()),
            synonyms: result.synonyms.clone(),
            warnings: result.warnings.clone(),
        }
    }

    fn is_empty(&self) -> bool {
        self.headword.is_none()
            && self.translations.is_empty()
            && self.nuance.is_none()
            && self.synonyms.is_empty()
            && self.warnings.is_empty()
    }
}

pub trait LlmProvider {
    fn transform(&self, request: &TransformRequest) -> Result<LexiResultV1, AppError>;
}

pub struct MockProvider;

impl LlmProvider for MockProvider {
    fn transform(&self, request: &TransformRequest) -> Result<LexiResultV1, AppError> {
        Ok(LexiResultV1 {
            schema_version: LEXI_RESULT_V1_SCHEMA_VERSION.to_string(),
            mode: "word-study".to_string(),
            source_language: "auto".to_string(),
            result_language: request.result_language.clone(),
            headword: mock_headword(&request.selected_text),
            translations: vec![crate::schema::Translation {
                text: "確認用の訳語".to_string(),
                note: None,
                example: ExampleSentence {
                    sentence: "This is a short example from the mock provider.".to_string(),
                    japanese: "これはモックプロバイダーによる短い例文です。".to_string(),
                },
            }],
            nuance: "MockProvider による構造化レスポンスです。".to_string(),
            synonyms: vec![],
            warnings: vec![
                "Provider 設定が mock のため、実際の API は呼び出していません。".to_string(),
            ],
        })
    }
}

fn mock_headword(selected_text: &str) -> String {
    let first_word = selected_text
        .split_whitespace()
        .next()
        .unwrap_or("selection")
        .trim_matches(|character: char| !character.is_alphanumeric())
        .to_ascii_lowercase();

    let lemma = match first_word.as_str() {
        "went" | "gone" => "go".to_string(),
        "ran" => "run".to_string(),
        "studied" => "study".to_string(),
        "better" | "best" => "good".to_string(),
        word if word.ends_with("ied") && word.len() > 4 => {
            format!("{}y", &word[..word.len() - 3])
        }
        word if word.ends_with("ed") && word.len() > 3 => word[..word.len() - 2].to_string(),
        word => word.to_string(),
    };

    lemma.chars().take(48).collect()
}

fn selected_text_preview(selected_text: &str) -> String {
    selected_text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(48)
        .collect()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModel {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelsResult {
    pub provider: ProviderKind,
    pub models: Vec<ProviderModel>,
    pub fetched: bool,
    pub warning: Option<String>,
}

#[tauri::command]
pub async fn list_provider_models(
    app: AppHandle,
    settings_state: tauri::State<'_, SettingsState>,
    provider: ProviderKind,
) -> Result<ProviderModelsResult, AppError> {
    if provider == ProviderKind::Mock {
        return Ok(ProviderModelsResult {
            provider,
            models: fallback_models(provider),
            fetched: true,
            warning: None,
        });
    }

    let Some(api_key) = settings_state
        .api_key(&app, provider)?
        .filter(|key| !key.trim().is_empty())
    else {
        return Ok(ProviderModelsResult {
            provider,
            models: fallback_models(provider),
            fetched: false,
            warning: Some("API key is not configured; showing default models.".to_string()),
        });
    };

    match fetch_provider_models(provider, &api_key).await {
        Ok(models) if !models.is_empty() => Ok(ProviderModelsResult {
            provider,
            models,
            fetched: true,
            warning: None,
        }),
        Ok(_) => Ok(ProviderModelsResult {
            provider,
            models: fallback_models(provider),
            fetched: false,
            warning: Some("Provider returned no usable models; showing defaults.".to_string()),
        }),
        Err(error) => Ok(ProviderModelsResult {
            provider,
            models: fallback_models(provider),
            fetched: false,
            warning: Some(error.diagnostic_message),
        }),
    }
}

pub fn start_transform_stream(
    app: AppHandle,
    selected_text: String,
    capture: TransformCaptureMetadata,
) {
    tauri::async_runtime::spawn(async move {
        let request_id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        if let Err(error) =
            run_transform_stream_for_capture(app.clone(), request_id, selected_text, capture).await
        {
            let _ = app.emit(
                TRANSFORM_EVENT,
                TransformEvent::Failed { request_id, error },
            );
        }
    });
}

#[tauri::command]
pub fn run_transform_stream(
    app: AppHandle,
    selected_text_state: tauri::State<'_, SelectedTextState>,
    capture: TransformCaptureInput,
) -> Result<(), AppError> {
    let selected_text = selected_text_state.current()?;
    start_transform_stream(app, selected_text, capture.try_into()?);
    Ok(())
}

async fn run_transform_stream_for_capture(
    app: AppHandle,
    request_id: u64,
    selected_text: String,
    capture: TransformCaptureMetadata,
) -> Result<(), AppError> {
    let settings_state = app.state::<SettingsState>();
    let settings = settings_state.load_settings(&app)?;
    let selected_text_preview = selected_text_preview(&selected_text);
    let request = TransformRequest {
        selected_text,
        result_language: settings.result_language.clone(),
        prompt_mode: settings.prompt_mode.clone(),
    };

    let _ = app.emit(
        TRANSFORM_EVENT,
        TransformEvent::Started {
            request_id,
            selected_text_preview,
            shortcut: capture.shortcut,
            capture_method: capture.capture_method,
            source_process: capture.source_process,
            source_window_title: capture.source_window_title,
            character_count: capture.character_count,
            multiline: capture.multiline,
            provider: settings.provider,
            model: settings.model.clone(),
        },
    );

    if settings.provider == ProviderKind::Mock {
        let result = MockProvider.transform(&request)?;
        let partial = LexiPartialResult::from_result(&result);
        let _ = app.emit(
            TRANSFORM_EVENT,
            TransformEvent::Streaming {
                request_id,
                partial: partial.clone(),
            },
        );
        let _ = app.emit(
            TRANSFORM_EVENT,
            TransformEvent::Ready {
                request_id,
                result,
                provider: settings.provider,
                model: settings.model,
            },
        );
        return Ok(());
    }

    let api_key = settings_state
        .api_key(&app, settings.provider)?
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| {
            AppError::provider_not_configured(format!(
                "{} API key is not configured",
                settings.provider.secret_user()
            ))
        })?;

    let raw_json = match settings.provider {
        ProviderKind::Gemini => {
            call_gemini_stream(&app, request_id, &api_key, &settings.model, &request).await?
        }
        ProviderKind::OpenAi => {
            call_openai_stream(&app, request_id, &api_key, &settings.model, &request).await?
        }
        ProviderKind::Mock => unreachable!("mock provider returned above"),
    };
    let partial = partial_from_json_fragment(&raw_json);
    let _ = app.emit(
        TRANSFORM_EVENT,
        TransformEvent::Validating {
            request_id,
            partial,
        },
    );
    let result = parse_lexi_result_v1(&raw_json)?;
    let _ = app.emit(
        TRANSFORM_EVENT,
        TransformEvent::Ready {
            request_id,
            result,
            provider: settings.provider,
            model: settings.model,
        },
    );

    Ok(())
}

impl TryFrom<TransformCaptureInput> for TransformCaptureMetadata {
    type Error = AppError;

    fn try_from(input: TransformCaptureInput) -> Result<Self, Self::Error> {
        let capture_method = match input.capture_method.as_str() {
            "uia-focused-element" => "uia-focused-element",
            "uia-foreground-window" => "uia-foreground-window",
            "clipboard-fallback" => "clipboard-fallback",
            _ => {
                return Err(AppError::invalid_model_output(
                    "retry capture metadata had an unknown capture method",
                ))
            }
        };

        Ok(Self {
            shortcut: input.shortcut,
            capture_method,
            source_process: input.source_process,
            source_window_title: input.source_window_title,
            character_count: input.character_count,
            multiline: input.multiline,
        })
    }
}

#[tauri::command]
pub async fn run_transform(
    app: AppHandle,
    selected_text_state: tauri::State<'_, SelectedTextState>,
    settings_state: tauri::State<'_, SettingsState>,
) -> Result<TransformResult, AppError> {
    let settings = settings_state.load_settings(&app)?;
    let request = TransformRequest {
        selected_text: selected_text_state.current()?,
        result_language: settings.result_language.clone(),
        prompt_mode: settings.prompt_mode.clone(),
    };

    if settings.provider == ProviderKind::Mock {
        let result = MockProvider.transform(&request)?;
        return Ok(TransformResult {
            result,
            provider: settings.provider,
            model: settings.model,
        });
    }

    let api_key = settings_state
        .api_key(&app, settings.provider)?
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| {
            AppError::provider_not_configured(format!(
                "{} API key is not configured",
                settings.provider.secret_user()
            ))
        })?;

    let result = match settings.provider {
        ProviderKind::Gemini => call_gemini(&api_key, &settings.model, &request).await?,
        ProviderKind::OpenAi => call_openai(&api_key, &settings.model, &request).await?,
        ProviderKind::Mock => unreachable!("mock provider returned above"),
    };

    Ok(TransformResult {
        result,
        provider: settings.provider,
        model: settings.model,
    })
}

fn build_word_study_prompt(request: &TransformRequest) -> String {
    format!(
        r#"You are Lexi's word-study formatter. Analyze the selected text and return one compact JSON object only.

Hard requirements:
- Output must match schemaVersion "{schema_version}" and mode "word-study".
- Do not include markdown, prose outside JSON, comments, or code fences.
- Use resultLanguage "{result_language}" for all explanations and Japanese meaning fields.
- Keep the result compact enough for a small desktop popup.
- If the selection is a single inflected word, set headword to its dictionary/base form, not the selected surface form. Examples: went -> go, ran -> run, studied -> study, better -> good.
- If the selection is a sentence, choose the central word or phrase as headword and normalize that headword to its dictionary/base form when possible.
- If reliable synonyms are unavailable, use an empty array instead of guessing.

Field contract:
- headword: canonical dictionary/base form or short phrase, max 48 characters. Do not copy an inflected selected word such as a past-tense verb when a base form is known.
- translations: dictionary-style Japanese sense entries, not nuance explanations, not a thesaurus, not a list of alternative Japanese renderings, and not a vocabulary expansion list. Return 1 to 3 items only when each item represents a distinct English dictionary sense that should be learned separately.
  - text must be a compact Japanese equivalent or established Japanese expression that can stand as a dictionary meaning entry.
  - Prefer one broad entry when several Japanese words translate the same English sense. Put comma-separated Japanese alternatives in one text value only when that is clearer than choosing one broad equivalent.
  - Separate entries only by real English-side sense boundaries such as part of speech, countable vs uncountable use, transitive vs intransitive use, concrete vs abstract use, legal/social vs technical use, or established idiomatic use.
  - Do not split entries merely because Japanese collocations differ. For example, "adoption" should not become separate entries for 採用 and 採択 unless you can point to different English dictionary senses, not just different Japanese objects.
  - Do not split entries merely because Japanese wording differs in register, domain, or naturalness. For example, "demonstration" should not become separate entries for デモ and 実演 when both mean showing how something works.
  - Do not create multiple entries by rephrasing the same sense, changing formality, or offering Japanese synonyms. For example, "近づく" and "接近する" are the same sense; keep only the natural broad entry "近づく".
  - If candidates differ only in wording, kanji/kana style, formality, specificity, or explanation length, keep the broadest common dictionary equivalent and omit the rest.
  - Do not output sentence-like glosses, usage explanations, source-text summaries, or "X after Y" definitions. Put usage feel in nuance instead.
  - note must be null or exactly one part-of-speech label from this list: 名詞, 動詞, 形容詞, 副詞, 前置詞, 接続詞, 代名詞, 助動詞, 冠詞, 間投詞, 句, 成句, 接頭辞, 接尾辞. Do not use semantic domains such as 数学, 数, 比, 専門, or technical field labels in note.
  - example is required for every translation item and must demonstrate that specific sense.
    - example.sentence: a simple English sentence, max 96 characters. Prefer common daily contexts and do not quote sensitive selected text unless necessary.
    - example.japanese: natural Japanese translation of example.sentence, max 96 characters.
- nuance: exactly 1 sentence, max 90 Japanese characters or 22 English words. Give an intuitive explanation that helps the user decide when the headword is appropriate.
- synonyms: 2 to 4 near words that are useful for learning how to use the headword more precisely. Do not include antonyms.
  - term: a real common near word.
  - japanese: concise meaning.
  - usageComparison: one direct sentence comparing the synonym with the headword. Explain when to choose the headword and when to choose this synonym, max 110 Japanese characters.
- warnings: empty unless the input is ambiguous, too short, not a word/phrase, or confidence is low.

Quality rules:
- Prefer precision over coverage.
- The translations array is a short sense inventory, not a Japanese synonym list. If you are unsure whether two entries are separate English senses, merge them.
- For translations, prefer dictionary sense entries over explanations. Use nuance for explanations, not translations.
- Before finalizing translations, compare every pair of translation entries. Merge or delete overlapping Japanese meanings unless they differ by a real English-side dictionary sense boundary. Different Japanese word choice alone is never enough.
- A good translation list should make the user think "these are different meanings or parts of speech", not "these are several ways to say the same thing in Japanese".
- A Japanese synonym, register difference, or wording preference is not a sense boundary. Do not split entries for pairs like 近づく/接近する, 始める/開始する, 使う/使用する, わずかな/少しの.
- Keep each translation example short and aligned with that translation's specific sense.
- Do not repeat the same information across headword nuance and synonym usageComparison.
- Do not pad arrays to hit counts.
- Preserve selected text privacy; never quote more than needed for headword/examples.

Selected text:
{text}"#,
        schema_version = LEXI_RESULT_V1_SCHEMA_VERSION,
        result_language = request.result_language,
        text = request.selected_text,
    )
}

async fn fetch_provider_models(
    provider: ProviderKind,
    api_key: &str,
) -> Result<Vec<ProviderModel>, AppError> {
    match provider {
        ProviderKind::Mock => Ok(fallback_models(provider)),
        ProviderKind::Gemini => fetch_gemini_models(api_key).await,
        ProviderKind::OpenAi => fetch_openai_models(api_key).await,
    }
}

async fn fetch_openai_models(api_key: &str) -> Result<Vec<ProviderModel>, AppError> {
    let client = reqwest_client()?;
    let response = client
        .get("https://api.openai.com/v1/models")
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|error| {
            AppError::provider_request_failed(
                format!("OpenAI models request failed: {error}"),
                true,
            )
        })?;

    if !response.status().is_success() {
        return Err(AppError::provider_request_failed(
            format!("OpenAI models request returned HTTP {}", response.status()),
            response.status().as_u16() == 429 || response.status().is_server_error(),
        ));
    }

    let payload = response
        .json::<OpenAiModelsResponse>()
        .await
        .map_err(|error| {
            AppError::provider_request_failed(format!("OpenAI models parse failed: {error}"), true)
        })?;
    let mut models = payload
        .data
        .into_iter()
        .filter(|model| is_openai_chat_model(&model.id))
        .map(|model| ProviderModel {
            label: model.id.clone(),
            id: model.id,
        })
        .collect::<Vec<_>>();
    models.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(models)
}

async fn fetch_gemini_models(api_key: &str) -> Result<Vec<ProviderModel>, AppError> {
    let client = reqwest_client()?;
    let response = client
        .get("https://generativelanguage.googleapis.com/v1beta/models")
        .query(&[("key", api_key)])
        .send()
        .await
        .map_err(|error| {
            AppError::provider_request_failed(
                format!("Gemini models request failed: {error}"),
                true,
            )
        })?;

    if !response.status().is_success() {
        return Err(AppError::provider_request_failed(
            format!("Gemini models request returned HTTP {}", response.status()),
            response.status().as_u16() == 429 || response.status().is_server_error(),
        ));
    }

    let payload = response
        .json::<GeminiModelsResponse>()
        .await
        .map_err(|error| {
            AppError::provider_request_failed(format!("Gemini models parse failed: {error}"), true)
        })?;
    let mut models = payload
        .models
        .into_iter()
        .filter(|model| {
            model
                .supported_generation_methods
                .iter()
                .any(|method| method == "generateContent")
        })
        .map(|model| {
            let id = model.name.trim_start_matches("models/").to_string();
            ProviderModel {
                label: model.display_name.unwrap_or_else(|| id.clone()),
                id,
            }
        })
        .collect::<Vec<_>>();
    models.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(models)
}

fn reqwest_client() -> Result<reqwest::Client, AppError> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(REQUEST_TIMEOUT_MS))
        .build()
        .map_err(|error| {
            AppError::provider_request_failed(format!("HTTP client build failed: {error}"), true)
        })
}

fn fallback_models(provider: ProviderKind) -> Vec<ProviderModel> {
    match provider {
        ProviderKind::Gemini => vec![
            ProviderModel {
                id: "gemini-2.5-flash-lite".to_string(),
                label: "Gemini 2.5 Flash-Lite".to_string(),
            },
            ProviderModel {
                id: "gemini-2.5-flash".to_string(),
                label: "Gemini 2.5 Flash".to_string(),
            },
        ],
        ProviderKind::OpenAi => vec![
            ProviderModel {
                id: "gpt-5.4-nano".to_string(),
                label: "GPT-5.4 nano".to_string(),
            },
            ProviderModel {
                id: "gpt-5-nano".to_string(),
                label: "GPT-5 nano".to_string(),
            },
            ProviderModel {
                id: "gpt-5.4-mini".to_string(),
                label: "GPT-5.4 mini".to_string(),
            },
        ],
        ProviderKind::Mock => vec![ProviderModel {
            id: "mock-word-study".to_string(),
            label: "Mock word-study".to_string(),
        }],
    }
}

fn is_openai_chat_model(id: &str) -> bool {
    id.starts_with("gpt-") || id.starts_with("o")
}

fn lexi_result_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "schemaVersion",
            "mode",
            "sourceLanguage",
            "resultLanguage",
            "headword",
            "translations",
            "nuance",
            "synonyms",
            "warnings"
        ],
        "properties": {
            "schemaVersion": { "type": "string", "enum": [LEXI_RESULT_V1_SCHEMA_VERSION] },
            "mode": { "type": "string", "enum": ["word-study"] },
            "sourceLanguage": { "type": "string" },
            "resultLanguage": { "type": "string" },
            "headword": { "type": "string" },
            "translations": {
                "type": "array",
                "minItems": 1,
                "maxItems": 3,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["text", "note", "example"],
                    "properties": {
                        "text": { "type": "string" },
                        "note": translation_note_json_schema(),
                        "example": { "$ref": "#/$defs/exampleSentence" }
                    }
                }
            },
            "nuance": { "type": "string" },
            "synonyms": { "type": "array", "minItems": 0, "maxItems": 4, "items": { "$ref": "#/$defs/relatedWord" } },
            "warnings": { "type": "array", "items": { "type": "string" } }
        },
        "$defs": {
            "exampleSentence": {
                "type": "object",
                "additionalProperties": false,
                "required": ["sentence", "japanese"],
                "properties": {
                    "sentence": { "type": "string" },
                    "japanese": { "type": "string" }
                }
            },
            "relatedWord": {
                "type": "object",
                "additionalProperties": false,
                "required": ["term", "japanese", "usageComparison"],
                "properties": {
                    "term": { "type": "string" },
                    "japanese": { "type": "string" },
                    "usageComparison": { "type": "string" }
                }
            }
        }
    })
}

fn translation_note_json_schema() -> Value {
    let mut values = TRANSLATION_NOTE_VALUES
        .iter()
        .map(|value| json!(value))
        .collect::<Vec<_>>();
    values.push(Value::Null);

    json!({
        "type": ["string", "null"],
        "enum": values
    })
}

async fn call_openai_stream(
    app: &AppHandle,
    request_id: u64,
    api_key: &str,
    model: &str,
    request: &TransformRequest,
) -> Result<String, AppError> {
    let client = reqwest_client()?;
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&json!({
            "model": model,
            "stream": true,
            "messages": [
                {
                    "role": "system",
                    "content": "You return only strict, compact JSON for Lexi's word-study schema. Keep every field short and contrastive."
                },
                {
                    "role": "user",
                    "content": build_word_study_prompt(request)
                }
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "lexi_result_v1",
                    "strict": true,
                    "schema": lexi_result_schema()
                }
            }
        }))
        .send()
        .await
        .map_err(|error| {
            AppError::provider_request_failed(format!("OpenAI stream request failed: {error}"), true)
        })?;

    if !response.status().is_success() {
        return Err(AppError::provider_request_failed(
            format!("OpenAI stream request returned HTTP {}", response.status()),
            response.status().as_u16() == 429 || response.status().is_server_error(),
        ));
    }

    read_sse_stream(app, request_id, response, parse_openai_stream_text).await
}

async fn call_gemini_stream(
    app: &AppHandle,
    request_id: u64,
    api_key: &str,
    model: &str,
    request: &TransformRequest,
) -> Result<String, AppError> {
    let client = reqwest_client()?;
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent"
    );
    let response = client
        .post(url)
        .query(&[("key", api_key), ("alt", "sse")])
        .json(&json!({
            "contents": [
                {
                    "role": "user",
                    "parts": [{ "text": build_word_study_prompt(request) }]
                }
            ],
            "generationConfig": {
                "temperature": 0.2,
                "maxOutputTokens": MAX_OUTPUT_TOKENS,
                "responseMimeType": "application/json",
                "responseSchema": gemini_lexi_result_schema()
            }
        }))
        .send()
        .await
        .map_err(|error| {
            AppError::provider_request_failed(
                format!("Gemini stream request failed: {error}"),
                true,
            )
        })?;

    if !response.status().is_success() {
        return Err(AppError::provider_request_failed(
            format!("Gemini stream request returned HTTP {}", response.status()),
            response.status().as_u16() == 429 || response.status().is_server_error(),
        ));
    }

    read_sse_stream(app, request_id, response, parse_gemini_stream_text).await
}

async fn read_sse_stream(
    app: &AppHandle,
    request_id: u64,
    response: reqwest::Response,
    parse_text: fn(&str) -> Result<StreamTextDelta, AppError>,
) -> Result<String, AppError> {
    let mut stream = response.bytes_stream();
    let mut sse_buffer = String::new();
    let mut content = String::new();
    let mut last_partial = LexiPartialResult::default();
    let mut finish_reason = None;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| {
            AppError::provider_request_failed(format!("provider stream read failed: {error}"), true)
        })?;
        let text = String::from_utf8_lossy(&chunk);
        sse_buffer.push_str(&text);

        while let Some(event) = pop_sse_event(&mut sse_buffer) {
            let delta = parse_sse_event_text(&event, parse_text)?;
            if delta.finish_reason.is_some() {
                finish_reason = delta.finish_reason;
            }
            if let Some(text) = delta.text {
                content.push_str(&text);
                let partial = partial_from_json_fragment(&content);
                if !partial.is_empty() && partial != last_partial {
                    last_partial = partial.clone();
                    let _ = app.emit(
                        TRANSFORM_EVENT,
                        TransformEvent::Streaming {
                            request_id,
                            partial,
                        },
                    );
                }
            }
        }
    }

    if let Some(reason) = finish_reason.as_deref() {
        if provider_finish_reason_indicates_truncation(reason) {
            return Err(AppError::invalid_model_output(format!(
                "provider stream ended before complete JSON: {reason}"
            )));
        }
    }

    if content.trim().is_empty() {
        return Err(AppError::invalid_model_output(
            "provider stream completed without JSON content",
        ));
    }

    Ok(content)
}

fn pop_sse_event(buffer: &mut String) -> Option<String> {
    let lf_index = buffer.find("\n\n").map(|index| (index, 2usize));
    let crlf_index = buffer.find("\r\n\r\n").map(|index| (index, 4usize));
    let (index, delimiter_len) = match (lf_index, crlf_index) {
        (Some(left), Some(right)) => {
            if left.0 <= right.0 {
                left
            } else {
                right
            }
        }
        (Some(value), None) | (None, Some(value)) => value,
        (None, None) => return None,
    };

    let event = buffer[..index].to_string();
    buffer.drain(..index + delimiter_len);
    Some(event)
}

fn parse_sse_event_text(
    event: &str,
    parse_text: fn(&str) -> Result<StreamTextDelta, AppError>,
) -> Result<StreamTextDelta, AppError> {
    let data = sse_data_payload(event);
    if data.is_empty() || data == "[DONE]" {
        return Ok(StreamTextDelta::default());
    }

    parse_text(&data)
}

fn sse_data_payload(event: &str) -> String {
    event
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(|data| data.strip_prefix(' ').unwrap_or(data))
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_openai_stream_text(data: &str) -> Result<StreamTextDelta, AppError> {
    let chunk = serde_json::from_str::<OpenAiStreamChunk>(data).map_err(|error| {
        AppError::invalid_model_output(format!("OpenAI stream chunk parse failed: {error}"))
    })?;

    let choice = chunk.choices.first();
    Ok(StreamTextDelta {
        text: choice.and_then(|choice| choice.delta.content.clone()),
        finish_reason: choice.and_then(|choice| choice.finish_reason.clone()),
    })
}

fn parse_gemini_stream_text(data: &str) -> Result<StreamTextDelta, AppError> {
    let chunk = serde_json::from_str::<GeminiResponse>(data).map_err(|error| {
        AppError::invalid_model_output(format!("Gemini stream chunk parse failed: {error}"))
    })?;

    let candidate = chunk.candidates.first();
    Ok(StreamTextDelta {
        text: candidate
            .and_then(gemini_candidate_text)
            .map(str::to_string),
        finish_reason: candidate.and_then(|candidate| candidate.finish_reason.clone()),
    })
}

fn provider_finish_reason_indicates_truncation(reason: &str) -> bool {
    matches!(
        reason.to_ascii_uppercase().as_str(),
        "MAX_TOKENS" | "LENGTH"
    )
}

fn gemini_candidate_text(candidate: &GeminiCandidate) -> Option<&str> {
    candidate
        .content
        .as_ref()
        .and_then(|content| content.parts.first())
        .map(|part| part.text.as_str())
}

async fn call_openai(
    api_key: &str,
    model: &str,
    request: &TransformRequest,
) -> Result<LexiResultV1, AppError> {
    let client = reqwest_client()?;

    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&json!({
            "model": model,
            "messages": [
                {
                    "role": "system",
                    "content": "You return only strict, compact JSON for Lexi's word-study schema. Keep every field short and contrastive."
                },
                {
                    "role": "user",
                    "content": build_word_study_prompt(request)
                }
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "lexi_result_v1",
                    "strict": true,
                    "schema": lexi_result_schema()
                }
            }
        }))
        .send()
        .await
        .map_err(|error| {
            AppError::provider_request_failed(format!("OpenAI request failed: {error}"), true)
        })?;

    if !response.status().is_success() {
        return Err(AppError::provider_request_failed(
            format!("OpenAI request returned HTTP {}", response.status()),
            response.status().as_u16() == 429 || response.status().is_server_error(),
        ));
    }

    let payload = response
        .json::<OpenAiChatResponse>()
        .await
        .map_err(|error| {
            AppError::invalid_model_output(format!("OpenAI response parse failed: {error}"))
        })?;
    let content = payload
        .choices
        .first()
        .map(|choice| choice.message.content.as_str())
        .ok_or_else(|| AppError::invalid_model_output("OpenAI response had no choices"))?;

    parse_lexi_result_v1(content)
}

async fn call_gemini(
    api_key: &str,
    model: &str,
    request: &TransformRequest,
) -> Result<LexiResultV1, AppError> {
    let client = reqwest_client()?;
    let url =
        format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent");

    let response = client
        .post(url)
        .query(&[("key", api_key)])
        .json(&json!({
            "contents": [
                {
                    "role": "user",
                    "parts": [{ "text": build_word_study_prompt(request) }]
                }
            ],
            "generationConfig": {
                "temperature": 0.2,
                "maxOutputTokens": MAX_OUTPUT_TOKENS,
                "responseMimeType": "application/json",
                "responseSchema": gemini_lexi_result_schema()
            }
        }))
        .send()
        .await
        .map_err(|error| {
            AppError::provider_request_failed(format!("Gemini request failed: {error}"), true)
        })?;

    if !response.status().is_success() {
        return Err(AppError::provider_request_failed(
            format!("Gemini request returned HTTP {}", response.status()),
            response.status().as_u16() == 429 || response.status().is_server_error(),
        ));
    }

    let payload = response.json::<GeminiResponse>().await.map_err(|error| {
        AppError::invalid_model_output(format!("Gemini response parse failed: {error}"))
    })?;
    let content = payload
        .candidates
        .first()
        .and_then(gemini_candidate_text)
        .ok_or_else(|| AppError::invalid_model_output("Gemini response had no text part"))?;

    parse_lexi_result_v1(content)
}

fn gemini_lexi_result_schema() -> Value {
    json!({
        "type": "OBJECT",
        "required": [
            "schemaVersion",
            "mode",
            "sourceLanguage",
            "resultLanguage",
            "headword",
            "translations",
            "nuance",
            "synonyms",
            "warnings"
        ],
        "properties": {
            "schemaVersion": { "type": "STRING", "enum": [LEXI_RESULT_V1_SCHEMA_VERSION] },
            "mode": { "type": "STRING", "enum": ["word-study"] },
            "sourceLanguage": { "type": "STRING" },
            "resultLanguage": { "type": "STRING" },
            "headword": { "type": "STRING" },
            "translations": {
                "type": "ARRAY",
                "minItems": 1,
                "maxItems": 3,
                "items": {
                    "type": "OBJECT",
                    "required": ["text", "note", "example"],
                    "properties": {
                        "text": { "type": "STRING" },
                        "note": {
                            "type": "STRING",
                            "nullable": true,
                            "enum": TRANSLATION_NOTE_VALUES
                        },
                        "example": gemini_example_sentence_schema()
                    }
                }
            },
            "nuance": { "type": "STRING" },
            "synonyms": {
                "type": "ARRAY",
                "minItems": 0,
                "maxItems": 4,
                "items": gemini_related_word_schema()
            },
            "warnings": { "type": "ARRAY", "items": { "type": "STRING" } }
        }
    })
}

fn gemini_example_sentence_schema() -> Value {
    json!({
        "type": "OBJECT",
        "required": ["sentence", "japanese"],
        "properties": {
            "sentence": { "type": "STRING" },
            "japanese": { "type": "STRING" }
        }
    })
}

fn gemini_related_word_schema() -> Value {
    json!({
        "type": "OBJECT",
        "required": ["term", "japanese", "usageComparison"],
        "properties": {
            "term": { "type": "STRING" },
            "japanese": { "type": "STRING" },
            "usageComparison": { "type": "STRING" }
        }
    })
}

fn partial_from_json_fragment(fragment: &str) -> LexiPartialResult {
    if let Ok(result) = serde_json::from_str::<LexiResultV1>(fragment) {
        return LexiPartialResult::from_result(&result);
    }

    LexiPartialResult {
        headword: extract_string_field(fragment, "headword"),
        translations: extract_object_array::<Translation>(fragment, "translations"),
        nuance: extract_string_field(fragment, "nuance"),
        synonyms: extract_object_array::<RelatedWord>(fragment, "synonyms"),
        warnings: extract_string_array(fragment, "warnings"),
    }
}

fn extract_string_field(fragment: &str, field: &str) -> Option<String> {
    let pattern = format!("\"{field}\"");
    let field_index = fragment.find(&pattern)?;
    let after_field = &fragment[field_index + pattern.len()..];
    let colon_index = after_field.find(':')?;
    let after_colon = after_field[colon_index + 1..].trim_start();
    let string_end = complete_json_string_end(after_colon)?;
    serde_json::from_str::<String>(&after_colon[..string_end]).ok()
}

fn extract_object_array<T>(fragment: &str, field: &str) -> Vec<T>
where
    T: for<'de> Deserialize<'de>,
{
    let Some(array_content) = extract_array_prefix(fragment, field) else {
        return vec![];
    };

    complete_json_objects(array_content)
        .into_iter()
        .filter_map(|object| serde_json::from_str::<T>(&object).ok())
        .collect()
}

fn extract_string_array(fragment: &str, field: &str) -> Vec<String> {
    let Some(array_content) = extract_array_prefix(fragment, field) else {
        return vec![];
    };

    let mut strings = Vec::new();
    let mut rest = array_content.trim_start();
    while let Some(start) = rest.find('"') {
        let candidate = &rest[start..];
        let Some(end) = complete_json_string_end(candidate) else {
            break;
        };
        if let Ok(value) = serde_json::from_str::<String>(&candidate[..end]) {
            strings.push(value);
        }
        rest = &candidate[end..];
    }

    strings
}

fn extract_array_prefix<'a>(fragment: &'a str, field: &str) -> Option<&'a str> {
    let pattern = format!("\"{field}\"");
    let field_index = fragment.find(&pattern)?;
    let after_field = &fragment[field_index + pattern.len()..];
    let colon_index = after_field.find(':')?;
    let after_colon = after_field[colon_index + 1..].trim_start();
    let array_start = after_colon.find('[')?;
    Some(&after_colon[array_start + 1..])
}

fn complete_json_objects(array_content: &str) -> Vec<String> {
    let mut objects = Vec::new();
    let mut start = None;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (index, ch) in array_content.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    start = Some(index);
                }
                depth += 1;
            }
            '}' => {
                if depth == 0 {
                    continue;
                }
                depth -= 1;
                if depth == 0 {
                    if let Some(start_index) = start.take() {
                        objects.push(array_content[start_index..=index].to_string());
                    }
                }
            }
            ']' if depth == 0 => break,
            _ => {}
        }
    }

    objects
}

fn complete_json_string_end(value: &str) -> Option<usize> {
    if !value.starts_with('"') {
        return None;
    }

    let mut escaped = false;
    for (index, ch) in value.char_indices().skip(1) {
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some(index + ch.len_utf8());
        }
    }

    None
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChunk {
    choices: Vec<OpenAiStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChoice {
    delta: OpenAiStreamDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamDelta {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModel>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModel {
    id: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiModelsResponse {
    models: Vec<GeminiModel>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiModel {
    name: String,
    display_name: Option<String>,
    supported_generation_methods: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiCandidate {
    content: Option<GeminiContent>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Deserialize)]
struct GeminiPart {
    text: String,
}

#[cfg(test)]
mod tests {
    use super::{
        build_word_study_prompt, gemini_lexi_result_schema, lexi_result_schema, mock_headword,
        parse_gemini_stream_text, parse_openai_stream_text, parse_sse_event_text, pop_sse_event,
        provider_finish_reason_indicates_truncation, selected_text_preview, sse_data_payload,
    };

    #[test]
    fn sse_parser_splits_crlf_delimited_events() {
        let mut buffer = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"{\\\"headword\\\":\"}}]}\r\n\r\n",
            "data: [DONE]\r\n\r\n"
        )
        .to_string();

        let first = pop_sse_event(&mut buffer).expect("first event");
        assert_eq!(
            parse_sse_event_text(&first, parse_openai_stream_text).expect("first event parses"),
            super::StreamTextDelta {
                text: Some("{\"headword\":".to_string()),
                finish_reason: None,
            }
        );

        let second = pop_sse_event(&mut buffer).expect("done event");
        assert_eq!(
            parse_sse_event_text(&second, parse_openai_stream_text).expect("done event parses"),
            super::StreamTextDelta::default()
        );
        assert!(buffer.is_empty());
    }

    #[test]
    fn sse_parser_splits_lf_delimited_events() {
        let mut buffer = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"steady\"}}]}\n\n",
            "data: [DONE]\n\n"
        )
        .to_string();

        let first = pop_sse_event(&mut buffer).expect("first event");
        assert_eq!(
            parse_sse_event_text(&first, parse_openai_stream_text).expect("first event parses"),
            super::StreamTextDelta {
                text: Some("steady".to_string()),
                finish_reason: None,
            }
        );
        assert!(pop_sse_event(&mut buffer).is_some());
        assert!(buffer.is_empty());
    }

    #[test]
    fn sse_data_payload_joins_multiple_data_lines() {
        let event = "event: message\ndata: {\"a\":\ndata: 1}";
        assert_eq!(sse_data_payload(event), "{\"a\":\n1}");
    }

    #[test]
    fn openai_stream_parser_keeps_finish_reason() {
        let data = r#"{"choices":[{"delta":{},"finish_reason":"length"}]}"#;
        let delta = parse_openai_stream_text(data).expect("finish event parses");
        assert_eq!(delta.text, None);
        assert_eq!(delta.finish_reason, Some("length".to_string()));
    }

    #[test]
    fn gemini_stream_parser_keeps_finish_reason_without_content() {
        let data = r#"{"candidates":[{"finishReason":"MAX_TOKENS"}]}"#;
        let delta = parse_gemini_stream_text(data).expect("finish event parses");
        assert_eq!(delta.text, None);
        assert_eq!(delta.finish_reason, Some("MAX_TOKENS".to_string()));
    }

    #[test]
    fn provider_finish_reason_detects_truncation() {
        assert!(provider_finish_reason_indicates_truncation("MAX_TOKENS"));
        assert!(provider_finish_reason_indicates_truncation("length"));
        assert!(!provider_finish_reason_indicates_truncation("STOP"));
    }

    #[test]
    fn openai_schema_matches_result_validation_cardinality() {
        let schema = lexi_result_schema();

        assert_eq!(schema["properties"]["translations"]["minItems"], 1);
        assert_eq!(schema["properties"]["translations"]["maxItems"], 3);
        assert_eq!(
            schema["properties"]["translations"]["items"]["required"][2],
            "example"
        );
        assert_eq!(
            schema["properties"]["translations"]["items"]["properties"]["note"]["enum"][0],
            "名詞"
        );
        assert!(
            schema["properties"]["translations"]["items"]["properties"]["note"]["enum"]
                .as_array()
                .expect("note enum")
                .contains(&serde_json::Value::Null)
        );
        assert_eq!(schema["properties"]["synonyms"]["minItems"], 0);
        assert_eq!(schema["properties"]["synonyms"]["maxItems"], 4);
        assert!(!schema["$defs"]["relatedWord"]["required"]
            .as_array()
            .expect("related word required")
            .contains(&serde_json::Value::String("nuance".to_string())));
    }

    #[test]
    fn gemini_schema_matches_result_validation_cardinality() {
        let schema = gemini_lexi_result_schema();

        assert_eq!(schema["properties"]["translations"]["minItems"], 1);
        assert_eq!(schema["properties"]["translations"]["maxItems"], 3);
        assert_eq!(
            schema["properties"]["translations"]["items"]["required"][2],
            "example"
        );
        assert_eq!(
            schema["properties"]["translations"]["items"]["properties"]["note"]["enum"][0],
            "名詞"
        );
        assert_eq!(schema["properties"]["synonyms"]["minItems"], 0);
        assert_eq!(schema["properties"]["synonyms"]["maxItems"], 4);
        assert!(!schema["properties"]["synonyms"]["items"]["required"]
            .as_array()
            .expect("related word required")
            .contains(&serde_json::Value::String("nuance".to_string())));
    }

    #[test]
    fn prompt_requires_single_word_headword_lemma() {
        let prompt = build_word_study_prompt(&super::TransformRequest {
            selected_text: "went".to_string(),
            result_language: "ja".to_string(),
            prompt_mode: "word-study".to_string(),
        });

        assert!(prompt.contains("single inflected word"));
        assert!(prompt.contains("went -> go"));
        assert!(prompt.contains("dictionary/base form"));
        assert!(prompt.contains("part-of-speech label"));
        assert!(prompt.contains("数学"));
        assert!(prompt.contains("dictionary-style Japanese sense entries"));
        assert!(prompt.contains("part of speech, countable vs uncountable use"));
        assert!(prompt.contains("not a list of alternative Japanese renderings"));
        assert!(prompt.contains("distinct English dictionary sense"));
        assert!(prompt.contains("adoption"));
        assert!(prompt.contains("採用"));
        assert!(prompt.contains("採択"));
        assert!(prompt.contains("demonstration"));
        assert!(prompt.contains("デモ"));
        assert!(prompt.contains("実演"));
        assert!(prompt.contains("not a Japanese synonym list"));
        assert!(prompt.contains("Different Japanese word choice alone is never enough"));
        assert!(prompt.contains("Do not create multiple entries by rephrasing the same sense"));
        assert!(prompt.contains("近づく"));
        assert!(prompt.contains("接近する"));
        assert!(prompt.contains("Merge or delete overlapping Japanese meanings"));
        assert!(prompt.contains("example is required for every translation item"));
    }

    #[test]
    fn mock_provider_displays_base_form_for_common_inflections() {
        assert_eq!(mock_headword("went"), "go");
        assert_eq!(mock_headword("studied"), "study");
        assert_eq!(mock_headword("walked"), "walk");
    }

    #[test]
    fn selected_text_preview_collapses_whitespace_and_truncates() {
        assert_eq!(selected_text_preview("  subtle\nchange  "), "subtle change");
        assert_eq!(selected_text_preview(&"a".repeat(50)).chars().count(), 48);
    }

    #[test]
    fn started_event_serializes_selected_text_preview() {
        let value = serde_json::to_value(super::TransformEvent::Started {
            request_id: 7,
            selected_text_preview: "subtle".to_string(),
            shortcut: "Ctrl+Shift+X".to_string(),
            capture_method: "uia-foreground-window",
            source_process: Some("notepad.exe".to_string()),
            source_window_title: None,
            character_count: 6,
            multiline: false,
            provider: crate::settings::ProviderKind::Mock,
            model: "mock-word-study".to_string(),
        })
        .expect("started event serializes");

        assert_eq!(value["status"], "started");
        assert_eq!(value["selectedTextPreview"], "subtle");
    }
}
