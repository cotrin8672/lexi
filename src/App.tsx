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
  LEXI_RESULT_V1_SCHEMA_VERSION,
  type LexiResultV1,
  validateLexiResultV1,
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

type ResultTab = "meaning" | "nuance" | "usage" | "related";
type CopyStatus = "idle" | "copied" | "failed";

export type PopupState =
  | { kind: "idle"; shortcut: string }
  | { kind: "capturing"; shortcut: string }
  | { kind: "requesting"; shortcut: string; capture: CaptureMetadata }
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

const MOCK_RESULT: LexiResultV1 = {
  schemaVersion: LEXI_RESULT_V1_SCHEMA_VERSION,
  mode: "word-study",
  sourceLanguage: "en",
  resultLanguage: "ja",
  headword: "subtle",
  translations: [
    {
      text: "微妙な",
      note: "形容詞",
    },
    {
      text: "それとなく",
      note: "副詞的",
    },
    {
      text: "繊細な",
      note: "形容詞",
    },
  ],
  nuance:
    "注意しないと見落とすほど控えめで、露骨ではない感覚があります。強く主張せず、読み取る側の観察力や文脈理解が少し必要です。",
  synonyms: [
    {
      term: "delicate",
      japanese: "繊細な",
      nuance: "細部の美しさ、壊れやすさ、扱いの慎重さに焦点があります。",
    },
    {
      term: "slight",
      japanese: "わずかな",
      nuance:
        "単に量や程度が小さいことを表します。気づきにくさは必ずしも含みません。",
    },
    {
      term: "implicit",
      japanese: "暗黙の",
      nuance: "はっきり言わないが、文脈から読み取れる意味に焦点があります。",
    },
  ],
  usageComparisons: [
    {
      terms: ["subtle", "slight"],
      explanation:
        "slight は量の小ささ。subtle は小ささに加えて、見落としやすい・読み取る必要がある感じ。",
      examples: [
        "a subtle difference = 注意しないとわからない違い",
        "a slight difference = わずかな違い",
      ],
    },
    {
      terms: ["subtle", "obvious"],
      explanation:
        "obvious は誰でもすぐわかる状態。subtle は逆に、露骨ではなく控えめに現れる状態。",
      examples: [
        "a subtle hint = それとなく出したヒント",
        "an obvious hint = 明らかなヒント",
      ],
    },
  ],
  antonyms: [
    {
      term: "obvious",
      japanese: "明らかな",
      nuance: "見ればすぐわかる、説明がほぼ不要な状態。",
    },
    {
      term: "blunt",
      japanese: "率直すぎる",
      nuance: "遠回しではなく、柔らかさや含みが少ない言い方。",
    },
  ],
  warnings: ["Phase 4 の mock 結果です。選択語の反映は Phase 5 で接続します。"],
};

function App() {
  const [state, setState] = createSignal<PopupState>({
    kind: "idle",
    shortcut: "Ctrl+Shift+X",
  });
  const [copyStatus, setCopyStatus] = createSignal<CopyStatus>("idle");
  const [settingsOpen, setSettingsOpen] = createSignal(false);
  const [activeResultTab, setActiveResultTab] =
    createSignal<ResultTab>("meaning");
  let mockRequestTimer: number | undefined;

  function clearMockRequest() {
    if (mockRequestTimer !== undefined) {
      window.clearTimeout(mockRequestTimer);
      mockRequestTimer = undefined;
    }
  }

  function startMockRequest(shortcut: string, capture: CaptureMetadata) {
    clearMockRequest();
    setCopyStatus("idle");
    setSettingsOpen(false);
    setActiveResultTab("meaning");
    setState({ kind: "requesting", shortcut, capture });

    mockRequestTimer = window.setTimeout(() => {
      const validation = validateLexiResultV1(MOCK_RESULT);

      if (!validation.ok) {
        setState({
          kind: "error",
          shortcut,
          error: {
            code: "InvalidModelOutput",
            userMessage: "結果を表示できませんでした。",
            diagnosticMessage: validation.reason,
            retryable: true,
          },
          context: {
            selectionErrorCode: "InvalidModelOutput",
            captureMethod: capture.captureMethod,
            sourceProcess: capture.sourceProcess,
            sourceWindowTitle: capture.sourceWindowTitle,
            retryCapture: capture,
          },
        });
        return;
      }

      setState({ kind: "ready", shortcut, capture, result: validation.result });
    }, 280);
  }

  async function closePopup() {
    await getCurrentWindow().hide();
  }

  function retryCurrent() {
    const current = state();

    if (current.kind === "ready" || current.kind === "requesting") {
      startMockRequest(current.shortcut, current.capture);
      return;
    }

    if (current.kind === "error") {
      if (current.context.retryCapture) {
        startMockRequest(current.shortcut, current.context.retryCapture);
        return;
      }

      if (current.error.retryable) {
        setState({ kind: "idle", shortcut: current.shortcut });
      }
    }
  }

  async function copyCurrentResult() {
    const current = state();

    if (current.kind !== "ready") {
      return;
    }

    try {
      await navigator.clipboard.writeText(
        formatResultForClipboard(current.result),
      );
      setCopyStatus("copied");
    } catch {
      setCopyStatus("failed");
    }
  }

  onMount(() => {
    let cleanup: (() => void) | undefined;

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
        clearMockRequest();
        setCopyStatus("idle");
        setSettingsOpen(false);
        setState({ kind: "capturing", shortcut: payload.shortcut });
        return;
      }

      if (payload.status === "captured") {
        startMockRequest(payload.shortcut, {
          captureMethod: payload.captureMethod,
          sourceProcess: payload.sourceProcess,
          sourceWindowTitle: payload.sourceWindowTitle,
          characterCount: payload.characterCount,
          multiline: payload.multiline,
        });
        return;
      }

      clearMockRequest();
      setCopyStatus("idle");
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
      cleanup = unlisten;
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
      cleanup?.();
      clearMockRequest();
      window.removeEventListener("keydown", keyHandler);
    });
  });

  return (
    <PopupView
      state={state()}
      copyStatus={copyStatus()}
      settingsOpen={settingsOpen()}
      activeResultTab={activeResultTab()}
      onClose={closePopup}
      onCopy={copyCurrentResult}
      onRetry={retryCurrent}
      onToggleSettings={() => setSettingsOpen((open) => !open)}
      onSetResultTab={setActiveResultTab}
    />
  );
}

export function PopupView(props: {
  state: PopupState;
  copyStatus: CopyStatus;
  settingsOpen: boolean;
  activeResultTab: ResultTab;
  onClose: () => void;
  onCopy: () => void;
  onRetry: () => void;
  onToggleSettings: () => void;
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
        <button
          class="icon-button"
          type="button"
          aria-label="閉じる"
          onClick={props.onClose}
        >
          x
        </button>
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

          <Match when={readyState(props.state)}>
            {(ready) => (
              <ResultView
                state={ready()}
                copyStatus={props.copyStatus}
                settingsOpen={props.settingsOpen}
                activeResultTab={props.activeResultTab}
                onCopy={props.onCopy}
                onRetry={props.onRetry}
                onClose={props.onClose}
                onToggleSettings={props.onToggleSettings}
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

function ResultView(props: {
  state: Extract<PopupState, { kind: "ready" }>;
  copyStatus: CopyStatus;
  settingsOpen: boolean;
  activeResultTab: ResultTab;
  onCopy: () => void;
  onRetry: () => void;
  onClose: () => void;
  onToggleSettings: () => void;
  onSetResultTab: (tab: ResultTab) => void;
}) {
  const result = () => props.state.result;

  return (
    <div class="result-layout">
      <section class="hero-summary">
        <div class="hero-word">
          <p class="field-label">word study</p>
          <h2>{result().headword}</h2>
        </div>
        <div class="translation-stack">
          <For each={result().translations.slice(0, 3)}>
            {(translation) => <span>{translation.text}</span>}
          </For>
        </div>
      </section>

      <nav class="result-tabs" aria-label="結果表示">
        <TabButton
          active={props.activeResultTab === "meaning"}
          label="意味"
          onClick={() => props.onSetResultTab("meaning")}
        />
        <TabButton
          active={props.activeResultTab === "nuance"}
          label="ニュアンス"
          onClick={() => props.onSetResultTab("nuance")}
        />
        <TabButton
          active={props.activeResultTab === "usage"}
          label="使い分け"
          onClick={() => props.onSetResultTab("usage")}
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
          <Match when={props.activeResultTab === "nuance"}>
            <NuancePane result={result()} />
          </Match>
          <Match when={props.activeResultTab === "usage"}>
            <UsagePane result={result()} />
          </Match>
          <Match when={props.activeResultTab === "related"}>
            <RelatedPane result={result()} />
          </Match>
        </Switch>
      </section>

      <footer class="action-bar">
        <button class="primary-action" type="button" onClick={props.onCopy}>
          コピー
        </button>
        <button type="button" onClick={props.onRetry}>
          再試行
        </button>
        <button
          type="button"
          aria-expanded={props.settingsOpen}
          onClick={props.onToggleSettings}
        >
          設定
        </button>
        <button type="button" onClick={props.onClose}>
          閉じる
        </button>
      </footer>

      <Show when={props.copyStatus !== "idle" || props.settingsOpen}>
        <aside class="utility-panel">
          <Show when={props.copyStatus === "copied"}>
            <p>コピーしました。</p>
          </Show>
          <Show when={props.copyStatus === "failed"}>
            <p>コピーできませんでした。</p>
          </Show>
          <Show when={props.settingsOpen}>
            <dl>
              <div>
                <dt>Shortcut</dt>
                <dd>{props.state.shortcut}</dd>
              </div>
              <div>
                <dt>Provider</dt>
                <dd>Mock</dd>
              </div>
            </dl>
          </Show>
        </aside>
      </Show>
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

function MeaningPane(props: { result: LexiResultV1 }) {
  return (
    <div class="pane-grid meaning-pane">
      <section>
        <h3>意味</h3>
        <div class="meaning-list">
          <For each={props.result.translations}>
            {(translation) => (
              <article>
                <strong>{translation.text}</strong>
                <Show when={translation.note}>
                  {(note) => <span>{note()}</span>}
                </Show>
              </article>
            )}
          </For>
        </div>
      </section>
      <Show when={props.result.warnings.length > 0}>
        <p class="warning-line">{props.result.warnings[0]}</p>
      </Show>
    </div>
  );
}

function NuancePane(props: { result: LexiResultV1 }) {
  return (
    <div class="pane-grid nuance-pane">
      <section>
        <h3>ニュアンス</h3>
        <p>{props.result.nuance}</p>
      </section>
    </div>
  );
}

function UsagePane(props: { result: LexiResultV1 }) {
  return (
    <div class="pane-grid">
      <For each={props.result.usageComparisons}>
        {(comparison) => (
          <section class="usage-item">
            <h3>{comparison.terms.join(" / ")}</h3>
            <p>{comparison.explanation}</p>
            <div class="example-row">
              <For each={comparison.examples}>
                {(example) => <span>{example}</span>}
              </For>
            </div>
          </section>
        )}
      </For>
    </div>
  );
}

function RelatedPane(props: { result: LexiResultV1 }) {
  return (
    <div class="related-columns">
      <RelatedWordList title="類似語" words={props.result.synonyms} />
      <RelatedWordList title="対義語" words={props.result.antonyms} />
    </div>
  );
}

function RelatedWordList(props: {
  title: string;
  words: Array<{ term: string; japanese: string; nuance: string }>;
}) {
  return (
    <section class="related-section">
      <h3>{props.title}</h3>
      <For each={props.words}>
        {(word) => (
          <article class="related-word">
            <div>
              <strong>{word.term}</strong>
              <span>{word.japanese}</span>
            </div>
            <p>{word.nuance}</p>
          </article>
        )}
      </For>
    </section>
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
    case "ready":
      return "語彙メモ";
    case "error":
      return "確認が必要";
    case "idle":
      return "待機中";
  }
}

function formatResultForClipboard(result: LexiResultV1): string {
  const translations = result.translations
    .map((translation) =>
      translation.note
        ? `- ${translation.text}: ${translation.note}`
        : `- ${translation.text}`,
    )
    .join("\n");
  const comparisons = result.usageComparisons
    .map(
      (comparison) =>
        `- ${comparison.terms.join(" / ")}: ${comparison.explanation}\n  ${comparison.examples.join("\n  ")}`,
    )
    .join("\n");

  return [
    result.headword,
    "",
    "訳語",
    translations,
    "",
    "ニュアンス",
    result.nuance,
    "",
    "使い分け",
    comparisons,
  ].join("\n");
}

export default App;
