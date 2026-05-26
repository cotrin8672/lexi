# Lexi

Lexi is a Windows-first Tauri desktop app concept for capturing selected text with a global shortcut and showing a compact LLM-generated result popup.

The repository currently starts from the Tauri v2 + SolidJS + TypeScript template. Product and technical direction are tracked in `docs`.

## Docs

- `docs/requirements.md`: initial release scope, requirements, schema, risks, milestones, and open questions.
- `docs/architecture.md`: proposed Tauri/Rust/Solid module boundaries and data flow.
- `docs/poc-ui-automation-selection.md`: PoC specification for Windows selected-text capture.
- `docs/implementation-plan.md`: detailed phase-by-phase build plan.

## Development

Use `rtk` when running commands in this repository.

```powershell
rtk pnpm install
rtk pnpm tauri dev
```

Recommended editor setup:

- VS Code
- Tauri extension
- rust-analyzer
