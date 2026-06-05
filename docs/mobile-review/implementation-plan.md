# Mobile Review Implementation Plan

Status: draft

## Goal

Build a mobile review app that turns Lexi's saved vocabulary cards into useful
practice without creating a separate generated-question store.

The review loop should optimize for efficient recall practice. Problems the user
often misses should be selected more often through weighted sampling, while
mastered problems should remain in the pool at low probability. The app should
not depend on due-time intervals or fixed spaced-repetition queues for the first
version.

## Product Scope

In scope for the first mobile review version:

- English sentence reorder questions from saved example sentences.
- English headword to Japanese meaning multiple-choice questions.
- Usage distinction questions from near-word and comparison data.
- Inflection questions from saved irregular and generated form aliases.
- Dynamic question generation from active vocabulary cards.
- Local-only answer statistics and optional local attempt logs.
- Weighted random question selection biased toward weak knowledge points.

Out of scope for the first version:

- Persisting generated question bodies in Supabase.
- Syncing review statistics across devices.
- Editing vocabulary cards from mobile.
- Generating new vocabulary cards from mobile selections.
- Long-form reading, document history, or raw selected-text history.
- LLM calls during normal quiz rendering.

Generated question persistence can be added later if the app needs curated
question variants, teacher-authored questions, or cross-device analysis of exact
option sets. It is not needed for the initial review loop because existing cards
already contain multiple translations, example sentences, similar words, and
form aliases.

## Data Sources

The mobile app should treat Supabase vocabulary data as read-only source data:

- `user_lexemes`: canonical vocabulary items and language keys.
- `card_snapshots`: active structured card content.
- `lexeme_forms`: canonical, regular, and irregular form aliases.

For personal-use MVPs, the mobile app can read the existing owner-scoped
Supabase data after Google sign-in. For wider release, the RLS model must move
from admin-only personal access to normal `auth.uid() = user_id` ownership before
the mobile app is distributed.

The mobile app should cache the fetched vocabulary data locally for offline
review. A simple refresh on app start is enough for v1. Incremental pull by
`vocabulary_changes.server_revision` can be added later if full sync efficiency
matters.

## Question Identity

The most important rule is that answer history belongs to a stable knowledge
point, not to one rendered question instance.

Choices, shuffled tokens, and distractors may change on every attempt. The
`questionKey` must not change when those presentation details change.

Recommended keys:

```text
meaning:v1:<lexemeId>:<hash(normalizedJapaneseMeaning)>
reorder:v1:<lexemeId>:<hash(normalizedEnglishSentence)>
usage:v1:<lexemeId>:<hash(normalizedHeadword + normalizedOtherTerm + normalizedComparison)>
inflection:v1:<lexemeId>:<relation>:<formKey>
```

Notes:

- Do not include the multiple-choice distractor set in `questionKey`.
- Avoid translation array indexes as the only identity. If card regeneration
  changes ordering, the same meaning should still map to the same key.
- Include a schema prefix such as `v1` so future key changes can coexist with old
  local stats.
- For analytics only, attempt logs may store rendered option keys and selected
  answer keys. The scheduler must still update stats by `questionKey`.

## Question Types

### Meaning Four-Choice

Source:

- `card_snapshots.content.headword`
- `card_snapshots.content.translations[]`

Question:

- Show the English headword and ask for the Japanese meaning.
- The correct answer is one translation sense.
- Distractors come from other cards' Japanese translation senses.

Distractor rules:

- Prefer distractors with the same part-of-speech note when available.
- Exclude the same `lexemeId`.
- Exclude duplicate normalized answer text.
- Fall back to all eligible translations when same-part-of-speech candidates are
  insufficient.
- Every rendered attempt must have one correct answer and three distinct
  distractors. If four unique choices cannot be built, skip the candidate.

Stats:

- Update the meaning `questionKey` for the correct sense.
- Do not create separate stats for different distractor sets.
- Attempt logs may store `optionAnswerKeys` to analyze which distractors fooled
  the user later.

### Sentence Reorder

Source:

- `translations[].example.sentence`
- Optional later sources: idiom examples, synonym examples, and ja2en candidate
  examples when they are useful English practice.

Question:

- Show the Japanese translation or a short prompt.
- Shuffle English tokens and ask the user to restore the sentence.

Tokenization for v1 can stay simple:

- Split on whitespace.
- Keep punctuation attached to the preceding token.
- Reject sentences that are too short, too long, or contain awkward quotation and
  bracket patterns.
- Suggested first filter: 4 to 12 tokens after splitting.
- Re-shuffle until the order differs from the original, with a bounded retry.

Stats:

- Key by normalized English sentence hash.
- Dynamic token order does not affect the key.
- A response is correct when the normalized submitted token order equals the
  normalized original sentence.

### Usage Distinction

Source:

- `card_snapshots.content.nuance`
- `card_snapshots.content.synonyms[]`
- `synonyms[].usageComparison`
- `synonyms[].japanese`

Question variants:

- Show the headword nuance and ask which English word best matches it.
- Show a synonym's Japanese meaning or usage note and ask which nearby word is
  more appropriate.
- Show a usage comparison with the answer term hidden.

Distractors:

- Prefer the headword plus the card's own synonym terms.
- Fill from other cards only when a card does not have enough near words.
- Skip candidates whose comparison text is too vague to identify a specific
  answer without memorizing wording.

Stats:

- Key the contrast being practiced, not the exact wording.
- Usage questions are more quality-sensitive than meaning or reorder questions,
  so keep them lower volume until extraction rules are validated.

### Inflection

Source:

- `card_snapshots.content.inflections[]`
- `lexeme_forms` rows with `relation = 'irregular'` or `relation = 'regular'`

Question variants:

- Base to form: "What is the past form of `go`?"
- Form to base: "What is the base form of `went`?"
- Kind recognition: "Which form is `went`?"

Rules:

- Prefer irregular forms for early versions because they are higher value.
- Include regular/generated forms only as lower-priority practice.
- Avoid forms that are identical to the headword.

Stats:

- Key by `lexemeId`, relation, and `formKey`.
- Presentation direction can be logged, but the main weakness should attach to
  the form knowledge point.

## Local Review Storage

Use mobile-local storage for review stats. SQLite is preferred because it keeps
querying and pruning simple, but a smaller app can start with an embedded local
database abstraction if it still supports indexed `questionKey` lookups.

Core table:

```sql
create table question_stats (
  question_key text primary key,
  question_type text not null,
  lexeme_id text not null,
  attempts integer not null default 0,
  correct_count integer not null default 0,
  wrong_count integer not null default 0,
  correct_streak integer not null default 0,
  wrong_streak integer not null default 0,
  difficulty_ema real not null default 0.5,
  last_result text,
  last_reviewed_at text,
  last_seen_sequence integer,
  created_at text not null,
  updated_at text not null
);
```

Optional attempt log:

```sql
create table attempt_logs (
  id text primary key,
  question_key text not null,
  question_type text not null,
  lexeme_id text not null,
  correct integer not null,
  selected_answer_key text,
  correct_answer_key text,
  option_answer_keys_json text,
  reviewed_at text not null
);
```

Attempt logs should be local and prunable. Keep only the latest bounded history
unless later analysis needs long-term data.

## Weighted Question Selection

Every generated candidate remains eligible. Do not use `nextDueAt` as the main
gate. Time may be used only as a soft freshness feature.

Selection flow:

1. Fetch or load active vocabulary cards.
2. Generate `QuestionCandidate` objects for all valid question types.
3. Attach local `question_stats` by `questionKey`.
4. Compute a positive weight for each candidate.
5. Draw one question by weighted random sampling.
6. Render fresh presentation details such as distractors or token order.
7. Update stats after the answer.

Candidate shape:

```ts
type QuestionCandidate = {
  questionKey: string;
  questionType: "meaning" | "reorder" | "usage" | "inflection";
  lexemeId: string;
  sourceHash: string;
  renderSeedHint: string;
};
```

Weight model:

```text
weight =
  base
  * typeBalance
  * weakness
  * recentMistakeBoost
  * masteryPenalty
  * freshnessPenalty
  * lexemeFreshnessPenalty
```

Suggested defaults:

```text
base = 1.0

weakness = 1 + 6 * difficultyEma

recentMistakeBoost =
  lastResult == "wrong" ? 2.5 : 1.0

masteryPenalty =
  correctStreak >= 5 ? 0.15 :
  correctStreak == 4 ? 0.25 :
  correctStreak == 3 ? 0.45 :
  correctStreak == 2 ? 0.70 :
  1.0

freshnessPenalty =
  shownInLast3Questions ? 0.05 :
  shownInLast10Questions ? 0.30 :
  1.0

lexemeFreshnessPenalty =
  sameLexemeInLast3Questions ? 0.25 : 1.0
```

Initial type balance:

```text
meaning: 0.40
reorder: 0.30
usage: 0.20
inflection: 0.10
```

These ratios are multipliers, not hard quotas. If one type has too few valid
candidates, weighted random sampling naturally shifts to the other types without
needing a fallback queue.

For new questions:

```text
difficultyEma = 0.5
attempts = 0
correctStreak = 0
wrongStreak = 0
```

This gives new questions meaningful weight without letting novelty dominate
known weak problems.

## Stats Update

Use an exponential moving average of recent wrongness. This emphasizes recent
mistakes without permanently punishing old failures.

```ts
const outcomeWrong = correct ? 0 : 1;
const alpha = 0.25;
nextDifficultyEma =
  attempts === 0
    ? 0.5 * (1 - alpha) + outcomeWrong * alpha
    : difficultyEma * (1 - alpha) + outcomeWrong * alpha;
```

On correct:

```text
attempts += 1
correctCount += 1
correctStreak += 1
wrongStreak = 0
lastResult = "correct"
```

On wrong:

```text
attempts += 1
wrongCount += 1
wrongStreak += 1
correctStreak = 0
lastResult = "wrong"
```

Always update `lastReviewedAt`, `lastSeenSequence`, and `updatedAt`.

## Mobile Architecture

Default implementation choice:

- Use Kotlin Multiplatform with an Android-first app.
- Use Jetpack Compose for the Android UI.
- Put review domain logic in a shared Kotlin module from the start so it can move
  to iOS, desktop, or another Kotlin target later.
- Keep Android-specific auth redirects, secure storage, and local database wiring
  behind platform adapters.
- Do not force shared UI for v1. Compose Multiplatform can be evaluated later if
  an iOS app becomes a real target, but the first delivery should optimize for a
  polished Android app.

Recommended Android/KMP stack:

- UI: Jetpack Compose, Material 3, AndroidX Lifecycle ViewModel, and StateFlow.
- Shared logic: Kotlin Multiplatform `commonMain` for card parsing, question
  extraction, option generation, weighting, and stats updates.
- Serialization: `kotlinx.serialization` for Supabase JSON payloads and local
  fixture tests.
- Network/auth: Supabase Kotlin client with Auth and PostgREST modules. Use PKCE
  OAuth and Android deep-link handling for Google sign-in.
- HTTP transport: Ktor client through the Supabase Kotlin stack.
- Local database: Room KMP as the first choice for Android-first development and
  future multiplatform reach. SQLDelight remains a reasonable alternative if the
  project later wants SQL-first generated APIs instead of Room entities/DAOs.
- Secure token storage: Android platform secure storage adapter. Keep Supabase
  refresh/access tokens out of normal preferences and logs.
- Build location: start with a separate `apps/mobile-android` app plus
  `shared/review-core`. Fold this into a Gradle workspace only when the first
  Android skeleton is ready.

Mobile modules:

```text
apps/mobile-android/
  app/
    src/main/java/.../review/
      ReviewSessionScreen.kt
      MeaningQuestion.kt
      ReorderQuestion.kt
      UsageQuestion.kt
      InflectionQuestion.kt

shared/review-core/
  src/commonMain/kotlin/
    schema/
      VocabularyCard.kt
      LexiResultV1.kt
      JapaneseWordCandidatesResultV1.kt
    review/
      QuestionCandidate.kt
      QuestionKey.kt
      QuestionExtraction.kt
      OptionGeneration.kt
      Weighting.kt
      StatsUpdate.kt
    storage/
      ReviewStore.kt
      VocabularyRepository.kt
  src/androidMain/kotlin/
    storage/
      AndroidReviewStore.kt
      AndroidVocabularyCache.kt
    supabase/
      AndroidSupabaseClient.kt
```

Data flow:

```text
Supabase active cards
  -> local vocabulary cache
  -> question extraction
  -> local stats join
  -> weighted sampler
  -> question renderer
  -> stats update
```

The mobile UI should stay focused on practice:

- Start directly in a review session or compact dashboard.
- Show one question at a time.
- Avoid explaining the scoring algorithm in-app.
- Show lightweight session progress and recent mistakes.
- Do not expose Supabase sync internals unless sign-in or refresh fails.

## Implementation Order

1. Add a static mobile-review fixture from existing card JSON shapes.
2. Implement shared Kotlin data models for the active card shapes.
3. Implement question extraction in `commonMain` with deterministic unit tests.
4. Implement stable `questionKey` generation and prove option changes do not
   change stats identity.
5. Implement local `question_stats` storage and stats update through an Android
   SQLite-backed adapter.
6. Implement weighted sampling with seeded tests for weak-question preference,
   recent-question penalties, and small candidate sets.
7. Build the first Android Compose UI with meaning four-choice and sentence
   reorder.
8. Add Android Supabase sign-in and read-only active-card fetch.
9. Add local vocabulary cache and app-start refresh.
10. Add usage distinction and inflection question renderers.
11. Tune filters, type balance, and weighting from real session behavior.
12. Add optional local attempt logs and pruning.

## Test Plan

Question identity:

- Four-choice option shuffling does not change `questionKey`.
- Different distractor sets for the same meaning update the same stats row.
- Translation reorder in the source card does not change a meaning key when the
  normalized Japanese meaning is unchanged.
- Reorder token shuffling does not change a sentence key.

Question extraction:

- Meaning questions require one correct answer and three unique distractors.
- Reorder questions skip too-short and too-long example sentences.
- Usage questions are skipped when comparison text cannot identify a concrete
  contrast.
- Inflection questions skip forms identical to the headword.

Weighted selection:

- A high `difficultyEma` candidate is sampled more often than a low one over a
  seeded repeated draw.
- A recently missed question receives a larger weight than an otherwise equal
  recently correct question.
- `freshnessPenalty` prevents immediate repetition without removing the problem
  from the candidate set.
- Small candidate sets still produce a valid weighted draw without time-based
  fallback logic.

Stats update:

- Correct answers decrease `difficultyEma` gradually.
- Wrong answers increase `difficultyEma` immediately.
- Streaks reset in the correct direction.
- Attempt logs do not affect scheduler identity.

Privacy and sync:

- Mobile logs do not contain raw card content beyond explicit development test
  fixtures.
- Review stats remain local for v1.
- Supabase reads use the signed-in user's token and no service-role key.
- Wider-release tests must confirm RLS owner-scoped access before distribution.

## Open Questions

- Whether the project should use Android-only Compose UI first or start with
  Compose Multiplatform UI immediately. Android-only UI is the default until iOS
  becomes a concrete delivery target.
- Whether review stats should later sync to Supabase for multi-device mobile use.
- Whether usage distinction questions need an LLM-assisted curation pass before
  they become high-volume.
- Whether regular/generated inflection forms are valuable enough to include by
  default or should remain opt-in.
- Whether ja2en candidate examples should join sentence reorder practice in the
  first release or stay out until English word-study review is stable.
