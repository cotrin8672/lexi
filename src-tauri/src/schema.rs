use crate::errors::AppError;
use serde::{Deserialize, Serialize};

pub const LEXI_RESULT_V1_SCHEMA_VERSION: &str = "lexi.result.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Translation {
    pub text: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedWord {
    pub term: String,
    pub japanese: String,
    pub nuance: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageComparison {
    pub terms: Vec<String>,
    pub explanation: String,
    pub examples: Vec<String>,
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
    pub usage_comparisons: Vec<UsageComparison>,
    pub antonyms: Vec<RelatedWord>,
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
        validate_non_empty("translations", self.translations.len())?;

        for (index, translation) in self.translations.iter().enumerate() {
            validate_required(&format!("translations[{index}].text"), &translation.text)?;
            if let Some(note) = &translation.note {
                validate_required(&format!("translations[{index}].note"), note)?;
            }
        }

        for (index, synonym) in self.synonyms.iter().enumerate() {
            validate_related_word(&format!("synonyms[{index}]"), synonym)?;
        }

        for (index, comparison) in self.usage_comparisons.iter().enumerate() {
            validate_non_empty(
                &format!("usageComparisons[{index}].terms"),
                comparison.terms.len(),
            )?;
            for (term_index, term) in comparison.terms.iter().enumerate() {
                validate_required(
                    &format!("usageComparisons[{index}].terms[{term_index}]"),
                    term,
                )?;
            }
            validate_required(
                &format!("usageComparisons[{index}].explanation"),
                &comparison.explanation,
            )?;
            for (example_index, example) in comparison.examples.iter().enumerate() {
                validate_required(
                    &format!("usageComparisons[{index}].examples[{example_index}]"),
                    example,
                )?;
            }
        }

        for (index, antonym) in self.antonyms.iter().enumerate() {
            validate_related_word(&format!("antonyms[{index}]"), antonym)?;
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

fn validate_related_word(field: &str, value: &RelatedWord) -> Result<(), AppError> {
    validate_required(&format!("{field}.term"), &value.term)?;
    validate_required(&format!("{field}.japanese"), &value.japanese)?;
    validate_required(&format!("{field}.nuance"), &value.nuance)?;

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
                note: Some("文脈により肯定的にも否定的にも使われる".to_string()),
            }],
            nuance: "Small or delicate enough that it may be hard to notice.".to_string(),
            synonyms: vec![super::RelatedWord {
                term: "delicate".to_string(),
                japanese: "繊細な".to_string(),
                nuance: "Sensitivity or fine detail is emphasized.".to_string(),
            }],
            usage_comparisons: vec![super::UsageComparison {
                terms: vec!["subtle".to_string(), "slight".to_string()],
                explanation:
                    "subtle implies hard-to-notice nuance; slight mainly means small in amount."
                        .to_string(),
                examples: vec!["There is a subtle difference.".to_string()],
            }],
            antonyms: vec![super::RelatedWord {
                term: "obvious".to_string(),
                japanese: "明らかな".to_string(),
                nuance: "Easy to notice or understand.".to_string(),
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
            "translations": [{"text": "微妙な", "note": null}],
            "nuance": "Small or delicate enough that it may be hard to notice.",
            "synonyms": [],
            "usageComparisons": [],
            "antonyms": [],
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
}
