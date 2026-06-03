# ja2en Implementation Plan

## Goal

Japanese-to-English word lookup should handle a Japanese word or short phrase and
return a compact list of plausible English word candidates. This is separate from
the existing English `word-study` flow and from DeepL sentence translation.

The first version should optimize for lookup while writing Japanese: select a
Japanese term such as `採用`, `微妙`, or `責任を取る`, invoke Lexi, and see English
candidates with enough context to choose the right word. Each English candidate
must include its own example sentence.

## Product Behavior

- Route short Japanese selections to a new `jp-word-candidates` result mode.
- Keep sentence-like Japanese selections in the existing `text-translation` mode.
- Keep English words and short English phrases in the existing `word-study` mode.
- Show the selected Japanese query in the popup header while results stream.
- Render candidate rows with the English term as the primary text, then part of
  speech, Japanese nuance, confidence, and one short example.
- Do not show the existing headword pronunciation button for the whole result.
  Candidate-level pronunciation can be added later by reusing the Rust SAPI
  command if it becomes part of the candidate-row design.

Out of scope for v1:

- Translating long Japanese sentences into English.
- Turning each candidate into a separate saved English vocabulary card.
- Adding a primary mode switch in settings.
- Syncing raw prompts, provider raw responses, full selected sentences, or
  clipboard contents.

## Result Contract

Add a new schema version:

```json
{
  "schemaVersion": "lexi.jp-word-candidates.v1",
  "mode": "jp-word-candidates",
  "sourceLanguage": "ja",
  "resultLanguage": "en",
  "query": "採用",
  "candidates": [
    {
      "term": "adopt",
      "partOfSpeech": "動詞",
      "japaneseNuance": "方針・方法・制度などを選んで使い始める",
      "usageNote": "案や制度を公式に取り入れる文脈で使う。",
      "example": {
        "sentence": "The team adopted a new policy.",
        "japanese": "チームは新しい方針を採用した。"
      },
      "confidence": "high"
    }
  ],
  "warnings": []
}
```

Field rules:

- `query`: normalized Japanese lookup term, max 32 characters.
- `candidates`: 1 to 8 items. Prefer 3 to 6 useful candidates over padding.
- `term`: an English lemma or short fixed phrase, max 48 characters. Prefer
  single words; allow short phrases only when a single word would be unnatural.
- `partOfSpeech`: reuse the existing Japanese part-of-speech enum where possible
  (`名詞`, `動詞`, `形容詞`, `副詞`, `句`, `成句`, etc.).
- `japaneseNuance`: concise meaning of this candidate in Japanese, max 80 chars.
- `usageNote`: one short Japanese sentence explaining when to choose this
  candidate over nearby candidates, max 120 chars.
- `example`: required for every candidate. `sentence` is a short natural English
  sentence using that candidate; `japanese` is its Japanese translation.
- `confidence`: enum `high`, `medium`, or `low`; do not use numeric model
  confidence.
- `warnings`: empty unless the Japanese query is context-dependent, ambiguous,
  slang-like, or too short to rank confidently.

Prompt quality rules:

- Do not output Japanese translations as candidates.
- Do not list inflected English forms as separate candidates; use lemmas.
- Merge duplicates by lemma and sense. For example, do not list both `use` and
  `utilize` unless register is the actual useful distinction.
- Rank candidates by practical usefulness for a Japanese user choosing an English
  word, not by literalness alone.
- Keep examples generic and do not quote surrounding selected context.

## Backend Implementation

Schema and parsing:

- Add `LEXI_JP_WORD_CANDIDATES_V1_SCHEMA_VERSION`.
- Add Rust structs for `JapaneseWordCandidatesResultV1`,
  `EnglishCandidate`, `CandidateExample`, and `CandidateConfidence`.
- Extend the `LexiResult` enum with `JapaneseWordCandidates`.
- Add `parse_japanese_word_candidates_result_v1` with the same strict validation
  style as `parse_lexi_result_v1`.
- Mirror the new types and validation in `src/lib/schema.ts`.

Mode routing:

- Extend `TransformMode` with `JapaneseWordCandidates`.
- Evaluate sentence-like input first:
  - newline or carriage return;
  - terminal punctuation `. ? ! 。 ？ ！`;
  - clause punctuation `, ; : 、 ， ； ：`;
  - five or more whitespace-delimited tokens;
  - Japanese text over 32 non-whitespace characters.
- If not sentence-like and the selection contains Hiragana, Katakana, or CJK
  ideographs, route to `JapaneseWordCandidates`.
- Preserve existing English routing to `WordStudy`.
- Keep empty selections mapped to the existing empty-selection error path.

Provider flow:

- Reuse the configured word provider (`Gemini`, `OpenAI`, or `Mock`) for
  `JapaneseWordCandidates`. Do not use DeepL for this mode.
- Add `build_japanese_word_candidates_prompt`.
- Add OpenAI JSON Schema and Gemini response schema for
  `lexi.jp-word-candidates.v1`.
- Refactor provider calls so the selected transform mode chooses the prompt,
  response schema, streaming partial extractor, and final parser.
- Mock provider should return deterministic candidates for at least `採用`,
  `微妙`, and a generic fallback.

Streaming:

- Replace the word-study-only partial payload with a mode-aware partial payload,
  or add optional `query` and `candidates` fields while preserving existing
  `headword`, `translations`, `synonyms`, and `idioms`.
- Emit completed candidate rows as soon as they can be safely parsed from a JSON
  fragment.
- The final `ready` event must always use the strictly validated result.

## Frontend Implementation

Types and state:

- Extend `ResultMode` with `jp-word-candidates`.
- Extend `PopupState.streaming` to carry mode-specific partial data.
- Add rendering branches for:
  - ready `jp-word-candidates`;
  - streaming `jp-word-candidates`;
  - fallback skeletons while no candidate rows have streamed.

UI:

- Add `JapaneseWordCandidatesBody`.
- Candidate row layout:
  - English term, visually primary;
  - compact part-of-speech label and confidence;
  - Japanese nuance and usage note;
  - English example sentence and Japanese example translation.
- Keep rows dense enough for a desktop popup; avoid turning each candidate into a
  large standalone card.
- Copy/retry/close/settings behavior should stay consistent with existing result
  modes.
- `speakableHeadwordForState` should return `null` for candidate-list results
  until candidate-level pronunciation is deliberately added.

Acceptance examples:

- `採用` should produce candidates such as `adopt`, `hire`, `employ`, and
  `accept`, with examples that make the object difference clear.
- `微妙` should produce candidates such as `subtle`, `questionable`, `awkward`,
  and `delicate`, with warnings or usage notes about context dependence.
- `責任を取る` may include short phrases such as `take responsibility` and `be
  accountable`; phrases are acceptable here because a single word would be less
  natural.

## Vocabulary Cache and Supabase Sync

Use the existing vocabulary architecture instead of adding a separate sync
system. A ja2en result is a validated vocabulary card whose canonical lexeme is
the Japanese query and whose card content is the English candidate list.

Local SQLite mapping:

- `user_lexemes.language`: `ja`.
- `user_lexemes.canonical_text`: normalized Japanese `query`.
- `user_lexemes.canonical_key`: normalized lookup key for the Japanese query.
- `user_lexemes.part_of_speech`: `null` for v1; candidate part-of-speech remains
  inside `content_json.candidates[]`.
- `card_snapshots.schema_version`: `lexi.jp-word-candidates.v1`.
- `card_snapshots.result_language`: `en`.
- `card_snapshots.content_json`: the validated result object.
- `lexeme_forms`: canonical alias for the query, plus an observed alias only when
  the captured short phrase normalizes differently from the canonical query.

Backend save/load API:

- Add `save_japanese_word_candidates_result(app, result, provider, model,
  selected_text)`.
- Add `load_japanese_word_candidates(app, selected_text)` that looks up
  `language = 'ja'`, `result_language = 'en'`, and
  `schema_version = 'lexi.jp-word-candidates.v1'`.
- Refactor shared cache code so lookup language and parser are parameters rather
  than hard-coded to English `LexiResultV1`.
- Keep automatic save after validated provider results, matching current
  word-study behavior.
- Schedule sync after a successful save, as `persist_word_study_result` already
  does.

Mutation payload:

```json
{
  "language": "ja",
  "canonicalText": "採用",
  "canonicalKey": "採用",
  "resultLanguage": "en",
  "schemaVersion": "lexi.jp-word-candidates.v1",
  "provider": "gemini",
  "model": "gemini-2.5-flash-lite",
  "content": {
    "schemaVersion": "lexi.jp-word-candidates.v1",
    "mode": "jp-word-candidates",
    "sourceLanguage": "ja",
    "resultLanguage": "en",
    "query": "採用",
    "candidates": []
  },
  "forms": [
    {
      "formText": "採用",
      "formKey": "採用",
      "relation": "canonical",
      "source": "provider"
    }
  ]
}
```

Supabase:

- Do not add new tables for v1.
- Add a migration that updates `apply_vocabulary_mutation` to be schema-aware:
  - keep existing `lexi.result.v1` behavior for English word-study cards;
  - allow `lexi.jp-word-candidates.v1`;
  - derive `part_of_speech` from `content.translations[0].note` only for
    `lexi.result.v1`;
  - always write the canonical `lexeme_forms` row for any saved card;
  - add irregular aliases only when `content.inflections` exists and the saved
    card is an English word-study card.
- Keep `pull_vocabulary_changes` unchanged if the payload remains
  schema-agnostic.
- Keep `lookup_vocabulary_card` compatible by passing `language = 'ja'` and
  `result_language = 'en'` if remote lookup is ever used; the current desktop
  path should still prefer SQLite after bootstrap.
- RLS remains unchanged: all ja2en rows are user-owned vocabulary rows scoped by
  authenticated `user_id`.

Bootstrap and pull:

- Update local pull handling so `card_snapshot` changes branch by
  `schemaVersion`.
- For `lexi.jp-word-candidates.v1`, parse into
  `JapaneseWordCandidatesResultV1`, store `content_json`, and ensure only
  Japanese canonical/observed aliases.
- Do not call `ensure_lexeme_forms_from_content_json` with a word-study parser on
  ja2en content.
- Existing devices that bootstrap after the migration should receive ja2en cards
  naturally because they already copy `user_lexemes`, `lexeme_forms`, and active
  `card_snapshots`.

Privacy:

- The Japanese query is stored because it is the canonical vocabulary lookup
  term. Keep the mode restricted to short word/phrase input so this does not
  become sentence-history sync.
- Do not store prompt text, raw provider response, full surrounding context, API
  keys, or clipboard contents.
- If a Japanese selection is sentence-like, route it to `text-translation` and do
  not save it as a vocabulary card.

## Implementation Order

1. Add docs-facing requirements for the new mode after this plan is accepted.
2. Add Rust and TypeScript schema types and validation tests.
3. Add mode classification and provider prompt/schema selection.
4. Add mock provider output for ja2en candidate lists.
5. Add frontend ready/streaming rendering.
6. Refactor local vocabulary save/load to support `language = 'ja'` candidate
   cards.
7. Add Supabase RPC migration for schema-aware alias handling.
8. Update bootstrap/pull paths and local repair helpers to branch by schema
   version.
9. Add integration tests around local save, pending mutation payloads, sync pull,
   and cache hits.
10. Manually verify the popup with short Japanese terms and sentence-like
    Japanese text.

## Test Plan

Rust schema tests:

- Accept a valid result with multiple candidates and required examples.
- Reject empty `query`, zero candidates, more than eight candidates, empty
  `term`, missing `example`, invalid `confidence`, and overlong fields.

Rust routing tests:

- `採用`, `微妙`, `責任を取る` -> `JapaneseWordCandidates`.
- `これはテストです。`, `A、B`, newline Japanese, and long Japanese text ->
  `TextTranslation`.
- `subtle`, `take off`, `one two three four` -> existing `WordStudy`.
- `one two three four five` -> existing `TextTranslation`.

Provider tests:

- OpenAI and Gemini schemas match Rust validation cardinality.
- Prompts require examples per candidate and prohibit Japanese candidates.
- Streaming extraction can emit partial candidate rows without accepting invalid
  final JSON.

Frontend tests:

- `validateLexiResultV1` accepts `lexi.jp-word-candidates.v1`.
- Ready state renders candidate terms, part-of-speech labels, Japanese nuances,
  and each candidate's example sentence.
- Streaming state shows query and partial candidates.
- Headword voice button is absent for `jp-word-candidates`.
- Existing word-study and text-translation tests continue to pass.

Vocabulary and sync tests:

- Local save inserts `user_lexemes.language = 'ja'` and
  `card_snapshots.result_language = 'en'`.
- Pending mutation payload does not include raw prompt or provider raw response.
- Local cache hit returns ja2en content by Japanese query.
- Ambiguous duplicate English candidates are preserved inside the snapshot, but
  lookup still resolves by the Japanese query.
- Supabase migration accepts ja2en `save_card_snapshot` payloads.
- Pulling a ja2en `card_snapshot` stores the snapshot and does not try to parse
  it as `LexiResultV1`.

Verification commands:

```powershell
rtk cargo check
rtk cargo test
rtk pnpm test -- --run
rtk pnpm build
rtk git diff --check
```

