import { describe, expect, it } from "vitest";
import {
  LEXI_RESULT_V1_SCHEMA_VERSION,
  LEXI_TEXT_TRANSLATION_V1_SCHEMA_VERSION,
  TRANSLATION_NOTE_VALUES,
  validateLexiResultV1,
  type LexiResultV1,
  type TextTranslationResultV1,
} from "./schema";

function mockWordStudyResult(): LexiResultV1 {
  return {
    schemaVersion: LEXI_RESULT_V1_SCHEMA_VERSION,
    mode: "word-study",
    sourceLanguage: "auto",
    resultLanguage: "ja",
    headword: "subtle",
    inflections: [],
    translations: [
      {
        text: "微妙な",
        note: TRANSLATION_NOTE_VALUES[2],
        example: {
          sentence: "She noticed a subtle change.",
          japanese: "彼女は微妙な変化に気づいた。",
        },
      },
    ],
    nuance: "Understated rather than obvious.",
    synonyms: [],
    idioms: [],
    warnings: [],
  };
}

function mockTextTranslationResult(): TextTranslationResultV1 {
  return {
    schemaVersion: LEXI_TEXT_TRANSLATION_V1_SCHEMA_VERSION,
    mode: "text-translation",
    sourceLanguage: "auto",
    detectedSourceLanguage: "EN",
    resultLanguage: "ja",
    translatedText: "これはテストです。",
    segments: [
      {
        source: "This is a test.",
        translation: "これはテストです。",
      },
    ],
    warnings: [],
  };
}

describe("validateLexiResultV1 word-study", () => {
  it("rejects unsupported schema versions", () => {
    const result = mockWordStudyResult();
    result.schemaVersion = "lexi.result.v2" as typeof result.schemaVersion;

    expect(validateLexiResultV1(result)).toEqual({
      ok: false,
      reason: "unsupported schemaVersion",
    });
  });

  it("allows null translation notes", () => {
    const result = mockWordStudyResult();
    result.translations[0].note = null;

    expect(validateLexiResultV1(result).ok).toBe(true);
  });

  it("rejects more than three inflections", () => {
    const result = mockWordStudyResult();
    result.inflections = [
      { kind: "plural", form: "children" },
      { kind: "past", form: "went" },
      { kind: "pastParticiple", form: "gone" },
      { kind: "past", form: "ran" },
    ];

    expect(validateLexiResultV1(result)).toEqual({
      ok: false,
      reason: "inflections must be an array of irregular forms",
    });
  });

  it("rejects more than three idioms", () => {
    const result = mockWordStudyResult();
    result.idioms = [
      {
        idiom: "a",
        japanese: "a",
        example: "a",
      },
      {
        idiom: "b",
        japanese: "b",
        example: "b",
      },
      {
        idiom: "c",
        japanese: "c",
        example: "c",
      },
      {
        idiom: "d",
        japanese: "d",
        example: "d",
      },
    ];

    expect(validateLexiResultV1(result)).toEqual({
      ok: false,
      reason: "idioms must be an array of idiom entries",
    });
  });
});

describe("validateLexiResultV1 text-translation", () => {
  it("rejects unsupported schema versions", () => {
    const result = mockTextTranslationResult();
    result.schemaVersion =
      "lexi.text-translation.v2" as typeof result.schemaVersion;

    expect(validateLexiResultV1(result)).toEqual({
      ok: false,
      reason: "unsupported schemaVersion",
    });
  });

  it("rejects empty translated text", () => {
    const result = mockTextTranslationResult();
    result.translatedText = "   ";

    expect(validateLexiResultV1(result)).toEqual({
      ok: false,
      reason: "required field 'translatedText' is missing or empty",
    });
  });

  it("accepts null detected source language", () => {
    const result = mockTextTranslationResult();
    result.detectedSourceLanguage = null;

    expect(validateLexiResultV1(result).ok).toBe(true);
  });

  it("accepts empty segments arrays", () => {
    const result = mockTextTranslationResult();
    result.segments = [];

    expect(validateLexiResultV1(result).ok).toBe(true);
  });

  it("rejects invalid segment entries", () => {
    const result = mockTextTranslationResult();
    result.segments = [{ source: " ", translation: "訳文" }];

    expect(validateLexiResultV1(result)).toEqual({
      ok: false,
      reason: "segments must be a translation segment array",
    });
  });
});

describe("validateLexiResultV1 frontend/backend parity", () => {
  it("rejects overlong nuance to match the Rust schema limit", () => {
    const result = mockWordStudyResult();
    result.nuance = "a".repeat(121);

    expect(validateLexiResultV1(result)).toEqual({
      ok: false,
      reason: "nuance exceeds maximum length",
    });
  });

  it("rejects more than three translations to match the Rust schema limit", () => {
    const result = mockWordStudyResult();
    result.translations = [
      result.translations[0],
      result.translations[0],
      result.translations[0],
      result.translations[0],
    ];

    expect(validateLexiResultV1(result)).toEqual({
      ok: false,
      reason: "translations must be a non-empty array with part-of-speech notes",
    });
  });

  it("rejects overlong headwords to match the Rust schema limit", () => {
    const result = mockWordStudyResult();
    result.headword = "a".repeat(49);

    expect(validateLexiResultV1(result)).toEqual({
      ok: false,
      reason: "headword exceeds maximum length",
    });
  });

  it("rejects more than four synonyms to match the Rust schema limit", () => {
    const result = mockWordStudyResult();
    result.synonyms = Array.from({ length: 5 }, (_, index) => ({
      term: `term-${index}`,
      japanese: "意味",
      usageComparison: "comparison",
    }));

    expect(validateLexiResultV1(result)).toEqual({
      ok: false,
      reason: "synonyms must be an array of related words",
    });
  });
});
