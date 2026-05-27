import { beforeEach, describe, expect, it, vi } from "vitest";
import { render } from "solid-js/web";
import { invoke } from "@tauri-apps/api/core";
import { LEXI_RESULT_V1_SCHEMA_VERSION, type LexiResultV1 } from "./lib/schema";
import App, { PopupView, type PopupState } from "./App";

const noop = () => undefined;
const tauriMocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
  hide: vi.fn(),
  listeners: {} as Record<string, Array<(event: { payload: unknown }) => void>>,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: tauriMocks.invoke,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: tauriMocks.listen,
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ hide: tauriMocks.hide }),
}));

beforeEach(() => {
  tauriMocks.invoke.mockReset();
  tauriMocks.listen.mockReset();
  tauriMocks.hide.mockReset();
  tauriMocks.listeners = {};
  tauriMocks.listen.mockImplementation((event: string, handler: (event: { payload: unknown }) => void) => {
    tauriMocks.listeners[event] = [...(tauriMocks.listeners[event] ?? []), handler];
    return Promise.resolve(() => undefined);
  });
  document.body.innerHTML = "";
});

function renderPopup(
  state: PopupState,
  activeResultTab: "meaning" | "related" = "meaning",
) {
  const root = document.createElement("div");
  render(
    () => (
      <PopupView
        state={state}
        settingsOpen={false}
        providerSettings={null}
        activeResultTab={activeResultTab}
        onClose={noop}
        onRetry={noop}
        onToggleSettings={noop}
        onSaveSettings={async () => undefined}
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
      {
        term: "delicate",
        japanese: "繊細な",
        nuance: "細部や壊れやすさに焦点があります。",
        usageComparison:
          "subtle は気づきにくさ、delicate は細かさや壊れやすさを言う時に使います。",
      },
      {
        term: "slight",
        japanese: "わずかな",
        nuance: "量や程度が小さい感じです。",
        usageComparison:
          "subtle は読み取りにくさ、slight は単に量が小さいことを言う時に使います。",
      },
    ],
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

  it("renders LexiResultV1 content without the old bottom actions", () => {
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
    expect(root.querySelector('button[aria-label="設定"]')).toBeInstanceOf(
      HTMLButtonElement,
    );
    expect(root.textContent).not.toContain("コピー");
    expect(root.textContent).not.toContain("再試行");
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
    expect(root.textContent).toContain("使い分け");
    expect(root.textContent).toContain("subtle は読み取りにくさ");
    expect(root.textContent).not.toContain("対義語");
  });

  it("renders partial streaming content before final validation", () => {
    const state: PopupState = {
      kind: "streaming",
      shortcut: "Ctrl+Shift+X",
      requestId: 7,
      phase: "streaming",
      capture: {
        captureMethod: "uia-foreground-window",
        sourceProcess: "notepad.exe",
        sourceWindowTitle: "note.txt - Notepad",
        characterCount: 42,
        multiline: true,
      },
      partial: {
        headword: "subtle",
        translations: [{ text: "微妙な", note: "形容詞" }],
        nuance: "露骨ではなく、注意して初めて伝わる感じ。",
        synonyms: [],
        warnings: [],
      },
    };

    const root = renderPopup(state);

    expect(root.textContent).toContain("生成中");
    expect(root.textContent).toContain("subtle");
    expect(root.textContent).toContain("微妙な");
    expect(root.textContent).toContain("露骨ではなく");
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

  it("opens provider settings from the header gear", () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      provider: "gemini",
      models: [
        { id: "gemini-2.5-flash-lite", label: "Gemini 2.5 Flash-Lite" },
      ],
      fetched: false,
      warning: "API key is not configured; showing default models.",
    });
    const onSaveSettings = vi.fn(async () => undefined);
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
          settingsOpen
          providerSettings={{
            provider: "gemini",
            model: "gemini-2.5-flash-lite",
            resultLanguage: "ja",
            promptMode: "word-study",
            apiKeyConfigured: false,
          }}
          activeResultTab="meaning"
          onClose={noop}
          onRetry={noop}
          onToggleSettings={noop}
          onSaveSettings={onSaveSettings}
          onSetResultTab={noop}
        />
      ),
      root,
    );

    expect(root.textContent).toContain("Provider");
    expect(root.textContent).toContain("Gemini");
    expect(
      Array.from(root.querySelectorAll("select")).some(
        (select) => select.value === "gemini-2.5-flash-lite",
      ),
    ).toBe(true);
    expect(
      Array.from(root.querySelectorAll("select")).some(
        (select) => select.value === "ja",
      ),
    ).toBe(true);
    expect(invoke).toHaveBeenCalledWith("list_provider_models", {
      provider: "gemini",
    });

    root.remove();
  });

  it("saves the selected model from the dropdown", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      provider: "gemini",
      models: [
        { id: "gemini-2.5-flash-lite", label: "Gemini 2.5 Flash-Lite" },
        { id: "gemini-2.5-flash", label: "Gemini 2.5 Flash" },
      ],
      fetched: true,
      warning: null,
    });
    const onSaveSettings = vi.fn(async () => undefined);
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
          settingsOpen
          providerSettings={{
            provider: "gemini",
            model: "gemini-2.5-flash-lite",
            resultLanguage: "ja",
            promptMode: "word-study",
            apiKeyConfigured: true,
          }}
          activeResultTab="meaning"
          onClose={noop}
          onRetry={noop}
          onToggleSettings={noop}
          onSaveSettings={onSaveSettings}
          onSetResultTab={noop}
        />
      ),
      root,
    );

    await Promise.resolve();
    const selects = Array.from(root.querySelectorAll("select"));
    const modelSelect = selects.find((select) =>
      Array.from(select.options).some((option) => option.value === "gemini-2.5-flash"),
    );
    expect(modelSelect).toBeInstanceOf(HTMLSelectElement);

    modelSelect!.value = "gemini-2.5-flash";
    modelSelect!.dispatchEvent(new Event("change", { bubbles: true }));
    root.querySelector("form")?.dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
    await Promise.resolve();

    expect(onSaveSettings).toHaveBeenCalledWith({
      provider: "gemini",
      model: "gemini-2.5-flash",
      resultLanguage: "ja",
      promptMode: "word-study",
      apiKey: null,
    });

    root.remove();
  });
});

describe("App stream flow", () => {
  it("retries through the streaming command instead of the non-stream transform path", async () => {
    vi.mocked(invoke).mockImplementation((command) => {
      if (command === "get_provider_settings") {
        return Promise.resolve({
          provider: "gemini",
          model: "gemini-2.5-flash-lite",
          resultLanguage: "ja",
          promptMode: "word-study",
          apiKeyConfigured: true,
        });
      }
      if (command === "get_shortcut_status") {
        return Promise.resolve({
          shortcut: "Ctrl+Shift+X",
          registered: true,
          registrationError: null,
        });
      }
      if (command === "run_transform_stream") {
        return Promise.resolve();
      }

      return Promise.reject(new Error(`unexpected command ${command}`));
    });

    const root = document.createElement("div");
    document.body.appendChild(root);
    render(() => <App />, root);
    await Promise.resolve();
    await Promise.resolve();

    tauriMocks.listeners["lexi:transform"][0]({
      payload: {
        status: "started",
        requestId: 42,
        shortcut: "Ctrl+Shift+X",
        captureMethod: "uia-foreground-window",
        sourceProcess: "notepad.exe",
        sourceWindowTitle: "note.txt - Notepad",
        characterCount: 6,
        multiline: false,
        provider: "gemini",
        model: "gemini-2.5-flash-lite",
      },
    });
    tauriMocks.listeners["lexi:transform"][0]({
      payload: {
        status: "failed",
        requestId: 42,
        error: {
          code: "InvalidModelOutput",
          userMessage: "結果を表示できませんでした。",
          diagnosticMessage: "provider stream completed without JSON content",
          retryable: true,
        },
      },
    });

    const retryButton = Array.from(root.querySelectorAll("button")).find(
      (button) => button.textContent === "再試行",
    );
    expect(retryButton).toBeInstanceOf(HTMLButtonElement);
    retryButton!.click();
    await Promise.resolve();

    expect(invoke).toHaveBeenCalledWith("run_transform_stream", {
      capture: {
        shortcut: "Ctrl+Shift+X",
        captureMethod: "uia-foreground-window",
        sourceProcess: "notepad.exe",
        sourceWindowTitle: "note.txt - Notepad",
        characterCount: 6,
        multiline: false,
      },
    });
    expect(invoke).not.toHaveBeenCalledWith("run_transform");

    root.remove();
  });
});
