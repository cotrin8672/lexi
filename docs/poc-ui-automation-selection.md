# UI Automation Selected Text PoC

Status: complete for Phase 1

## Goal

Prove whether Lexi can reliably read selected text from the current foreground application on Windows using UI Automation before building the full shortcut-to-LLM workflow.

The PoC should answer two questions:

- Can selected text be acquired without modifying the clipboard?
- Which target applications expose enough UI Automation support for the first release?

## Background

Microsoft UI Automation exposes text through TextPattern and TextRange. `IUIAutomationTextPattern::GetSelection` returns the currently selected text ranges for a text-based control when the control supports text selection. It may return empty ranges for an insertion point, or no usable ranges when the control does not support text selection.

Because support is provider-dependent, success in one app does not imply success in another.

## Non-Goals

- Do not integrate an LLM provider.
- Do not design the final popup UI.
- Do not mutate the clipboard as part of the primary PoC path.
- Do not claim cross-platform support.

## Candidate Rust API

```rust
pub struct CapturedSelection {
    pub text: String,
    pub source_process: Option<String>,
    pub source_window_title: Option<String>,
}

pub enum SelectionCaptureError {
    NoForegroundWindow,
    FocusedElementUnavailable,
    TextPatternUnavailable,
    SelectionUnsupported,
    EmptySelection,
    AccessDenied,
    WindowsApiFailure(String),
}

pub fn capture_selected_text() -> Result<CapturedSelection, SelectionCaptureError>;
```

## PoC Steps

1. Locate the foreground window and focused UI Automation element.
2. Attempt to retrieve TextPattern from the focused element.
3. If the focused element has no TextPattern, inspect a limited set of relevant descendants before failing.
4. Call GetSelection.
5. Convert non-empty selected ranges to plain text.
6. Normalize line endings to `\n` for internal processing.
7. Return explicit error codes for unsupported, empty, access denied, and API failure cases.
8. Print only metadata and redacted text length in PoC logs.

## Target Matrix

Minimum applications to test:

| Application | Scenario | Expected result |
| --- | --- | --- |
| Notepad | Plain selected text | Should capture |
| Microsoft Word | Rich document selection | Should capture or document limitation |
| VS Code | Editor selected text | Should capture or document limitation |
| Browser text field | Input/textarea selection | Should capture |
| Browser web page | Static page text selection | Should capture or document limitation |
| PDF viewer | Selected text in PDF | Document actual behavior |
| Terminal | Selected console text | Document actual behavior |

For each result, record:

- App name and version.
- Control type if available.
- Whether TextPattern was present.
- Whether GetSelection returned ranges.
- Character count.
- Whether multiline text preserved line breaks.
- Failure code when capture failed.

## Acceptance Criteria

- The PoC can distinguish unsupported source, empty selection, and API failure.
- Logs contain no raw selected text.
- Representative browser and document-reader targets are tested.
- The final PoC note recommends one of:
  - proceed with UI Automation as primary capture path;
  - use UI Automation with documented app-specific limitations;
  - reject UI Automation as primary path and evaluate another capture method.

## Implementation Notes

- Keep the PoC behind a small command or local binary so it can be removed or promoted cleanly.
- Do not mix provider calls or popup logic into the PoC module.
- Prefer typed Rust errors from the start; they will become product errors later.
- If using a Windows Rust crate, document why it was chosen and whether it wraps COM safely enough for this use case.

Current implementation:

- Uses the official `windows` crate rather than a wrapper crate so UI Automation COM calls, HRESULTs, and pattern availability remain visible during the PoC.
- Adds `selection::capture_selected_text()`, a temporary Tauri command named `capture_selection_diagnostics`, and a dev-only binary named `capture_selection_poc`.
- The command returns only redacted metadata: success flag, stable code, source process when available, source window title, character count, and multiline flag.
- The selected text is normalized to `\n` internally and is not returned to the frontend by the diagnostic command.
- The capture path is strategy-based. It currently tries `uia-focused-element` and then `uia-foreground-window`.
- Each strategy checks the target element and a bounded set of `TextPattern` descendants, then returns the first non-empty selection.
- Diagnostics include `captureMethod` to make future app-specific fallback decisions without exposing raw selected text.

Manual test command:

```powershell
rtk cargo run --bin capture_selection_poc -- 3000
```

After starting the command, focus the target application and select text before the delay elapses. The output is JSON diagnostics only and does not include the selected text.

Manual matrix results:

| Application | Scenario | Result | Notes |
| --- | --- | --- | --- |
| Chrome | Browser selection | Success | User-confirmed selected text capture. Raw text was not printed. |
| Zen Browser | Browser page selection | Success | `sourceProcess=zen.exe`, `characterCount=48`, `multiline=true`. |
| Zotero | Document/PDF-reader selection | Success | `sourceProcess=zotero.exe`, `characterCount=78`, `multiline=false`. |
| Notepad | Plain selected text | Deferred | Not required for the Phase 1 go/no-go decision. |
| Microsoft Word | Rich document selection | Deferred | Not required for the Phase 1 go/no-go decision. |
| VS Code | Editor selected text | Deferred | Not required for the Phase 1 go/no-go decision. |
| Terminal | Selected console text | Deferred | Not required for the Phase 1 go/no-go decision. |

## Phase 1 Recommendation

Proceed with Windows UI Automation as the primary selected-text capture path for the next implementation phase.

The PoC captured selected text from representative browser and document-reader surfaces without printing raw selected text. Additional application coverage, including Notepad, VS Code, Word, and terminal behavior, should be treated as compatibility expansion rather than a blocker for Phase 2.

## Open Questions

- Which exact apps are must-pass before the project proceeds?
- Should the PoC include elevated and non-elevated target application comparisons?
- What is the acceptable fallback if UI Automation fails: user-visible unsupported error, manual paste, or clipboard-preserving copy simulation?

## References

- Microsoft UI Automation TextPattern GetSelection: https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationtextpattern-getselection
- Microsoft Text and TextRange patterns: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-about-text-and-textrange-patterns
- Microsoft supported control patterns guidance: https://learn.microsoft.com/en-us/dotnet/framework/ui-automation/get-supported-ui-automation-control-patterns
