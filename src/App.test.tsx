import { describe, expect, it } from "vitest";
import { render } from "solid-js/web";
import { PopupView, type PopupState } from "./App";

describe("PopupView", () => {
  it("renders captured metadata from the capture event contract", () => {
    const state: PopupState = {
      kind: "captured",
      shortcut: "Ctrl+Shift+X",
      captureMethod: "uia-foreground-window",
      sourceProcess: "notepad.exe",
      sourceWindowTitle: "note.txt - Notepad",
      characterCount: 42,
      multiline: true,
    };

    const root = document.createElement("div");
    render(() => <PopupView state={state} onClose={() => undefined} />, root);

    expect(root.textContent).toContain("Captured");
    expect(root.textContent).toContain("42");
    expect(root.textContent).toContain("notepad.exe");
    expect(root.textContent).toContain("uia-foreground-window");
    expect(root.textContent).toContain("Yes");
  });

  it("renders user-safe errors and diagnostics", () => {
    const state: PopupState = {
      kind: "error",
      shortcut: "Ctrl+Shift+X",
      error: {
        code: "SelectionUnavailable",
        userMessage: "This app does not expose selected text to Lexi.",
        diagnosticMessage:
          "The active control does not support a selected-text UI Automation pattern.",
        retryable: false,
      },
      selectionErrorCode: "SelectionUnsupported",
      captureMethod: "uia-foreground-window",
      sourceProcess: "example.exe",
      sourceWindowTitle: "Example",
    };

    const root = document.createElement("div");
    render(() => <PopupView state={state} onClose={() => undefined} />, root);

    expect(root.textContent).toContain("Needs attention");
    expect(root.textContent).toContain("This app does not expose selected text to Lexi.");
    expect(root.textContent).toContain("SelectionUnsupported");
    expect(root.textContent).toContain("uia-foreground-window");
    expect(root.textContent).toContain("example.exe");
    expect(root.textContent).toContain(
      "The active control does not support a selected-text UI Automation pattern.",
    );
  });
});
