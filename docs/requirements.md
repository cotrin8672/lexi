# Lexi Requirements

Status: draft

## Purpose

Lexi's initial release should prove a fast desktop workflow: the user selects text in another application, presses a global shortcut, and receives a compact popup with an LLM-generated result in a predictable schema.

The initial product should optimize for reliability, privacy, and low interruption over broad feature coverage.

## Initial Release Scope

In scope:

- Windows-first desktop app built with Tauri v2, Rust, SolidJS, TypeScript, and Vite.
- Global shortcut activation.
- Selected-text acquisition through Windows UI Automation where supported.
- Compact popup UI for loading, result, copy, retry, and error states.
- One LLM transformation workflow with a typed output schema.
- Local settings for shortcut, provider configuration, model name, and prompt preset.
- Focused logging that excludes selected text and model payloads.

Out of scope for the first release:

- Cross-platform selected-text capture parity.
- Long-term history, semantic search, or document library features.
- Complex prompt marketplace or multi-agent orchestration.
- Automatic clipboard overwrite as the primary capture mechanism.
- Background processing of arbitrary windows without explicit shortcut activation.

## Functional Requirements

- The app registers a configurable global shortcut on startup.
- When the shortcut fires, the app attempts to read the current foreground selection.
- If selected text is available, the app opens a small popup near the active context or at a deterministic fallback position.
- The app sends the selected text and selected prompt preset to the configured LLM provider.
- The app validates the response against the expected schema before rendering it.
- The user can copy the primary result.
- The user can retry a failed request.
- The user can open settings from the popup.
- The app shows actionable errors for unsupported selection source, empty selection, shortcut registration failure, provider failure, and schema validation failure.

## Non-Functional Requirements

- Startup should remain lightweight; avoid eager provider calls.
- Shortcut-to-popup feedback should feel immediate even when LLM processing is pending.
- Raw selected text must not be written to logs.
- API keys must not be stored in plaintext project files.
- Failures should be recoverable without restarting the app whenever possible.
- The UI should be usable with keyboard and pointer.
- The result schema should be versioned so prompt changes do not silently break rendering.

## Technical Requirements

- Use Tauri v2's command boundary for frontend-to-backend calls.
- Use a dedicated Rust module for selection capture so UI Automation behavior is isolated from LLM and window code.
- Use a dedicated Rust module for LLM provider integration.
- Use `serde` structs for command inputs, command outputs, provider responses, and UI error payloads.
- Use Solid fine-grained state for popup state transitions: idle, capturing, requesting, ready, error.
- Keep Tauri capabilities narrow and explicit.

## UI Requirements

- First screen is the actual popup/work surface, not a landing page.
- The popup should have stable dimensions with responsive constraints so loading text, long words, and errors do not resize the shell unexpectedly.
- Primary actions: copy result, retry, close, settings.
- Avoid persistent instructional text in the main popup.
- Use compact controls and clear state changes rather than decorative panels.
- For errors, show a short user-facing message plus a details affordance when technical information exists.

## LLM Output Schema

Initial schema draft for the first word-study workflow:

```json
{
  "schemaVersion": "lexi.result.v1",
  "mode": "word-study",
  "sourceLanguage": "auto",
  "resultLanguage": "ja",
  "headword": "string",
  "translations": [
    {
      "text": "string",
      "note": "string or null"
    }
  ],
  "nuance": "string",
  "synonyms": [
    {
      "term": "string",
      "japanese": "string",
      "nuance": "string"
    }
  ],
  "usageComparisons": [
    {
      "terms": ["string"],
      "explanation": "string",
      "examples": ["string"]
    }
  ],
  "antonyms": [
    {
      "term": "string",
      "japanese": "string",
      "nuance": "string"
    }
  ],
  "warnings": ["string"]
}
```

Rules:

- `schemaVersion` is required.
- `headword`, `translations`, and `nuance` are required for rendering.
- `translations` must contain at least one Japanese translation.
- `synonyms`, `usageComparisons`, `antonyms`, and `warnings` may be empty arrays when the model cannot provide a useful item without guessing.
- `synonyms` and `antonyms` should include the English term, Japanese meaning, and nuance.
- `usageComparisons` should explain practical differences between related terms and may include examples.
- The renderer must reject unknown or missing schema versions instead of guessing.

## Error Handling

- `ShortcutRegistrationFailed`: shortcut conflict or OS-level registration failure.
- `SelectionUnavailable`: active control does not expose a supported selected-text pattern.
- `SelectionEmpty`: insertion point exists but no text is selected.
- `SelectionPermissionDenied`: OS or target application blocks access.
- `ProviderNotConfigured`: missing provider settings or API key.
- `ProviderRequestFailed`: network, provider, or rate-limit failure.
- `InvalidModelOutput`: response could not be parsed or failed schema validation.

Each error should include a stable code, a short user message, and a diagnostic message safe for logs.

## Test Requirements

- Rust unit tests for schema validation, error mapping, and provider response parsing.
- Frontend tests for popup state rendering and result rendering.
- Manual Windows PoC matrix for selected-text capture before relying on UI Automation in product code.
- Build verification for Tauri integration before release.

## Risks and Mitigations

- UI Automation support varies by app. Mitigation: run the PoC matrix first and keep unsupported-source handling explicit.
- Global shortcut conflicts are common. Mitigation: make the shortcut configurable and expose registration failure clearly.
- LLM output can drift from schema. Mitigation: validate strictly and version schemas.
- Sensitive text can leak through logs. Mitigation: structured redaction and no raw payload logging.
- Popup positioning can be inconsistent across monitors and DPI settings. Mitigation: use deterministic fallback placement and test high-DPI multi-monitor setups.

## Milestones

1. UI Automation selected-text PoC.
2. Global shortcut registration and popup shell.
3. Typed capture command and frontend state flow.
4. LLM provider adapter with schema validation.
5. Settings and local configuration.
6. Error-state polish and release verification.

## Future Extensions

- Additional prompt modes such as translate, summarize, rewrite, define, and code explain.
- macOS Accessibility API and Linux selection support.
- Provider abstraction for multiple LLM vendors.
- Optional local history with explicit retention controls.
- Streaming responses.
- Tray menu and quick mode switching.

## Open Questions

- Which exact first workflow should ship: explain, translate, rewrite, or summarize?
- Which LLM provider and model should be the default?
- Should API keys use OS keychain storage in the first release?
- Should the popup appear near cursor, near selected text when possible, or at a fixed screen edge?
- Which applications are must-support targets for the first Windows release?

## References

- Tauri create project docs: https://v2.tauri.app/start/create-project/
- Tauri global shortcut plugin docs: https://v2.tauri.app/plugin/global-shortcut/
- Tauri capabilities docs: https://v2.tauri.app/security/capabilities/
- Microsoft UI Automation TextPattern GetSelection: https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationtextpattern-getselection
