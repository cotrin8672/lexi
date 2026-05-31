# Lexi Requirements

Status: draft

## Purpose

Lexi's initial release should prove a fast desktop workflow: the user selects text in another application, presses a global shortcut, and receives a compact popup with an LLM-generated result in a predictable schema.

The initial product should optimize for reliability, privacy, and low interruption over broad feature coverage.

## Initial Release Scope

In scope:

- Windows-first desktop app built with Tauri v2, Rust, SolidJS, TypeScript, and Vite.
- System tray residency while the app is running.
- Global shortcut activation.
- Selected-text acquisition through a native Windows backend pipeline: clipboard-preserving copy first for low latency, then Windows UI Automation where supported.
- Compact popup UI for loading, result, copy, retry, and error states.
- Two typed transformation workflows: word study for single words and short phrases, and text translation for sentence-like selections.
- Local settings for capture shortcut, close shortcut, provider configuration, model name, API key state, and prompt preset.
- Focused logging that excludes selected text and model payloads.

Out of scope for the first release:

- Cross-platform selected-text capture parity.
- Long-term history, semantic search, or document library features.
- Complex prompt marketplace or multi-agent orchestration.
- Clipboard mutation without restoration or explicit user action.
- Background processing of arbitrary windows without explicit shortcut activation.

## Functional Requirements

- The app registers a configurable global shortcut on startup.
- The app creates a system tray icon on startup and remains running when the popup window is hidden or closed.
- Clicking the tray icon shows the popup. The tray menu provides explicit show and quit actions.
- When the shortcut fires, the app attempts to read the current foreground selection.
- If selected text is available, the app opens a small popup near the active context or at a deterministic fallback position.
- The app classifies the selected text after backend capture succeeds. Single words and short phrase-like selections use the configured LLM word-study provider; sentence-like selections use DeepL text translation.
- Once the provider request starts, the popup should show a normalized preview of the selected word/text in the headword slot while the structured response is still pending.
- The app should consume provider responses as streams where supported and render completed partial fields before the final response is validated.
- While selection capture or structured response fields are pending, the popup should reserve the final result layout with skeleton placeholders and fade each field in as soon as that field is available.
- The app validates the response against the expected schema before rendering it.
- The default low-cost word-study provider is Gemini, with OpenAI available as the fallback provider when Gemini responses are not stable enough. Sentence-like translation uses DeepL when a DeepL API key is configured.
- The user can change capture shortcut, close shortcut, provider, model, result language, and API key from the popup settings panel. DeepL keys are stored through the same provider-key mechanism and are used for sentence-like translation.
- The user can adjust and persist the popup backdrop opacity from the popup settings panel.
- Model settings are selected from a provider model-list endpoint when an API key is configured, with a small default fallback list when model-list retrieval is unavailable.
- Result language settings are selected from an embedded dropdown list instead of a free-form text field.
- Shortcut settings are recorded from an actual key chord, normalized as a `+`-separated accelerator such as `Ctrl+Shift+X`, and re-registered without restarting the app.
- Close shortcut settings are recorded from an actual key chord, default to `Escape`, and may omit modifier keys.
- The user can open settings from a gear button in the popup header.
- The user can dismiss the popup with the configured close shortcut or the close affordance without quitting the app.
- Result UI actions such as copy and retry are not shown in the bottom action bar.
- The app shows actionable errors for unsupported selection source, empty selection, shortcut registration failure, provider failure, and schema validation failure.

## Non-Functional Requirements

- Startup should remain lightweight; avoid eager provider calls.
- Shortcut-to-popup feedback should feel immediate even when LLM processing is pending.
- Shortcut-to-request latency should avoid unnecessary frontend command round trips after capture.
- Raw selected text must not be written to logs.
- Clipboard-based capture must restore the previous clipboard contents before returning.
- Raw model stream chunks must not be emitted directly to the frontend.
- API keys must not be stored in plaintext project files or plaintext app config files.
- API keys must be read from dotenvx-injected environment variables first, with OS-backed secret storage on Windows as the fallback.
- API key values must not be returned to the frontend after save; the frontend only receives configured/not-configured state.
- Failures should be recoverable without restarting the app whenever possible.
- The UI should be usable with keyboard and pointer.
- The result schema should be versioned so prompt changes do not silently break rendering.

## Technical Requirements

- Use Tauri v2's command boundary for frontend-to-backend calls.
- Use a dedicated Rust module for selection capture so clipboard and UI Automation behavior is isolated from LLM and window code.
- Use a dedicated Rust module for LLM provider integration.
- Use `serde` structs for command inputs, command outputs, provider responses, and UI error payloads.
- Use Solid fine-grained state for popup state transitions: idle, capturing, requesting, ready, error.
- Keep Tauri capabilities narrow and explicit.

## UI Requirements

- First screen is the actual popup/work surface, not a landing page.
- The popup should open at a stable default size but remain user-resizable within minimum constraints; loading text, long words, and errors should wrap or scroll inside their panes instead of clipping.
- Word-study result rendering is a single dictionary-card layout with the headword, nuance, translations, and similar words visible in one scrollable surface. Text-translation result rendering is a simpler translation surface with the translated text and a source/translation segment view.
- The desktop popup window should support a transparent webview background, with the page backdrop rendered as a subtle translucent layer rather than an opaque full-window fill.
- When native window decorations are hidden, the popup should provide a narrow draggable region at the top edge without covering primary controls.
- The popup should not appear as a normal taskbar window while hidden; the tray icon is the persistent entry point.
- Pending capture and result areas should use skeleton placeholders instead of repeated loading text, so the result layout remains stable while streaming fields arrive.
- Settings opens from a header gear button, not a bottom action bar.
- Settings includes a compact persisted backdrop opacity control that updates the translucent popup background immediately.
- The current result view should not reserve a bottom action row for copy/retry/settings.
- Avoid persistent instructional text in the main popup.
- Use compact controls and clear state changes rather than decorative panels.
- For word-study results, keep the headword, nuance, meanings, and related-word details scannable in the resizable popup, with only the card body scrolling when content exceeds available height.
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
  "inflections": [
    {
      "kind": "plural | past | pastParticiple",
      "form": "string"
    }
  ],
  "translations": [
    {
      "text": "string",
      "note": "string or null",
      "example": {
        "sentence": "string",
        "japanese": "string"
      }
    }
  ],
  "nuance": "string",
  "synonyms": [
    {
      "term": "string",
      "japanese": "string",
      "usageComparison": "string"
    }
  ],
  "idioms": [
    {
      "idiom": "string",
      "japanese": "string",
      "example": "string"
    }
  ],
  "warnings": ["string"]
}
```

Rules:

- `schemaVersion` is required.
- `headword`, `translations`, and `nuance` are required for rendering.
- `headword` should be the dictionary/base form for a single inflected word when the base form is known, for example `went` should render as `go`.
- `inflections` should contain only irregular English forms for the headword: irregular noun plurals, irregular verb past forms, and irregular verb past participles. Use an empty array for regular forms or unavailable data.
- `translations` must contain at least one Japanese translation.
- `translations` should be dictionary-style Japanese sense entries, not explanation sentences, Japanese synonym lists, or multiple Japanese renderings of the same English meaning. Use one to three entries only when they represent real English-side dictionary sense boundaries such as part of speech, countable versus uncountable use, transitive versus intransitive use, concrete versus abstract use, legal/social versus technical use, or established idiomatic use. Near-duplicate Japanese paraphrases should be collapsed into the broadest common dictionary equivalent. Different Japanese collocations alone are not enough to split entries; for example, `採用` and `採択` should not be separate entries for `adoption` unless they reflect genuinely different English dictionary senses, and `デモ` and `実演` should not be separate entries for the same showing-how-something-works sense of `demonstration`.
- Each translation `note` must be `null` or one of these part-of-speech labels: `名詞`, `動詞`, `形容詞`, `副詞`, `前置詞`, `接続詞`, `代名詞`, `助動詞`, `冠詞`, `間投詞`, `句`, `成句`, `接頭辞`, `接尾辞`. Semantic domains such as math, comparison, or technical field labels are not allowed in `note`.
- Each translation must include one short natural example sentence in `example.sentence` and its Japanese translation in `example.japanese`. The example should demonstrate that translation entry's specific sense.
- `nuance` should be an intuitive explanation for deciding when the headword is appropriate.
- `synonyms` may be empty when reliable near words are unavailable; otherwise it should contain near words that help the user learn practical usage distinctions.
- Each synonym must include the English term, Japanese meaning, and a direct `usageComparison` sentence against the headword.
- `idioms` may be empty when reliable idioms are unavailable; otherwise it should contain up to three common idioms or fixed expressions associated with the headword.
- Each idiom must include the English idiom, Japanese meaning, and one short English example sentence.
- Antonyms are intentionally omitted from the first word-study result.
- The renderer must reject unknown or missing schema versions instead of guessing.

Text translation schema:

```json
{
  "schemaVersion": "lexi.text-translation.v1",
  "mode": "text-translation",
  "sourceLanguage": "auto",
  "detectedSourceLanguage": "string or null",
  "resultLanguage": "ja",
  "translatedText": "string",
  "segments": [
    {
      "source": "string",
      "translation": "string"
    }
  ],
  "warnings": ["string"]
}
```

Rules:

- `translatedText` is required and should contain the full translated selection.
- `segments` may start as a single source/translation pair and can be expanded later for sentence-by-sentence alignment.
- Raw provider responses must be wrapped in this schema before frontend rendering.
- Sentence-like selections are detected by backend heuristics such as newline, sentence punctuation, clause punctuation, or five or more whitespace-delimited tokens.

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
- Manual Windows PoC matrix for selected-text capture before relying on clipboard or UI Automation behavior in product code.
- Build verification for Tauri integration before release.

## Risks and Mitigations

- UI Automation support varies by app. Mitigation: run the PoC matrix first and keep unsupported-source handling explicit.
- Clipboard-based capture can fail or be unsafe when the current clipboard contains formats Lexi cannot duplicate. Mitigation: fail before clearing the clipboard and fall back to UI Automation.
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
- Whether OpenAI should become the default if Gemini structured responses are unstable in daily use.
- Whether non-Windows builds need equivalent OS keychain implementations in the first release.
- Should the popup appear near cursor, near selected text when possible, or at a fixed screen edge?
- Which applications are must-support targets for the first Windows release?

## References

- Tauri create project docs: https://v2.tauri.app/start/create-project/
- Tauri global shortcut plugin docs: https://v2.tauri.app/plugin/global-shortcut/
- Tauri capabilities docs: https://v2.tauri.app/security/capabilities/
- Microsoft UI Automation TextPattern GetSelection: https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationtextpattern-getselection
