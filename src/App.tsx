import {
  For,
  Match,
  Show,
  Switch,
  onCleanup,
  onMount,
  createSignal,
} from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { AppError } from "./lib/errors";
import {
  type LexiResultV1,
  type RelatedWord,
  type Translation,
} from "./lib/schema";
import "./App.css";

type ShortcutStatus = {
  shortcut: string;
  registered: boolean;
  registrationError: AppError | null;
};

type CaptureEvent =
  | { status: "capturing"; shortcut: string }
  | {
      status: "captured";
      shortcut: string;
      captureMethod: string;
      sourceProcess: string | null;
      sourceWindowTitle: string | null;
      characterCount: number;
      multiline: boolean;
    }
  | {
      status: "failed";
      shortcut: string;
      error: AppError;
      selectionErrorCode: string;
      captureMethod: string | null;
      sourceProcess: string | null;
      sourceWindowTitle: string | null;
    };

type CaptureMetadata = {
  captureMethod: string;
  sourceProcess: string | null;
  sourceWindowTitle: string | null;
  characterCount: number;
  multiline: boolean;
};

type PopupErrorContext = {
  selectionErrorCode: string;
  captureMethod: string | null;
  sourceProcess: string | null;
  sourceWindowTitle: string | null;
  retryCapture: CaptureMetadata | null;
};

type ResultTab = "meaning" | "related";

type ProviderKind = "mock" | "gemini" | "open-ai";

type ProviderSettings = {
  provider: ProviderKind;
  model: string;
  resultLanguage: string;
  promptMode: string;
  apiKeyConfigured: boolean;
};

type ProviderSettingsUpdate = {
  provider: ProviderKind;
  model: string;
  resultLanguage: string;
  promptMode: string;
  apiKey: string | null;
};

type LexiPartialResult = {
  headword: string | null;
  translations: Translation[];
  nuance: string | null;
  synonyms: RelatedWord[];
  warnings: string[];
};

type ResultLike = {
  headword?: string | null;
  translations: Translation[];
  nuance?: string | null;
  synonyms: RelatedWord[];
  warnings: string[];
};

type TransformEvent =
  | {
      status: "started";
      requestId: number;
      shortcut: string;
      captureMethod: string;
      sourceProcess: string | null;
      sourceWindowTitle: string | null;
      characterCount: number;
      multiline: boolean;
      provider: ProviderKind;
      model: string;
    }
  | { status: "streaming"; requestId: number; partial: LexiPartialResult }
  | { status: "validating"; requestId: number; partial: LexiPartialResult }
  | {
      status: "ready";
      requestId: number;
      result: LexiResultV1;
      provider: ProviderKind;
      model: string;
    }
  | { status: "failed"; requestId: number; error: AppError };

type ProviderModel = {
  id: string;
  label: string;
};

type ProviderModelsResult = {
  provider: ProviderKind;
  models: ProviderModel[];
  fetched: boolean;
  warning: string | null;
};

const FALLBACK_MODEL_OPTIONS: Record<ProviderKind, ProviderModel[]> = {
  gemini: [
    { id: "gemini-2.5-flash-lite", label: "Gemini 2.5 Flash-Lite" },
    { id: "gemini-2.5-flash", label: "Gemini 2.5 Flash" },
  ],
  "open-ai": [
    { id: "gpt-5.4-nano", label: "GPT-5.4 nano" },
    { id: "gpt-5-nano", label: "GPT-5 nano" },
    { id: "gpt-5.4-mini", label: "GPT-5.4 mini" },
  ],
  mock: [{ id: "mock-word-study", label: "Mock word-study" }],
};

const RESULT_LANGUAGE_OPTIONS = [
  { value: "ja", label: "日本語" },
  { value: "en", label: "English" },
  { value: "ko", label: "한국어" },
  { value: "zh", label: "中文" },
] as const;

export type PopupState =
  | { kind: "idle"; shortcut: string }
  | { kind: "capturing"; shortcut: string }
  | { kind: "requesting"; shortcut: string; capture: CaptureMetadata }
  | {
      kind: "streaming";
      shortcut: string;
      capture: CaptureMetadata;
      requestId: number;
      partial: LexiPartialResult;
      phase: "requesting" | "streaming" | "validating";
    }
  | {
      kind: "ready";
      shortcut: string;
      capture: CaptureMetadata;
      result: LexiResultV1;
    }
  | {
      kind: "error";
      shortcut: string;
      error: AppError;
      context: PopupErrorContext;
    };

function App() {
  const [state, setState] = createSignal<PopupState>({
    kind: "idle",
    shortcut: "Ctrl+Shift+X",
  });
  const [settingsOpen, setSettingsOpen] = createSignal(false);
  const [providerSettings, setProviderSettings] =
    createSignal<ProviderSettings | null>(null);
  const [activeResultTab, setActiveResultTab] =
    createSignal<ResultTab>("meaning");
  let activeRequestId: number | null = null;

  async function startTransform(shortcut: string, capture: CaptureMetadata) {
    setSettingsOpen(false);
    setActiveResultTab("meaning");
    setState({ kind: "requesting", shortcut, capture });

    try {
      await invoke("run_transform_stream", {
        capture: {
          shortcut,
          captureMethod: capture.captureMethod,
          sourceProcess: capture.sourceProcess,
          sourceWindowTitle: capture.sourceWindowTitle,
          characterCount: capture.characterCount,
          multiline: capture.multiline,
        },
      });
    } catch (error) {
      setState({
        kind: "error",
        shortcut,
        error: normalizeAppError(error),
        context: {
          selectionErrorCode: "TransformFailed",
          captureMethod: capture.captureMethod,
          sourceProcess: capture.sourceProcess,
          sourceWindowTitle: capture.sourceWindowTitle,
          retryCapture: capture,
        },
      });
    }
  }

  async function closePopup() {
    await getCurrentWindow().hide();
  }

  function retryCurrent() {
    const current = state();

    if (current.kind === "ready" || current.kind === "requesting") {
      void startTransform(current.shortcut, current.capture);
      return;
    }

    if (current.kind === "error") {
      if (current.context.retryCapture) {
        void startTransform(current.shortcut, current.context.retryCapture);
        return;
      }

      if (current.error.retryable) {
        setState({ kind: "idle", shortcut: current.shortcut });
      }
    }
  }

  onMount(() => {
    let cleanupCapture: (() => void) | undefined;
    let cleanupTransform: (() => void) | undefined;

    void invoke<ProviderSettings>("get_provider_settings").then(setProviderSettings);

    void invoke<ShortcutStatus>("get_shortcut_status").then((status) => {
      if (status.registrationError) {
        setState({
          kind: "error",
          shortcut: status.shortcut,
          error: status.registrationError,
          context: {
            selectionErrorCode: "ShortcutRegistrationFailed",
            captureMethod: null,
            sourceProcess: null,
            sourceWindowTitle: null,
            retryCapture: null,
          },
        });
        return;
      }

      setState({ kind: "idle", shortcut: status.shortcut });
    });

    void listen<CaptureEvent>("lexi:capture", (event) => {
      const payload = event.payload;

      if (payload.status === "capturing") {
        setSettingsOpen(false);
        setState({ kind: "capturing", shortcut: payload.shortcut });
        return;
      }

      if (payload.status === "captured") {
        setState({
          kind: "requesting",
          shortcut: payload.shortcut,
          capture: {
            captureMethod: payload.captureMethod,
            sourceProcess: payload.sourceProcess,
            sourceWindowTitle: payload.sourceWindowTitle,
            characterCount: payload.characterCount,
            multiline: payload.multiline,
          },
        });
        return;
      }

      activeRequestId = null;
      setSettingsOpen(false);
      setState({
        kind: "error",
        shortcut: payload.shortcut,
        error: payload.error,
        context: {
          selectionErrorCode: payload.selectionErrorCode,
          captureMethod: payload.captureMethod,
          sourceProcess: payload.sourceProcess,
          sourceWindowTitle: payload.sourceWindowTitle,
          retryCapture: null,
        },
      });
    }).then((unlisten) => {
      cleanupCapture = unlisten;
    });

    void listen<TransformEvent>("lexi:transform", (event) => {
      const payload = event.payload;

      if (payload.status === "started") {
        activeRequestId = payload.requestId;
        setSettingsOpen(false);
        setActiveResultTab("meaning");
        setState({
          kind: "streaming",
          shortcut: payload.shortcut,
          requestId: payload.requestId,
          capture: {
            captureMethod: payload.captureMethod,
            sourceProcess: payload.sourceProcess,
            sourceWindowTitle: payload.sourceWindowTitle,
            characterCount: payload.characterCount,
            multiline: payload.multiline,
          },
          partial: emptyPartialResult(),
          phase: "requesting",
        });
        return;
      }

      if (activeRequestId !== payload.requestId) {
        return;
      }

      const current = state();
      const fallbackCapture =
        current.kind === "requesting" ||
        current.kind === "streaming" ||
        current.kind === "ready"
          ? current.capture
          : null;

      if (payload.status === "streaming" || payload.status === "validating") {
        if (!fallbackCapture) {
          return;
        }
        setState({
          kind: "streaming",
          shortcut: current.shortcut,
          requestId: payload.requestId,
          capture: fallbackCapture,
          partial: payload.partial,
          phase: payload.status,
        });
        return;
      }

      if (payload.status === "ready") {
        if (!fallbackCapture) {
          return;
        }
        activeRequestId = null;
        setState({
          kind: "ready",
          shortcut: current.shortcut,
          capture: fallbackCapture,
          result: payload.result,
        });
        return;
      }

      if (payload.status === "failed") {
        activeRequestId = null;
        const capture =
          current.kind === "requesting" || current.kind === "streaming"
            ? current.capture
            : null;
        setState({
          kind: "error",
          shortcut: current.shortcut,
          error: payload.error,
          context: {
            selectionErrorCode: payload.error.code,
            captureMethod: capture?.captureMethod ?? null,
            sourceProcess: capture?.sourceProcess ?? null,
            sourceWindowTitle: capture?.sourceWindowTitle ?? null,
            retryCapture: capture,
          },
        });
      }
    }).then((unlisten) => {
      cleanupTransform = unlisten;
    });

    const keyHandler = (event: KeyboardEvent) => {
      if (event.defaultPrevented) {
        return;
      }

      if (event.key === "Escape") {
        event.preventDefault();
        void closePopup();
        return;
      }

      const current = state();
      if (
        event.key === "Enter" &&
        current.kind === "error" &&
        current.error.retryable
      ) {
        event.preventDefault();
        retryCurrent();
      }
    };

    window.addEventListener("keydown", keyHandler);

    onCleanup(() => {
      cleanupCapture?.();
      cleanupTransform?.();
      window.removeEventListener("keydown", keyHandler);
    });
  });

  return (
    <PopupView
      state={state()}
      settingsOpen={settingsOpen()}
      providerSettings={providerSettings()}
      activeResultTab={activeResultTab()}
      onClose={closePopup}
      onRetry={retryCurrent}
      onToggleSettings={() => setSettingsOpen((open) => !open)}
      onSaveSettings={async (update) => {
        const saved = await invoke<ProviderSettings>("update_provider_settings", {
          update,
        });
        setProviderSettings(saved);
        setSettingsOpen(false);
      }}
      onSetResultTab={setActiveResultTab}
    />
  );
}

export function PopupView(props: {
  state: PopupState;
  settingsOpen: boolean;
  providerSettings: ProviderSettings | null;
  activeResultTab: ResultTab;
  onClose: () => void;
  onRetry: () => void;
  onToggleSettings: () => void;
  onSaveSettings: (update: ProviderSettingsUpdate) => Promise<void>;
  onSetResultTab: (tab: ResultTab) => void;
}) {
  return (
    <main class={`popup-shell state-${props.state.kind}`}>
      <header class="popup-header">
        <div class="brand-lockup">
          <span class="brand-mark" aria-hidden="true">
            L
          </span>
          <div class="title-block">
            <p class="eyebrow">Lexi</p>
            <h1>{titleForState(props.state)}</h1>
          </div>
        </div>
        <div class="header-actions">
          <button
            class="icon-button"
            type="button"
            aria-label="設定"
            aria-expanded={props.settingsOpen}
            onClick={props.onToggleSettings}
            title="設定"
          >
            ⚙
          </button>
        </div>
      </header>

      <section class="popup-body" aria-live="polite">
        <Switch>
          <Match when={props.state.kind === "idle"}>
            <EmptyState shortcut={props.state.shortcut} />
          </Match>

          <Match when={props.state.kind === "capturing"}>
            <ProgressState
              title="選択テキストを確認中"
              detail="取得できた内容は画面に表示しません。"
            />
          </Match>

          <Match when={requestingState(props.state)}>
            {(requesting) => (
              <ProgressState
                title="結果を組み立て中"
                detail={`${requesting().capture.characterCount} 文字を取得しました。`}
              />
            )}
          </Match>

          <Match when={streamingState(props.state)}>
            {(streaming) => (
              <StreamingResultView
                state={streaming()}
                activeResultTab={props.activeResultTab}
                onSetResultTab={props.onSetResultTab}
              />
            )}
          </Match>

          <Match when={readyState(props.state)}>
            {(ready) => (
              <ResultView
                state={ready()}
                activeResultTab={props.activeResultTab}
                onSetResultTab={props.onSetResultTab}
              />
            )}
          </Match>

          <Match when={errorState(props.state)}>
            {(failed) => (
              <ErrorView
                state={failed()}
                onRetry={props.onRetry}
                onClose={props.onClose}
              />
            )}
          </Match>
        </Switch>
      </section>
      <Show when={props.settingsOpen && props.providerSettings}>
        {(settings) => (
          <SettingsPanel
            settings={settings()}
            onSave={props.onSaveSettings}
            onClose={props.onToggleSettings}
          />
        )}
      </Show>
    </main>
  );
}

function EmptyState(props: { shortcut: string }) {
  return (
    <div class="center-state">
      <div class="status-orbit" aria-hidden="true">
        <span />
      </div>
      <p class="status-text">待機中</p>
      <p class="support-text">{props.shortcut}</p>
    </div>
  );
}

function ProgressState(props: { title: string; detail: string }) {
  return (
    <div class="center-state">
      <div class="spinner" aria-hidden="true" />
      <p class="status-text">{props.title}</p>
      <p class="support-text">{props.detail}</p>
    </div>
  );
}

function SettingsPanel(props: {
  settings: ProviderSettings;
  onSave: (update: ProviderSettingsUpdate) => Promise<void>;
  onClose: () => void;
}) {
  const [provider, setProvider] = createSignal<ProviderKind>(
    props.settings.provider,
  );
  const [model, setModel] = createSignal(props.settings.model);
  const [resultLanguage, setResultLanguage] = createSignal(
    props.settings.resultLanguage,
  );
  const [apiKey, setApiKey] = createSignal("");
  const [models, setModels] = createSignal<ProviderModel[]>(
    ensureSelectedModel(
      FALLBACK_MODEL_OPTIONS[props.settings.provider],
      props.settings.model,
    ),
  );
  const [modelsLoading, setModelsLoading] = createSignal(false);
  const [modelsWarning, setModelsWarning] = createSignal<string | null>(null);
  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  let modelLoadSequence = 0;

  const defaultModel = (kind: ProviderKind) => FALLBACK_MODEL_OPTIONS[kind][0].id;

  async function loadModels(nextProvider: ProviderKind, selectedModel: string) {
    const sequence = ++modelLoadSequence;
    setModelsLoading(true);
    setModelsWarning(null);

    try {
      const result = await invoke<ProviderModelsResult>("list_provider_models", {
        provider: nextProvider,
      });
      const nextModels = ensureSelectedModel(
        result.models.length > 0
          ? result.models
          : FALLBACK_MODEL_OPTIONS[nextProvider],
        selectedModel,
      );
      if (sequence !== modelLoadSequence || provider() !== nextProvider) {
        return;
      }
      setModels(nextModels);
      setModelsWarning(result.warning);
      if (!nextModels.some((option) => option.id === model())) {
        setModel(nextModels[0].id);
      }
    } catch (caught) {
      const fallback = ensureSelectedModel(
        FALLBACK_MODEL_OPTIONS[nextProvider],
        selectedModel,
      );
      if (sequence !== modelLoadSequence || provider() !== nextProvider) {
        return;
      }
      setModels(fallback);
      setModelsWarning(normalizeAppError(caught).userMessage);
      if (!fallback.some((option) => option.id === model())) {
        setModel(fallback[0].id);
      }
    } finally {
      if (sequence === modelLoadSequence && provider() === nextProvider) {
        setModelsLoading(false);
      }
    }
  }

  onMount(() => {
    void loadModels(provider(), model());
  });

  async function submit(event: Event) {
    event.preventDefault();
    setSaving(true);
    setError(null);

    try {
      await props.onSave({
        provider: provider(),
        model: model().trim(),
        resultLanguage: resultLanguage().trim(),
        promptMode: "word-study",
        apiKey: apiKey().trim().length > 0 ? apiKey().trim() : null,
      });
    } catch (caught) {
      setError(normalizeAppError(caught).userMessage);
    } finally {
      setSaving(false);
    }
  }

  return (
    <aside class="settings-panel" aria-label="設定">
      <form onSubmit={submit}>
        <div class="settings-header">
          <h2>設定</h2>
          <button type="button" class="icon-button" onClick={props.onClose}>
            ×
          </button>
        </div>

        <label>
          <span>Provider</span>
          <select
            value={provider()}
            onChange={(event) => {
              const next = event.currentTarget.value as ProviderKind;
              const nextModel = defaultModel(next);
              setProvider(next);
              setModel(nextModel);
              void loadModels(next, nextModel);
            }}
          >
            <option value="gemini">Gemini</option>
            <option value="open-ai">OpenAI</option>
            <option value="mock">Mock</option>
          </select>
        </label>

        <label>
          <span>Model</span>
          <select
            value={model()}
            onChange={(event) => setModel(event.currentTarget.value)}
            disabled={modelsLoading()}
          >
            <For each={models()}>
              {(option) => (
                <option value={option.id} selected={option.id === model()}>
                  {option.label}
                </option>
              )}
            </For>
          </select>
        </label>

        <label>
          <span>API key</span>
          <input
            type="password"
            value={apiKey()}
            placeholder={
              props.settings.apiKeyConfigured
                ? "保存済み。変更時だけ入力"
                : "API key"
            }
            onInput={(event) => setApiKey(event.currentTarget.value)}
            autocomplete="off"
          />
        </label>

        <label>
          <span>Result language</span>
          <select
            value={resultLanguage()}
            onChange={(event) => setResultLanguage(event.currentTarget.value)}
          >
            <For each={RESULT_LANGUAGE_OPTIONS}>
              {(option) => <option value={option.value}>{option.label}</option>}
            </For>
          </select>
        </label>

        <p class="settings-note">
          API key は保存後も画面に戻しません。既存 key は空欄のまま保存すると維持されます。
        </p>
        <Show when={modelsWarning()}>
          {(message) => <p class="settings-note">{message()}</p>}
        </Show>

        <Show when={error()}>
          {(message) => <p class="settings-error">{message()}</p>}
        </Show>

        <button class="settings-save" type="submit" disabled={saving()}>
          {saving() ? "保存中" : "保存"}
        </button>
      </form>
    </aside>
  );
}

function ResultView(props: {
  state: Extract<PopupState, { kind: "ready" }>;
  activeResultTab: ResultTab;
  onSetResultTab: (tab: ResultTab) => void;
}) {
  const result = () => props.state.result;

  return (
    <div class="result-layout">
      <section class="hero-summary">
        <div class="hero-word">
          <h2>{result().headword}</h2>
        </div>
        <p class="hero-nuance">{result().nuance}</p>
      </section>

      <nav class="result-tabs" aria-label="結果表示">
        <TabButton
          active={props.activeResultTab === "meaning"}
          label="意味"
          onClick={() => props.onSetResultTab("meaning")}
        />
        <TabButton
          active={props.activeResultTab === "related"}
          label="関連語"
          onClick={() => props.onSetResultTab("related")}
        />
      </nav>

      <section class="detail-pane">
        <Switch>
          <Match when={props.activeResultTab === "meaning"}>
            <MeaningPane result={result()} />
          </Match>
          <Match when={props.activeResultTab === "related"}>
            <RelatedPane result={result()} />
          </Match>
        </Switch>
      </section>
    </div>
  );
}

function StreamingResultView(props: {
  state: Extract<PopupState, { kind: "streaming" }>;
  activeResultTab: ResultTab;
  onSetResultTab: (tab: ResultTab) => void;
}) {
  const result = () => props.state.partial;
  const phaseText = () =>
    props.state.phase === "validating" ? "検証中" : "生成中";

  return (
    <div class="result-layout streaming-layout">
      <section class="hero-summary">
        <div class="hero-word">
          <h2>{result().headword ?? "..."}</h2>
        </div>
        <p class="hero-nuance">{result().nuance ?? phaseText()}</p>
      </section>

      <nav class="result-tabs" aria-label="結果表示">
        <TabButton
          active={props.activeResultTab === "meaning"}
          label="意味"
          onClick={() => props.onSetResultTab("meaning")}
        />
        <TabButton
          active={props.activeResultTab === "related"}
          label="関連語"
          onClick={() => props.onSetResultTab("related")}
        />
      </nav>

      <section class="detail-pane">
        <Switch>
          <Match when={props.activeResultTab === "meaning"}>
            <MeaningPane result={result()} pendingLabel={phaseText()} />
          </Match>
          <Match when={props.activeResultTab === "related"}>
            <RelatedPane result={result()} pendingLabel={phaseText()} />
          </Match>
        </Switch>
      </section>
    </div>
  );
}

function TabButton(props: {
  active: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      classList={{ active: props.active }}
      aria-pressed={props.active}
      onClick={props.onClick}
    >
      {props.label}
    </button>
  );
}

function MeaningPane(props: { result: ResultLike; pendingLabel?: string }) {
  return (
    <div class="pane-grid meaning-pane">
      <section>
        <h3>意味</h3>
        <div class="meaning-list">
          <Show
            when={props.result.translations.length > 0}
            fallback={<p class="streaming-line">{props.pendingLabel ?? "生成中"}</p>}
          >
            <For each={props.result.translations}>
            {(translation) => (
              <article>
                <strong>{translation.text}</strong>
                <Show when={translation.note}>
                  {(note) => (
                    <span class="part-of-speech-mark">
                      {partOfSpeechMark(note())}
                    </span>
                  )}
                </Show>
              </article>
            )}
            </For>
          </Show>
        </div>
      </section>
      <Show when={props.result.warnings.length > 0}>
        <p class="warning-line">{props.result.warnings[0]}</p>
      </Show>
    </div>
  );
}

function partOfSpeechMark(note: string): string {
  if (note.includes("形容")) {
    return "形";
  }
  if (note.includes("副")) {
    return "副";
  }
  if (note.includes("名")) {
    return "名";
  }
  if (note.includes("動")) {
    return "動";
  }
  return note.slice(0, 1);
}

function RelatedPane(props: { result: ResultLike; pendingLabel?: string }) {
  return (
    <div class="related-single">
      <Show
        when={props.result.synonyms.length > 0}
        fallback={<p class="streaming-line">{props.pendingLabel ?? "生成中"}</p>}
      >
        <RelatedWordList title="類似語" words={props.result.synonyms} />
      </Show>
    </div>
  );
}

function RelatedWordList(props: {
  title: string;
  words: Array<{
    term: string;
    japanese: string;
    nuance: string;
    usageComparison: string;
  }>;
}) {
  return (
    <section class="related-section">
      <h3>{props.title}</h3>
      <For each={props.words}>
        {(word) => <RelatedWordItem word={word} />}
      </For>
    </section>
  );
}

function RelatedWordItem(props: {
  word: {
    term: string;
    japanese: string;
    nuance: string;
    usageComparison: string;
  };
}) {
  const [open, setOpen] = createSignal(false);

  return (
    <article class="related-word" classList={{ expanded: open() }}>
      <button
        class="related-word-trigger"
        type="button"
        aria-expanded={open()}
        onClick={() => setOpen((current) => !current)}
      >
        <span class="related-word-label">
          <strong>{props.word.term}</strong>
          <span>{props.word.japanese}</span>
        </span>
      </button>
      <div class="related-word-detail-shell" aria-hidden={!open()}>
        <div class="related-word-detail">
          <p>{props.word.nuance}</p>
          <section class="usage-item">
            <h4>使い分け</h4>
            <p>{props.word.usageComparison}</p>
          </section>
        </div>
      </div>
    </article>
  );
}

function ErrorView(props: {
  state: Extract<PopupState, { kind: "error" }>;
  onRetry: () => void;
  onClose: () => void;
}) {
  return (
    <div class="error-panel">
      <p class="error-message">{props.state.error.userMessage}</p>
      <div class="error-actions">
        <Show when={props.state.error.retryable}>
          <button type="button" onClick={props.onRetry}>
            再試行
          </button>
        </Show>
        <button type="button" onClick={props.onClose}>
          閉じる
        </button>
      </div>
      <details>
        <summary>Details</summary>
        <dl class="diagnostics">
          <div>
            <dt>Code</dt>
            <dd>{props.state.context.selectionErrorCode}</dd>
          </div>
          <div>
            <dt>App</dt>
            <dd>{props.state.context.sourceProcess ?? "Unknown"}</dd>
          </div>
          <div>
            <dt>Method</dt>
            <dd>{props.state.context.captureMethod ?? "Unknown"}</dd>
          </div>
          <div>
            <dt>Window</dt>
            <dd>{props.state.context.sourceWindowTitle ?? "Unknown"}</dd>
          </div>
        </dl>
        <p>{props.state.error.diagnosticMessage}</p>
      </details>
    </div>
  );
}

function requestingState(
  state: PopupState,
): Extract<PopupState, { kind: "requesting" }> | null {
  return state.kind === "requesting" ? state : null;
}

function streamingState(
  state: PopupState,
): Extract<PopupState, { kind: "streaming" }> | null {
  return state.kind === "streaming" ? state : null;
}

function readyState(
  state: PopupState,
): Extract<PopupState, { kind: "ready" }> | null {
  return state.kind === "ready" ? state : null;
}

function errorState(
  state: PopupState,
): Extract<PopupState, { kind: "error" }> | null {
  return state.kind === "error" ? state : null;
}

function titleForState(state: PopupState): string {
  switch (state.kind) {
    case "capturing":
      return "取得中";
    case "requesting":
      return "処理中";
    case "streaming":
      return "生成中";
    case "ready":
      return "語彙メモ";
    case "error":
      return "確認が必要";
    case "idle":
      return "待機中";
  }
}

function normalizeAppError(error: unknown): AppError {
  if (isAppError(error)) {
    return error;
  }

  return {
    code: "ProviderRequestFailed",
    userMessage: "LLM request failed.",
    diagnosticMessage: typeof error === "string" ? error : "unknown frontend error",
    retryable: true,
  };
}

function emptyPartialResult(): LexiPartialResult {
  return {
    headword: null,
    translations: [],
    nuance: null,
    synonyms: [],
    warnings: [],
  };
}

function ensureSelectedModel(
  models: ProviderModel[],
  selectedModel: string,
): ProviderModel[] {
  if (
    selectedModel.trim().length === 0 ||
    models.some((model) => model.id === selectedModel)
  ) {
    return models;
  }

  return [{ id: selectedModel, label: selectedModel }, ...models];
}

function isAppError(value: unknown): value is AppError {
  if (typeof value !== "object" || value === null) {
    return false;
  }

  const record = value as Record<string, unknown>;
  return (
    typeof record.code === "string" &&
    typeof record.userMessage === "string" &&
    typeof record.diagnosticMessage === "string" &&
    typeof record.retryable === "boolean"
  );
}

export default App;
