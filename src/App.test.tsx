import { describe, expect, it, vi } from "vitest";
import { render } from "solid-js/web";
import { LEXI_RESULT_V1_SCHEMA_VERSION, type LexiResultV1 } from "./lib/schema";
import { PopupView, type PopupState } from "./App";

const noop = () => undefined;

function renderPopup(
  state: PopupState,
  activeResultTab: "meaning" | "related" = "meaning",
) {
  const root = document.createElement("div");
  render(
    () => (
      <PopupView
        state={state}
        copyStatus="idle"
        settingsOpen={false}
        activeResultTab={activeResultTab}
        onClose={noop}
        onCopy={noop}
        onRetry={noop}
        onToggleSettings={noop}
        onSetResultTab={noop}
      />
    ),
    root,
  );
  return root;
}

function mockResult(): LexiResultV1 {
  return {
    schemaVersion: LEXI_RESULT_V1_SCHEMA_VERSION,
    mode: "word-study",
    sourceLanguage: "en",
    resultLanguage: "ja",
    headword: "subtle",
    translations: [{ text: "微妙な", note: "気づきにくい差を表します。" }],
    nuance: "注意しないと見落とすほど控えめで、露骨ではない感覚があります。",
    synonyms: [
      { term: "delicate", japanese: "繊細な", nuance: "Fine detail." },
      { term: "slight", japanese: "わずかな", nuance: "Small amount." },
    ],
    usageComparisons: [
      {
        terms: ["subtle", "slight"],
        explanation: "subtle は見落としやすさ、slight は量の小ささに焦点があります。",
        examples: ["There is a subtle difference."],
      },
      {
        terms: ["subtle", "obvious"],
        explanation: "obvious は誰でもすぐわかる状態です。",
        examples: ["That hint was obvious."],
      },
    ],
    antonyms: [{ term: "obvious", japanese: "明らかな", nuance: "Easy to notice." }],
    warnings: ["Mock result."],
  };
}

describe("PopupView", () => {
  it("renders the requesting state after capture metadata is available", () => {
    const state: PopupState = {
      kind: "requesting",
      shortcut: "Ctrl+Shift+X",
      capture: {
        captureMethod: "uia-foreground-window",
        sourceProcess: "notepad.exe",
        sourceWindowTitle: "note.txt - Notepad",
        characterCount: 42,
        multiline: true,
      },
    };

    const root = renderPopup(state);

    expect(root.textContent).toContain("処理中");
    expect(root.textContent).toContain("結果を組み立て中");
    expect(root.textContent).toContain("42 文字を取得しました");
  });

  it("renders mock LexiResultV1 content and result actions", () => {
    const state: PopupState = {
      kind: "ready",
      shortcut: "Ctrl+Shift+X",
      capture: {
        captureMethod: "uia-foreground-window",
        sourceProcess: "notepad.exe",
        sourceWindowTitle: "note.txt - Notepad",
        characterCount: 42,
        multiline: true,
      },
      result: mockResult(),
    };

    const root = renderPopup(state);

    expect(root.textContent).toContain("語彙メモ");
    expect(root.textContent).toContain("subtle");
    expect(root.textContent).toContain("微妙な");
    expect(root.textContent).toContain("意味");
    expect(root.textContent).toContain("微妙な");
    expect(root.textContent).not.toContain("ニュアンス");
    expect(root.textContent).toContain("注意しないと見落とすほど控えめ");
    expect(root.textContent).not.toContain("使い分け");
    expect(root.textContent).toContain("関連語");
    expect(root.textContent).toContain("コピー");
    expect(root.textContent).toContain("再試行");
    expect(root.textContent).toContain("設定");
    expect(root.textContent).toContain("閉じる");
  });

  it("keeps nuance content next to the headword", () => {
    const state: PopupState = {
      kind: "ready",
      shortcut: "Ctrl+Shift+X",
      capture: {
        captureMethod: "uia-foreground-window",
        sourceProcess: "notepad.exe",
        sourceWindowTitle: "note.txt - Notepad",
        characterCount: 42,
        multiline: true,
      },
      result: mockResult(),
    };

    const root = renderPopup(state);

    expect(root.textContent).toContain("注意しないと見落とすほど控えめ");
    expect(root.textContent).not.toContain("ニュアンス");
  });

  it("renders related words as expandable rows with usage details", () => {
    const state: PopupState = {
      kind: "ready",
      shortcut: "Ctrl+Shift+X",
      capture: {
        captureMethod: "uia-foreground-window",
        sourceProcess: "notepad.exe",
        sourceWindowTitle: "note.txt - Notepad",
        characterCount: 42,
        multiline: true,
      },
      result: mockResult(),
    };

    const root = renderPopup(state, "related");

    expect(root.querySelectorAll(".related-word").length).toBeGreaterThan(0);
    expect(root.querySelector(".related-word-trigger")?.getAttribute("aria-expanded")).toBe(
      "false",
    );
    expect(root.textContent).toContain("slight");
    expect(root.textContent).toContain("わずかな");
    expect(root.textContent).toContain("subtle は見落としやすさ");
    expect(root.textContent).toContain("obvious");
    expect(root.textContent).not.toContain("obvious は誰でもすぐわかる状態");
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
      context: {
        selectionErrorCode: "SelectionUnsupported",
        captureMethod: "uia-foreground-window",
        sourceProcess: "example.exe",
        sourceWindowTitle: "Example",
        retryCapture: null,
      },
    };

    const root = renderPopup(state);

    expect(root.textContent).toContain("確認が必要");
    expect(root.textContent).toContain("This app does not expose selected text to Lexi.");
    expect(root.textContent).toContain("SelectionUnsupported");
    expect(root.textContent).toContain("uia-foreground-window");
    expect(root.textContent).toContain("example.exe");
    expect(root.textContent).toContain(
      "The active control does not support a selected-text UI Automation pattern.",
    );
  });

  it("wires copy as a keyboard-reachable button", () => {
    const onCopy = vi.fn();
    const root = document.createElement("div");
    document.body.appendChild(root);
    const state: PopupState = {
      kind: "ready",
      shortcut: "Ctrl+Shift+X",
      capture: {
        captureMethod: "uia-foreground-window",
        sourceProcess: "notepad.exe",
        sourceWindowTitle: "note.txt - Notepad",
        characterCount: 42,
        multiline: true,
      },
      result: mockResult(),
    };

    render(
      () => (
        <PopupView
          state={state}
          copyStatus="idle"
          settingsOpen={false}
          activeResultTab="meaning"
          onClose={noop}
          onCopy={onCopy}
          onRetry={noop}
          onToggleSettings={noop}
          onSetResultTab={noop}
        />
      ),
      root,
    );

    const copyButton = Array.from(root.querySelectorAll("button")).find(
      (button) => button.textContent === "コピー",
    );

    expect(copyButton).toBeInstanceOf(HTMLButtonElement);
    copyButton?.click();
    expect(onCopy).toHaveBeenCalledOnce();
    root.remove();
  });
});
