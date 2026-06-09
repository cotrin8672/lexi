use crate::{
    errors::AppError,
    schema::{
        parse_japanese_word_candidates_result_v1, parse_lexi_result_v1, CandidateConfidence,
        CandidateExample, EnglishCandidate, ExampleSentence, Idiom, Inflection,
        JapaneseWordCandidatesResultV1, LexiResult, LexiResultV1, RelatedWord,
        TextTranslationResultV1, Translation, TranslationSegment,
        LEXI_JP_WORD_CANDIDATES_V1_SCHEMA_VERSION, LEXI_RESULT_V1_SCHEMA_VERSION,
        LEXI_TEXT_TRANSLATION_V1_SCHEMA_VERSION, TRANSLATION_NOTE_VALUES,
    },
    secrets,
    settings::{ProviderKind, SettingsState},
    sync, vocabulary,
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

fn finalize_word_study_result(
    mut result: LexiResultV1,
    result_language: &str,
) -> LexiResultV1 {
    result.result_language = result_language.trim().to_string();
    result.source_language = "en".to_string();
    result
}

fn persist_word_study_result(
    app: &AppHandle,
    result: &LexiResultV1,
    provider: ProviderKind,
    model: &str,
    selected_text: &str,
) {
    if vocabulary::save_word_study_result(app, result, provider, model, selected_text).is_ok() {
        sync::schedule_sync(app.clone());
    }
}

fn persist_japanese_word_candidates_result(
    app: &AppHandle,
    result: &JapaneseWordCandidatesResultV1,
    provider: ProviderKind,
    model: &str,
    selected_text: &str,
) {
    if vocabulary::save_japanese_word_candidates_result(app, result, provider, model, selected_text)
        .is_ok()
    {
        sync::schedule_sync(app.clone());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransformMode {
    WordStudy,
    TextTranslation,
    JapaneseWordCandidates,
}

fn transform_mode_label(mode: TransformMode) -> &'static str {
    match mode {
        TransformMode::WordStudy => "word-study",
        TransformMode::TextTranslation => "text-translation",
        TransformMode::JapaneseWordCandidates => "jp-word-candidates",
    }
}

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
    pub result: LexiResult,
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
    pub query: Option<String>,
    pub candidates: Vec<EnglishCandidate>,
    pub headword: Option<String>,
    pub inflections: Vec<Inflection>,
    pub translations: Vec<Translation>,
    pub nuance: Option<String>,
    pub synonyms: Vec<RelatedWord>,
    pub idioms: Vec<Idiom>,
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
        selected_text: Option<String>,
        shortcut: String,
        capture_method: &'static str,
        source_process: Option<String>,
        source_window_title: Option<String>,
        character_count: usize,
        multiline: bool,
        transform_mode: &'static str,
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
        result: LexiResult,
        provider: ProviderKind,
        model: String,
    },
    Failed {
        request_id: u64,
        error: AppError,
    },
}

impl LexiPartialResult {
    fn from_word_study_result(result: &LexiResultV1) -> Self {
        Self {
            headword: Some(result.headword.clone()),
            inflections: result.inflections.clone(),
            translations: result.translations.clone(),
            nuance: Some(result.nuance.clone()),
            synonyms: result.synonyms.clone(),
            idioms: result.idioms.clone(),
            warnings: result.warnings.clone(),
            ..Self::default()
        }
    }

    fn from_japanese_word_candidates_result(result: &JapaneseWordCandidatesResultV1) -> Self {
        Self {
            query: Some(result.query.clone()),
            candidates: result.candidates.clone(),
            warnings: result.warnings.clone(),
            ..Self::default()
        }
    }

    fn is_empty(&self) -> bool {
        self.query.is_none()
            && self.candidates.is_empty()
            && self.headword.is_none()
            && self.inflections.is_empty()
            && self.translations.is_empty()
            && self.nuance.is_none()
            && self.synonyms.is_empty()
            && self.idioms.is_empty()
            && self.warnings.is_empty()
    }
}

pub trait LlmProvider {
    fn transform(&self, request: &TransformRequest) -> Result<LexiResult, AppError>;
}

pub struct MockProvider;

impl LlmProvider for MockProvider {
    fn transform(&self, request: &TransformRequest) -> Result<LexiResult, AppError> {
        let mode = classify_transform_mode(&request.selected_text);
        if mode == TransformMode::TextTranslation {
            return Err(AppError::provider_not_configured(
                "Mock provider does not support text translation",
            ));
        }
        if mode == TransformMode::JapaneseWordCandidates {
            return Ok(LexiResult::JapaneseWordCandidates(
                mock_japanese_word_candidates(&request.selected_text, &request.result_language),
            ));
        }

        Ok(LexiResult::WordStudy(LexiResultV1 {
            schema_version: LEXI_RESULT_V1_SCHEMA_VERSION.to_string(),
            mode: "word-study".to_string(),
            source_language: "auto".to_string(),
            result_language: request.result_language.clone(),
            headword: mock_headword(&request.selected_text),
            inflections: mock_inflections(&request.selected_text),
            translations: vec![crate::schema::Translation {
                text: "確認用の訳語".to_string(),
                note: None,
                example: ExampleSentence {
                    sentence: "This is a short example from the mock provider.".to_string(),
                    japanese: "これはモックプロバイダーによる短い例文です。".to_string(),
                },
                sense_kind: None,
                base_word: None,
            }],
            nuance: "MockProvider による構造化レスポンスです。".to_string(),
            synonyms: vec![],
            idioms: vec![Idiom {
                idiom: "in practice".to_string(),
                japanese: "in practice".to_string(),
                example: "In practice, the mock provider returns fixed data.".to_string(),
            }],
            warnings: vec![
                "Provider 設定が mock のため、実際の API は呼び出していません。".to_string(),
            ],
        }))
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

fn mock_inflections(selected_text: &str) -> Vec<Inflection> {
    match mock_headword(selected_text).as_str() {
        "go" => vec![
            Inflection {
                kind: "past".to_string(),
                form: "went".to_string(),
            },
            Inflection {
                kind: "pastParticiple".to_string(),
                form: "gone".to_string(),
            },
        ],
        "run" => vec![
            Inflection {
                kind: "past".to_string(),
                form: "ran".to_string(),
            },
            Inflection {
                kind: "pastParticiple".to_string(),
                form: "run".to_string(),
            },
        ],
        _ => vec![],
    }
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

fn contains_japanese_script(text: &str) -> bool {
    text.chars().any(|character| {
        matches!(
            character,
            '\u{3040}'..='\u{309F}'
                | '\u{30A0}'..='\u{30FF}'
                | '\u{3400}'..='\u{4DBF}'
                | '\u{4E00}'..='\u{9FFF}'
                | '\u{F900}'..='\u{FAFF}'
        )
    })
}

fn non_whitespace_char_count(text: &str) -> usize {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .count()
}

fn is_sentence_like_selection(selected_text: &str) -> bool {
    let trimmed = selected_text.trim();
    if trimmed.is_empty() {
        return false;
    }

    let token_count = trimmed.split_whitespace().count();
    let has_newline = trimmed.contains('\n') || trimmed.contains('\r');
    let has_sentence_terminal = trimmed
        .chars()
        .any(|character| matches!(character, '.' | '?' | '!' | '。' | '？' | '！'));
    let has_clause_punctuation = trimmed
        .chars()
        .any(|character| matches!(character, ',' | ';' | ':' | '、' | '，' | '；' | '：'));

    has_newline
        || has_sentence_terminal
        || has_clause_punctuation
        || token_count >= 5
        || (contains_japanese_script(trimmed) && non_whitespace_char_count(trimmed) > 32)
}

fn classify_transform_mode(selected_text: &str) -> TransformMode {
    let trimmed = selected_text.trim();
    if trimmed.is_empty() {
        return TransformMode::WordStudy;
    }

    if is_sentence_like_selection(trimmed) {
        return TransformMode::TextTranslation;
    }

    if contains_japanese_script(trimmed) {
        TransformMode::JapaneseWordCandidates
    } else {
        TransformMode::WordStudy
    }
}

fn provider_for_mode(settings_provider: ProviderKind, mode: TransformMode) -> ProviderKind {
    match mode {
        TransformMode::WordStudy | TransformMode::JapaneseWordCandidates => settings_provider,
        TransformMode::TextTranslation => ProviderKind::DeepL,
    }
}

fn model_for_mode(settings_model: &str, mode: TransformMode) -> &str {
    match mode {
        TransformMode::WordStudy | TransformMode::JapaneseWordCandidates => settings_model,
        TransformMode::TextTranslation => ProviderKind::DeepL.default_model(),
    }
}

fn result_language_for_mode(settings_result_language: &str, mode: TransformMode) -> String {
    match mode {
        TransformMode::JapaneseWordCandidates => "en".to_string(),
        TransformMode::WordStudy | TransformMode::TextTranslation => {
            settings_result_language.to_string()
        }
    }
}

fn mock_japanese_word_candidates(
    selected_text: &str,
    result_language: &str,
) -> JapaneseWordCandidatesResultV1 {
    let query = normalize_japanese_query(selected_text);
    let candidates = match query.as_str() {
        "採用" => vec![
            mock_english_candidate(
                "adopt",
                "動詞",
                "方針・方法・制度などを選んで使い始める",
                "案や制度を公式に取り入れる文脈で使う。",
                "The team adopted a new policy.",
                "チームは新しい方針を採用した。",
                CandidateConfidence::High,
            ),
            mock_english_candidate(
                "hire",
                "動詞",
                "人を雇う",
                "人材を採用する文脈で使う。",
                "They hired a new engineer.",
                "新しいエンジニアを採用した。",
                CandidateConfidence::High,
            ),
            mock_english_candidate(
                "employ",
                "動詞",
                "雇用する",
                "組織が人を使う一般的な文脈で使う。",
                "The company employs 200 people.",
                "その会社は200人を雇用している。",
                CandidateConfidence::Medium,
            ),
            mock_english_candidate(
                "accept",
                "動詞",
                "受け入れる",
                "提案や条件を承認する文脈で使う。",
                "The board accepted the plan.",
                "取締役会はその案を採用した。",
                CandidateConfidence::Medium,
            ),
        ],
        "微妙" => vec![
            mock_english_candidate(
                "subtle",
                "形容詞",
                "気づきにくいほど繊細な",
                "変化や違いが小さいときに使う。",
                "There was a subtle change in tone.",
                "口調に微妙な変化があった。",
                CandidateConfidence::High,
            ),
            mock_english_candidate(
                "questionable",
                "形容詞",
                "疑わしい・微妙な",
                "良し悪しがはっきりしない評価で使う。",
                "That decision looks questionable.",
                "その判断は微妙に見える。",
                CandidateConfidence::Medium,
            ),
            mock_english_candidate(
                "awkward",
                "形容詞",
                "気まずい・ぎこちない",
                "人間関係や雰囲気がぎこちないときに使う。",
                "The silence felt awkward.",
                "沈黙が気まずく感じられた。",
                CandidateConfidence::Medium,
            ),
            mock_english_candidate(
                "delicate",
                "形容詞",
                "扱いにくい・繊細な",
                "状況や話題がデリケートなときに使う。",
                "It is a delicate situation.",
                "それは微妙な状況だ。",
                CandidateConfidence::Low,
            ),
        ],
        _ => vec![mock_english_candidate(
            "example",
            "名詞",
            "例として示す語",
            "モック用の汎用候補です。",
            "This is an example sentence.",
            "これは例文です。",
            CandidateConfidence::Medium,
        )],
    };

    let warnings = if query == "微妙" {
        vec!["文脈によって最適な英語語が大きく変わります。".to_string()]
    } else {
        vec!["Provider 設定が mock のため、実際の API は呼び出していません。".to_string()]
    };

    JapaneseWordCandidatesResultV1 {
        schema_version: LEXI_JP_WORD_CANDIDATES_V1_SCHEMA_VERSION.to_string(),
        mode: "jp-word-candidates".to_string(),
        source_language: "ja".to_string(),
        result_language: result_language.to_string(),
        query,
        candidates,
        warnings,
    }
}

fn normalize_japanese_query(selected_text: &str) -> String {
    selected_text.trim().chars().take(32).collect()
}

fn mock_english_candidate(
    term: &str,
    part_of_speech: &str,
    japanese_nuance: &str,
    usage_note: &str,
    sentence: &str,
    japanese: &str,
    confidence: CandidateConfidence,
) -> EnglishCandidate {
    EnglishCandidate {
        term: term.to_string(),
        part_of_speech: part_of_speech.to_string(),
        japanese_nuance: japanese_nuance.to_string(),
        usage_note: usage_note.to_string(),
        example: CandidateExample {
            sentence: sentence.to_string(),
            japanese: japanese.to_string(),
        },
        confidence,
    }
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
    if provider == ProviderKind::Mock || provider == ProviderKind::DeepL {
        return Ok(ProviderModelsResult {
            provider,
            models: fallback_models(provider),
            fetched: true,
            warning: None,
        });
    }

    let api_key = match settings_state.api_key(&app, provider) {
        Ok(Some(key)) if !key.trim().is_empty() => key,
        Ok(_) => {
            return Ok(ProviderModelsResult {
                provider,
                models: fallback_models(provider),
                fetched: false,
                warning: Some("API key is not configured; showing default models.".to_string()),
            });
        }
        Err(error) => {
            return Ok(ProviderModelsResult {
                provider,
                models: fallback_models(provider),
                fetched: false,
                warning: Some(error.user_message),
            });
        }
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
            warning: Some(error.user_message),
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
    let transform_mode = classify_transform_mode(&selected_text);
    let request = TransformRequest {
        selected_text,
        result_language: result_language_for_mode(&settings.result_language, transform_mode),
        prompt_mode: settings.prompt_mode.clone(),
    };

    let _ = app.emit(
        TRANSFORM_EVENT,
        TransformEvent::Started {
            request_id,
            selected_text_preview,
            selected_text: if transform_mode == TransformMode::TextTranslation {
                Some(request.selected_text.clone())
            } else {
                None
            },
            shortcut: capture.shortcut,
            capture_method: capture.capture_method,
            source_process: capture.source_process,
            source_window_title: capture.source_window_title,
            character_count: capture.character_count,
            multiline: capture.multiline,
            transform_mode: transform_mode_label(transform_mode),
            provider: provider_for_mode(settings.provider, transform_mode),
            model: model_for_mode(&settings.model, transform_mode).to_string(),
        },
    );

    if transform_mode == TransformMode::TextTranslation {
        let api_key = secrets::read_api_key(ProviderKind::DeepL)?
            .filter(|key| !key.trim().is_empty())
            .ok_or_else(|| AppError::provider_not_configured("DeepL API key is not configured"))?;
        let result = call_deepl_text_translation(&api_key, &request).await?;
        let _ = app.emit(
            TRANSFORM_EVENT,
            TransformEvent::Ready {
                request_id,
                result: LexiResult::TextTranslation(result),
                provider: ProviderKind::DeepL,
                model: ProviderKind::DeepL.default_model().to_string(),
            },
        );
        return Ok(());
    }

    if settings.provider == ProviderKind::DeepL {
        return Err(AppError::provider_not_configured(
            "DeepL only supports text translation; choose Gemini or OpenAI for word study and Japanese lookup",
        ));
    }

    if transform_mode == TransformMode::JapaneseWordCandidates {
        if let Ok(Some(result)) = vocabulary::load_japanese_word_candidates(
            &app,
            &request.selected_text,
            &request.result_language,
        )
        .await
        {
            let _ = app.emit(
                TRANSFORM_EVENT,
                TransformEvent::Ready {
                    request_id,
                    result: LexiResult::JapaneseWordCandidates(result),
                    provider: settings.provider,
                    model: settings.model,
                },
            );
            return Ok(());
        }
    } else if let Ok(Some(result)) =
        vocabulary::load_word_study(&app, &request.selected_text, &request.result_language).await
    {
        let _ = app.emit(
            TRANSFORM_EVENT,
            TransformEvent::Ready {
                request_id,
                result: LexiResult::WordStudy(result),
                provider: settings.provider,
                model: settings.model,
            },
        );
        return Ok(());
    }

    if settings.provider == ProviderKind::Mock {
        let result = MockProvider.transform(&request)?;
        return emit_mock_stream_result(
            &app,
            request_id,
            result,
            settings.provider,
            &settings.model,
            &request.selected_text,
            &request.result_language,
        );
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
            call_gemini_stream(
                &app,
                request_id,
                &api_key,
                &settings.model,
                &request,
                transform_mode,
            )
            .await?
        }
        ProviderKind::OpenAi => {
            call_openai_stream(
                &app,
                request_id,
                &api_key,
                &settings.model,
                &request,
                transform_mode,
            )
            .await?
        }
        ProviderKind::Mock => unreachable!("mock provider returned above"),
        ProviderKind::DeepL => unreachable!("DeepL text translation returned above"),
    };
    let partial = partial_from_json_fragment(&raw_json, transform_mode);
    let _ = app.emit(
        TRANSFORM_EVENT,
        TransformEvent::Validating {
            request_id,
            partial,
        },
    );

    match transform_mode {
        TransformMode::JapaneseWordCandidates => {
            let result = parse_japanese_word_candidates_result_v1(&raw_json)?;
            persist_japanese_word_candidates_result(
                &app,
                &result,
                settings.provider,
                &settings.model,
                &request.selected_text,
            );
            let _ = app.emit(
                TRANSFORM_EVENT,
                TransformEvent::Ready {
                    request_id,
                    result: LexiResult::JapaneseWordCandidates(result),
                    provider: settings.provider,
                    model: settings.model,
                },
            );
        }
        TransformMode::WordStudy => {
            let result = finalize_word_study_result(
                parse_lexi_result_v1(&raw_json)?,
                &request.result_language,
            );
            persist_word_study_result(
                &app,
                &result,
                settings.provider,
                &settings.model,
                &request.selected_text,
            );
            let _ = app.emit(
                TRANSFORM_EVENT,
                TransformEvent::Ready {
                    request_id,
                    result: LexiResult::WordStudy(result),
                    provider: settings.provider,
                    model: settings.model,
                },
            );
        }
        TransformMode::TextTranslation => unreachable!("text translation returned above"),
    }

    Ok(())
}

fn emit_mock_stream_result(
    app: &AppHandle,
    request_id: u64,
    result: LexiResult,
    provider: ProviderKind,
    model: &str,
    selected_text: &str,
    result_language: &str,
) -> Result<(), AppError> {
    match result {
        LexiResult::WordStudy(word_result) => {
            let word_result = finalize_word_study_result(word_result, result_language);
            let partial = LexiPartialResult::from_word_study_result(&word_result);
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
                    result: LexiResult::WordStudy(word_result.clone()),
                    provider,
                    model: model.to_string(),
                },
            );
            persist_word_study_result(app, &word_result, provider, model, selected_text);
        }
        LexiResult::JapaneseWordCandidates(ja_result) => {
            let partial = LexiPartialResult::from_japanese_word_candidates_result(&ja_result);
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
                    result: LexiResult::JapaneseWordCandidates(ja_result.clone()),
                    provider,
                    model: model.to_string(),
                },
            );
            persist_japanese_word_candidates_result(
                app,
                &ja_result,
                provider,
                model,
                selected_text,
            );
        }
        LexiResult::TextTranslation(_) => {
            return Err(AppError::provider_not_configured(
                "Mock provider does not support text translation",
            ));
        }
    }

    Ok(())
}

impl TryFrom<TransformCaptureInput> for TransformCaptureMetadata {
    type Error = AppError;

    fn try_from(input: TransformCaptureInput) -> Result<Self, Self::Error> {
        let capture_method = match input.capture_method.as_str() {
            "clipboard-copy" => "clipboard-copy",
            "uia-focused-element" => "uia-focused-element",
            "uia-foreground-window" => "uia-foreground-window",
            "clipboard-fallback" => "clipboard-fallback",
            _ => {
                return Err(AppError::invalid_model_output(
                    "retry capture metadata had an unknown capture method",
                ));
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
    let selected_text = selected_text_state.current()?;
    let transform_mode = classify_transform_mode(&selected_text);
    let request = TransformRequest {
        selected_text,
        result_language: result_language_for_mode(&settings.result_language, transform_mode),
        prompt_mode: settings.prompt_mode.clone(),
    };

    if transform_mode == TransformMode::TextTranslation {
        let api_key = secrets::read_api_key(ProviderKind::DeepL)?
            .filter(|key| !key.trim().is_empty())
            .ok_or_else(|| AppError::provider_not_configured("DeepL API key is not configured"))?;
        let result = call_deepl_text_translation(&api_key, &request).await?;
        return Ok(TransformResult {
            result: LexiResult::TextTranslation(result),
            provider: ProviderKind::DeepL,
            model: ProviderKind::DeepL.default_model().to_string(),
        });
    }

    if settings.provider == ProviderKind::DeepL {
        return Err(AppError::provider_not_configured(
            "DeepL only supports text translation; choose Gemini or OpenAI for word study and Japanese lookup",
        ));
    }

    if transform_mode == TransformMode::JapaneseWordCandidates {
        if let Ok(Some(result)) = vocabulary::load_japanese_word_candidates(
            &app,
            &request.selected_text,
            &request.result_language,
        )
        .await
        {
            return Ok(TransformResult {
                result: LexiResult::JapaneseWordCandidates(result),
                provider: settings.provider,
                model: settings.model,
            });
        }
    } else if let Ok(Some(result)) =
        vocabulary::load_word_study(&app, &request.selected_text, &request.result_language).await
    {
        return Ok(TransformResult {
            result: LexiResult::WordStudy(result),
            provider: settings.provider,
            model: settings.model,
        });
    }

    if settings.provider == ProviderKind::Mock {
        let result = MockProvider.transform(&request)?;
        match &result {
            LexiResult::WordStudy(word_result) => persist_word_study_result(
                &app,
                &finalize_word_study_result(word_result.clone(), &request.result_language),
                settings.provider,
                &settings.model,
                &request.selected_text,
            ),
            LexiResult::JapaneseWordCandidates(ja_result) => {
                persist_japanese_word_candidates_result(
                    &app,
                    ja_result,
                    settings.provider,
                    &settings.model,
                    &request.selected_text,
                )
            }
            LexiResult::TextTranslation(_) => {}
        }
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

    let lexi_result = match transform_mode {
        TransformMode::JapaneseWordCandidates => {
            let result = match settings.provider {
                ProviderKind::Gemini => {
                    call_gemini_japanese_word_candidates(&api_key, &settings.model, &request)
                        .await?
                }
                ProviderKind::OpenAi => {
                    call_openai_japanese_word_candidates(&api_key, &settings.model, &request)
                        .await?
                }
                ProviderKind::Mock => unreachable!("mock provider returned above"),
                ProviderKind::DeepL => unreachable!("DeepL text translation returned above"),
            };
            persist_japanese_word_candidates_result(
                &app,
                &result,
                settings.provider,
                &settings.model,
                &request.selected_text,
            );
            LexiResult::JapaneseWordCandidates(result)
        }
        TransformMode::WordStudy => {
            let result = match settings.provider {
                ProviderKind::Gemini => call_gemini(&api_key, &settings.model, &request).await?,
                ProviderKind::OpenAi => call_openai(&api_key, &settings.model, &request).await?,
                ProviderKind::Mock => unreachable!("mock provider returned above"),
                ProviderKind::DeepL => unreachable!("DeepL text translation returned above"),
            };
            persist_word_study_result(
                &app,
                &result,
                settings.provider,
                &settings.model,
                &request.selected_text,
            );
            LexiResult::WordStudy(result)
        }
        TransformMode::TextTranslation => unreachable!("text translation returned above"),
    };

    Ok(TransformResult {
        result: lexi_result,
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
- sourceLanguage must be "en" and resultLanguage must be "{result_language}".
- Use resultLanguage "{result_language}" for all explanations and meaning fields.
- Keep the result compact enough for a small desktop popup.
- If the selection is a single inflected word with no independent dictionary meaning, set headword to its dictionary/base form, not the selected surface form. Examples: went -> go, ran -> run, playing -> play, studied -> study, quantified -> quantify.
- If the selected form is also a standalone dictionary word (for example saw as a tool, left as a direction), keep that form as headword and return dictionary senses for that headword. Mention ambiguity in warnings when useful.
- If the selection is a sentence, choose the central word or phrase as headword and normalize that headword to its dictionary/base form when possible.
- Return inflections only for irregular noun plural forms or irregular verb past/past participle forms. Use an empty inflections array for regular forms, adjectives, adverbs, phrases without a clear headword, or uncertain data.
- If reliable synonyms are unavailable, use an empty array instead of guessing.
- If useful simple idioms, phrasal verbs, or short collocations containing the headword are unavailable, use an empty idioms array instead of guessing.

Field contract:
- headword: canonical dictionary/base form or short phrase, max 48 characters. Do not copy an inflected selected word such as a past-tense verb when a base form is known.
- inflections: 0 to 3 irregular forms for the headword only.
  - kind must be exactly "plural", "past", or "pastParticiple".
  - form is the irregular English form only, max 48 characters.
  - Include noun plural only when irregular, for example child -> children or mouse -> mice. Do not include regular plurals like books.
  - Include verb past and/or past participle only when irregular, for example go -> went/gone, see -> saw/seen, or write -> wrote/written. Do not include regular forms like studied/studied.
  - For common irregular verbs, never leave inflections empty when the selected or headword base form clearly has irregular forms; for example see must include saw and seen.
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
  - Do not include senseKind or baseWord in translation items. Translation items are dictionary senses only; the headword must already be normalized to the base form when the selection is an inflected word.
  - example is required for every translation item and must demonstrate that specific sense.
    - example.sentence: a simple English sentence, max 96 characters. Prefer common daily contexts and do not quote sensitive selected text unless necessary.
    - example.japanese: natural Japanese translation of example.sentence, max 96 characters.
- nuance: exactly 1 sentence, max 90 Japanese characters or 22 English words. Give an intuitive explanation that helps the user decide when the headword is appropriate.
- synonyms: 2 to 4 near words that are useful for learning how to use the headword more precisely. Do not include antonyms.
  - term: a real common near word.
  - japanese: concise meaning.
  - usageComparison: one direct sentence comparing the synonym with the headword. Explain when to choose the headword and when to choose this synonym, max 110 Japanese characters.
- idioms: 0 to 3 simple, high-frequency idioms, phrasal verbs, or short fixed collocations that contain or strongly feature the headword. These are compact learner-facing pattern labels, not full sentence templates or ornate set phrases.
  - Prefer the shortest established expression that still teaches natural usage. For headword "tend", return "tend to", not "tend to one's needs" or other padded expansions.
  - idiom: the minimal English collocation or idiom only, max 64 characters. Do not add possessives, objects, or extra words unless they are inseparable from the established expression. Good: "tend to", "look up", "in spite of". Bad: "tend to one's needs", "look up the answer in the dictionary".
  - Phrasal-verb style entries are preferred over proverb-like or overly specific fixed expressions.
  - japanese: concise Japanese meaning of that minimal expression, max 64 characters.
  - example: one short natural English example sentence using the idiom, max 120 characters. The example sentence may be longer than the idiom label, but the idiom field itself must stay minimal.
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
- Do not pad idioms. Return only established expressions; do not invent examples as idiom labels. Do not inflate a simple collocation into a longer ornate idiom. If "tend to" is enough, do not output "tend to someone's needs".
- Do not pad inflections. Regular forms must be omitted.
- Do not pad arrays to hit counts.
- Preserve selected text privacy; never quote more than needed for headword/examples.

Selected text:
{text}"#,
        schema_version = LEXI_RESULT_V1_SCHEMA_VERSION,
        result_language = request.result_language,
        text = request.selected_text,
    )
}

fn build_japanese_word_candidates_prompt(request: &TransformRequest) -> String {
    format!(
        r#"You are Lexi's Japanese-to-English word lookup formatter. Analyze the selected Japanese word or short phrase and return one compact JSON object only.

Hard requirements:
- Output must match schemaVersion "{schema_version}" and mode "jp-word-candidates".
- Do not include markdown, prose outside JSON, comments, or code fences.
- sourceLanguage must be "ja" and resultLanguage must be "{result_language}".
- query must be the normalized Japanese lookup term from the selection, max 32 characters.
- candidates must contain 1 to 8 English word or short phrase options. Prefer 3 to 6 useful candidates.
- Do not output Japanese translations as candidates.
- Do not list inflected English forms as separate candidates; use lemmas.
- Merge duplicates by lemma and sense unless register is the useful distinction.
- Rank candidates by practical usefulness for a Japanese user choosing an English word.
- Every candidate must include example.sentence (natural English using that candidate) and example.japanese (its Japanese translation).
- Keep examples generic; do not quote surrounding selected context.
- confidence must be exactly "high", "medium", or "low".
- warnings: empty unless the Japanese query is context-dependent, ambiguous, slang-like, or too short to rank confidently.

Field contract:
- term: English lemma or short fixed phrase, max 48 characters.
- partOfSpeech: exactly one label from: 名詞, 動詞, 形容詞, 副詞, 前置詞, 接続詞, 代名詞, 助動詞, 冠詞, 間投詞, 句, 成句, 接頭辞, 接尾辞.
- japaneseNuance: concise meaning of this candidate in Japanese, max 80 characters.
- usageNote: one short Japanese sentence explaining when to choose this candidate over nearby ones, max 120 characters.

Selected text:
{text}"#,
        schema_version = LEXI_JP_WORD_CANDIDATES_V1_SCHEMA_VERSION,
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
        ProviderKind::DeepL => Ok(fallback_models(provider)),
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
        ProviderKind::DeepL => vec![ProviderModel {
            id: "deepl-translate".to_string(),
            label: "DeepL Translate".to_string(),
        }],
    }
}

fn is_openai_chat_model(id: &str) -> bool {
    id.starts_with("gpt-") || id.starts_with("o")
}

fn lexi_result_schema(result_language: &str) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "schemaVersion",
            "mode",
            "sourceLanguage",
            "resultLanguage",
            "headword",
            "inflections",
            "translations",
            "nuance",
            "synonyms",
            "idioms",
            "warnings"
        ],
        "properties": {
            "schemaVersion": { "type": "string", "enum": [LEXI_RESULT_V1_SCHEMA_VERSION] },
            "mode": { "type": "string", "enum": ["word-study"] },
            "sourceLanguage": { "type": "string", "enum": ["en"] },
            "resultLanguage": { "type": "string", "enum": [result_language] },
            "headword": { "type": "string" },
            "inflections": { "type": "array", "minItems": 0, "maxItems": 3, "items": { "$ref": "#/$defs/inflection" } },
            "translations": { "type": "array", "minItems": 1, "maxItems": 3, "items": { "$ref": "#/$defs/translation" } },
            "nuance": { "type": "string" },
            "synonyms": { "type": "array", "minItems": 0, "maxItems": 4, "items": { "$ref": "#/$defs/relatedWord" } },
            "idioms": { "type": "array", "minItems": 0, "maxItems": 3, "items": { "$ref": "#/$defs/idiom" } },
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
            "translation": {
                "type": "object",
                "additionalProperties": false,
                "required": ["text", "note", "example"],
                "properties": {
                    "text": { "type": "string" },
                    "note": translation_note_json_schema(),
                    "example": { "$ref": "#/$defs/exampleSentence" }
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
            },
            "idiom": {
                "type": "object",
                "additionalProperties": false,
                "required": ["idiom", "japanese", "example"],
                "properties": {
                    "idiom": { "type": "string" },
                    "japanese": { "type": "string" },
                    "example": { "type": "string" }
                }
            },
            "inflection": {
                "type": "object",
                "additionalProperties": false,
                "required": ["kind", "form"],
                "properties": {
                    "kind": { "type": "string", "enum": ["plural", "past", "pastParticiple"] },
                    "form": { "type": "string" }
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
    transform_mode: TransformMode,
) -> Result<String, AppError> {
    let (system_prompt, user_prompt, schema_name, schema) = match transform_mode {
        TransformMode::JapaneseWordCandidates => (
            "You return only strict, compact JSON for Lexi's Japanese word-candidates schema.",
            build_japanese_word_candidates_prompt(request),
            "lexi_jp_word_candidates_v1",
            lexi_jp_word_candidates_schema(),
        ),
        TransformMode::WordStudy => (
            "You return only strict, compact JSON for Lexi's word-study schema. Keep every field short and contrastive.",
            build_word_study_prompt(request),
            "lexi_result_v1",
            lexi_result_schema(&request.result_language),
        ),
        TransformMode::TextTranslation => unreachable!("text translation uses DeepL"),
    };

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
                    "content": system_prompt
                },
                {
                    "role": "user",
                    "content": user_prompt
                }
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": schema_name,
                    "strict": true,
                    "schema": schema
                }
            }
        }))
        .send()
        .await
        .map_err(|error| {
            AppError::provider_request_failed(
                format!("OpenAI stream request failed: {error}"),
                true,
            )
        })?;

    if !response.status().is_success() {
        return Err(AppError::provider_request_failed(
            format!("OpenAI stream request returned HTTP {}", response.status()),
            response.status().as_u16() == 429 || response.status().is_server_error(),
        ));
    }

    read_sse_stream(
        app,
        request_id,
        response,
        parse_openai_stream_text,
        transform_mode,
    )
    .await
}

async fn call_gemini_stream(
    app: &AppHandle,
    request_id: u64,
    api_key: &str,
    model: &str,
    request: &TransformRequest,
    transform_mode: TransformMode,
) -> Result<String, AppError> {
    let (prompt, response_schema) = match transform_mode {
        TransformMode::JapaneseWordCandidates => (
            build_japanese_word_candidates_prompt(request),
            gemini_jp_word_candidates_schema(),
        ),
        TransformMode::WordStudy => (
            build_word_study_prompt(request),
            gemini_lexi_result_schema(&request.result_language),
        ),
        TransformMode::TextTranslation => unreachable!("text translation uses DeepL"),
    };

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
                    "parts": [{ "text": prompt }]
                }
            ],
            "generationConfig": {
                "temperature": 0.2,
                "maxOutputTokens": MAX_OUTPUT_TOKENS,
                "responseMimeType": "application/json",
                "responseSchema": response_schema
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

    read_sse_stream(
        app,
        request_id,
        response,
        parse_gemini_stream_text,
        transform_mode,
    )
    .await
}

async fn read_sse_stream(
    app: &AppHandle,
    request_id: u64,
    response: reqwest::Response,
    parse_text: fn(&str) -> Result<StreamTextDelta, AppError>,
    transform_mode: TransformMode,
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
                let partial = partial_from_json_fragment(&content, transform_mode);
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
        .and_then(|content| content.parts.iter().find_map(|part| part.text.as_deref()))
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
                    "schema": lexi_result_schema(&request.result_language)
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

    Ok(finalize_word_study_result(
        parse_lexi_result_v1(content)?,
        &request.result_language,
    ))
}

async fn call_openai_japanese_word_candidates(
    api_key: &str,
    model: &str,
    request: &TransformRequest,
) -> Result<JapaneseWordCandidatesResultV1, AppError> {
    let client = reqwest_client()?;

    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&json!({
            "model": model,
            "messages": [
                {
                    "role": "system",
                    "content": "You return only strict, compact JSON for Lexi's Japanese word-candidates schema."
                },
                {
                    "role": "user",
                    "content": build_japanese_word_candidates_prompt(request)
                }
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "lexi_jp_word_candidates_v1",
                    "strict": true,
                    "schema": lexi_jp_word_candidates_schema()
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

    parse_japanese_word_candidates_result_v1(content)
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
                "responseSchema": gemini_lexi_result_schema(&request.result_language)
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

    Ok(finalize_word_study_result(
        parse_lexi_result_v1(content)?,
        &request.result_language,
    ))
}

async fn call_gemini_japanese_word_candidates(
    api_key: &str,
    model: &str,
    request: &TransformRequest,
) -> Result<JapaneseWordCandidatesResultV1, AppError> {
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
                    "parts": [{ "text": build_japanese_word_candidates_prompt(request) }]
                }
            ],
            "generationConfig": {
                "temperature": 0.2,
                "maxOutputTokens": MAX_OUTPUT_TOKENS,
                "responseMimeType": "application/json",
                "responseSchema": gemini_jp_word_candidates_schema()
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

    parse_japanese_word_candidates_result_v1(content)
}

async fn call_deepl_text_translation(
    api_key: &str,
    request: &TransformRequest,
) -> Result<TextTranslationResultV1, AppError> {
    let client = reqwest_client()?;
    let target_lang = deepl_target_language(&request.result_language);
    let response = client
        .post(deepl_translate_url(api_key))
        .header("Authorization", format!("DeepL-Auth-Key {api_key}"))
        .form(&[
            ("text", request.selected_text.as_str()),
            ("target_lang", target_lang.as_str()),
        ])
        .send()
        .await
        .map_err(|error| {
            AppError::provider_request_failed(format!("DeepL request failed: {error}"), true)
        })?;

    if !response.status().is_success() {
        return Err(AppError::provider_request_failed(
            format!("DeepL request returned HTTP {}", response.status()),
            response.status().as_u16() == 429 || response.status().is_server_error(),
        ));
    }

    let payload = response
        .json::<DeepLTranslateResponse>()
        .await
        .map_err(|error| {
            AppError::invalid_model_output(format!("DeepL response parse failed: {error}"))
        })?;
    let translation = payload
        .translations
        .first()
        .ok_or_else(|| AppError::invalid_model_output("DeepL response had no translations"))?;

    TextTranslationResultV1 {
        schema_version: LEXI_TEXT_TRANSLATION_V1_SCHEMA_VERSION.to_string(),
        mode: "text-translation".to_string(),
        source_language: "auto".to_string(),
        detected_source_language: translation.detected_source_language.clone(),
        result_language: request.result_language.clone(),
        translated_text: translation.text.clone(),
        segments: vec![TranslationSegment {
            source: request.selected_text.clone(),
            translation: translation.text.clone(),
        }],
        warnings: vec![],
    }
    .validate()
}

fn deepl_target_language(result_language: &str) -> String {
    match result_language.trim().to_ascii_lowercase().as_str() {
        "ja" | "jp" => "JA".to_string(),
        "en" => "EN-US".to_string(),
        "ko" => "KO".to_string(),
        "zh" | "zh-cn" | "zh_cn" => "ZH-HANS".to_string(),
        "zh-tw" | "zh_tw" => "ZH-HANT".to_string(),
        other => other.to_ascii_uppercase(),
    }
}

fn deepl_translate_url(api_key: &str) -> &'static str {
    if api_key.trim().ends_with(":fx") {
        "https://api-free.deepl.com/v2/translate"
    } else {
        "https://api.deepl.com/v2/translate"
    }
}

fn gemini_lexi_result_schema(result_language: &str) -> Value {
    json!({
        "type": "OBJECT",
        "required": [
            "schemaVersion",
            "mode",
            "sourceLanguage",
            "resultLanguage",
            "headword",
            "inflections",
            "translations",
            "nuance",
            "synonyms",
            "idioms",
            "warnings"
        ],
        "properties": {
            "schemaVersion": { "type": "STRING", "enum": [LEXI_RESULT_V1_SCHEMA_VERSION] },
            "mode": { "type": "STRING", "enum": ["word-study"] },
            "sourceLanguage": { "type": "STRING", "enum": ["en"] },
            "resultLanguage": { "type": "STRING", "enum": [result_language] },
            "headword": { "type": "STRING" },
            "inflections": {
                "type": "ARRAY",
                "minItems": 0,
                "maxItems": 3,
                "items": gemini_inflection_schema()
            },
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
            "idioms": {
                "type": "ARRAY",
                "minItems": 0,
                "maxItems": 3,
                "items": gemini_idiom_schema()
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

fn gemini_inflection_schema() -> Value {
    json!({
        "type": "OBJECT",
        "required": ["kind", "form"],
        "properties": {
            "kind": { "type": "STRING", "enum": ["plural", "past", "pastParticiple"] },
            "form": { "type": "STRING" }
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

fn gemini_idiom_schema() -> Value {
    json!({
        "type": "OBJECT",
        "required": ["idiom", "japanese", "example"],
        "properties": {
            "idiom": { "type": "STRING" },
            "japanese": { "type": "STRING" },
            "example": { "type": "STRING" }
        }
    })
}

fn lexi_jp_word_candidates_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "schemaVersion",
            "mode",
            "sourceLanguage",
            "resultLanguage",
            "query",
            "candidates",
            "warnings"
        ],
        "properties": {
            "schemaVersion": { "type": "string", "enum": [LEXI_JP_WORD_CANDIDATES_V1_SCHEMA_VERSION] },
            "mode": { "type": "string", "enum": ["jp-word-candidates"] },
            "sourceLanguage": { "type": "string" },
            "resultLanguage": { "type": "string" },
            "query": { "type": "string" },
            "candidates": {
                "type": "array",
                "minItems": 1,
                "maxItems": 8,
                "items": { "$ref": "#/$defs/candidate" }
            },
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
            "candidate": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                    "term",
                    "partOfSpeech",
                    "japaneseNuance",
                    "usageNote",
                    "example",
                    "confidence"
                ],
                "properties": {
                    "term": { "type": "string" },
                    "partOfSpeech": {
                        "type": "string",
                        "enum": TRANSLATION_NOTE_VALUES
                    },
                    "japaneseNuance": { "type": "string" },
                    "usageNote": { "type": "string" },
                    "example": { "$ref": "#/$defs/exampleSentence" },
                    "confidence": {
                        "type": "string",
                        "enum": ["high", "medium", "low"]
                    }
                }
            }
        }
    })
}

fn gemini_jp_word_candidates_schema() -> Value {
    json!({
        "type": "OBJECT",
        "required": [
            "schemaVersion",
            "mode",
            "sourceLanguage",
            "resultLanguage",
            "query",
            "candidates",
            "warnings"
        ],
        "properties": {
            "schemaVersion": { "type": "STRING", "enum": [LEXI_JP_WORD_CANDIDATES_V1_SCHEMA_VERSION] },
            "mode": { "type": "STRING", "enum": ["jp-word-candidates"] },
            "sourceLanguage": { "type": "STRING" },
            "resultLanguage": { "type": "STRING" },
            "query": { "type": "STRING" },
            "candidates": {
                "type": "ARRAY",
                "minItems": 1,
                "maxItems": 8,
                "items": gemini_jp_candidate_schema()
            },
            "warnings": { "type": "ARRAY", "items": { "type": "STRING" } }
        }
    })
}

fn gemini_jp_candidate_schema() -> Value {
    json!({
        "type": "OBJECT",
        "required": [
            "term",
            "partOfSpeech",
            "japaneseNuance",
            "usageNote",
            "example",
            "confidence"
        ],
        "properties": {
            "term": { "type": "STRING" },
            "partOfSpeech": {
                "type": "STRING",
                "enum": TRANSLATION_NOTE_VALUES
            },
            "japaneseNuance": { "type": "STRING" },
            "usageNote": { "type": "STRING" },
            "example": gemini_example_sentence_schema(),
            "confidence": {
                "type": "STRING",
                "enum": ["high", "medium", "low"]
            }
        }
    })
}

fn partial_from_json_fragment(fragment: &str, transform_mode: TransformMode) -> LexiPartialResult {
    match transform_mode {
        TransformMode::JapaneseWordCandidates => {
            if let Ok(result) = serde_json::from_str::<JapaneseWordCandidatesResultV1>(fragment) {
                return LexiPartialResult::from_japanese_word_candidates_result(&result);
            }

            LexiPartialResult {
                query: extract_string_field(fragment, "query"),
                candidates: extract_object_array::<EnglishCandidate>(fragment, "candidates"),
                warnings: extract_string_array(fragment, "warnings"),
                ..LexiPartialResult::default()
            }
        }
        TransformMode::WordStudy => {
            if let Ok(result) = serde_json::from_str::<LexiResultV1>(fragment) {
                return LexiPartialResult::from_word_study_result(&result);
            }

            LexiPartialResult {
                headword: extract_string_field(fragment, "headword"),
                inflections: extract_object_array::<Inflection>(fragment, "inflections"),
                translations: extract_object_array::<Translation>(fragment, "translations"),
                nuance: extract_string_field(fragment, "nuance"),
                synonyms: extract_object_array::<RelatedWord>(fragment, "synonyms"),
                idioms: extract_object_array::<Idiom>(fragment, "idioms"),
                warnings: extract_string_array(fragment, "warnings"),
                ..LexiPartialResult::default()
            }
        }
        TransformMode::TextTranslation => LexiPartialResult::default(),
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
    #[serde(default)]
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Deserialize)]
struct GeminiPart {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeepLTranslateResponse {
    translations: Vec<DeepLTranslation>,
}

#[derive(Debug, Deserialize)]
struct DeepLTranslation {
    text: String,
    detected_source_language: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{
        build_japanese_word_candidates_prompt, build_word_study_prompt, classify_transform_mode,
        finalize_word_study_result, gemini_lexi_result_schema, lexi_result_schema, mock_headword,
        mock_inflections, parse_gemini_stream_text, parse_openai_stream_text, parse_sse_event_text,
        partial_from_json_fragment, pop_sse_event, provider_finish_reason_indicates_truncation,
        result_language_for_mode, selected_text_preview, sse_data_payload, TransformMode,
        TransformRequest,
    };
    use crate::schema::{
        parse_lexi_result_v1, CandidateConfidence, ExampleSentence, Idiom, Inflection,
        LexiResultV1, RelatedWord, Translation, LEXI_RESULT_V1_SCHEMA_VERSION,
    };

    #[test]
    fn japanese_word_candidates_force_english_result_language() {
        assert_eq!(
            result_language_for_mode("ja", TransformMode::JapaneseWordCandidates),
            "en"
        );
        assert_eq!(
            result_language_for_mode("ja", TransformMode::WordStudy),
            "ja"
        );
        assert_eq!(
            result_language_for_mode("ja", TransformMode::TextTranslation),
            "ja"
        );
    }

    fn sample_word_study_result(result_language: &str) -> LexiResultV1 {
        LexiResultV1 {
            schema_version: LEXI_RESULT_V1_SCHEMA_VERSION.to_string(),
            mode: "word-study".to_string(),
            source_language: "ja".to_string(),
            result_language: result_language.to_string(),
            headword: "play".to_string(),
            inflections: vec![],
            translations: vec![Translation {
                text: "遊ぶ".to_string(),
                note: Some("動詞".to_string()),
                example: ExampleSentence {
                    sentence: "They play outside.".to_string(),
                    japanese: "彼らは外で遊ぶ。".to_string(),
                },
                sense_kind: None,
                base_word: None,
            }],
            nuance: "テスト用の説明。".to_string(),
            synonyms: vec![],
            idioms: vec![],
            warnings: vec![],
        }
    }

    #[test]
    fn finalize_word_study_result_aligns_request_result_language() {
        let result = finalize_word_study_result(sample_word_study_result("en"), "ja");

        assert_eq!(result.result_language, "ja");
        assert_eq!(result.source_language, "en");
    }

    #[test]
    fn finalize_word_study_result_fixes_parsed_provider_output() {
        let raw_json = format!(
            r#"{{
              "schemaVersion": "{LEXI_RESULT_V1_SCHEMA_VERSION}",
              "mode": "word-study",
              "sourceLanguage": "ja",
              "resultLanguage": "en",
              "headword": "play",
              "inflections": [],
              "translations": [{{
                "text": "遊ぶ",
                "note": "動詞",
                "example": {{
                  "sentence": "They play outside.",
                  "japanese": "彼らは外で遊ぶ。"
                }}
              }}],
              "nuance": "テスト用の説明。",
              "synonyms": [],
              "idioms": [],
              "warnings": []
            }}"#
        );

        let parsed = parse_lexi_result_v1(&raw_json).expect("provider output parses");
        let fixed = finalize_word_study_result(parsed, "ja");

        assert_eq!(fixed.result_language, "ja");
        assert_eq!(fixed.source_language, "en");
    }

    #[test]
    fn word_study_schema_tracks_settings_result_language() {
        for language in ["ja", "en", "ko", "zh"] {
            assert_eq!(
                lexi_result_schema(language)["properties"]["resultLanguage"]["enum"][0],
                language
            );
            assert_eq!(
                lexi_result_schema(language)["properties"]["sourceLanguage"]["enum"][0],
                "en"
            );
            assert_eq!(
                gemini_lexi_result_schema(language)["properties"]["resultLanguage"]["enum"][0],
                language
            );
            assert_eq!(
                gemini_lexi_result_schema(language)["properties"]["sourceLanguage"]["enum"][0],
                "en"
            );
        }
    }

    #[test]
    fn word_study_prompt_declares_source_and_result_language() {
        let prompt = build_word_study_prompt(&TransformRequest {
            selected_text: "play".to_string(),
            result_language: "ja".to_string(),
            prompt_mode: "word-study".to_string(),
        });

        assert!(prompt.contains(r#"sourceLanguage must be "en""#));
        assert!(prompt.contains(r#"resultLanguage must be "ja""#));
    }

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
    fn gemini_stream_parser_accepts_content_without_parts() {
        let data = r#"{"candidates":[{"content":{"role":"model"},"finishReason":"STOP"}]}"#;
        let delta = parse_gemini_stream_text(data).expect("empty content event parses");
        assert_eq!(delta.text, None);
        assert_eq!(delta.finish_reason, Some("STOP".to_string()));
    }

    #[test]
    fn gemini_stream_parser_skips_non_text_parts() {
        let data = r#"{"candidates":[{"content":{"parts":[{"thought":true},{"text":"{\"schemaVersion\""}]}}]}"#;
        let delta = parse_gemini_stream_text(data).expect("mixed parts event parses");
        assert_eq!(delta.text, Some("{\"schemaVersion\"".to_string()));
        assert_eq!(delta.finish_reason, None);
    }

    #[test]
    fn provider_finish_reason_detects_truncation() {
        assert!(provider_finish_reason_indicates_truncation("MAX_TOKENS"));
        assert!(provider_finish_reason_indicates_truncation("length"));
        assert!(!provider_finish_reason_indicates_truncation("STOP"));
    }

    #[test]
    fn openai_schema_matches_result_validation_cardinality() {
        let schema = lexi_result_schema("ja");

        assert_eq!(
            schema["properties"]["sourceLanguage"]["enum"][0],
            "en"
        );
        assert_eq!(schema["properties"]["resultLanguage"]["enum"][0], "ja");
        assert_eq!(schema["properties"]["translations"]["minItems"], 1);
        assert_eq!(schema["properties"]["translations"]["maxItems"], 3);
        assert_eq!(schema["properties"]["inflections"]["minItems"], 0);
        assert_eq!(schema["properties"]["inflections"]["maxItems"], 3);
        assert_eq!(
            schema["$defs"]["inflection"]["properties"]["kind"]["enum"][0],
            "plural"
        );
        assert_eq!(
            schema["properties"]["translations"]["items"]["$ref"],
            "#/$defs/translation"
        );
        assert_eq!(schema["$defs"]["translation"]["required"][2], "example");
        assert_eq!(
            schema["$defs"]["translation"]["properties"]["note"]["enum"][0],
            "名詞"
        );
        assert!(schema["$defs"]["translation"]["properties"]["note"]["enum"]
            .as_array()
            .expect("note enum")
            .contains(&serde_json::Value::Null));
        assert!(!schema["$defs"]["translation"]["properties"]
            .as_object()
            .expect("translation properties")
            .contains_key("baseWord"));
        assert!(!schema["$defs"]["translation"]["properties"]
            .as_object()
            .expect("translation properties")
            .contains_key("senseKind"));
        assert_eq!(schema["properties"]["synonyms"]["minItems"], 0);
        assert_eq!(schema["properties"]["synonyms"]["maxItems"], 4);
        assert_eq!(schema["properties"]["idioms"]["minItems"], 0);
        assert_eq!(schema["properties"]["idioms"]["maxItems"], 3);
        assert_eq!(schema["$defs"]["idiom"]["required"][0], "idiom");
        assert!(!schema["$defs"]["relatedWord"]["required"]
            .as_array()
            .expect("related word required")
            .contains(&serde_json::Value::String("nuance".to_string())));
    }

    #[test]
    fn gemini_schema_matches_result_validation_cardinality() {
        let schema = gemini_lexi_result_schema("ja");

        assert_eq!(
            schema["properties"]["sourceLanguage"]["enum"][0],
            "en"
        );
        assert_eq!(schema["properties"]["resultLanguage"]["enum"][0], "ja");
        assert_eq!(schema["properties"]["translations"]["minItems"], 1);
        assert_eq!(schema["properties"]["translations"]["maxItems"], 3);
        assert_eq!(schema["properties"]["inflections"]["minItems"], 0);
        assert_eq!(schema["properties"]["inflections"]["maxItems"], 3);
        assert_eq!(
            schema["properties"]["inflections"]["items"]["properties"]["kind"]["enum"][0],
            "plural"
        );
        assert_eq!(
            schema["properties"]["translations"]["items"]["required"][2],
            "example"
        );
        assert_eq!(
            schema["properties"]["translations"]["items"]["properties"]["note"]["enum"][0],
            "名詞"
        );
        assert!(!schema["properties"]["translations"]["items"]["required"]
            .as_array()
            .expect("translation required")
            .contains(&serde_json::Value::String("baseWord".to_string())));
        assert!(!schema["properties"]["translations"]["items"]["properties"]
            .as_object()
            .expect("translation properties")
            .contains_key("baseWord"));
        assert!(!schema["properties"]["translations"]["items"]["properties"]
            .as_object()
            .expect("translation properties")
            .contains_key("senseKind"));
        assert_eq!(schema["properties"]["synonyms"]["minItems"], 0);
        assert_eq!(schema["properties"]["synonyms"]["maxItems"], 4);
        assert_eq!(schema["properties"]["idioms"]["minItems"], 0);
        assert_eq!(schema["properties"]["idioms"]["maxItems"], 3);
        assert_eq!(
            schema["properties"]["idioms"]["items"]["required"][0],
            "idiom"
        );
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
        assert!(prompt.contains("quantified -> quantify"));
        assert!(prompt.contains("dictionary/base form"));
        assert!(prompt.contains("inflections: 0 to 3"));
        assert!(prompt.contains("Regular forms must be omitted"));
        assert!(prompt.contains("see -> saw/seen"));
        assert!(prompt.contains("see must include saw and seen"));
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
        assert!(prompt.contains("Do not include senseKind or baseWord"));
        assert!(prompt.contains("example is required for every translation item"));
        assert!(prompt.contains("idioms: 0 to 3"));
        assert!(prompt.contains("tend to"));
        assert!(prompt.contains("tend to one's needs"));
        assert!(prompt.contains("minimal English collocation"));
        assert!(prompt.contains("Do not pad idioms"));
        assert!(prompt.contains("Do not inflate a simple collocation"));
    }

    #[test]
    fn mock_provider_displays_base_form_for_common_inflections() {
        assert_eq!(mock_headword("went"), "go");
        assert_eq!(mock_headword("studied"), "study");
        assert_eq!(mock_headword("walked"), "walk");
        assert!(mock_inflections("walked").is_empty());
        assert_eq!(mock_inflections("went")[0].form, "went");
    }

    #[test]
    fn selected_text_preview_collapses_whitespace_and_truncates() {
        assert_eq!(selected_text_preview("  subtle\nchange  "), "subtle change");
        assert_eq!(selected_text_preview(&"a".repeat(50)).chars().count(), 48);
    }

    #[test]
    fn classifies_japanese_word_candidates() {
        for query in ["採用", "微妙", "責任を取る"] {
            assert_eq!(
                classify_transform_mode(query),
                TransformMode::JapaneseWordCandidates,
                "expected JapaneseWordCandidates for '{query}'"
            );
        }
    }

    #[test]
    fn classifies_japanese_sentence_like_for_text_translation() {
        assert_eq!(
            classify_transform_mode("これはテストです。"),
            TransformMode::TextTranslation
        );
        assert_eq!(
            classify_transform_mode("A、B"),
            TransformMode::TextTranslation
        );
        assert_eq!(
            classify_transform_mode("一行目\n二行目"),
            TransformMode::TextTranslation
        );
        let long_japanese = "あ".repeat(33);
        assert_eq!(
            classify_transform_mode(&long_japanese),
            TransformMode::TextTranslation
        );
    }

    #[test]
    fn classifies_english_word_study_and_text_translation() {
        assert_eq!(
            classify_transform_mode("This is a selected sentence."),
            TransformMode::TextTranslation
        );
        assert_eq!(
            classify_transform_mode("one two three four five"),
            TransformMode::TextTranslation
        );
        assert_eq!(
            classify_transform_mode("one two three four"),
            TransformMode::WordStudy
        );
        assert_eq!(classify_transform_mode("subtle"), TransformMode::WordStudy);
        assert_eq!(
            classify_transform_mode("take off"),
            TransformMode::WordStudy
        );
    }

    #[test]
    fn classifies_newline_and_clause_punctuation_for_text_translation() {
        assert_eq!(
            classify_transform_mode("line one\nline two"),
            TransformMode::TextTranslation
        );
        assert_eq!(
            classify_transform_mode("hello, world"),
            TransformMode::TextTranslation
        );
        assert_eq!(classify_transform_mode("   "), TransformMode::WordStudy);
    }

    #[test]
    fn japanese_word_candidates_prompt_requires_examples() {
        let prompt = build_japanese_word_candidates_prompt(&super::TransformRequest {
            selected_text: "採用".to_string(),
            result_language: "en".to_string(),
            prompt_mode: "jp-word-candidates".to_string(),
        });

        assert!(prompt.contains("lexi.jp-word-candidates.v1"));
        assert!(prompt.contains("jp-word-candidates"));
        assert!(prompt.contains("example.sentence"));
        assert!(prompt.contains("example.japanese"));
        assert!(prompt.contains("Every candidate must include example"));
        assert!(prompt.contains("Do not output Japanese translations as candidates"));
    }

    #[test]
    fn partial_json_fragment_extracts_headword_before_completion() {
        let partial = partial_from_json_fragment(
            r#"{"schemaVersion":"lexi.result.v1","headword":"subtle","translations":[{"#,
            TransformMode::WordStudy,
        );

        assert_eq!(partial.headword.as_deref(), Some("subtle"));
        assert!(partial.translations.is_empty());
    }

    #[test]
    fn partial_json_fragment_extracts_completed_translation_rows() {
        let partial = partial_from_json_fragment(
            r#"{"headword":"go","translations":[{"text":"行く","note":"動詞","example":{"sentence":"I go.","japanese":"行く。"}}],"nuance":"#,
            TransformMode::WordStudy,
        );

        assert_eq!(partial.headword.as_deref(), Some("go"));
        assert_eq!(partial.translations.len(), 1);
        assert_eq!(partial.translations[0].text, "行く");
        assert!(partial.nuance.is_none());
    }

    #[test]
    fn partial_json_fragment_extracts_japanese_word_candidates() {
        let partial = partial_from_json_fragment(
            r#"{"query":"採用","candidates":[{"term":"adopt","partOfSpeech":"動詞","japaneseNuance":"取り入れる","usageNote":"制度を採用する。","example":{"sentence":"They adopted it.","japanese":"採用した。"},"confidence":"high"}],"warnings":["#,
            TransformMode::JapaneseWordCandidates,
        );

        assert_eq!(partial.query.as_deref(), Some("採用"));
        assert_eq!(partial.candidates.len(), 1);
        assert_eq!(partial.candidates[0].term, "adopt");
        assert_eq!(partial.candidates[0].example.sentence, "They adopted it.");
        assert_eq!(partial.candidates[0].confidence, CandidateConfidence::High);
        assert!(partial.headword.is_none());
    }

    #[test]
    fn started_event_serializes_selected_text_preview() {
        let value = serde_json::to_value(super::TransformEvent::Started {
            request_id: 7,
            selected_text_preview: "subtle".to_string(),
            selected_text: None,
            shortcut: "Ctrl+E".to_string(),
            capture_method: "uia-foreground-window",
            source_process: Some("notepad.exe".to_string()),
            source_window_title: None,
            character_count: 6,
            multiline: false,
            transform_mode: "word-study",
            provider: crate::settings::ProviderKind::Mock,
            model: "mock-word-study".to_string(),
        })
        .expect("started event serializes");

        assert_eq!(value["status"], "started");
        assert_eq!(value["selectedTextPreview"], "subtle");
    }
}
