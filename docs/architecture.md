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
- `errors`: defines stable error codes and user-safe diagnostics.
- `schema`: defines `LexiResultV1` and rejects missing required fields or unknown schema versions.
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
2. User selects text in another application.
3. User presses the shortcut.
4. Backend receives the shortcut event and requests selected text from `selection`.
5. Backend completes selected-text capture before focusing the Lexi popup, so the active target application is not replaced by Lexi.
6. Popup opens with a capture, mock-transform loading, result, or error state.
7. In Phase 4, the frontend renders a validated mock `LexiResultV1` result so the popup can be exercised before provider integration.
8. In Phase 5, after capture succeeds, the backend immediately starts the configured LLM provider request instead of waiting for a frontend command round trip.
9. Provider responses are received as streams where supported. Backend accumulates the JSON, extracts completed safe partial fields, and emits progress events for incremental UI rendering.
10. Backend validates the completed provider response against `schemaVersion`.
11. Frontend renders skeleton placeholders for pending result fields, fades in completed partial content while streaming, then swaps to the validated final result or error state.

## Tauri Boundary

Prefer a small command surface:

- `get_app_state() -> AppState`
- `update_settings(input: SettingsUpdate) -> Settings`
- `capture_selection() -> CaptureResult`
- `run_transform(input: TransformRequest) -> TransformResult`
- `list_provider_models(provider: ProviderKind) -> ProviderModelsResult`
- `get_provider_settings() -> ProviderSettingsView`
- `update_provider_settings(input: ProviderSettingsUpdate) -> ProviderSettingsView`
- `copy_result(input: CopyRequest) -> CopyResult`

Do not expose broad filesystem, shell, HTTP, or clipboard permissions directly to frontend code unless a specific feature requires them and the capability file is updated deliberately.

Temporary PoC command:

- `capture_selection_diagnostics() -> SelectionDiagnostics`
- Returns redacted capture metadata only: success flag, stable code, source process/title when available, character count, and multiline flag.
- Does not return raw selected text, prompt data, or clipboard contents.

Phase 3 shortcut shell command:

- `get_shortcut_status() -> ShortcutStatus`
- Returns the configured default shortcut and any startup registration error.
- Does not register shortcuts from the frontend; registration is backend-owned.

Phase 3 frontend event:

- `lexi:capture`
- Emits `capturing`, `captured`, or `failed` states after the global shortcut fires.
- The `captured` payload contains only redacted metadata: capture method, optional source process/title, character count, and multiline flag.
- The selected text remains inside the Rust process and is not emitted to the frontend.

Phase 5 backend transform event:

- `lexi:transform`
- Emits `started`, `streaming`, `validating`, `ready`, or `failed`.
- `started` carries a whitespace-normalized, 48-character `selectedTextPreview` so the popup can show the selected word immediately after the provider request begins.
- `streaming` and `validating` carry a `LexiPartialResult` extracted from completed JSON fields only: headword, translations, nuance, synonyms, and warnings.
- Full raw selected text, raw prompt bodies, raw model chunks, and API keys are not emitted to the frontend; `selectedTextPreview` is the narrow UI-display exception.

Phase 4 popup state:

- Solid state variants are `idle`, `capturing`, `requesting`, `ready`, and `error`.
- `captured` events are treated as redacted metadata only and transition into a mock `requesting` state.
- The frontend validates the mock `LexiResultV1` with `validateLexiResultV1` before rendering the `ready` state.
- The word-study result renders as a single dictionary-card surface modeled after the editorial preview: a headword header, a nuance callout, translation rows with part-of-speech marks and examples, and similar-word rows with expandable usage comparisons.
- Result actions are copy, retry, close, and settings. Copy writes only the structured mock result text, not captured source text.
- Phase 5 removes the bottom result action bar. Settings opens from a header gear button and exposes provider, provider-backed model dropdown, embedded result-language dropdown, and API key update fields.
- The settings panel also exposes a frontend-owned background opacity slider. It updates a CSS custom property on the popup shell for background fills, borders, and shadows without reducing text or icon opacity, and does not cross the Tauri command boundary.
- The popup window opens at a 500 by 620 default size with 360 by 360 minimum constraints and remains resizable. The frontend shell uses responsive constraints and pane-level scrolling so long result text does not clip at narrow widths.
- The Tauri window enables `transparent`, and the frontend keeps `html`, `body`, `#root`, and the full-window shell free of opaque fills so the popup backdrop can be translucent on supported desktops.
- During capturing, requesting, and streaming states, the dictionary-card body keeps the final layout visible with skeleton placeholders. Completed nuance, translation, and similar-word fields are inserted into their final positions with a short fade-in animation.

Temporary PoC binary:

- `capture_selection_poc [delay_ms]`
- Waits for the requested delay, then prints the same redacted diagnostics JSON for manual UI Automation matrix testing.

## Permissions and Capabilities

Tauri v2 capabilities define which permissions are available to windows/webviews. Keep capabilities scoped to the main window unless additional windows are introduced.

For global shortcuts, the official plugin requires explicit permissions for commands such as register, unregister, and is_registered. Add only the permissions used by the implementation.

The Phase 3 implementation registers the default shortcut from Rust, so the frontend does not receive global-shortcut plugin permissions.

## Selected Text Capture Strategy

Initial Windows strategy:

- Use Windows UI Automation from Rust.
- Start from the focused or foreground automation element.
- Query for TextPattern support.
- Use GetSelection to read selected ranges.
- Treat degenerate ranges as empty selection.
- Return an explicit unsupported error if the focused element does not expose usable text selection.
- The Phase 1 PoC runs ordered capture strategies:
  - `uia-focused-element`;
  - `uia-foreground-window`.
- Each strategy can inspect the current element and a bounded set of descendants with `IsTextPatternAvailable`.
- Diagnostics include the successful or final attempted `captureMethod` so app-specific limitations can be mapped without exposing selected text.
- Future app-specific or fallback capture methods should be added as new strategies instead of branching the public command surface.

Fallback strategies should be decided after PoC evidence. Clipboard simulation is intentionally not the first path because it can mutate user clipboard state and has higher privacy risk.

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

Provider responses must be parsed into a versioned Rust struct before frontend rendering. The frontend should render validated data, not raw model text. For the first workflow, the model is expected to return structured word-study data: a dictionary/base-form headword for single inflected words, one to three dictionary-style Japanese sense entries with a `null` or enumerated part-of-speech note, one short example sentence plus Japanese translation per sense entry, an intuitive usage nuance for the headword, optional near-word synonyms with per-word usage comparisons, and warnings when useful data is unavailable. Translation entries should be separated by real English-side dictionary sense boundaries such as part of speech, countability, transitivity, concrete versus abstract use, legal/social versus technical use, or idiomatic use. The provider prompt should collapse near-duplicate Japanese paraphrases, alternative renderings, and collocation differences instead of filling the list with repeated explanations or Japanese synonyms such as `近づく` and `接近する`; examples like `採用` versus `採択` for `adoption`, or `デモ` versus `実演` for the same `demonstration` sense, should be merged unless they represent truly separate English senses. Antonyms are omitted from the current result contract.

## Security and Privacy

- Redact raw selected text, prompt bodies, provider responses, and credentials from logs.
- Store non-secret provider settings separately from API key material. API keys are read from dotenvx-injected environment variables first (`GEMINI_API_KEY`, `GOOGLE_API_KEY`, `OPENAI_API_KEY`), then from Windows Credential Manager. The frontend receives only configured/not-configured state.
- Keep default CSP non-null before release.
- Store secrets through an OS-appropriate mechanism when provider configuration is implemented.
- Avoid remote content in the Tauri webview unless explicitly required.

## Testing Strategy

- Rust: unit tests for selection error mapping, LLM schema parsing, settings serialization, and redaction.
- Frontend: component tests for popup state, result rendering, settings validation, and error rendering.
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
