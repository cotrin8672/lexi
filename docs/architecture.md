# Lexi Architecture

Status: draft

## Baseline

The repository currently matches a minimal Tauri v2 + SolidJS + TypeScript scaffold:

- `src-tauri`: Rust application shell, Tauri commands, capabilities, and native integrations.
- `src`: SolidJS frontend rendered by Vite.
- `src-tauri/capabilities/default.json`: current frontend permissions for the main window.
- `src-tauri/tauri.conf.json`: Tauri app, build, window, and bundle configuration.
- `src-tauri/src/selection`: Windows selected-text capture PoC using UI Automation through the official `windows` crate.

Tauri's official create-project flow includes Solid as a maintained template, so the current stack is aligned with the intended direction.

## Proposed Module Boundaries

Rust backend:

- `selection`: reads selected text from the active OS context.
- `shortcut`: owns global shortcut registration and event mapping.
- `llm`: sends typed requests to the configured model provider and validates response shape.
- `settings`: loads and saves user settings.
- `dictionary`: imports and queries global dictionary reference data such as EJDict.
- `vocabulary`: owns local lexeme, alias, card, lookup-event, and sync-projection storage.
- `sync`: persists local mutations, pushes them to Supabase, and pulls server revisions into SQLite.
- `tray`: owns the system tray icon, tray menu, and popup re-show behavior.
- `errors`: defines stable error codes and user-safe diagnostics.
- `schema`: defines word-study, Japanese word-candidates, and text-translation result contracts and rejects missing required fields or unknown schema versions.
- `commands`: exposes narrow Tauri commands to the frontend.

Solid frontend:

- `App.tsx` currently owns the Phase 4 popup shell, state transitions, mock result rendering, and popup actions while the app is still small.
- `features/popup`: future home for popup shell, state transitions, and result rendering once the current single-file implementation starts duplicating logic.
- `features/settings`: shortcut, provider, model, and prompt preset settings.
- `lib/commands`: typed wrappers around Tauri commands.
- `lib/schema`: shared TypeScript representation of validated result payloads.
- `lib/errors`: shared TypeScript representation of stable app error payloads.

## Data Flow

1. App starts and registers the configured global shortcut.
2. App creates a system tray icon and keeps the main popup hidden until activation.
3. User selects text in another application.
4. User presses the shortcut.
5. Backend receives the shortcut event and requests selected text from `selection`.
6. Backend completes selected-text capture before focusing the Lexi popup, so the active target application is not replaced by Lexi.
7. Popup opens with a capture, mock-transform loading, result, or error state.
8. In Phase 4, the frontend renders a validated mock `LexiResultV1` result so the popup can be exercised before provider integration.
9. In Phase 5, after capture succeeds, the backend immediately starts the configured LLM provider request instead of waiting for a frontend command round trip.
10. Provider responses are received as streams where supported. Backend accumulates the JSON, extracts completed safe partial fields, and emits progress events for incremental UI rendering.
11. Backend validates the completed provider response against `schemaVersion`.
12. Frontend renders skeleton placeholders for pending result fields, fades in completed partial content while streaming, then swaps to the validated final result or error state.

## Tauri Boundary

Prefer a small command surface:

- `get_app_state() -> AppState`
- `update_settings(input: SettingsUpdate) -> Settings`
- `capture_selection() -> CaptureResult`
- `run_transform(input: TransformRequest) -> TransformResult`
- `list_provider_models(provider: ProviderKind) -> ProviderModelsResult`
- `get_provider_settings() -> ProviderSettingsView`
- `update_provider_settings(input: ProviderSettingsUpdate) -> ProviderSettingsView`
  - Includes the configurable capture shortcut, close shortcut, persisted background opacity, provider, model, result language, prompt mode, and optional provider API key updates. Supabase connection values are app-owned configuration and are not editable from frontend settings.
- `get_sync_auth_status() -> SyncAuthStatus`
- `start_google_sign_in() -> GoogleSignInStart`
  - Starts a Google OAuth PKCE flow for Supabase Auth using a Rust-owned localhost callback listener at `http://localhost:38271/auth/callback`.
- `sign_out_sync() -> ()`
  - Deletes the locally stored Supabase session.
- `get_sync_status() -> SyncStatus`
  - Returns compact vocabulary sync lifecycle state: pending mutation count, last server revision, last sync time, and the latest user-safe sync error when actionable.
- `retry_sync() -> ()`
  - Schedules a background push/pull cycle without blocking popup lookup.
- `hide_main_window() -> ()`
  - Hides the popup to the tray for close-shortcut and close-button dismissal.
- `copy_result(input: CopyRequest) -> CopyResult`

Do not expose broad filesystem, shell, HTTP, or clipboard permissions directly to frontend code unless a specific feature requires them and the capability file is updated deliberately.

Temporary PoC command:

- `capture_selection_diagnostics() -> SelectionDiagnostics`
- Returns redacted capture metadata only: success flag, stable code, source process/title when available, character count, and multiline flag.
- Does not return raw selected text, prompt data, or clipboard contents.

Phase 3 shortcut shell command:

- `get_shortcut_status() -> ShortcutStatus`
- Returns the configured shortcut and any startup registration error.
- Does not register shortcuts from the frontend; registration is backend-owned.

System tray behavior:

- The backend creates the tray icon during setup using the configured app icon.
- Left-clicking the tray icon shows and focuses the main popup.
- The tray menu exposes `Show Lexi` and `Quit Lexi`. Window close requests hide the popup instead of exiting; the tray quit action exits the process.
- The main window is configured with `skipTaskbar` so the tray remains the persistent desktop entry point.
- Re-showing the main popup does not reposition it; because close requests hide the existing window, the user's last position is preserved for the current app session.

Phase 3 frontend event:

- `lexi:capture`
- Emits `capturing`, `captured`, or `failed` states after the global shortcut fires.
- The `captured` payload contains only redacted metadata: capture method, optional source process/title, character count, and multiline flag.
- The selected text remains inside the Rust process and is not emitted to the frontend.

Phase 5 backend transform event:

- `lexi:transform`
- Emits `started`, `streaming`, `validating`, `ready`, or `failed`.
- `started` carries a whitespace-normalized, 48-character `selectedTextPreview` so the popup can show the selected word or text preview immediately after the provider request begins.
- `streaming` and `validating` carry a `LexiPartialResult` extracted from completed JSON fields only: headword, irregular inflections, translations, nuance, synonyms, idioms, and warnings.
- Full raw selected text, raw prompt bodies, raw model chunks, and API keys are not emitted to the frontend; `selectedTextPreview` is the narrow UI-display exception.

Phase 4 popup state:

- Solid state variants are `idle`, `capturing`, `requesting`, `ready`, and `error`.
- `captured` events are treated as redacted metadata only and transition into a mock `requesting` state.
- The frontend validates the mock `LexiResultV1` with `validateLexiResultV1` before rendering the `ready` state.
- The word-study result renders as a single dictionary-card surface modeled after the editorial preview: a headword header with optional irregular inflections, a nuance callout, translation rows with part-of-speech marks and examples, similar-word rows with expandable usage comparisons, and idiom rows with Japanese meanings and examples.
- The text-translation result renders as a simpler translation surface with the translated text first and a compact source/translation segment section below it.
- Result actions are copy, retry, close, and settings. Copy writes only the structured mock result text, not captured source text.
- The configured close shortcut and close action hide the popup to the tray rather than terminating the process.
- Phase 5 removes the bottom result action bar. Settings opens from a header gear button and exposes provider, provider-backed model dropdown, embedded result-language dropdown, and API key update fields.
- The settings panel also exposes shortcut recorders for capture and close actions. Saving settings sends the recorded key chords through the Rust command boundary; the backend validates, normalizes, persists, and re-registers the capture shortcut immediately. If registration fails, the previous registered capture shortcut is restored and the frontend receives a `ShortcutRegistrationFailed` error. The close shortcut is local to the popup and may omit modifier keys.
- The settings panel also exposes a persisted background opacity slider. It updates a CSS custom property on the popup shell for background fills, borders, and shadows without reducing text or icon opacity, and saves through the Rust-owned settings file.
- When no Supabase session is stored, the frontend shows a first-run Japanese Google sign-in gate instead of the dictionary popup. The normal idle popup is hidden until auth status is known, and the window is resized to a centered auth layout before opening the browser-based Supabase Google OAuth flow.
- The popup window opens at a 500 by 620 default size with 360 by 360 minimum constraints and remains resizable. The frontend shell uses responsive constraints and pane-level scrolling so long result text does not clip at narrow widths.
- The Tauri window enables `transparent`, and the frontend keeps `html`, `body`, `#root`, and the full-window shell free of opaque fills so the popup backdrop can be translucent on supported desktops.
- During capturing, requesting, and streaming states, the dictionary-card body keeps the final layout visible with skeleton placeholders. Completed nuance, translation, similar-word, and idiom fields are inserted into their final positions with a short fade-in animation.
- Word-study pronunciation uses Rust-owned Windows SAPI commands (`speak_headword`, `stop_headword_speech`). The backend selects an installed `Language=409` (`en-US`) voice token when available and speaks the displayed headword synchronously on a worker thread. The popup exposes a header voice button and a configurable popup-local pronunciation shortcut (`Ctrl+Shift+P` by default) while the popup is focused. Headword text is not sent to external TTS providers in this flow.
- Voice selection prefers `en-US` voices and explicitly excludes Japanese voices. If no English voice is installed in Windows, pronunciation cannot be corrected in-app and the user must add an English language pack with speech support.

Temporary PoC binary:

- `capture_selection_poc [delay_ms]`
- Waits for the requested delay, then prints the same redacted diagnostics JSON for manual UI Automation matrix testing.

## Permissions and Capabilities

Tauri v2 capabilities define which permissions are available to windows/webviews. Keep capabilities scoped to the main window unless additional windows are introduced.

For global shortcuts, the official plugin requires explicit permissions for commands such as register, unregister, and is_registered. Add only the permissions used by the implementation.

The Phase 3 implementation registers the default shortcut from Rust, so the frontend does not receive global-shortcut plugin permissions.

## Selected Text Capture Strategy

Initial Windows strategy:

- Use an ordered native capture backend pipeline from Rust:
  - `clipboard-copy` first, using a clipboard-preserving `Ctrl+C` simulation for the lowest popup latency when the active app supports normal copy;
  - UI Automation next, using a dedicated STA worker thread so COM initialization and the `CUIAutomation` instance are reused across shortcut captures.
- Start from the focused or foreground automation element.
- Query for TextPattern support.
- Use GetSelection to read selected ranges.
- Treat degenerate ranges as empty selection.
- Return an explicit unsupported error if the focused element does not expose usable text selection.
- The Phase 1 PoC runs ordered capture strategies:
  - `uia-focused-element`;
  - `uia-foreground-window`.
- Each strategy tries the current element's `TextPattern` before inspecting a bounded set of descendants with `IsTextPatternAvailable`.
- Clipboard and UI Automation results pass through the same backend finalization path, including line-ending normalization, empty-selection handling, source metadata, and `captureMethod` diagnostics.
- Diagnostics include the successful or final attempted `captureMethod` so app-specific limitations can be mapped without exposing selected text.
- Future app-specific or fallback capture methods should be added as new strategies instead of branching the public command surface.

The clipboard backend must preserve and restore the clipboard around the simulated copy. If the current clipboard contains a format Lexi cannot safely duplicate, the backend fails before clearing the clipboard and the pipeline falls back to UI Automation.

## LLM Strategy

The backend should own provider calls because it can centralize key handling, redaction, retries, timeout policy, and schema validation.

Initial provider policy:

- Gemini is the default low-cost API provider, using `gemini-2.5-flash-lite`.
- OpenAI is the fallback API provider, using `gpt-5.4-nano`.
- Mock remains available for deterministic local verification.
- Provider selection and model names are user-configurable through settings.
- Model dropdowns are populated from provider model-list APIs when a key is configured: OpenAI uses `/v1/models`, Gemini uses `v1beta/models`. If listing fails or the key is missing, the backend returns a small default fallback list with a warning.
- API key values are accepted from the settings UI but are never returned to the frontend after save.
- Gemini and OpenAI transform calls use provider streaming endpoints where available. Partial JSON is accumulated in Rust, then only completed schema fields are emitted as partial UI state. The final response must still pass strict `LexiResultV1` validation before the app treats it as complete.

Provider responses must be parsed into a versioned Rust struct before frontend rendering. The frontend should render validated data, not raw model text. Word-study mode uses the configured LLM provider and expects structured dictionary data: a dictionary/base-form headword for single inflected words, optional irregular inflections for noun plurals and verb past or past participle forms, one to three dictionary-style Japanese sense entries with a `null` or enumerated part-of-speech note, one short example sentence plus Japanese translation per sense entry, an intuitive usage nuance for the headword, optional near-word synonyms with per-word usage comparisons, optional idioms with Japanese meanings and short English examples, and warnings when useful data is unavailable. Translation entries should be separated by real English-side dictionary sense boundaries such as part of speech, countability, transitivity, concrete versus abstract use, legal/social versus technical use, or idiomatic use. The provider prompt should collapse near-duplicate Japanese paraphrases, alternative renderings, and collocation differences instead of filling the list with repeated explanations or Japanese synonyms such as `近づく` and `接近する`; examples like `採用` versus `採択` for `adoption`, or `デモ` versus `実演` for the same `demonstration` sense, should be merged unless they represent truly separate English senses. Antonyms are omitted from the word-study contract.

Japanese word-candidates mode is selected when the captured text is not sentence-like and contains Hiragana, Katakana, or CJK ideographs. It uses the configured Gemini, OpenAI, or Mock provider (not DeepL) and wraps the response in `lexi.jp-word-candidates.v1` with a normalized Japanese `query`, one to eight English `candidates`, and warnings. Each candidate includes part of speech, Japanese nuance, usage note, confidence, and a required English example with Japanese translation. The popup header shows the Japanese query during streaming and ready states; headword pronunciation is omitted in v1.

Text-translation mode is selected by backend heuristics before provider dispatch when the captured text looks sentence-like: newline, sentence punctuation, clause punctuation, five or more whitespace-delimited tokens, or Japanese text longer than 32 non-whitespace characters. It uses DeepL and wraps the response in `lexi.text-translation.v1` with `translatedText`, optional detected source language, source/translation segments, and warnings. The first implementation emits one full-selection segment; later alignment can split segments without changing the top-level mode.

Transform events include `transformMode` on `Started` so the frontend can render the correct skeleton and header before the provider identity is inferred.

## Persistence and Sync Architecture

Vocabulary persistence should use Supabase as the cloud source of truth and SQLite as the device-local cache, read projection, EJDict cache, and durable mutation queue. SQLite exists to keep the popup fast and offline-tolerant; Supabase owns account-backed canonical state, cross-device merge rules, RLS, and server revision assignment.

The main local tables are expected to separate:

- global dictionary entries imported from EJDict;
- user lexemes keyed by canonical text and language (`en` for English word-study headwords, `ja` for Japanese lookup queries);
- lexeme forms that alias selected or inflected forms to candidate lexemes;
- AI-generated card snapshots or enrichment records;
- lookup events;
- a mutation outbox for pending user changes;
- a sync state table holding the last acknowledged server revision.

The main Supabase tables are expected to separate:

- global dictionary sources and dictionary entries;
- user-owned lexemes, forms, cards, and lookup events protected by RLS;
- accepted mutation or change records with monotonically increasing server revisions.

Initial Supabase access is intentionally single-user. The first migrations should still enable RLS on every exposed vocabulary or sync table, but policies may allow only the owner's current Supabase Auth user id to perform all operations. This keeps personal use simple while preventing leaked anon/public keys or unrelated authenticated users from reading or mutating vocabulary data. Tables should still carry `user_id uuid not null default auth.uid()` so the later multi-user policy can switch to `auth.uid() = user_id` without reshaping stored data. Do not use email addresses, `auth.email()`, user-editable metadata, or app-shipped `service_role` keys for authorization.

The write flow is:

1. Apply the user action to SQLite in a local transaction.
2. Insert a pending mutation record in the same transaction.
3. Return optimistic UI success from the local projection.
4. Push the mutation to a Supabase RPC such as `apply_vocabulary_mutation`.
5. Let the server validate ownership, merge aliases, deduplicate canonical lexemes, write affected rows transactionally, and issue a server revision.
6. Mark the local mutation acknowledged and store the returned server revision.
7. Pull any later revisions and update the SQLite projection.

Supabase remains the source of truth. SQLite holds optimistic local projection, read cache, and the durable mutation outbox until Supabase acknowledges each operation with a server revision.

Phase 10 sync engine:

- Rust module `sync` pushes pending `mutation_outbox` rows through Supabase RPC `apply_vocabulary_mutation`. The RPC is schema-aware: English word-study cards may derive `part_of_speech` from `content.translations[0].note` and irregular aliases from `content.inflections`; Japanese word-candidates cards keep `part_of_speech` null and only persist canonical plus payload `forms`.
- On first sync for a signed-in user, `vocabulary_bootstrap` copies canonical vocabulary tables from Supabase into SQLite.
- Incremental pull uses Supabase RPC `pull_vocabulary_changes` keyed by monotonically increasing `server_revision`.
- Background sync starts after app setup, successful Google sign-in, local vocabulary saves, and a periodic one-minute timer. Concurrent requests collapse into one in-flight sync plus a follow-up cycle.
- A sync cycle drains pending `mutation_outbox` rows in batches until no retryable pending rows remain or a push fails.
- The frontend listens for `lexi:sync-status` and shows compact sync notes only in settings or auth-adjacent surfaces. The settings sync retry control is available both for actionable errors and for signed-in states with pending local mutations.
- Sync payloads exclude raw selected text, prompts, provider raw responses, and credentials.

Reads should use the local SQLite replica after bootstrap. On first sign-in or when the local replica is incomplete, the backend copies the user's `user_lexemes`, `lexeme_forms`, and active `card_snapshots` from Supabase into SQLite, then continues with incremental `vocabulary_changes` pull. Popup lookup does not call Supabase on every word request. If the local replica has no match, the client falls back to LLM. EJDict reference data is one-way source data: it can be bundled, downloaded, or mirrored into Supabase and then imported into SQLite, but user vocabulary writes must not mutate global dictionary rows.

Inflection handling should treat observed forms as aliases of lexemes instead of independent saved cards. For example, `went` should attach to canonical `go` when that relationship is known. Ambiguous forms such as `saw` may map to multiple candidate lexemes; the model, dictionary lookup, or user selection can choose which candidate to attach for a given card.

AI enrichment should consume dictionary seed data when available. EJDict can provide common Japanese translation candidates and reduce model drift; the model should remain responsible for missing nuance, learner-friendly examples, usage comparisons, and cases where dictionary data is unavailable or too ambiguous.

Persistence should be inserted behind the existing backend-owned transform flow. The Solid frontend should not gain new persistence-specific workflows unless the feature explicitly requires user input. Existing result rendering should receive the same structured result shape whether the data came from a fresh provider response or a SQLite cache hit. Background sync state should not reshape the popup. If sync/cache work fails, the backend should keep serving local data where possible and expose only a compact, transient user notification for actionable failures.

Do not sync raw selected text, raw prompt bodies, raw provider responses, or credentials by default. Synchronized card data should be the validated structured result, dictionary references, alias metadata, and explicit user state.

## Security and Privacy

- Redact raw selected text, prompt bodies, provider responses, and credentials from logs.
- Store non-secret provider settings separately from API key material. API keys are read from dotenvx-injected environment variables first (`GEMINI_API_KEY`, `GOOGLE_API_KEY`, `OPENAI_API_KEY`), then from Windows Credential Manager. The frontend receives only configured/not-configured state.
- Supabase project URL and anon/public key are app-owned configuration. Runtime lookup checks `SUPABASE_URL` or `LEXI_SUPABASE_URL` for the project URL and `SUPABASE_ANON_KEY`, `SUPABASE_PUBLISHABLE_KEY`, or `LEXI_SUPABASE_ANON_KEY` for the public key before falling back to any existing local app configuration. These values are not returned to the frontend; Supabase OAuth sessions are stored through Windows Credential Manager and are never returned to the frontend beyond configured/not-configured state, signed-in status, user id, email, and callback URL.
- Keep default CSP non-null before release.
- Store secrets through an OS-appropriate mechanism when provider configuration is implemented.
- Avoid remote content in the Tauri webview unless explicitly required.

## Mobile Review Stats

The Android review app under `mobile/` keeps motivational statistics on-device only.
Supabase vocabulary sync remains read-only; review events and study sessions are
not pushed upstream in v1.

Local Room tables:

- `question_stats`: weighted sampling state per stable `questionKey`.
- `review_attempt_events`: per-answer events for dashboard aggregation.
- `study_sessions`: foreground study time and answer counts per review session.
- `cached_user_lexemes.created_at`: mirrors Supabase lexeme creation time for
  per-day new-word counts.

Flow:

1. `ReviewViewModel` starts a `study_sessions` row when a review session begins.
2. `StudySessionTracker` measures foreground active time with a 5-minute idle cap.
3. Each checked answer writes `question_stats`, `review_attempt_events`, and
   session counters.
4. `StatsViewModel` aggregates local history through `StatsAggregator` and
   renders `StatsDashboardScreen` from mode select.

## Testing Strategy

- Rust: unit tests for selection error mapping, LLM schema parsing, settings serialization, and redaction.
- Frontend: component tests for popup state, result rendering, settings validation, and error rendering.
- Mobile: unit tests for stats aggregation, session tracking, sync `createdAt`
  mapping, `StatsViewModel`, and review-session event recording.
- Integration: Tauri command tests where possible; manual desktop verification where OS APIs are involved.
- PoC: app-by-app UI Automation matrix before committing to support claims.

## Open Questions

- Whether selection capture should be triggered entirely from Rust shortcut handlers or through frontend plugin bindings.
- Whether settings storage should use a Tauri store plugin, a Rust-owned config file, or OS keychain-backed storage for secrets.
- Whether popup positioning requires native window APIs beyond the default Tauri window controls.

## References

- Tauri create project docs: https://v2.tauri.app/start/create-project/
- Tauri global shortcut plugin docs: https://v2.tauri.app/plugin/global-shortcut/
- Tauri capabilities docs: https://v2.tauri.app/security/capabilities/
- Tauri CSP docs: https://v2.tauri.app/security/csp/
