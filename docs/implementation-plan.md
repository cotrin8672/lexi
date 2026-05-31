# Lexi Implementation Plan

Status: draft

## Assumptions

- Initial release targets Windows desktop only.
- UI Automation is the primary selected-text capture path until the PoC proves otherwise.
- The first user workflow is `word-study`, matching the current `lexi.result.v1` schema draft for English word lookup.
- Raw selected text should stay in the Rust backend flow and should not be rendered or logged unless a later requirement explicitly adds preview behavior.
- LLM provider selection is still open, so early implementation should support a mock provider and a narrow provider trait before binding the product to one vendor.

## Implementation Principles

- Prove native capture before building LLM-dependent UX.
- Keep OS integration in Rust and presentation in Solid.
- Use typed Rust structs and TypeScript types at the Tauri boundary.
- Treat all selected text and provider payloads as sensitive.
- Add permissions only when the related command or plugin is introduced.
- Keep each phase shippable, testable, and reversible.

## Target File Layout

Rust:

```text
src-tauri/src/
  commands.rs
  dictionary.rs
  errors.rs
  lib.rs
  llm/
    mod.rs
    mock.rs
    schema.rs
  selection/
    mod.rs
    windows.rs
  settings.rs
  shortcut.rs
  sync.rs
  vocabulary.rs
```

Frontend:

```text
src/
  app/
    App.tsx
  features/
    popup/
      Popup.tsx
      ResultView.tsx
      ErrorView.tsx
    settings/
      SettingsView.tsx
  lib/
    commands.ts
    errors.ts
    schema.ts
```

This layout can stay flat while the app is small. Add deeper abstractions only when duplicated logic appears.

## Phase 0: Baseline and Tooling

Goal: make the scaffold reproducible before feature work.

Tasks:

- Install dependencies with the package manager already implied by `tauri.conf.json`: `pnpm`.
- Commit or generate `pnpm-lock.yaml` and `src-tauri/Cargo.lock`.
- Verify the template baseline with `rtk pnpm build` and `rtk cargo check` from `src-tauri`.
- Replace placeholder metadata in `src-tauri/Cargo.toml` and `package.json`.
- Decide whether this repo will use frontend unit tests immediately or defer them until popup state work begins.
- Add minimal formatting conventions only if the project lacks editor defaults.

Acceptance criteria:

- A clean checkout can install and build.
- Baseline failures, if any, are documented before feature code is added.
- The repo has lockfiles for both Node and Rust dependencies.

## Phase 1: UI Automation Selection PoC

Goal: prove selected-text capture in native Rust without involving shortcuts, popup UI, or LLM calls.

Implementation status: complete for Phase 1. The Rust PoC path is implemented, redacted diagnostics are available, and representative browser/document-reader captures have succeeded.

Tasks:

- Add `selection` module with a Windows-only implementation.
- Choose the Windows API crate after a small spike:
  - Option A: use the official `windows` crate for direct UI Automation COM access.
  - Option B: use a wrapper crate only if it meaningfully reduces COM boilerplate without hiding failure modes.
- Implement `capture_selected_text()` returning typed success or `SelectionCaptureError`.
- Normalize captured line endings to `\n`.
- Add a temporary Tauri command or dev-only binary that returns redacted diagnostics:
  - success/failure code;
  - source window title if safe;
  - source process if available;
  - character count;
  - multiline flag.
- Run the target matrix from `docs/poc-ui-automation-selection.md`.
- Update the PoC doc with actual app results and recommendation.

Acceptance criteria:

- No raw selected text is printed in logs.
- Empty selection, unsupported source, and API failure are distinguishable.
- Representative browser and document-reader targets are tested.
- The team can decide whether UI Automation remains the primary capture path.

Result: UI Automation remains the primary capture path. Broader app compatibility testing is deferred to later PoC hardening and release verification.

## Phase 2: Error and Schema Foundation

Goal: define stable contracts before UI and provider integration depend on them.

Implementation status: complete for Phase 2. Stable app errors, selection-error mapping, Rust and TypeScript result schemas, and strict `word-study` schema validation are implemented and verified.

Tasks:

- Add `AppError` with stable code, user message, diagnostic message, and retryable flag.
- Map `SelectionCaptureError` to `AppError`.
- Add Rust structs for `LexiResultV1`, translations with per-sense example sentences, synonyms, and per-synonym usage comparisons.
- Add strict schema validation for required fields and `schemaVersion`.
- Mirror result and error types in `src/lib/schema.ts` and `src/lib/errors.ts`.
- Add unit tests for error mapping and schema validation.

Acceptance criteria:

- Frontend can render typed errors and results without inspecting arbitrary JSON.
- Unknown result schema versions are rejected.
- Tests cover success and invalid model output paths.

Result: The first AI result contract is `lexi.result.v1` with `mode: "word-study"`. Provider output must include a headword, Japanese translations with one example sentence and Japanese translation per sense, an intuitive usage nuance, near-word synonyms with a direct usage comparison for each synonym, and warnings, then pass backend validation before the UI renders it. Antonyms are intentionally omitted.

## Phase 3: Shortcut and Native Window Shell

Goal: wire the explicit user action that starts the capture flow.

Implementation status: complete for Phase 3. The Rust backend registers `Ctrl+Shift+X`, captures from the active app before focusing Lexi, opens a hidden compact popup with redacted capture results, emits capture lifecycle events, and reports startup registration conflicts through `ShortcutRegistrationFailed`.

Tasks:

- Add Tauri global shortcut dependencies:
  - Rust plugin: `tauri-plugin-global-shortcut`.
  - JS package: `@tauri-apps/plugin-global-shortcut` only if shortcut registration is frontend-owned.
- Prefer backend-owned registration if it keeps selected-text capture fully native.
- Add only required permissions to `src-tauri/capabilities/default.json`.
- Define default shortcut as `Ctrl+Shift+X` on Windows to keep the `Ctrl+Shift+` pattern while avoiding browser `Ctrl+L` selection/focus conflicts.
- Add shortcut registration, unregister on shutdown, and conflict reporting.
- Configure a compact popup window:
  - hidden on startup;
  - fixed minimum size;
  - resizable only if needed;
  - always-on-top only if it does not create focus problems.
- Emit a frontend event when shortcut capture starts, succeeds, or fails.

Acceptance criteria:

- Shortcut conflict shows `ShortcutRegistrationFailed`.
- Pressing the shortcut captures from the currently active app before Lexi takes focus, then opens the popup with captured metadata or an error state.
- App shutdown unregisters shortcuts cleanly.
- Capabilities list only the used global-shortcut permissions.

Result: Shortcut ownership stays in the Rust backend. Capture runs before Lexi takes focus, and the frontend listens for `lexi:capture` to render captured metadata or error states without receiving raw selected text.

Automated coverage:

- Rust tests assert that `lexi:capture` payload fields serialize with the camelCase names consumed by the frontend.

## Phase 4: Popup UI

Goal: build the first usable surface independent of a real LLM provider.

Implementation status: complete for Phase 4. The Solid frontend now uses the `idle`, `capturing`, `requesting`, `ready`, and `error` states, renders a Japanese mock `LexiResultV1` word-study result after capture metadata arrives, and exposes copy, retry, close, and settings actions from the popup.

Tasks:

- Replace the template UI with a compact popup layout.
- Add Solid state machine:
  - `idle`;
  - `capturing`;
  - `requesting`;
  - `ready`;
  - `error`.
- Render mock `LexiResultV1` data first.
- Add result actions: copy, retry, close, settings.
- Add error rendering with short message and optional details.
- Add keyboard handling:
  - Escape closes;
  - Enter retries when error is retryable;
  - copy button is keyboard reachable.
- Make dimensions stable for long words, Japanese text, and multiline summaries.

Acceptance criteria:

- The popup is the first screen and has no landing-page content.
- Loading, result, and error states are visually distinct.
- Long text does not overflow controls or resize the window unexpectedly.
- The UI works with mock data before provider integration.

Result: Phase 4 keeps raw selected text out of the frontend. The popup transitions from capture metadata to a mock transformation result, validates the mock result with the TypeScript schema guard before rendering, and keeps provider work behind the Phase 5 boundary. The result UI is organized into compact `意味` and `関連語` panes: nuance stays next to the headword, and usage comparisons are folded into expandable related-word rows so the fixed popup is not dependent on whole-window scrolling for normal content.

Automated coverage:

- Frontend tests assert requesting-state rendering, mock result rendering, error diagnostics, and keyboard-reachable copy action wiring.

## Phase 5: LLM Provider Adapter

Goal: convert captured text into validated `LexiResultV1`.

Tasks:

- Add `LlmProvider` trait with a minimal method such as `transform(request)`.
- Keep `MockProvider` available for deterministic local verification.
- Add Gemini and OpenAI API adapters, with Gemini as the low-cost default and OpenAI as the fallback provider.
- Add prompt builder for the first `word-study` workflow:
  - Japanese translations;
  - intuitive headword nuance;
  - short example sentences and Japanese translations for each meaning entry;
  - near-word synonyms;
  - practical usage difference for each synonym.
- Add timeout and retry policy.
- Use provider streaming endpoints where supported and emit partial result events from completed JSON fields.
- Parse provider response into `LexiResultV1`.
- Map provider failures to stable app errors:
  - not configured;
  - request failed;
  - rate limited;
  - invalid output.
- Add redaction helpers so selected text, prompts, responses, and credentials are never logged raw.
- Add provider settings commands so the popup can change provider, model, result language, and API key state without returning API key values to the frontend.
- Populate model dropdowns from provider model-list endpoints, with default fallback options when model listing is unavailable.
- Remove the result-view bottom action bar and open settings from a header gear button.

Acceptance criteria:

- The UI can run the full path with `MockProvider`.
- The UI can switch between Gemini and OpenAI settings.
- Invalid provider output becomes `InvalidModelOutput`.
- Provider payloads are not logged.
- Additional provider integrations can be added behind the same command/provider boundary.

Result: Phase 5 adds a backend `llm` module with a `LlmProvider` trait, a deterministic `MockProvider`, structured `word-study` prompt/schema builders, and Gemini/OpenAI API calls behind `run_transform`. Captured text remains in Rust state and is not emitted to the frontend. Shortcut capture success now starts the provider request immediately in Rust, emits `lexi:transform` stream events, and lets the UI render completed partial fields before final validation. Provider settings default to Gemini `gemini-2.5-flash-lite`, expose OpenAI `gpt-5.4-nano` as the fallback option, and return only API-key configured state to the UI. API keys are read from dotenvx-injected environment variables first, then Windows Credential Manager, and are never stored in plaintext app config files. Model dropdowns are populated through provider model-list endpoints when available, with backend fallback lists otherwise. The popup now opens settings from a header gear button and removes the old result bottom action bar.

## Phase 6: Settings and Secret Handling

Goal: allow configuration without weakening privacy.

Tasks:

- Define settings:
  - shortcut;
  - provider type;
  - model name;
  - result language;
  - prompt mode;
  - popup placement preference.
- Store non-secret settings in a local app config or Tauri store file.
- Store API keys outside the normal settings JSON, preferably through OS-backed secret storage.
- Add settings load, save, reset, and validation commands.
- Re-register shortcut after a shortcut setting change.
- Add frontend settings form with validation.

Acceptance criteria:

- Invalid shortcut or model settings are rejected before save.
- API key values are never returned back to the frontend after save; expose only configured/not-configured state.
- Changing shortcut does not require restarting the app.

## Phase 7: End-to-End Flow

Goal: connect shortcut, selection, provider, and popup into the product loop.

Tasks:

- Implement a backend orchestration command or event flow:
  - shortcut pressed;
  - capture selected text;
  - open popup;
  - call provider;
  - validate result;
  - emit result or error.
- Decide whether the frontend initiates provider calls or only listens to backend events.
- Add request IDs to prevent stale responses from overwriting newer popup state.
- Add cancel/close behavior for in-flight requests if supported.
- Add copy result command and avoid copying raw selected text.

Acceptance criteria:

- Pressing the shortcut with selected text shows a result.
- Pressing the shortcut without selection shows a useful error.
- Repeated shortcut presses do not mix old and new responses.
- Closing the popup does not crash or leave inconsistent state.

## Phase 8: Security, Privacy, and Release Hardening

Goal: prepare for a usable first release.

Tasks:

- Replace `csp: null` with an explicit Content Security Policy before release.
- Audit `src-tauri/capabilities/default.json`.
- Add log redaction tests.
- Confirm no raw selected text appears in normal logs.
- Add app metadata, icon decision, and bundle target policy.
- Add update notes to README for installation and known limitations.
- Run full verification:
  - `rtk pnpm build`;
  - `rtk cargo check`;
  - `rtk pnpm tauri build`;
  - manual UI Automation matrix;
  - manual shortcut and popup smoke test.

Acceptance criteria:

- Release build succeeds.
- Known unsupported apps are documented.
- Security-sensitive defaults are explicit.
- The first release can be installed and used without developer tooling.

## Phase 9: Local Vocabulary Store and EJDict Seed

Goal: introduce persistence without making popup reads depend on cloud availability.

Tasks:

- Add a Rust-owned SQLite store for vocabulary data.
- Add local tables for dictionary entries, lexemes, lexeme forms, card snapshots, lookup events, mutation outbox, and sync state.
- Import EJDict as global dictionary seed data. Treat EJDict entries as reference data, not user-editable vocabulary rows.
- Add normalization helpers for lookup keys, including case folding and whitespace normalization.
- Store word-study results by canonical lexeme rather than by selected surface form.
- Store observed and inferred forms as lexeme aliases, including selected form, lemma, irregular plural, irregular past, and irregular past participle.
- Allow ambiguous form aliases to point at multiple candidate lexemes instead of forcing `form_key` to be globally unique.
- Use EJDict hits as translation candidates and AI prompt context when available.
- Keep AI responsible for missing nuance, examples, usage comparisons, sense selection, and fallback generation when EJDict has no useful hit.
- Add Tauri commands only for narrow vocabulary reads and explicit user actions; avoid exposing raw SQL or filesystem access to the frontend.

Acceptance criteria:

- Existing popup reads can display a saved or recently generated card from SQLite without a network call.
- Looking up an irregular form such as `went` can attach the card to the canonical lexeme `go` when the provider result supplies that relationship.
- Repeating a lookup for an already-known alias does not create a duplicate saved card.
- EJDict data can be refreshed or reimported without overwriting user lexeme/card state.
- Raw selected text, raw prompts, and raw provider responses are not stored in vocabulary tables by default.

## Phase 10: Supabase Cloud Sync

Goal: sync account-backed vocabulary while keeping SQLite as the local cache, read projection, and durable mutation queue.

Design decision:

- Supabase is the cloud source of truth for synchronized user vocabulary.
- Local SQLite is a read-through cache, local projection, EJDict cache, and durable queue for pending mutations.
- SQLite-only rows are pending optimistic state until acknowledged by Supabase.
- The first Supabase deployment is single-user: enable RLS immediately, but allow only the configured owner Supabase Auth user id to read and write vocabulary/sync tables. Keep `user_id default auth.uid()` columns on user-owned rows so later multi-user policies can move to owner-scoped access without a table rewrite.

Tasks:

- Add Supabase schema migrations for dictionary sources, dictionary entries, user lexemes, lexeme forms, card snapshots, lookup events, mutations or change records, and server revisions.
- Add RLS policies in the same migrations as the tables. The initial policy may be owner-only `for all to authenticated` with both `using` and `with check` pinned to the owner's Supabase Auth user id; do not leave tables exposed without RLS.
- Add a server-side RPC such as `apply_vocabulary_mutation(payload)` that applies user mutations transactionally.
- Make the RPC own validation, ownership checks, canonical lexeme deduplication, alias attachment, conflict handling, and server revision assignment.
- Add local mutation records for operations such as save lookup, save card snapshot, attach alias, favorite, delete, and user note updates.
- Push pending local mutations asynchronously and retry failed pushes with backoff.
- Pull Supabase changes by monotonically increasing server revision rather than by client wall-clock timestamps.
- Update the SQLite projection from acknowledged mutations and pulled revisions.
- Keep lookup events append-only so repeated pushes are idempotent by client-generated operation IDs.
- Add conflict policies per entity type instead of one global last-write-wins rule.

Acceptance criteria:

- A local save succeeds immediately while offline and syncs after connectivity returns.
- A leaked anon/public key cannot read or mutate vocabulary tables without the owner's authenticated JWT.
- Re-sending the same pending mutation is idempotent.
- Two devices adding the same canonical lexeme converge to one server lexeme when the canonical key matches.
- Server revisions prevent pull gaps caused by client clock skew.
- Deletions use soft-delete/tombstone behavior so they sync across devices.
- Supabase is never required for basic dictionary lookup or popup rendering when the local cache is warm.

## Phase 11: Sync UX and Account Controls

Goal: make cloud sync understandable and controllable.

Tasks:

- Add sign-in and sign-out states after the local vocabulary store is stable.
- Show compact sync status only where it affects user action: signed out, pending, failed, or synced.
- Add manual retry and clear-failed-mutation diagnostics without exposing sensitive payloads.
- Add retention controls before syncing any context beyond structured vocabulary cards.
- Add an export path for user vocabulary data.

Acceptance criteria:

- Signed-out use remains fully functional with local SQLite vocabulary.
- Signing in starts background sync without blocking lookup.
- Sync failures do not lose local vocabulary changes.
- The user can distinguish local-only pending data from cloud-synced data.

## Suggested Build Order

1. Baseline lockfiles and build checks.
2. UI Automation PoC.
3. Error and schema foundation.
4. Popup UI with mock result.
5. Global shortcut and popup window integration.
6. Mock end-to-end flow.
7. Real provider integration.
8. Settings and secret storage.
9. Release hardening.
10. Local vocabulary store and EJDict seed.
11. Supabase cloud sync.
12. Sync UX and account controls.

This order intentionally delays real LLM integration until capture and UX are proven.

## Test Plan by Layer

Rust unit tests:

- `AppError` serialization.
- Selection error mapping.
- Schema parse and validation.
- Provider invalid-output handling.
- Redaction helpers.

Frontend tests:

- Popup state transitions.
- Result rendering.
- Error rendering.
- Settings validation.

Manual tests:

- UI Automation target matrix.
- Shortcut registration conflict.
- Multi-monitor and high-DPI popup placement.
- Empty selection.
- Unsupported source.
- Provider unavailable.
- Invalid provider output.
- Offline vocabulary save and later sync retry.
- Duplicate alias lookup across canonical and inflected forms.

## Decisions Needed

- First workflow: keep `explain` or switch to translate, summarize, or rewrite.
- Default provider and model.
- Default shortcut.
- Popup placement rule: cursor, active window center, selected-text vicinity when available, or fixed screen edge.
- Must-support Windows applications for the UI Automation PoC.
- Fallback behavior when UI Automation fails.
- Whether Supabase sign-in is optional or required for cloud vocabulary sync.
- Initial EJDict import path: bundled asset, first-run download, or Supabase mirror plus local cache.

## Immediate Next Step

Start with Phase 0 and Phase 1:

1. Install dependencies and generate lockfiles.
2. Add the selection module skeleton.
3. Implement the Windows UI Automation PoC path.
4. Run and document the target matrix.

Only after that should the project commit to the final shortcut-to-LLM flow.

## References

- Tauri global shortcut API: https://v2.tauri.app/reference/javascript/global-shortcut/
- Tauri global shortcut plugin guide: https://v2.tauri.app/plugin/global-shortcut/
- Tauri store plugin guide: https://v2.tauri.app/plugin/store/
- Tauri capabilities: https://v2.tauri.app/security/capabilities/
- Microsoft UI Automation TextPattern GetSelection: https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationtextpattern-getselection
