import { beforeEach, describe, expect, it, vi } from "vitest";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { invoke } from "@tauri-apps/api/core";
import {
  LEXI_JP_WORD_CANDIDATES_V1_SCHEMA_VERSION,
  LEXI_TEXT_TRANSLATION_V1_SCHEMA_VERSION,
  LEXI_RESULT_V1_SCHEMA_VERSION,
  TRANSLATION_NOTE_VALUES,
  validateLexiResultV1,
  type JapaneseWordCandidatesResultV1,
  type LexiResult,
  type LexiResultV1,
  type TextTranslationResultV1,
} from "./lib/schema";
import App, {
  DEFAULT_CAPTURE_SHORTCUT,
  DEFAULT_CLOSE_SHORTCUT,
  DEFAULT_PRONUNCIATION_SHORTCUT,
  PopupView,
  speakableHeadwordForState,
  type PopupState,
  type ProviderSettings,
  type SyncStatus,
} from "./App";
import { SettingsView } from "./SettingsApp";

const noop = () => undefined;
const tauriMocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
  hide: vi.fn(),
  setMinSize: vi.fn(),
  setSize: vi.fn(),
  center: vi.fn(),
  show: vi.fn(),
  listeners: {} as Record<string, Array<(event: { payload: unknown }) => void>>,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: tauriMocks.invoke,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: tauriMocks.listen,
}));

vi.mock("@tauri-apps/api/window", () => ({
  LogicalSize: class LogicalSize {
    width: number;
    height: number;

    constructor(width: number, height: number) {
      this.width = width;
      this.height = height;
    }
  },
  getCurrentWindow: () => ({
    label: "main",
    hide: tauriMocks.hide,
    setMinSize: tauriMocks.setMinSize,
    setSize: tauriMocks.setSize,
    center: tauriMocks.center,
    show: tauriMocks.show,
    startDragging: vi.fn(async () => undefined),
  }),
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn(async () => undefined),
}));

beforeEach(() => {
  tauriMocks.invoke.mockReset();
  tauriMocks.listen.mockReset();
  tauriMocks.hide.mockReset();
  tauriMocks.setMinSize.mockReset();
  tauriMocks.setSize.mockReset();
  tauriMocks.center.mockReset();
  tauriMocks.show.mockReset();
  tauriMocks.listeners = {};
  tauriMocks.listen.mockImplementation(
    (event: string, handler: (event: { payload: unknown }) => void) => {
      tauriMocks.listeners[event] = [
        ...(tauriMocks.listeners[event] ?? []),
        handler,
      ];
      return Promise.resolve(() => undefined);
    },
  );
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
        providerSettings={null}
        syncAuthStatus={{
          configured: true,
          signedIn: true,
          userId: "user-1",
          userEmail: "lexi@example.com",
          callbackUrl: "http://localhost:38271/auth/callback",
        }}
        activeResultTab={activeResultTab}
        themeMode="light"
        onClose={noop}
        onRetry={noop}
        onSetResultTab={noop}
      />
    ),
    root,
  );
  return root;
}

function defaultSyncStatus(overrides: Partial<SyncStatus> = {}): SyncStatus {
  return {
    configured: true,
    signedIn: true,
    lifecycle: "idle",
    pendingMutations: 0,
    lastServerRevision: 0,
    lastSyncAt: null,
    lastError: null,
    ...overrides,
  };
}

function defaultProviderSettings(
  overrides: Partial<ProviderSettings> = {},
): ProviderSettings {
  return {
    shortcut: DEFAULT_CAPTURE_SHORTCUT,
    pronunciationShortcut: DEFAULT_PRONUNCIATION_SHORTCUT,
    closeShortcut: DEFAULT_CLOSE_SHORTCUT,
    backgroundOpacity: 0.94,
    theme: "light",
    provider: "gemini",
    model: "gemini-2.5-flash-lite",
    resultLanguage: "ja",
    promptMode: "word-study",
    apiKeyConfigured: false,
    deeplApiKeyConfigured: false,
    ...overrides,
  };
}

function renderSettingsView(
  overrides: Partial<ProviderSettings> = {},
  options: {
    themeMode?: "light" | "dark";
    backgroundOpacity?: number | (() => number);
    onSave?: (update: unknown) => Promise<void>;
    onToggleTheme?: () => void;
    onSetBackgroundOpacity?: (opacity: number) => void;
  } = {},
) {
  const settings = defaultProviderSettings(overrides);
  const root = document.createElement("div");
  document.body.appendChild(root);
  render(
    () => (
      <SettingsView
        settings={settings}
        syncAuthStatus={null}
        syncStatus={defaultSyncStatus({ signedIn: false })}
        themeMode={options.themeMode ?? "light"}
        backgroundOpacity={
          typeof options.backgroundOpacity === "function"
            ? options.backgroundOpacity()
            : (options.backgroundOpacity ?? settings.backgroundOpacity)
        }
        onSave={options.onSave ?? (async () => undefined)}
        onToggleTheme={options.onToggleTheme ?? noop}
        onSetBackgroundOpacity={options.onSetBackgroundOpacity ?? noop}
      />
    ),
    root,
  );
  return root;
}

function readyState(result: LexiResult = mockResult()): PopupState {
  return {
    kind: "ready",
    shortcut: DEFAULT_CAPTURE_SHORTCUT,
    capture: {
      captureMethod: "uia-foreground-window",
      sourceProcess: "notepad.exe",
      sourceWindowTitle: "note.txt - Notepad",
      characterCount: 42,
      multiline: true,
    },
    result,
  };
}

function readyCapture() {
  const state = readyState();
  if (state.kind !== "ready") {
    throw new Error("readyState helper returned non-ready state");
  }
  return state.capture;
}

function mockResult(): LexiResultV1 {
  return {
    schemaVersion: LEXI_RESULT_V1_SCHEMA_VERSION,
    mode: "word-study",
    sourceLanguage: "en",
    resultLanguage: "ja",
    headword: "subtle",
    inflections: [],
    translations: [
      {
        text: "delicate",
        note: TRANSLATION_NOTE_VALUES[2],
        example: {
          sentence: "She noticed a subtle change in his voice.",
          japanese: "She operates a small business.",
        },
      },
    ],
    nuance: "Used for something understated and easy to miss.",
    synonyms: [
      {
        term: "delicate",
        japanese: "She operates a small business.",
        usageComparison:
          "Choose subtle for hard-to-notice differences; choose delicate for fine detail.",
      },
      {
        term: "slight",
        japanese: "She operates a small business.",
        usageComparison:
          "Choose subtle for understated meaning; choose slight for a small degree.",
      },
    ],
    idioms: [
      {
        idiom: "a subtle hint",
        japanese: "She operates a small business.",
        example: "She gave me a subtle hint.",
      },
    ],
    warnings: ["Mock result."],
  };
}

function mockTextTranslationResult(): TextTranslationResultV1 {
  return {
    schemaVersion: LEXI_TEXT_TRANSLATION_V1_SCHEMA_VERSION,
    mode: "text-translation",
    sourceLanguage: "auto",
    detectedSourceLanguage: "EN",
    resultLanguage: "ja",
    translatedText: "This is a translated test.",
    segments: [
      {
        source: "This is a test.",
        translation: "This is a translated test.",
      },
    ],
    warnings: [],
  };
}

function mockJapaneseWordCandidatesResult(): JapaneseWordCandidatesResultV1 {
  return {
    schemaVersion: LEXI_JP_WORD_CANDIDATES_V1_SCHEMA_VERSION,
    mode: "jp-word-candidates",
    sourceLanguage: "ja",
    resultLanguage: "en",
    query: "採用",
    candidates: [
      {
        term: "adopt",
        partOfSpeech: "動詞",
        japaneseNuance: "方針・方法・制度などを選んで使い始める",
        usageNote: "案や制度を公式に取り入れる文脈で使う。",
        example: {
          sentence: "The team adopted a new policy.",
          japanese: "チームは新しい方針を採用した。",
        },
        confidence: "high",
      },
      {
        term: "hire",
        partOfSpeech: "動詞",
        japaneseNuance: "人を雇う",
        usageNote: "人材を採用する文脈で使う。",
        example: {
          sentence: "They hired a new engineer.",
          japanese: "新しいエンジニアを採用した。",
        },
        confidence: "medium",
      },
    ],
    warnings: ["Context can change the best choice."],
  };
}

function emptyPartialResultForTest() {
  return {
    query: null,
    candidates: [],
    headword: null,
    inflections: [],
    translations: [],
    nuance: null,
    synonyms: [],
    idioms: [],
    warnings: [],
  };
}

describe("PopupView", () => {
  it("does not render Lexi as a placeholder headword while idle", () => {
    const root = renderPopup({ kind: "idle", shortcut: DEFAULT_CAPTURE_SHORTCUT });

    expect(root.querySelector(".headword")?.textContent).toBe("");
    expect(root.textContent).toContain("待機中");
  });

  it("starts the capture flow with the skeleton result layout", () => {
    const root = renderPopup({ kind: "capturing", shortcut: DEFAULT_CAPTURE_SHORTCUT });

    expect(root.querySelector(".headword")?.textContent).toBe("");
    expect(root.querySelector(".skeleton-block")).toBeInstanceOf(HTMLElement);
    expect(root.textContent).not.toContain("Similar words");
    expect(root.textContent).not.toContain("Idioms");
    expect(root.querySelector('[aria-busy="true"]')).toBeInstanceOf(
      HTMLElement,
    );
    expect(root.textContent).not.toContain("選択テキストを確認中");
  });

  it("renders the requesting state after capture metadata is available", () => {
    const state: PopupState = {
      kind: "requesting",
      shortcut: DEFAULT_CAPTURE_SHORTCUT,
      capture: {
        captureMethod: "uia-foreground-window",
        sourceProcess: "notepad.exe",
        sourceWindowTitle: "note.txt - Notepad",
        characterCount: 42,
        multiline: true,
      },
    };

    const root = renderPopup(state);

    expect(root.querySelector(".headword")?.textContent).toBe("");
    expect(root.querySelector(".skeleton-block")).toBeInstanceOf(HTMLElement);
    expect(root.textContent).not.toContain("Similar words");
    expect(root.textContent).not.toContain("Idioms");
    const busyRegion = root.querySelector('[aria-busy="true"]');
    expect(busyRegion).toBeInstanceOf(HTMLElement);
    expect(busyRegion?.getAttribute("aria-label")).toContain("42");
    expect(root.textContent).not.toContain("生成中");
  });

  it("renders a headword voice button for word-study results", () => {
    const root = renderPopup(readyState());
    const button = root.querySelector(".headword-voice-button");

    expect(button).toBeInstanceOf(HTMLButtonElement);
    expect(button?.getAttribute("aria-label")).toContain("subtle");
    expect(button?.getAttribute("aria-label")).toContain(DEFAULT_PRONUNCIATION_SHORTCUT);
    expect(speakableHeadwordForState(readyState())).toBe("subtle");
  });

  it("does not render a headword voice button for text translation results", () => {
    const root = renderPopup(readyState(mockTextTranslationResult()));

    expect(root.querySelector(".headword-voice-button")).toBeNull();
    expect(speakableHeadwordForState(readyState(mockTextTranslationResult()))).toBeNull();
  });

  it("renders japanese word candidate results with query, nuances, and examples", () => {
    const root = renderPopup(readyState(mockJapaneseWordCandidatesResult()));

    expect(root.querySelector(".headword")?.textContent).toBe("採用");
    expect(root.querySelector(".headword-voice-button")).toBeNull();
    expect(speakableHeadwordForState(readyState(mockJapaneseWordCandidatesResult()))).toBeNull();
    expect(root.textContent).toContain("adopt");
    expect(root.textContent).toContain("hire");
    expect(root.textContent).toContain("方針・方法・制度などを選んで使い始める");
    expect(root.textContent).toContain("The team adopted a new policy.");
    expect(root.textContent).toContain("チームは新しい方針を採用した。");
    expect(root.querySelectorAll(".candidate-row").length).toBe(2);
    expect(root.querySelector(".jp-candidates-layout")).toBeInstanceOf(HTMLElement);
  });

  it("renders streaming japanese word candidates with query and partial rows", () => {
    const root = renderPopup({
      kind: "streaming",
      shortcut: DEFAULT_CAPTURE_SHORTCUT,
      requestId: 11,
      mode: "jp-word-candidates",
      sourceText: "採用",
      phase: "streaming",
      capture: readyCapture(),
      partial: {
        ...emptyPartialResultForTest(),
        query: "採用",
        candidates: [mockJapaneseWordCandidatesResult().candidates[0]],
      },
    });

    expect(root.querySelector(".headword")?.textContent).toBe("採用");
    expect(root.querySelector(".headword-voice-button")).toBeNull();
    expect(root.textContent).toContain("adopt");
    expect(root.querySelectorAll(".candidate-row.content-reveal").length).toBe(1);
    expect(root.querySelector(".candidate-row.skeleton-row")).toBeNull();
  });

  it("renders candidate skeleton rows while streaming before any candidates arrive", () => {
    const root = renderPopup({
      kind: "streaming",
      shortcut: DEFAULT_CAPTURE_SHORTCUT,
      requestId: 12,
      mode: "jp-word-candidates",
      sourceText: "微妙",
      phase: "requesting",
      capture: readyCapture(),
      partial: {
        ...emptyPartialResultForTest(),
        query: "微妙",
      },
    });

    expect(root.querySelector(".headword")?.textContent).toBe("微妙");
    expect(root.querySelector(".candidate-row.skeleton-row")).toBeInstanceOf(
      HTMLElement,
    );
    expect(root.querySelector(".candidate-row.content-reveal")).toBeNull();
  });

  it("renders LexiResultV1 meaning content without old bottom actions", () => {
    const root = renderPopup(readyState());

    expect(root.textContent).toContain("subtle");
    expect(root.textContent).toContain("delicate");
    expect(root.textContent).toContain("a subtle hint");
    expect(root.textContent).toContain("She noticed a subtle change");
    expect(root.textContent).toContain(
      "Used for something understated and easy to miss.",
    );
    expect(root.querySelector(".example-target")?.textContent).toBe("subtle");
    expect(root.querySelector(".example-target")?.textContent).not.toBe(
      "She noticed a subtle change in his voice.",
    );
    expect(root.querySelector(".error-actions")).toBeNull();
  });

  it("renders text translation results with the simple translation layout", () => {
    const root = renderPopup(readyState(mockTextTranslationResult()));

    expect(root.querySelector(".headword")?.textContent).toBe("");
    expect(
      (root.querySelector(".translation-source-text") as HTMLTextAreaElement)
        ?.value,
    ).toBe(
      "This is a test.",
    );
    expect(root.querySelector(".translation-arrow")?.textContent).toContain("↓");
    expect(
      (root.querySelector(".translated-text") as HTMLTextAreaElement)?.value,
    ).toContain(
      "This is a translated test.",
    );
    expect(root.querySelectorAll("textarea.translation-field")).toHaveLength(2);
    expect(root.textContent).not.toContain("Source");
    expect(root.textContent).not.toContain("Translation");
    expect(root.textContent).not.toContain("Similar words");
    expect(root.textContent).not.toContain("Idioms");
  });

  it("renders text translation pending state without dictionary fields", () => {
    const root = renderPopup({
      kind: "streaming",
      shortcut: DEFAULT_CAPTURE_SHORTCUT,
      requestId: 7,
      mode: "text-translation",
      sourceText: "This is pending.",
      phase: "requesting",
      capture: readyCapture(),
      partial: emptyPartialResultForTest(),
    });

    expect(root.querySelector(".headword")?.textContent).toBe("");
    expect(root.querySelector(".text-translation-layout")).toBeInstanceOf(
      HTMLElement,
    );
    expect(
      (root.querySelector(".translation-source-text") as HTMLTextAreaElement)
        ?.value,
    ).toBe("This is pending.");
    expect(root.querySelector(".translated-text")).toBeNull();
    expect(root.querySelector(".translation-field-skeleton.translated")).toBeInstanceOf(
      HTMLElement,
    );
    expect(root.textContent).not.toContain("Translations");
    expect(root.textContent).not.toContain("Similar words");
  });

  it("renders irregular inflections under the headword", () => {
    const result = mockResult();
    result.headword = "go";
    result.inflections = [
      { kind: "past", form: "went" },
      { kind: "pastParticiple", form: "gone" },
    ];

    const root = renderPopup(readyState(result));

    expect(root.querySelector(".inflection-line")?.textContent).toBe(
      "gowentgone",
    );
    expect(root.querySelectorAll(".verb-flow-icon").length).toBe(2);
  });

  it("renders irregular noun plurals without hyphenating the headword", () => {
    const result = mockResult();
    result.headword = "child";
    result.inflections = [{ kind: "plural", form: "children" }];

    const root = renderPopup(readyState(result));

    expect(root.querySelector(".inflection-line")?.textContent).toBe("children");
    expect(root.querySelector(".plural-icon")).toBeInstanceOf(HTMLElement);
  });

  it("renders inflection translation sense with base word label", () => {
    const result = mockResult();
    result.headword = "saw";
    result.translations = [
      {
        text: "のこぎり",
        note: "名詞",
        example: {
          sentence: "He used a saw.",
          japanese: "彼はのこぎりを使った。",
        },
        senseKind: "dictionary",
      },
      {
        text: "見た",
        note: "動詞",
        example: {
          sentence: "I saw him yesterday.",
          japanese: "私は昨日彼に会った。",
        },
        senseKind: "inflection",
        baseWord: "see",
      },
    ];

    const root = renderPopup(readyState(result));

    expect(root.querySelector(".inflection-sense-label")?.textContent).toBe(
      "see の活用",
    );
  });

  it("hides optional sections instead of showing skeletons when ready data is empty", () => {
    const result = mockResult();
    result.synonyms = [];
    result.idioms = [];

    const root = renderPopup(readyState(result));

    expect(root.textContent).not.toContain("Similar words");
    expect(root.textContent).not.toContain("Idioms");
    expect(root.querySelector(".skeleton-block")).toBeNull();
  });

  it("does not clip an example when the headword is a stem inside an inflected word", () => {
    const result = mockResult();
    result.headword = "operate";
    result.translations[0].example = {
      sentence: "She operates a small business.",
      japanese: "She operates a small business.",
    };

    const root = renderPopup(readyState(result));

    expect(root.querySelector(".example-en")?.textContent).toBe(
      "She operates a small business.",
    );
  });

  it("renders related words as expandable rows with usage details but no per-entry nuance", () => {
    const result = mockResult() as LexiResultV1 & {
      synonyms: Array<LexiResultV1["synonyms"][number] & { nuance?: string }>;
    };
    result.synonyms[0].nuance = "SHOULD_NOT_RENDER";

    const root = renderPopup(readyState(result), "related");

    expect(root.querySelectorAll(".synonym-row").length).toBeGreaterThan(0);
    expect(
      root.querySelector(".synonym-trigger")?.getAttribute("aria-expanded"),
    ).toBe("false");
    expect(root.textContent).toContain("slight");
    expect(root.textContent).toContain("Choose subtle for understated meaning");
    expect(root.textContent).not.toContain("SHOULD_NOT_RENDER");
  });

  it("renders partial streaming content before final validation", () => {
    const state: PopupState = {
      kind: "streaming",
      shortcut: DEFAULT_CAPTURE_SHORTCUT,
      requestId: 7,
      mode: "word-study",
      sourceText: null,
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
        inflections: [],
        translations: [
          {
            text: "delicate",
            note: TRANSLATION_NOTE_VALUES[2],
            example: {
              sentence: "The room had a subtle scent.",
              japanese: "She operates a small business.",
            },
          },
        ],
        nuance: "Understated rather than obvious.",
        synonyms: [],
        idioms: [],
        warnings: [],
      },
    };

    const root = renderPopup(state);

    expect(root.textContent).toContain("subtle");
    expect(root.textContent).toContain("delicate");
    expect(root.textContent).toContain("The room had a subtle scent.");
    expect(root.textContent).toContain("Understated rather than obvious.");
    expect(root.querySelector(".nuance.content-reveal")).toBeInstanceOf(
      HTMLElement,
    );
    expect(root.querySelector(".translation-row.content-reveal")).toBeInstanceOf(
      HTMLElement,
    );
    expect(root.querySelector(".skeleton-block")).toBeNull();
    expect(root.textContent).not.toContain("Similar words");
    expect(root.textContent).not.toContain("Idioms");
    expect(root.textContent).not.toContain("生成中");
  });

  it("keeps the result body mounted when streaming becomes ready", async () => {
    const streamingState: PopupState = {
      kind: "streaming",
      shortcut: DEFAULT_CAPTURE_SHORTCUT,
      requestId: 7,
      mode: "word-study",
      sourceText: null,
      phase: "streaming",
      capture: readyCapture(),
      partial: {
        headword: "subtle",
        inflections: mockResult().inflections,
        translations: mockResult().translations,
        nuance: mockResult().nuance,
        synonyms: mockResult().synonyms,
        idioms: mockResult().idioms,
        warnings: [],
      },
    };
    const [state, setState] = createSignal<PopupState>(streamingState);
    const root = document.createElement("div");
    render(
      () => (
        <PopupView
          state={state()}
          providerSettings={null}
          activeResultTab="meaning"
          themeMode="light"
          onClose={noop}
          onRetry={noop}
          onSetResultTab={noop}
        />
      ),
      root,
    );

    const bodyBefore = root.querySelector(".dictionary-layout");
    setState(readyState());
    await Promise.resolve();

    expect(root.querySelector(".dictionary-layout")).toBe(bodyBefore);
  });

  it("updates similar words when a streaming partial replaces same-index content", async () => {
    const first: PopupState = {
      kind: "streaming",
      shortcut: DEFAULT_CAPTURE_SHORTCUT,
      requestId: 7,
      mode: "word-study",
      sourceText: null,
      phase: "streaming",
      capture: readyCapture(),
      partial: {
        headword: "subtle",
        inflections: [],
        translations: [],
        nuance: null,
        synonyms: [
          {
            term: "delicate",
            japanese: "She operates a small business.",
            usageComparison: "first comparison",
          },
        ],
        idioms: [],
        warnings: [],
      },
    };
    const [state, setState] = createSignal<PopupState>(first);
    const root = document.createElement("div");
    render(
      () => (
        <PopupView
          state={state()}
          providerSettings={null}
          activeResultTab="meaning"
          themeMode="light"
          onClose={noop}
          onRetry={noop}
          onSetResultTab={noop}
        />
      ),
      root,
    );

    setState({
      ...first,
      partial: {
        ...first.partial,
        synonyms: [
          {
            term: "slight",
            japanese: "She operates a small business.",
            usageComparison: "second comparison",
          },
        ],
      },
    });
    await Promise.resolve();

    expect(root.textContent).toContain("slight");
    expect(root.textContent).toContain("second comparison");
    expect(root.textContent).not.toContain("delicate");
  });

  it("hides retry for non-retryable errors", () => {
    const state: PopupState = {
      kind: "error",
      shortcut: DEFAULT_CAPTURE_SHORTCUT,
      error: {
        code: "SelectionEmpty",
        userMessage: "Select text before running Lexi.",
        diagnosticMessage: "No selected text was available from the active control.",
        retryable: false,
      },
      context: {
        selectionErrorCode: "SelectionEmpty",
        captureMethod: null,
        sourceProcess: null,
        sourceWindowTitle: null,
        retryCapture: null,
      },
    };

    const root = renderPopup(state);

    expect(root.textContent).toContain("Select text before running Lexi.");
    expect(root.querySelector(".error-actions button")?.textContent).toBe("Close");
  });

  it("renders streaming dictionary content during validating phase", () => {
    const state: PopupState = {
      kind: "streaming",
      shortcut: DEFAULT_CAPTURE_SHORTCUT,
      requestId: 9,
      mode: "word-study",
      sourceText: null,
      phase: "validating",
      capture: readyCapture(),
      partial: {
        headword: "subtle",
        inflections: [],
        translations: [
          {
            text: "delicate",
            note: TRANSLATION_NOTE_VALUES[2],
            example: {
              sentence: "A subtle scent.",
              japanese: "かすかな香り。",
            },
          },
        ],
        nuance: "Understated rather than obvious.",
        synonyms: [],
        idioms: [],
        warnings: [],
      },
    };

    const root = renderPopup(state);

    expect(root.textContent).toContain("subtle");
    expect(root.textContent).toContain("delicate");
    expect(root.querySelector(".dictionary-layout.streaming")).toBeInstanceOf(
      HTMLElement,
    );
  });

  it("renders user-safe errors and diagnostics", () => {
    const state: PopupState = {
      kind: "error",
      shortcut: DEFAULT_CAPTURE_SHORTCUT,
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

    expect(root.textContent).toContain(
      "This app does not expose selected text to Lexi.",
    );
    expect(root.textContent).toContain("SelectionUnsupported");
    expect(root.textContent).toContain("uia-foreground-window");
    expect(root.textContent).toContain("example.exe");
    expect(root.textContent).toContain(
      "The active control does not support a selected-text UI Automation pattern.",
    );
  });

  it("renders the settings window", () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      provider: "gemini",
      models: [
        { id: "gemini-2.5-flash-lite", label: "Gemini 2.5 Flash-Lite" },
      ],
      fetched: false,
      warning: "API key is not configured; showing default models.",
    });
    const onSave = vi.fn(async () => undefined);
    const onToggleTheme = vi.fn();
    const onSetBackgroundOpacity = vi.fn();
    const root = renderSettingsView(
      { backgroundOpacity: 0.3, apiKeyConfigured: false },
      {
        backgroundOpacity: 0.3,
        onSave,
        onToggleTheme,
        onSetBackgroundOpacity,
      },
    );

    expect(root.textContent).toContain("Word provider");
    expect(root.textContent).toContain("Capture shortcut");
    expect(root.textContent).toContain("Close shortcut");
    expect(root.textContent).toContain("Pronunciation shortcut");
    expect(root.textContent).toContain("Theme");
    expect(root.textContent).toContain("Background opacity");
    expect(root.textContent).toContain("30%");
    expect(root.textContent).toContain("Dark");
    expect(root.textContent).toContain("Gemini");
    expect(root.textContent).toContain("DeepL APIキー");
    expect(
      Array.from(root.querySelectorAll("option")).some(
        (option) => option.value === "deep-l",
      ),
    ).toBe(false);
    expect(root.querySelector(".settings-save")?.hasAttribute("disabled")).toBe(
      true,
    );
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
    const themeButton = root.querySelector(".settings-theme-toggle");
    expect(themeButton).toBeInstanceOf(HTMLButtonElement);
    themeButton!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    expect(onToggleTheme).toHaveBeenCalledOnce();
    const opacityInput = root.querySelector(
      'input[type="range"]',
    ) as HTMLInputElement;
    expect(opacityInput).toBeInstanceOf(HTMLInputElement);
    opacityInput.value = "0.45";
    opacityInput.dispatchEvent(new Event("input", { bubbles: true }));
    expect(onSetBackgroundOpacity).toHaveBeenCalledWith(0.45);
    const shortcutButton = root.querySelector(".shortcut-recorder");
    expect(shortcutButton).toBeInstanceOf(HTMLButtonElement);
    expect(shortcutButton!.textContent).toBe(DEFAULT_CAPTURE_SHORTCUT);

    root.remove();
  });

  it("saves a DeepL key separately from the word provider key", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      provider: "gemini",
      models: [{ id: "gemini-2.5-flash-lite", label: "Gemini 2.5 Flash-Lite" }],
      fetched: true,
      warning: null,
    });
    const onSave = vi.fn(async () => undefined);
    const root = renderSettingsView(
      { apiKeyConfigured: true, deeplApiKeyConfigured: false },
      { onSave },
    );

    const deeplInput = Array.from(root.querySelectorAll("input")).find(
      (input) => input.placeholder === "DeepL APIキー",
    );
    expect(deeplInput).toBeInstanceOf(HTMLInputElement);
    deeplInput!.value = "deepl-secret";
    deeplInput!.dispatchEvent(new Event("input", { bubbles: true }));

    root
      .querySelector("form")
      ?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await Promise.resolve();

    expect(onSave).toHaveBeenCalledWith({
      shortcut: DEFAULT_CAPTURE_SHORTCUT,
      pronunciationShortcut: DEFAULT_PRONUNCIATION_SHORTCUT,
      closeShortcut: "Escape",
      backgroundOpacity: 0.94,
      theme: "light",
      provider: "gemini",
      model: "gemini-2.5-flash-lite",
      resultLanguage: "ja",
      promptMode: "word-study",
      apiKey: null,
      deeplApiKey: "deepl-secret",
    });

    root.remove();
  });

  it("records shortcut changes from a key chord", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      provider: "gemini",
      models: [{ id: "gemini-2.5-flash-lite", label: "Gemini 2.5 Flash-Lite" }],
      fetched: true,
      warning: null,
    });
    const onSave = vi.fn(async () => undefined);
    const root = renderSettingsView(
      { apiKeyConfigured: true, deeplApiKeyConfigured: false },
      { onSave },
    );

    const shortcutButton = root.querySelector(".shortcut-recorder");
    expect(shortcutButton).toBeInstanceOf(HTMLButtonElement);
    shortcutButton!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    shortcutButton!.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "Control",
        ctrlKey: true,
        bubbles: true,
      }),
    );
    expect(shortcutButton!.textContent).toBe("Ctrl+...");
    shortcutButton!.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "(",
        ctrlKey: true,
        shiftKey: true,
        bubbles: true,
      }),
    );
    expect(shortcutButton!.textContent).toBe("Ctrl+Shift+(");
    root
      .querySelector("form")
      ?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await Promise.resolve();

    expect(onSave).toHaveBeenCalledWith({
      shortcut: "Ctrl+Shift+(",
      closeShortcut: DEFAULT_CLOSE_SHORTCUT,
      pronunciationShortcut: DEFAULT_PRONUNCIATION_SHORTCUT,
      backgroundOpacity: 0.94,
      theme: "light",
      provider: "gemini",
      model: "gemini-2.5-flash-lite",
      resultLanguage: "ja",
      promptMode: "word-study",
      apiKey: null,
      deeplApiKey: null,
    });

    root.remove();
  });

  it("records close shortcut changes without requiring a modifier", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      provider: "gemini",
      models: [{ id: "gemini-2.5-flash-lite", label: "Gemini 2.5 Flash-Lite" }],
      fetched: true,
      warning: null,
    });
    const onSave = vi.fn(async () => undefined);
    const root = renderSettingsView(
      {
        closeShortcut: "Escape",
        apiKeyConfigured: true,
        deeplApiKeyConfigured: false,
      },
      { onSave },
    );

    const shortcutButtons = root.querySelectorAll(".shortcut-recorder");
    const closeShortcutButton = shortcutButtons[1];
    expect(closeShortcutButton).toBeInstanceOf(HTMLButtonElement);
    closeShortcutButton.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    closeShortcutButton.dispatchEvent(
      new KeyboardEvent("keydown", { key: "F9", bubbles: true }),
    );
    expect(closeShortcutButton.textContent).toBe("F9");
    root
      .querySelector("form")
      ?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await Promise.resolve();

    expect(onSave).toHaveBeenCalledWith({
      shortcut: DEFAULT_CAPTURE_SHORTCUT,
      pronunciationShortcut: DEFAULT_PRONUNCIATION_SHORTCUT,
      closeShortcut: "F9",
      backgroundOpacity: 0.94,
      theme: "light",
      provider: "gemini",
      model: "gemini-2.5-flash-lite",
      resultLanguage: "ja",
      promptMode: "word-study",
      apiKey: null,
      deeplApiKey: null,
    });

    root.remove();
  });

  it("saves background opacity with provider settings", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      provider: "gemini",
      models: [{ id: "gemini-2.5-flash-lite", label: "Gemini 2.5 Flash-Lite" }],
      fetched: true,
      warning: null,
    });
    const [backgroundOpacity, setBackgroundOpacity] = createSignal(0.94);
    const onSave = vi.fn(async () => undefined);
    const root = renderSettingsView(
      { apiKeyConfigured: true, deeplApiKeyConfigured: false },
      {
        backgroundOpacity: () => backgroundOpacity(),
        onSetBackgroundOpacity: setBackgroundOpacity,
        onSave,
      },
    );

    const opacityInput = root.querySelector(
      'input[type="range"]',
    ) as HTMLInputElement;
    opacityInput.value = "0.6";
    opacityInput.dispatchEvent(new Event("input", { bubbles: true }));
    await Promise.resolve();

    root
      .querySelector("form")
      ?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await Promise.resolve();

    expect(onSave).toHaveBeenCalledWith({
      shortcut: DEFAULT_CAPTURE_SHORTCUT,
      pronunciationShortcut: DEFAULT_PRONUNCIATION_SHORTCUT,
      closeShortcut: "Escape",
      backgroundOpacity: 0.6,
      theme: "light",
      provider: "gemini",
      model: "gemini-2.5-flash-lite",
      resultLanguage: "ja",
      promptMode: "word-study",
      apiKey: null,
      deeplApiKey: null,
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
    const onSave = vi.fn(async () => undefined);
    const root = renderSettingsView(
      { apiKeyConfigured: true, deeplApiKeyConfigured: false },
      { onSave },
    );

    await Promise.resolve();
    const selects = Array.from(root.querySelectorAll("select"));
    const modelSelect = selects.find((select) =>
      Array.from(select.options).some(
        (option) => option.value === "gemini-2.5-flash",
      ),
    );
    expect(modelSelect).toBeInstanceOf(HTMLSelectElement);

    modelSelect!.value = "gemini-2.5-flash";
    modelSelect!.dispatchEvent(new Event("change", { bubbles: true }));
    expect(root.querySelector(".settings-save")?.hasAttribute("disabled")).toBe(
      false,
    );
    root
      .querySelector("form")
      ?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await Promise.resolve();

    expect(onSave).toHaveBeenCalledWith({
      shortcut: DEFAULT_CAPTURE_SHORTCUT,
      pronunciationShortcut: DEFAULT_PRONUNCIATION_SHORTCUT,
      closeShortcut: "Escape",
      backgroundOpacity: 0.94,
      theme: "light",
      provider: "gemini",
      model: "gemini-2.5-flash",
      resultLanguage: "ja",
      promptMode: "word-study",
      apiKey: null,
      deeplApiKey: null,
    });

    root.remove();
  });
});

describe("Lexi result schema", () => {
  it("accepts japanese word candidate results", () => {
    expect(validateLexiResultV1(mockJapaneseWordCandidatesResult()).ok).toBe(true);
  });

  it("accepts text translation results", () => {
    expect(validateLexiResultV1(mockTextTranslationResult()).ok).toBe(true);
  });

  it("rejects translation notes that are not part-of-speech labels", () => {
    const result = mockResult();
    result.translations[0].note = "math" as never;

    expect(validateLexiResultV1(result)).toEqual({
      ok: false,
      reason: "translations must be a non-empty array with part-of-speech notes",
    });
  });

  it("allows empty synonyms", () => {
    const result = mockResult();
    result.synonyms = [];

    expect(validateLexiResultV1(result).ok).toBe(true);
  });

  it("allows empty idioms", () => {
    const result = mockResult();
    result.idioms = [];

    expect(validateLexiResultV1(result).ok).toBe(true);
  });

  it("allows irregular inflections", () => {
    const result = mockResult();
    result.inflections = [
      { kind: "plural", form: "children" },
      { kind: "past", form: "wrote" },
      { kind: "pastParticiple", form: "written" },
    ];

    expect(validateLexiResultV1(result).ok).toBe(true);
  });

  it("rejects unknown inflection kinds", () => {
    const result = mockResult();
    result.inflections = [{ kind: "comparative", form: "better" } as never];

    expect(validateLexiResultV1(result)).toEqual({
      ok: false,
      reason: "inflections must be an array of irregular forms",
    });
  });

  it("rejects idioms without examples", () => {
    const result = mockResult();
    result.idioms[0].example = "";

    expect(validateLexiResultV1(result)).toEqual({
      ok: false,
      reason: "idioms must be an array of idiom entries",
    });
  });

  it("rejects translation entries without examples", () => {
    const result = mockResult();
    result.translations[0].example = null as never;

    expect(validateLexiResultV1(result)).toEqual({
      ok: false,
      reason: "translations must be a non-empty array with part-of-speech notes",
    });
  });

  it("rejects inflection translation sense without baseWord", () => {
    const result = mockResult();
    result.translations[0].senseKind = "inflection";

    expect(validateLexiResultV1(result)).toEqual({
      ok: false,
      reason: "translations must be a non-empty array with part-of-speech notes",
    });
  });
});

describe("App stream flow", () => {
  it("does not show the normal idle popup while auth status is loading", async () => {
    vi.mocked(invoke).mockImplementation((command) => {
      if (command === "get_provider_settings") {
        return Promise.resolve({
          shortcut: DEFAULT_CAPTURE_SHORTCUT,
          pronunciationShortcut: DEFAULT_PRONUNCIATION_SHORTCUT,
          closeShortcut: DEFAULT_CLOSE_SHORTCUT,
          backgroundOpacity: 0.94,
          theme: "light",
          provider: "gemini",
          model: "gemini-2.5-flash-lite",
          resultLanguage: "ja",
          promptMode: "word-study",
          apiKeyConfigured: true,
          deeplApiKeyConfigured: false,
          supabaseAnonKeyConfigured: true,
          supabaseCallbackUrl: "http://localhost:38271/auth/callback",
        });
      }
      if (command === "get_shortcut_status") {
        return Promise.resolve({
          shortcut: DEFAULT_CAPTURE_SHORTCUT,
          registered: true,
          registrationError: null,
        });
      }
      if (command === "get_sync_auth_status") {
        return new Promise(() => undefined);
      }
      if (command === "get_sync_status") {
        return Promise.resolve(defaultSyncStatus({ signedIn: false }));
      }

      return Promise.reject(new Error(`unexpected command ${command}`));
    });

    const root = document.createElement("div");
    document.body.appendChild(root);
    render(() => <App />, root);
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();

    expect(root.textContent).toContain("Googleでログイン");
    expect(root.textContent).not.toContain("待機中");

    root.remove();
  });

  it("shows the first-run Google sign-in screen and resizes the window when signed out", async () => {
    vi.mocked(invoke).mockImplementation((command) => {
      if (command === "get_provider_settings") {
        return Promise.resolve({
          shortcut: DEFAULT_CAPTURE_SHORTCUT,
          pronunciationShortcut: DEFAULT_PRONUNCIATION_SHORTCUT,
          closeShortcut: DEFAULT_CLOSE_SHORTCUT,
          backgroundOpacity: 0.94,
          theme: "light",
          provider: "gemini",
          model: "gemini-2.5-flash-lite",
          resultLanguage: "ja",
          promptMode: "word-study",
          apiKeyConfigured: true,
          deeplApiKeyConfigured: false,
          supabaseUrl: "https://project-ref.supabase.co",
          supabaseAnonKeyConfigured: true,
          supabaseCallbackUrl: "http://localhost:38271/auth/callback",
        });
      }
      if (command === "get_shortcut_status") {
        return Promise.resolve({
          shortcut: DEFAULT_CAPTURE_SHORTCUT,
          registered: true,
          registrationError: null,
        });
      }
      if (command === "get_sync_auth_status") {
        return Promise.resolve({
          configured: true,
          signedIn: false,
          userId: null,
          userEmail: null,
          callbackUrl: "http://localhost:38271/auth/callback",
        });
      }
      if (command === "get_sync_status") {
        return Promise.resolve(defaultSyncStatus({ signedIn: false }));
      }

      return Promise.reject(new Error(`unexpected command ${command}`));
    });

    const root = document.createElement("div");
    document.body.appendChild(root);
    render(() => <App />, root);
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();

    expect(root.textContent).toContain("Googleでログイン");
    expect(root.textContent).not.toContain("Lexiにログイン");
    expect(root.textContent).not.toContain("Supabaseがアプリ側で設定されていません");
    expect(root.textContent).not.toContain("待機中");
    expect(tauriMocks.setSize).toHaveBeenCalledWith(
      expect.objectContaining({ width: 460, height: 560 }),
    );
    expect(tauriMocks.center).toHaveBeenCalled();
    expect(tauriMocks.show).toHaveBeenCalled();

    root.remove();
  });

  it("retries through the streaming command instead of the non-stream transform path", async () => {
    vi.mocked(invoke).mockImplementation((command) => {
      if (command === "get_provider_settings") {
        return Promise.resolve({
          shortcut: DEFAULT_CAPTURE_SHORTCUT,
          pronunciationShortcut: DEFAULT_PRONUNCIATION_SHORTCUT,
          closeShortcut: DEFAULT_CLOSE_SHORTCUT,
          backgroundOpacity: 0.72,
          theme: "light",
          provider: "gemini",
          model: "gemini-2.5-flash-lite",
          resultLanguage: "ja",
          promptMode: "word-study",
          apiKeyConfigured: true,
          deeplApiKeyConfigured: false,
        });
      }
      if (command === "get_shortcut_status") {
        return Promise.resolve({
          shortcut: DEFAULT_CAPTURE_SHORTCUT,
          registered: true,
          registrationError: null,
        });
      }
      if (command === "get_sync_auth_status") {
        return Promise.resolve({
          configured: true,
          signedIn: true,
          userId: "user-1",
          userEmail: "lexi@example.com",
          callbackUrl: "http://localhost:38271/auth/callback",
        });
      }
      if (command === "get_sync_status") {
        return Promise.resolve(defaultSyncStatus());
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
        selectedTextPreview: "subtle",
        shortcut: DEFAULT_CAPTURE_SHORTCUT,
        captureMethod: "uia-foreground-window",
        sourceProcess: "notepad.exe",
        sourceWindowTitle: "note.txt - Notepad",
        characterCount: 6,
        multiline: false,
        provider: "gemini",
        model: "gemini-2.5-flash-lite",
      },
    });
    expect(root.textContent).toContain("subtle");
    tauriMocks.listeners["lexi:transform"][0]({
      payload: {
        status: "failed",
        requestId: 42,
        error: {
          code: "InvalidModelOutput",
          userMessage: "The result could not be displayed.",
          diagnosticMessage: "provider stream completed without JSON content",
          retryable: true,
        },
      },
    });

    const retryButton = root.querySelector(".error-actions button");
    expect(retryButton).toBeInstanceOf(HTMLButtonElement);
    retryButton!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    await Promise.resolve();

    expect(invoke).toHaveBeenCalledWith("run_transform_stream", {
      capture: {
        shortcut: DEFAULT_CAPTURE_SHORTCUT,
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

  it("ignores transform events for stale request ids", async () => {
    vi.mocked(invoke).mockImplementation((command) => {
      if (command === "get_provider_settings") {
        return Promise.resolve({
          shortcut: DEFAULT_CAPTURE_SHORTCUT,
          pronunciationShortcut: DEFAULT_PRONUNCIATION_SHORTCUT,
          closeShortcut: DEFAULT_CLOSE_SHORTCUT,
          backgroundOpacity: 0.72,
          theme: "light",
          provider: "gemini",
          model: "gemini-2.5-flash-lite",
          resultLanguage: "ja",
          promptMode: "word-study",
          apiKeyConfigured: true,
          deeplApiKeyConfigured: false,
          supabaseAnonKeyConfigured: true,
          supabaseCallbackUrl: "http://localhost:38271/auth/callback",
        });
      }
      if (command === "get_shortcut_status") {
        return Promise.resolve({
          shortcut: DEFAULT_CAPTURE_SHORTCUT,
          registered: true,
          registrationError: null,
        });
      }
      if (command === "get_sync_auth_status") {
        return Promise.resolve({
          configured: true,
          signedIn: true,
          userId: "user-1",
          userEmail: "lexi@example.com",
          callbackUrl: "http://localhost:38271/auth/callback",
        });
      }
      if (command === "get_sync_status") {
        return Promise.resolve(defaultSyncStatus());
      }

      return Promise.reject(new Error(`unexpected command ${command}`));
    });

    const root = document.createElement("div");
    document.body.appendChild(root);
    render(() => <App />, root);
    await Promise.resolve();
    await Promise.resolve();

    const emit = tauriMocks.listeners["lexi:transform"][0];
    emit({
      payload: {
        status: "started",
        requestId: 1,
        selectedTextPreview: "first",
        shortcut: DEFAULT_CAPTURE_SHORTCUT,
        captureMethod: "uia-foreground-window",
        sourceProcess: "notepad.exe",
        sourceWindowTitle: "first.txt",
        characterCount: 5,
        multiline: false,
        provider: "gemini",
        model: "gemini-2.5-flash-lite",
      },
    });
    emit({
      payload: {
        status: "started",
        requestId: 2,
        selectedTextPreview: "second",
        shortcut: DEFAULT_CAPTURE_SHORTCUT,
        captureMethod: "uia-foreground-window",
        sourceProcess: "notepad.exe",
        sourceWindowTitle: "second.txt",
        characterCount: 6,
        multiline: false,
        provider: "gemini",
        model: "gemini-2.5-flash-lite",
      },
    });
    await Promise.resolve();

    expect(root.textContent).toContain("second");
    expect(root.textContent).not.toContain("first");

    emit({
      payload: {
        status: "ready",
        requestId: 1,
        result: mockResult(),
        provider: "gemini",
        model: "gemini-2.5-flash-lite",
      },
    });
    await Promise.resolve();

    expect(root.textContent).toContain("second");
    expect(root.textContent).not.toContain("subtle");

    root.remove();
  });

  it("shows auth callback errors on the sign-in screen", async () => {
    vi.mocked(invoke).mockImplementation((command) => {
      if (command === "get_provider_settings") {
        return Promise.resolve(defaultProviderSettings({ apiKeyConfigured: true }));
      }
      if (command === "get_shortcut_status") {
        return Promise.resolve({
          shortcut: DEFAULT_CAPTURE_SHORTCUT,
          registered: true,
          registrationError: null,
        });
      }
      if (command === "get_sync_auth_status") {
        return Promise.resolve({
          configured: true,
          signedIn: false,
          userId: null,
          userEmail: null,
          callbackUrl: "http://localhost:38271/auth/callback",
        });
      }
      if (command === "get_sync_status") {
        return Promise.resolve(defaultSyncStatus({ signedIn: false }));
      }
      return Promise.reject(new Error(`unexpected command ${command}`));
    });

    const root = document.createElement("div");
    document.body.appendChild(root);
    render(() => <App />, root);
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();

    expect(tauriMocks.listeners["lexi:sync-auth-error"]?.length).toBeGreaterThan(0);

    for (const handler of tauriMocks.listeners["lexi:sync-auth-error"] ?? []) {
      handler({
        payload: {
          code: "SyncAuthRequired",
          userMessage: "Googleログインに失敗しました。",
          diagnosticMessage: "callback rejected",
          retryable: true,
        },
      });
    }
    await Promise.resolve();

    expect(root.textContent).toContain("Googleログインに失敗しました。");

    root.remove();
  });

  it("shows sync retry controls in settings when sync fails", () => {
    const root = document.createElement("div");
    document.body.appendChild(root);
    render(
      () => (
        <SettingsView
          settings={defaultProviderSettings()}
          syncAuthStatus={{
            configured: true,
            signedIn: true,
            userId: "user-1",
            userEmail: "lexi@example.com",
            callbackUrl: "http://localhost:38271/auth/callback",
          }}
          syncStatus={defaultSyncStatus({
            lifecycle: "error",
            lastError: "Vocabulary sync failed.",
          })}
          themeMode="light"
          backgroundOpacity={0.94}
          onSave={async () => undefined}
          onRetrySync={async () => undefined}
        />
      ),
      root,
    );

    expect(root.textContent).toContain("Vocabulary sync failed.");
    expect(root.textContent).toContain("同期を再試行");

    root.remove();
  });

  it("shows sync retry controls in settings when pending mutations remain", () => {
    const root = document.createElement("div");
    document.body.appendChild(root);
    render(
      () => (
        <SettingsView
          settings={defaultProviderSettings()}
          syncAuthStatus={{
            configured: true,
            signedIn: true,
            userId: "user-1",
            userEmail: "lexi@example.com",
            callbackUrl: "http://localhost:38271/auth/callback",
          }}
          syncStatus={defaultSyncStatus({
            lifecycle: "synced",
            pendingMutations: 22,
          })}
          themeMode="light"
          backgroundOpacity={0.94}
          onSave={async () => undefined}
          onRetrySync={async () => undefined}
        />
      ),
      root,
    );

    expect(root.textContent).toContain("同期待ち: 22件");
    expect(root.textContent).toContain("同期を再試行");

    root.remove();
  });
});

