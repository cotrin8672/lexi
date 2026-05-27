use crate::errors::AppError;
use serde::{Deserialize, Serialize};

pub const LEXI_RESULT_V1_SCHEMA_VERSION: &str = "lexi.result.v1";
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
pub struct LexiResultV1 {
    pub schema_version: String,
    pub mode: String,
    pub source_language: String,
    pub result_language: String,
    pub headword: String,
    pub translations: Vec<Translation>,
    pub nuance: String,
    pub synonyms: Vec<RelatedWord>,
    pub warnings: Vec<String>,
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
        validate_non_empty("translations", self.translations.len())?;
        validate_max_len("translations", self.translations.len(), 3)?;
        validate_max_len("synonyms", self.synonyms.len(), 4)?;

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

#[cfg(test)]
mod tests {
    use super::{parse_lexi_result_v1, LexiResultV1, LEXI_RESULT_V1_SCHEMA_VERSION};
    use crate::errors::AppErrorCode;

    fn valid_result() -> LexiResultV1 {
        LexiResultV1 {
            schema_version: LEXI_RESULT_V1_SCHEMA_VERSION.to_string(),
            mode: "word-study".to_string(),
            source_language: "auto".to_string(),
            result_language: "ja".to_string(),
            headword: "subtle".to_string(),
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
}
