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
- Add Rust structs for `LexiResultV1`, translations, related words, usage comparisons, and related schema objects.
- Add strict schema validation for required fields and `schemaVersion`.
- Mirror result and error types in `src/lib/schema.ts` and `src/lib/errors.ts`.
- Add unit tests for error mapping and schema validation.

Acceptance criteria:

- Frontend can render typed errors and results without inspecting arbitrary JSON.
- Unknown result schema versions are rejected.
- Tests cover success and invalid model output paths.

Result: The first AI result contract is `lexi.result.v1` with `mode: "word-study"`. Provider output must include a headword, Japanese translations, nuance, similar words, usage comparisons, antonyms, and warnings, then pass backend validation before the UI renders it.

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

Result: Phase 4 keeps raw selected text out of the frontend. The popup transitions from capture metadata to a mock transformation result, validates the mock result with the TypeScript schema guard before rendering, and keeps provider work behind the Phase 5 boundary. The result UI is organized into compact `意味`, `ニュアンス`, `使い分け`, and `関連語` panes so each kind of explanation stays separate and the fixed popup is not dependent on whole-window scrolling for normal content.

Automated coverage:

- Frontend tests assert requesting-state rendering, mock result rendering, error diagnostics, and keyboard-reachable copy action wiring.

## Phase 5: LLM Provider Adapter

Goal: convert captured text into validated `LexiResultV1`.

Tasks:

- Add `LlmProvider` trait with a minimal method such as `transform(request)`.
- Keep `MockProvider` as the default until real provider configuration exists.
- Add prompt builder for the first `word-study` workflow:
  - Japanese translations;
  - nuance;
  - similar words;
  - practical usage differences;
  - antonyms.
- Add timeout and retry policy.
- Parse provider response into `LexiResultV1`.
- Map provider failures to stable app errors:
  - not configured;
  - request failed;
  - rate limited;
  - invalid output.
- Add redaction helpers so selected text, prompts, responses, and credentials are never logged raw.

Acceptance criteria:

- The UI can run the full path with `MockProvider`.
- Invalid provider output becomes `InvalidModelOutput`.
- Provider payloads are not logged.
- Real provider integration can be added behind the same trait after provider choice is settled.

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

## Decisions Needed

- First workflow: keep `explain` or switch to translate, summarize, or rewrite.
- Default provider and model.
- Whether streaming output is required for the first release.
- API key storage approach for the first release.
- Default shortcut.
- Popup placement rule: cursor, active window center, selected-text vicinity when available, or fixed screen edge.
- Must-support Windows applications for the UI Automation PoC.
- Fallback behavior when UI Automation fails.

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
