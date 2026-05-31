export const LEXI_RESULT_V1_SCHEMA_VERSION = "lexi.result.v1";
export const LEXI_TEXT_TRANSLATION_V1_SCHEMA_VERSION =
  "lexi.text-translation.v1";
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
export type TranslationSenseKind = "dictionary" | "inflection";

export interface Translation {
  text: string;
  note: TranslationNote | null;
  example: ExampleSentence;
  senseKind?: TranslationSenseKind | null;
  baseWord?: string | null;
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

export interface Idiom {
  idiom: string;
  japanese: string;
  example: string;
}

export type InflectionKind = "plural" | "past" | "pastParticiple";

export interface Inflection {
  kind: InflectionKind;
  form: string;
}

export interface LexiResultV1 {
  schemaVersion: typeof LEXI_RESULT_V1_SCHEMA_VERSION;
  mode: "word-study";
  sourceLanguage: string;
  resultLanguage: string;
  headword: string;
  inflections: Inflection[];
  translations: Translation[];
  nuance: string;
  synonyms: RelatedWord[];
  idioms: Idiom[];
  warnings: string[];
}

export interface TranslationSegment {
  source: string;
  translation: string;
}

export interface TextTranslationResultV1 {
  schemaVersion: typeof LEXI_TEXT_TRANSLATION_V1_SCHEMA_VERSION;
  mode: "text-translation";
  sourceLanguage: string;
  detectedSourceLanguage: string | null;
  resultLanguage: string;
  translatedText: string;
  segments: TranslationSegment[];
  warnings: string[];
}

export type LexiResult = LexiResultV1 | TextTranslationResultV1;

export type LexiResultValidation =
  | { ok: true; result: LexiResult }
  | { ok: false; reason: string };

export function validateLexiResultV1(value: unknown): LexiResultValidation {
  if (!isRecord(value)) {
    return { ok: false, reason: "result is not an object" };
  }

  if (value.schemaVersion === LEXI_TEXT_TRANSLATION_V1_SCHEMA_VERSION) {
    return validateTextTranslationResultV1(value);
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

  if (charCount(value.headword) > 48) {
    return { ok: false, reason: "headword exceeds maximum length" };
  }

  if (charCount(value.nuance) > 120) {
    return { ok: false, reason: "nuance exceeds maximum length" };
  }

  if (!isTranslationArray(value.translations) || value.translations.length === 0) {
    return {
      ok: false,
      reason: "translations must be a non-empty array with part-of-speech notes",
    };
  }

  if (!isInflectionArray(value.inflections)) {
    return { ok: false, reason: "inflections must be an array of irregular forms" };
  }

  if (!isRelatedWordArray(value.synonyms)) {
    return { ok: false, reason: "synonyms must be an array of related words" };
  }

  if (!isIdiomArray(value.idioms)) {
    return { ok: false, reason: "idioms must be an array of idiom entries" };
  }

  if (!isStringArray(value.warnings)) {
    return { ok: false, reason: "warnings must be an array of strings" };
  }

  if (!areWarningsValid(value.warnings)) {
    return { ok: false, reason: "warnings must be an array of strings" };
  }

  return { ok: true, result: value as unknown as LexiResultV1 };
}

function validateTextTranslationResultV1(
  value: Record<string, unknown>,
): LexiResultValidation {
  const required = [
    "mode",
    "sourceLanguage",
    "resultLanguage",
    "translatedText",
  ] as const;

  for (const field of required) {
    if (!isNonEmptyString(value[field])) {
      return { ok: false, reason: `required field '${field}' is missing or empty` };
    }
  }

  if (value.mode !== "text-translation") {
    return { ok: false, reason: "unsupported mode" };
  }

  if (
    value.detectedSourceLanguage !== null &&
    value.detectedSourceLanguage !== undefined &&
    !isNonEmptyString(value.detectedSourceLanguage)
  ) {
    return {
      ok: false,
      reason: "detectedSourceLanguage must be null or a language code",
    };
  }

  if (
    typeof value.detectedSourceLanguage === "string" &&
    charCount(value.detectedSourceLanguage) > 16
  ) {
    return {
      ok: false,
      reason: "detectedSourceLanguage exceeds maximum length",
    };
  }

  if (charCount(String(value.translatedText)) > 4000) {
    return { ok: false, reason: "translatedText exceeds maximum length" };
  }

  if (!isTranslationSegmentArray(value.segments)) {
    return { ok: false, reason: "segments must be a translation segment array" };
  }

  if (!isStringArray(value.warnings)) {
    return { ok: false, reason: "warnings must be an array of strings" };
  }

  if (!areWarningsValid(value.warnings)) {
    return { ok: false, reason: "warnings must be an array of strings" };
  }

  return { ok: true, result: value as unknown as TextTranslationResultV1 };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function charCount(value: string): number {
  return Array.from(value).length;
}

function areWarningsValid(value: unknown): value is string[] {
  return (
    Array.isArray(value) &&
    value.every(
      (item) => typeof item === "string" && item.trim().length > 0 && charCount(item) <= 120,
    )
  );
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === "string");
}

function isTranslationSenseKind(value: unknown): value is TranslationSenseKind {
  return value === "dictionary" || value === "inflection";
}

function isTranslationArray(value: unknown): value is Translation[] {
  return (
    Array.isArray(value) &&
    value.length <= 3 &&
    value.every((item) => {
      if (
        !isRecord(item) ||
        !isNonEmptyString(item.text) ||
        charCount(item.text) > 48 ||
        !(isTranslationNote(item.note) || item.note === null) ||
        !isExampleSentence(item.example)
      ) {
        return false;
      }

      const senseKind = item.senseKind;
      if (
        senseKind !== undefined &&
        senseKind !== null &&
        !isTranslationSenseKind(senseKind)
      ) {
        return false;
      }

      const baseWord = item.baseWord;
      if (baseWord !== undefined && baseWord !== null && typeof baseWord !== "string") {
        return false;
      }

      if (senseKind === "inflection" && !isNonEmptyString(baseWord)) {
        return false;
      }

      if (
        senseKind !== "inflection" &&
        baseWord !== undefined &&
        baseWord !== null &&
        typeof baseWord === "string" &&
        baseWord.trim().length > 0
      ) {
        return false;
      }

      return true;
    })
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

function isInflectionArray(value: unknown): value is Inflection[] {
  return (
    Array.isArray(value) &&
    value.length <= 3 &&
    value.every(
      (item) =>
        isRecord(item) &&
        isInflectionKind(item.kind) &&
        isNonEmptyString(item.form),
    )
  );
}

function isInflectionKind(value: unknown): value is InflectionKind {
  return value === "plural" || value === "past" || value === "pastParticiple";
}

function isRelatedWordArray(value: unknown): value is RelatedWord[] {
  return (
    Array.isArray(value) &&
    value.length <= 4 &&
    value.every(
      (item) =>
        isRecord(item) &&
        isNonEmptyString(item.term) &&
        isNonEmptyString(item.japanese) &&
        isNonEmptyString(item.usageComparison),
    )
  );
}

function isIdiomArray(value: unknown): value is Idiom[] {
  return (
    Array.isArray(value) &&
    value.length <= 3 &&
    value.every(
      (item) =>
        isRecord(item) &&
        isNonEmptyString(item.idiom) &&
        isNonEmptyString(item.japanese) &&
        isNonEmptyString(item.example),
    )
  );
}

function isTranslationSegmentArray(value: unknown): value is TranslationSegment[] {
  return (
    Array.isArray(value) &&
    value.length <= 24 &&
    value.every(
      (item) =>
        isRecord(item) &&
        isNonEmptyString(item.source) &&
        isNonEmptyString(item.translation) &&
        charCount(item.source) <= 1000 &&
        charCount(item.translation) <= 1000,
    )
  );
}
