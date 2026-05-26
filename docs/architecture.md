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

- `features/popup`: popup shell, state transitions, and result rendering.
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
6. Popup opens with captured metadata, a loading state for provider work, or an error state.
7. Backend calls the LLM provider with a structured prompt.
8. Backend validates the provider response against `schemaVersion`.
9. Frontend renders result or error state.

## Tauri Boundary

Prefer a small command surface:

- `get_app_state() -> AppState`
- `update_settings(input: SettingsUpdate) -> Settings`
- `capture_selection() -> CaptureResult`
- `run_transform(input: TransformRequest) -> TransformResult`
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
- The selected text remains inside the Rust process for later provider integration and is not emitted to the frontend in Phase 3.

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

Provider responses must be parsed into a versioned Rust struct before frontend rendering. The frontend should render validated data, not raw model text. For the first workflow, the model is expected to return structured word-study data: Japanese translations, nuance, similar words, usage differences, antonyms, and warnings when useful data is unavailable.

## Security and Privacy

- Redact raw selected text, prompt bodies, provider responses, and credentials from logs.
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
- Whether the first LLM provider should support streaming.
- Whether popup positioning requires native window APIs beyond the default Tauri window controls.

## References

- Tauri create project docs: https://v2.tauri.app/start/create-project/
- Tauri global shortcut plugin docs: https://v2.tauri.app/plugin/global-shortcut/
- Tauri capabilities docs: https://v2.tauri.app/security/capabilities/
- Tauri CSP docs: https://v2.tauri.app/security/csp/
