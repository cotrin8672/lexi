export const LEXI_RESULT_V1_SCHEMA_VERSION = "lexi.result.v1";

export interface Translation {
  text: string;
  note: string | null;
}

export interface RelatedWord {
  term: string;
  japanese: string;
  nuance: string;
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
    return { ok: false, reason: "translations must be a non-empty array" };
  }

  if (!isRelatedWordArray(value.synonyms) || value.synonyms.length === 0) {
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
        (typeof item.note === "string" || item.note === null),
    )
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
        isNonEmptyString(item.nuance) &&
        isNonEmptyString(item.usageComparison),
    )
  );
}
