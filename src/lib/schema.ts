export const LEXI_RESULT_V1_SCHEMA_VERSION = "lexi.result.v1";
export const TRANSLATION_NOTE_VALUES = [
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
] as const;

export type TranslationNote = (typeof TRANSLATION_NOTE_VALUES)[number];

export interface Translation {
  text: string;
  note: TranslationNote | null;
  example: ExampleSentence;
}

export interface ExampleSentence {
  sentence: string;
  japanese: string;
}

export interface RelatedWord {
  term: string;
  japanese: string;
  usageComparison: string;
}

export interface LexiResultV1 {
  schemaVersion: typeof LEXI_RESULT_V1_SCHEMA_VERSION;
  mode: string;
  sourceLanguage: string;
  resultLanguage: string;
  headword: string;
  translations: Translation[];
  nuance: string;
  synonyms: RelatedWord[];
  warnings: string[];
}

export type LexiResultValidation =
  | { ok: true; result: LexiResultV1 }
  | { ok: false; reason: string };

export function validateLexiResultV1(value: unknown): LexiResultValidation {
  if (!isRecord(value)) {
    return { ok: false, reason: "result is not an object" };
  }

  if (value.schemaVersion !== LEXI_RESULT_V1_SCHEMA_VERSION) {
    return { ok: false, reason: "unsupported schemaVersion" };
  }

  const required = [
    "mode",
    "sourceLanguage",
    "resultLanguage",
    "headword",
    "nuance",
  ] as const;

  for (const field of required) {
    if (!isNonEmptyString(value[field])) {
      return { ok: false, reason: `required field '${field}' is missing or empty` };
    }
  }

  if (!isTranslationArray(value.translations) || value.translations.length === 0) {
    return {
      ok: false,
      reason: "translations must be a non-empty array with part-of-speech notes",
    };
  }

  if (!isRelatedWordArray(value.synonyms)) {
    return { ok: false, reason: "synonyms must be an array of related words" };
  }

  if (!isStringArray(value.warnings)) {
    return { ok: false, reason: "warnings must be an array of strings" };
  }

  return { ok: true, result: value as unknown as LexiResultV1 };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === "string");
}

function isTranslationArray(value: unknown): value is Translation[] {
  return (
    Array.isArray(value) &&
    value.every(
      (item) =>
        isRecord(item) &&
        isNonEmptyString(item.text) &&
        (isTranslationNote(item.note) || item.note === null) &&
        isExampleSentence(item.example),
    )
  );
}

function isTranslationNote(value: unknown): value is TranslationNote {
  return (
    typeof value === "string" &&
    TRANSLATION_NOTE_VALUES.includes(value as TranslationNote)
  );
}

function isExampleSentence(value: unknown): value is ExampleSentence {
  return (
    isRecord(value) &&
    isNonEmptyString(value.sentence) &&
    isNonEmptyString(value.japanese)
  );
}

function isRelatedWordArray(value: unknown): value is RelatedWord[] {
  return (
    Array.isArray(value) &&
    value.every(
      (item) =>
        isRecord(item) &&
        isNonEmptyString(item.term) &&
        isNonEmptyString(item.japanese) &&
        isNonEmptyString(item.usageComparison),
    )
  );
}
