use crate::errors::AppError;
use serde::{Deserialize, Serialize};

pub const LEXI_RESULT_V1_SCHEMA_VERSION: &str = "lexi.result.v1";
pub const LEXI_TEXT_TRANSLATION_V1_SCHEMA_VERSION: &str = "lexi.text-translation.v1";
pub const TRANSLATION_NOTE_VALUES: &[&str] = &[
    "名詞",
    "動詞",
    "形容詞",
    "副詞",
    "前置詞",
    "接続詞",
    "代名詞",
    "助動詞",
    "冠詞",
    "間投詞",
    "句",
    "成句",
    "接頭辞",
    "接尾辞",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Translation {
    pub text: String,
    pub note: Option<String>,
    pub example: ExampleSentence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExampleSentence {
    pub sentence: String,
    pub japanese: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedWord {
    pub term: String,
    pub japanese: String,
    pub usage_comparison: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Idiom {
    pub idiom: String,
    pub japanese: String,
    pub example: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Inflection {
    pub kind: String,
    pub form: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LexiResultV1 {
    pub schema_version: String,
    pub mode: String,
    pub source_language: String,
    pub result_language: String,
    pub headword: String,
    pub inflections: Vec<Inflection>,
    pub translations: Vec<Translation>,
    pub nuance: String,
    pub synonyms: Vec<RelatedWord>,
    pub idioms: Vec<Idiom>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationSegment {
    pub source: String,
    pub translation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextTranslationResultV1 {
    pub schema_version: String,
    pub mode: String,
    pub source_language: String,
    pub detected_source_language: Option<String>,
    pub result_language: String,
    pub translated_text: String,
    pub segments: Vec<TranslationSegment>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LexiResult {
    WordStudy(LexiResultV1),
    TextTranslation(TextTranslationResultV1),
}

impl LexiResultV1 {
    pub fn validate(self) -> Result<Self, AppError> {
        if self.schema_version != LEXI_RESULT_V1_SCHEMA_VERSION {
            return Err(AppError::invalid_model_output(format!(
                "unsupported schemaVersion '{}'",
                self.schema_version
            )));
        }

        validate_required("mode", &self.mode)?;
        validate_required("sourceLanguage", &self.source_language)?;
        validate_required("resultLanguage", &self.result_language)?;
        validate_required("headword", &self.headword)?;
        validate_required("nuance", &self.nuance)?;
        validate_max_chars("headword", &self.headword, 48)?;
        validate_max_chars("nuance", &self.nuance, 120)?;
        validate_max_len("inflections", self.inflections.len(), 3)?;
        validate_non_empty("translations", self.translations.len())?;
        validate_max_len("translations", self.translations.len(), 3)?;
        validate_max_len("synonyms", self.synonyms.len(), 4)?;
        validate_max_len("idioms", self.idioms.len(), 3)?;

        for (index, inflection) in self.inflections.iter().enumerate() {
            validate_inflection(&format!("inflections[{index}]"), inflection)?;
        }

        for (index, translation) in self.translations.iter().enumerate() {
            validate_required(&format!("translations[{index}].text"), &translation.text)?;
            validate_max_chars(
                &format!("translations[{index}].text"),
                &translation.text,
                48,
            )?;
            if let Some(note) = &translation.note {
                validate_required(&format!("translations[{index}].note"), note)?;
                validate_translation_note(&format!("translations[{index}].note"), note)?;
            }
            validate_example_sentence(
                &format!("translations[{index}].example"),
                &translation.example,
            )?;
        }

        for (index, synonym) in self.synonyms.iter().enumerate() {
            validate_related_word(&format!("synonyms[{index}]"), synonym)?;
        }

        for (index, idiom) in self.idioms.iter().enumerate() {
            validate_idiom(&format!("idioms[{index}]"), idiom)?;
        }

        for (index, warning) in self.warnings.iter().enumerate() {
            validate_required(&format!("warnings[{index}]"), warning)?;
            validate_max_chars(&format!("warnings[{index}]"), warning, 120)?;
        }

        Ok(self)
    }
}

impl TextTranslationResultV1 {
    pub fn validate(self) -> Result<Self, AppError> {
        if self.schema_version != LEXI_TEXT_TRANSLATION_V1_SCHEMA_VERSION {
            return Err(AppError::invalid_model_output(format!(
                "unsupported schemaVersion '{}'",
                self.schema_version
            )));
        }

        validate_required("mode", &self.mode)?;
        if self.mode != "text-translation" {
            return Err(AppError::invalid_model_output(format!(
                "unsupported mode '{}'",
                self.mode
            )));
        }
        validate_required("sourceLanguage", &self.source_language)?;
        validate_required("resultLanguage", &self.result_language)?;
        validate_required("translatedText", &self.translated_text)?;
        validate_max_chars("translatedText", &self.translated_text, 4000)?;
        validate_max_len("segments", self.segments.len(), 24)?;

        if let Some(language) = &self.detected_source_language {
            validate_required("detectedSourceLanguage", language)?;
            validate_max_chars("detectedSourceLanguage", language, 16)?;
        }

        for (index, segment) in self.segments.iter().enumerate() {
            validate_required(&format!("segments[{index}].source"), &segment.source)?;
            validate_required(&format!("segments[{index}].translation"), &segment.translation)?;
            validate_max_chars(&format!("segments[{index}].source"), &segment.source, 1000)?;
            validate_max_chars(
                &format!("segments[{index}].translation"),
                &segment.translation,
                1000,
            )?;
        }

        for (index, warning) in self.warnings.iter().enumerate() {
            validate_required(&format!("warnings[{index}]"), warning)?;
            validate_max_chars(&format!("warnings[{index}]"), warning, 120)?;
        }

        Ok(self)
    }
}

pub fn parse_lexi_result_v1(raw_json: &str) -> Result<LexiResultV1, AppError> {
    let result = serde_json::from_str::<LexiResultV1>(raw_json).map_err(|error| {
        AppError::invalid_model_output(format!("model output JSON parse failed: {error}"))
    })?;

    result.validate()
}

fn validate_required(field: &str, value: &str) -> Result<(), AppError> {
    if value.trim().is_empty() {
        return Err(AppError::invalid_model_output(format!(
            "required field '{field}' is empty"
        )));
    }

    Ok(())
}

fn validate_non_empty(field: &str, len: usize) -> Result<(), AppError> {
    if len == 0 {
        return Err(AppError::invalid_model_output(format!(
            "required field '{field}' is empty"
        )));
    }

    Ok(())
}

fn validate_max_len(field: &str, len: usize, max: usize) -> Result<(), AppError> {
    if len > max {
        return Err(AppError::invalid_model_output(format!(
            "field '{field}' has {len} items, max is {max}"
        )));
    }

    Ok(())
}

fn validate_max_chars(field: &str, value: &str, max: usize) -> Result<(), AppError> {
    let count = value.chars().count();
    if count > max {
        return Err(AppError::invalid_model_output(format!(
            "field '{field}' has {count} characters, max is {max}"
        )));
    }

    Ok(())
}

fn validate_translation_note(field: &str, value: &str) -> Result<(), AppError> {
    if !TRANSLATION_NOTE_VALUES.contains(&value) {
        return Err(AppError::invalid_model_output(format!(
            "field '{field}' must be a part-of-speech label"
        )));
    }

    Ok(())
}

fn validate_example_sentence(field: &str, value: &ExampleSentence) -> Result<(), AppError> {
    validate_required(&format!("{field}.sentence"), &value.sentence)?;
    validate_required(&format!("{field}.japanese"), &value.japanese)?;
    validate_max_chars(&format!("{field}.sentence"), &value.sentence, 96)?;
    validate_max_chars(&format!("{field}.japanese"), &value.japanese, 96)?;

    Ok(())
}

fn validate_related_word(field: &str, value: &RelatedWord) -> Result<(), AppError> {
    validate_required(&format!("{field}.term"), &value.term)?;
    validate_required(&format!("{field}.japanese"), &value.japanese)?;
    validate_required(&format!("{field}.usageComparison"), &value.usage_comparison)?;
    validate_max_chars(&format!("{field}.term"), &value.term, 48)?;
    validate_max_chars(&format!("{field}.japanese"), &value.japanese, 48)?;
    validate_max_chars(
        &format!("{field}.usageComparison"),
        &value.usage_comparison,
        140,
    )?;

    Ok(())
}

fn validate_idiom(field: &str, value: &Idiom) -> Result<(), AppError> {
    validate_required(&format!("{field}.idiom"), &value.idiom)?;
    validate_required(&format!("{field}.japanese"), &value.japanese)?;
    validate_required(&format!("{field}.example"), &value.example)?;
    validate_max_chars(&format!("{field}.idiom"), &value.idiom, 64)?;
    validate_max_chars(&format!("{field}.japanese"), &value.japanese, 64)?;
    validate_max_chars(&format!("{field}.example"), &value.example, 120)?;

    Ok(())
}

fn validate_inflection(field: &str, value: &Inflection) -> Result<(), AppError> {
    validate_required(&format!("{field}.kind"), &value.kind)?;
    validate_required(&format!("{field}.form"), &value.form)?;
    if !matches!(value.kind.as_str(), "plural" | "past" | "pastParticiple") {
        return Err(AppError::invalid_model_output(format!(
            "field '{field}.kind' must be an inflection kind"
        )));
    }
    validate_max_chars(&format!("{field}.form"), &value.form, 48)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        parse_lexi_result_v1, LexiResultV1, TextTranslationResultV1, TranslationSegment,
        LEXI_RESULT_V1_SCHEMA_VERSION, LEXI_TEXT_TRANSLATION_V1_SCHEMA_VERSION,
    };
    use crate::errors::AppErrorCode;

    fn valid_result() -> LexiResultV1 {
        LexiResultV1 {
            schema_version: LEXI_RESULT_V1_SCHEMA_VERSION.to_string(),
            mode: "word-study".to_string(),
            source_language: "auto".to_string(),
            result_language: "ja".to_string(),
            headword: "subtle".to_string(),
            inflections: vec![],
            translations: vec![super::Translation {
                text: "微妙な".to_string(),
                note: Some("形容詞".to_string()),
                example: super::ExampleSentence {
                    sentence: "She noticed a subtle change in his voice.".to_string(),
                    japanese: "彼女は彼の声の微妙な変化に気づいた。".to_string(),
                },
            }],
            nuance: "Small or delicate enough that it may be hard to notice.".to_string(),
            synonyms: vec![super::RelatedWord {
                term: "delicate".to_string(),
                japanese: "繊細な".to_string(),
                usage_comparison: "subtle は気づきにくさ、delicate は細部や壊れやすさに焦点。"
                    .to_string(),
            }],
            idioms: vec![super::Idiom {
                idiom: "a subtle hint".to_string(),
                japanese: "subtle hint".to_string(),
                example: "She gave me a subtle hint.".to_string(),
            }],
            warnings: vec![],
        }
    }

    #[test]
    fn accepts_valid_result() {
        let result = valid_result().validate().expect("valid schema");

        assert_eq!(result.schema_version, LEXI_RESULT_V1_SCHEMA_VERSION);
    }

    #[test]
    fn rejects_unknown_schema_version() {
        let mut result = valid_result();
        result.schema_version = "lexi.result.v2".to_string();

        let error = result.validate().expect_err("unknown schema should fail");

        assert_eq!(error.code, AppErrorCode::InvalidModelOutput);
        assert!(error
            .diagnostic_message
            .contains("unsupported schemaVersion"));
    }

    #[test]
    fn rejects_missing_required_json_field() {
        let raw_json = r#"{
            "schemaVersion": "lexi.result.v1",
            "mode": "word-study",
            "sourceLanguage": "auto",
            "resultLanguage": "ja",
            "translations": [{"text": "微妙な", "note": null, "example": {"sentence": "A subtle smell filled the room.", "japanese": "かすかな匂いが部屋に広がった。"}}],
            "nuance": "Small or delicate enough that it may be hard to notice.",
            "synonyms": [],
            "warnings": []
        }"#;

        let error = parse_lexi_result_v1(raw_json).expect_err("missing headword should fail");

        assert_eq!(error.code, AppErrorCode::InvalidModelOutput);
    }

    #[test]
    fn rejects_empty_required_field() {
        let mut result = valid_result();
        result.headword = " ".to_string();

        let error = result.validate().expect_err("empty headword should fail");

        assert_eq!(error.code, AppErrorCode::InvalidModelOutput);
        assert!(error.diagnostic_message.contains("headword"));
    }

    #[test]
    fn rejects_empty_translations() {
        let mut result = valid_result();
        result.translations = vec![];

        let error = result
            .validate()
            .expect_err("empty translations should fail");

        assert_eq!(error.code, AppErrorCode::InvalidModelOutput);
        assert!(error.diagnostic_message.contains("translations"));
    }

    #[test]
    fn accepts_irregular_inflections() {
        let mut result = valid_result();
        result.headword = "go".to_string();
        result.inflections = vec![
            super::Inflection {
                kind: "past".to_string(),
                form: "went".to_string(),
            },
            super::Inflection {
                kind: "pastParticiple".to_string(),
                form: "gone".to_string(),
            },
        ];

        let result = result
            .validate()
            .expect("irregular inflections are allowed");

        assert_eq!(result.inflections.len(), 2);
    }

    #[test]
    fn rejects_unknown_inflection_kind() {
        let mut result = valid_result();
        result.inflections = vec![super::Inflection {
            kind: "comparative".to_string(),
            form: "better".to_string(),
        }];

        let error = result
            .validate()
            .expect_err("unknown inflection kind should fail");

        assert_eq!(error.code, AppErrorCode::InvalidModelOutput);
        assert!(error.diagnostic_message.contains("inflection kind"));
    }

    #[test]
    fn rejects_translation_without_japanese_example() {
        let mut result = valid_result();
        result.translations[0].example.japanese = " ".to_string();

        let error = result
            .validate()
            .expect_err("translation should include Japanese example translation");

        assert_eq!(error.code, AppErrorCode::InvalidModelOutput);
        assert!(error
            .diagnostic_message
            .contains("translations[0].example.japanese"));
    }

    #[test]
    fn rejects_translation_note_that_is_not_part_of_speech() {
        let mut result = valid_result();
        result.translations[0].note = Some("数学".to_string());

        let error = result
            .validate()
            .expect_err("semantic labels should not be accepted as part of speech");

        assert_eq!(error.code, AppErrorCode::InvalidModelOutput);
        assert!(error.diagnostic_message.contains("part-of-speech"));
    }

    #[test]
    fn accepts_empty_synonyms_when_unavailable() {
        let mut result = valid_result();
        result.synonyms = vec![];

        let result = result.validate().expect("empty synonyms are allowed");

        assert!(result.synonyms.is_empty());
    }

    #[test]
    fn accepts_empty_idioms_when_unavailable() {
        let mut result = valid_result();
        result.idioms = vec![];

        let result = result.validate().expect("empty idioms are allowed");

        assert!(result.idioms.is_empty());
    }

    #[test]
    fn rejects_idiom_without_example() {
        let mut result = valid_result();
        result.idioms[0].example = " ".to_string();

        let error = result
            .validate()
            .expect_err("idiom should include an example");

        assert_eq!(error.code, AppErrorCode::InvalidModelOutput);
        assert!(error.diagnostic_message.contains("idioms[0].example"));
    }

    #[test]
    fn rejects_overlong_nuance_for_popup_layout() {
        let mut result = valid_result();
        result.nuance = "a".repeat(121);

        let error = result.validate().expect_err("overlong nuance should fail");

        assert_eq!(error.code, AppErrorCode::InvalidModelOutput);
        assert!(error.diagnostic_message.contains("nuance"));
    }

    #[test]
    fn rejects_synonym_without_usage_comparison() {
        let mut result = valid_result();
        result.synonyms[0].usage_comparison = " ".to_string();

        let error = result
            .validate()
            .expect_err("synonym should explain usage difference");

        assert_eq!(error.code, AppErrorCode::InvalidModelOutput);
        assert!(error.diagnostic_message.contains("usageComparison"));
    }

    #[test]
    fn accepts_valid_text_translation_result() {
        let result = TextTranslationResultV1 {
            schema_version: LEXI_TEXT_TRANSLATION_V1_SCHEMA_VERSION.to_string(),
            mode: "text-translation".to_string(),
            source_language: "auto".to_string(),
            detected_source_language: Some("EN".to_string()),
            result_language: "ja".to_string(),
            translated_text: "これはテストです。".to_string(),
            segments: vec![TranslationSegment {
                source: "This is a test.".to_string(),
                translation: "これはテストです。".to_string(),
            }],
            warnings: vec![],
        }
        .validate()
        .expect("valid text translation");

        assert_eq!(result.mode, "text-translation");
    }
}
