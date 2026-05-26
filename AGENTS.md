@C:\Users\gummy\.codex\RTK.md

# Lexi Project Instructions

## Project Goal

Lexi is a small desktop popup app for capturing currently selected text, sending it to an LLM-backed transformation pipeline, and showing structured results quickly without disrupting the user's current workflow.

## Current Stack

- Desktop shell: Tauri v2.
- Backend: Rust under `src-tauri`.
- Frontend: SolidJS + TypeScript + Vite under `src`.
- Primary target for the initial PoC: Windows desktop.

## Working Rules

- Prefix shell commands with `rtk`.
- Keep the Rust backend responsible for OS integration, global shortcuts, UI Automation, filesystem, credentials, and LLM boundary calls.
- Keep the Solid frontend responsible for presentation state, popup interaction, user settings forms, and rendering structured results.
- Prefer typed request/response structs across the Tauri command boundary. Avoid passing loosely shaped JSON unless the data is intentionally provider-specific.
- Add Tauri permissions through `src-tauri/capabilities/*.json` with the narrowest command/plugin permissions needed for the active window.
- Treat selected text as sensitive. Do not log raw selected text, prompts, model responses, API keys, or clipboard contents.
- Do not add background persistence of captured text unless a doc explicitly says retention is required.

## Documentation Expectations

- Update `docs/requirements.md` when product scope, LLM output schema, UI behavior, or non-functional requirements change.
- Update `docs/architecture.md` when adding new commands, plugins, crates, frontend state boundaries, or storage decisions.
- Update `docs/poc-ui-automation-selection.md` while proving selected-text capture behavior across target applications.
- Record unresolved decisions in the relevant document's `Open Questions` section instead of encoding assumptions directly into implementation.

## Verification

- For frontend-only changes, run `rtk pnpm build` when dependencies are installed.
- For Rust-only changes, run `rtk cargo check` from `src-tauri`.
- For Tauri integration changes, run `rtk pnpm tauri build` when the local environment has Tauri prerequisites.
- For Windows UI Automation changes, verify manually against the PoC target matrix before treating the behavior as supported.
