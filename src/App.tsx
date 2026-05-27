import {
  For,
  Match,
  Show,
  Switch,
  createSignal,
  createMemo,
  onCleanup,
  onMount,
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
type ThemeMode = "light" | "dark";
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
      selectedTextPreview: string;
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
  const [themeMode, setThemeMode] = createSignal<ThemeMode>("light");
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

    void invoke<ProviderSettings>("get_provider_settings").then(
      setProviderSettings,
    );

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

    if (window.matchMedia?.("(prefers-color-scheme: dark)").matches) {
      setThemeMode("dark");
    }

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
          partial: {
            ...emptyPartialResult(),
            headword: payload.selectedTextPreview,
          },
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
      themeMode={themeMode()}
      onClose={closePopup}
      onRetry={retryCurrent}
      onToggleSettings={() => setSettingsOpen((open) => !open)}
      onToggleTheme={() =>
        setThemeMode((current) => (current === "dark" ? "light" : "dark"))
      }
      onSaveSettings={async (update) => {
        const saved = await invoke<ProviderSettings>(
          "update_provider_settings",
          { update },
        );
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
  themeMode: ThemeMode;
  onClose: () => void;
  onRetry: () => void;
  onToggleSettings: () => void;
  onToggleTheme: () => void;
  onSaveSettings: (update: ProviderSettingsUpdate) => Promise<void>;
  onSetResultTab: (tab: ResultTab) => void;
}) {
  return (
    <main
      class={`popup-shell state-${props.state.kind} theme-${props.themeMode}`}
    >
      <header class="lexi-header">
        <div class="title-block">
          <p class="app-label">Lexi vocabulary note</p>
          <h1 class="headword">{headwordForState(props.state)}</h1>
        </div>
        <div class="header-actions">
          <button
            class="button"
            type="button"
            aria-label="設定"
            aria-expanded={props.settingsOpen}
            onClick={props.onToggleSettings}
          >
            Settings
          </button>
        </div>
      </header>

      <section class="lexi-body" aria-live="polite">
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
          <div
            class="settings-overlay"
            role="presentation"
            onMouseDown={props.onToggleSettings}
          >
            <SettingsPanel
              settings={settings()}
              themeMode={props.themeMode}
              onSave={props.onSaveSettings}
              onToggleTheme={props.onToggleTheme}
            />
          </div>
        )}
      </Show>
    </main>
  );
}

function EmptyState(props: { shortcut: string }) {
  return (
    <div class="center-state">
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
  themeMode: ThemeMode;
  onSave: (update: ProviderSettingsUpdate) => Promise<void>;
  onToggleTheme: () => void;
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
  const hasChanges = createMemo(
    () =>
      provider() !== props.settings.provider ||
      model().trim() !== props.settings.model ||
      resultLanguage().trim() !== props.settings.resultLanguage ||
      apiKey().trim().length > 0,
  );

  const defaultModel = (kind: ProviderKind) =>
    FALLBACK_MODEL_OPTIONS[kind][0].id;

  async function loadModels(nextProvider: ProviderKind, selectedModel: string) {
    const sequence = ++modelLoadSequence;
    setModelsLoading(true);
    setModelsWarning(null);

    try {
      const result = await invoke<ProviderModelsResult>(
        "list_provider_models",
        { provider: nextProvider },
      );
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
    if (!hasChanges()) {
      return;
    }

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
    <aside
      class="settings-panel"
      role="dialog"
      aria-modal="true"
      aria-label="設定"
      onMouseDown={(event) => event.stopPropagation()}
    >
      <form onSubmit={submit}>
        <div class="settings-header">
          <h2>Settings</h2>
        </div>

        <div class="settings-field">
          <span>Theme</span>
          <button
            class="button settings-theme-toggle"
            type="button"
            aria-label={
              props.themeMode === "dark"
                ? "Switch to light mode"
                : "Switch to dark mode"
            }
            aria-pressed={props.themeMode === "dark"}
            onClick={props.onToggleTheme}
          >
            {props.themeMode === "dark" ? "Light" : "Dark"}
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
                ? "保存済み。変更時のみ入力"
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
          API key is never returned to the frontend after save.
        </p>
        <Show when={modelsWarning()}>
          {(message) => <p class="settings-note">{message()}</p>}
        </Show>

        <Show when={error()}>
          {(message) => <p class="settings-error">{message()}</p>}
        </Show>

        <button
          class="settings-save"
          type="submit"
          disabled={saving() || !hasChanges()}
        >
          {saving() ? "Saving" : "Save"}
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
  void props.activeResultTab;
  void props.onSetResultTab;
  return <DictionaryBody result={props.state.result} />;
}

function StreamingResultView(props: {
  state: Extract<PopupState, { kind: "streaming" }>;
  activeResultTab: ResultTab;
  onSetResultTab: (tab: ResultTab) => void;
}) {
  void props.activeResultTab;
  void props.onSetResultTab;
  const phaseText = () =>
    props.state.phase === "validating" ? "検証中" : "生成中";

  return (
    <DictionaryBody
      result={props.state.partial}
      pendingLabel={phaseText()}
      streaming
    />
  );
}

function DictionaryBody(props: {
  result: ResultLike;
  pendingLabel?: string;
  streaming?: boolean;
}) {
  return (
    <div class="dictionary-layout" classList={{ streaming: props.streaming }}>
      <p class="nuance">{props.result.nuance ?? props.pendingLabel}</p>

      <section class="section" aria-labelledby="translations-title">
        <h2 class="section-title" id="translations-title">
          Translations
        </h2>
        <div class="translation-list">
          <Show
            when={props.result.translations.length > 0}
            fallback={
              <p class="streaming-line">{props.pendingLabel ?? "生成中"}</p>
            }
          >
            <For each={props.result.translations}>
              {(translation) => (
                <article class="translation-row">
                  <div>
                    <Show when={translation.note}>
                      {(note) => (
                        <span class="pos-icon" aria-label={note()}>
                          {partOfSpeechMark(note())}
                        </span>
                      )}
                    </Show>
                  </div>
                  <div>
                    <div class="translation-head">
                      <span class="translation-text">{translation.text}</span>
                    </div>
                    <p class="example">
                      <HighlightedExample
                        sentence={translation.example.sentence}
                        target={props.result.headword ?? ""}
                      />
                      <span class="example-ja">
                        {translation.example.japanese}
                      </span>
                    </p>
                  </div>
                </article>
              )}
            </For>
          </Show>
        </div>
      </section>

      <section class="section" aria-labelledby="synonyms-title">
        <h2 class="section-title" id="synonyms-title">
          Similar words
        </h2>
        <Show
          when={props.result.synonyms.length > 0}
          fallback={
            <p class="streaming-line">{props.pendingLabel ?? "生成中"}</p>
          }
        >
          <RelatedWordList words={props.result.synonyms} />
        </Show>
      </section>

      <Show when={props.result.warnings.length > 0}>
        <p class="warning-line">{props.result.warnings[0]}</p>
      </Show>
    </div>
  );
}

function HighlightedExample(props: { sentence: string; target: string }) {
  return (
    <span class="example-en">
      <For each={highlightSegments(props.sentence, props.target)}>
        {(segment) => (
          <Show
            when={segment.highlighted}
            fallback={<span>{segment.text}</span>}
          >
            <strong class="example-target">{segment.text}</strong>
          </Show>
        )}
      </For>
    </span>
  );
}

function highlightSegments(
  sentence: string,
  target: string,
): Array<{ text: string; highlighted: boolean }> {
  const needle = target.trim();
  if (needle.length === 0) {
    return [{ text: sentence, highlighted: false }];
  }

  const lowerSentence = sentence.toLocaleLowerCase();
  const lowerNeedle = needle.toLocaleLowerCase();
  const segments: Array<{ text: string; highlighted: boolean }> = [];
  let cursor = 0;

  while (cursor < sentence.length) {
    const index = lowerSentence.indexOf(lowerNeedle, cursor);
    if (index < 0) {
      segments.push({ text: sentence.slice(cursor), highlighted: false });
      break;
    }

    const end = index + needle.length;
    if (!isTargetBoundary(sentence, index, end, needle)) {
      cursor = index + 1;
      continue;
    }

    if (index > cursor) {
      segments.push({ text: sentence.slice(cursor, index), highlighted: false });
    }
    segments.push({ text: sentence.slice(index, end), highlighted: true });
    cursor = end;
  }

  return segments.length > 0
    ? segments
    : [{ text: sentence, highlighted: false }];
}

function isTargetBoundary(
  sentence: string,
  start: number,
  end: number,
  target: string,
): boolean {
  if (!/^[A-Za-z0-9]+$/.test(target)) {
    return true;
  }

  const before = start > 0 ? sentence[start - 1] : "";
  const after = end < sentence.length ? sentence[end] : "";
  return !/[A-Za-z0-9]/.test(before) && !/[A-Za-z0-9]/.test(after);
}

function partOfSpeechMark(note: string): string {
  const first = Array.from(note.trim())[0];
  return first.length > 0 ? first : note;
}

function RelatedWordList(props: {
  words: Array<{
    term: string;
    japanese: string;
    usageComparison: string;
  }>;
}) {
  return (
    <div class="related-section">
      <For each={props.words}>{(word) => <RelatedWordItem word={word} />}</For>
    </div>
  );
}

function RelatedWordItem(props: {
  word: {
    term: string;
    japanese: string;
    usageComparison: string;
  };
}) {
  const [open, setOpen] = createSignal(false);

  return (
    <article class="synonym-row" classList={{ expanded: open() }}>
      <button
        class="synonym-trigger"
        type="button"
        aria-expanded={open()}
        onClick={() => setOpen((current) => !current)}
      >
        <span class="synonym-head">
          <span class="synonym-term">{props.word.term}</span>
          <span class="synonym-ja">{props.word.japanese}</span>
        </span>
      </button>
      <div class="synonym-detail-shell" aria-hidden={!open()}>
        <p>{props.word.usageComparison}</p>
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
            Retry
          </button>
        </Show>
        <button type="button" onClick={props.onClose}>
          Close
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
      return "Capturing";
    case "requesting":
      return "Loading";
    case "streaming":
      return "Loading";
    case "ready":
      return state.result.headword;
    case "error":
      return "Error";
    case "idle":
      return "Lexi";
  }
}

function headwordForState(state: PopupState): string {
  if (state.kind === "streaming") {
    return state.partial.headword ?? "Lexi";
  }

  return titleForState(state);
}

function normalizeAppError(error: unknown): AppError {
  if (isAppError(error)) {
    return error;
  }

  return {
    code: "ProviderRequestFailed",
    userMessage: "LLM request failed.",
    diagnosticMessage:
      typeof error === "string" ? error : "unknown frontend error",
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
