import {
  For,
  Index,
  Match,
  Show,
  Switch,
  createSignal,
  createMemo,
  createEffect,
  onCleanup,
  onMount,
} from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { AppError } from "./lib/errors";
import { speakText } from "./lib/speech";
import {
  type Idiom,
  type Inflection,
  type LexiResult,
  type RelatedWord,
  type TextTranslationResultV1,
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
export type ThemeMode = "light" | "dark";
type ProviderKind = "mock" | "gemini" | "open-ai" | "deep-l";
type ResultMode = "word-study" | "text-translation";

export type ProviderSettings = {
  shortcut: string;
  closeShortcut?: string;
  pronunciationShortcut?: string;
  backgroundOpacity: number;
  theme: ThemeMode;
  provider: ProviderKind;
  model: string;
  resultLanguage: string;
  promptMode: string;
  apiKeyConfigured: boolean;
  deeplApiKeyConfigured: boolean;
  supabaseAnonKeyConfigured?: boolean;
  supabaseCallbackUrl?: string;
};

export type ProviderSettingsUpdate = {
  shortcut: string;
  closeShortcut: string;
  pronunciationShortcut: string;
  backgroundOpacity: number;
  theme: ThemeMode;
  provider: ProviderKind;
  model: string;
  resultLanguage: string;
  promptMode: string;
  apiKey: string | null;
  deeplApiKey: string | null;
  supabaseUrl?: string;
  supabaseAnonKey?: string | null;
};

export type SyncAuthStatus = {
  configured: boolean;
  signedIn: boolean;
  userId: string | null;
  userEmail: string | null;
  callbackUrl: string;
};

export type SyncLifecycle = "idle" | "syncing" | "synced" | "error";

export type SyncStatus = {
  configured: boolean;
  signedIn: boolean;
  lifecycle: SyncLifecycle;
  pendingMutations: number;
  lastServerRevision: number;
  lastSyncAt: string | null;
  lastError: string | null;
};

export type SettingsUpdatedEvent = {
  settings: ProviderSettings;
  themeMode: ThemeMode;
};

type GoogleSignInStart = {
  authUrl: string;
  callbackUrl: string;
};

type LexiPartialResult = {
  headword: string | null;
  inflections: Inflection[];
  translations: Translation[];
  nuance: string | null;
  synonyms: RelatedWord[];
  idioms: Idiom[];
  warnings: string[];
};

type ResultLike = {
  headword?: string | null;
  inflections: Inflection[];
  translations: Translation[];
  nuance?: string | null;
  synonyms: RelatedWord[];
  idioms: Idiom[];
  warnings: string[];
};

type TransformEvent =
  | {
      status: "started";
      requestId: number;
      selectedTextPreview: string;
      selectedText: string | null;
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
      result: LexiResult;
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
  "deep-l": [{ id: "deepl-translate", label: "DeepL Translate" }],
};

const RESULT_LANGUAGE_OPTIONS = [
  { value: "ja", label: "日本語" },
  { value: "en", label: "English" },
  { value: "ko", label: "한국어" },
  { value: "zh", label: "中文" },
] as const;

export const DEFAULT_CAPTURE_SHORTCUT = "Ctrl+E";
export const DEFAULT_CLOSE_SHORTCUT = "Escape";
export const DEFAULT_PRONUNCIATION_SHORTCUT = "Ctrl+Shift+P";
function syncStatusLabel(status: SyncStatus | null): string | null {
  if (!status?.signedIn) {
    return "同期はログイン後に有効です";
  }

  switch (status.lifecycle) {
    case "syncing":
      return "同期中…";
    case "error":
      return status.lastError ?? "前回の同期に失敗しました";
    case "synced":
      return status.pendingMutations > 0
        ? `同期待ち: ${status.pendingMutations}件`
        : "同期済み";
    default:
      return status.pendingMutations > 0
        ? `同期待ち: ${status.pendingMutations}件`
        : null;
  }
}
const POPUP_WINDOW_SIZE = new LogicalSize(400, 700);
const AUTH_WINDOW_SIZE = new LogicalSize(460, 560);
const POPUP_MIN_SIZE = new LogicalSize(400, 360);
const AUTH_MIN_SIZE = new LogicalSize(440, 520);

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
      mode: ResultMode;
      sourceText: string | null;
      phase: "requesting" | "streaming" | "validating";
    }
  | {
      kind: "ready";
      shortcut: string;
      capture: CaptureMetadata;
      result: LexiResult;
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
    shortcut: DEFAULT_CAPTURE_SHORTCUT,
  });
  const [providerSettings, setProviderSettings] =
    createSignal<ProviderSettings | null>(null);
  const [syncAuthStatus, setSyncAuthStatus] =
    createSignal<SyncAuthStatus | null>(null);
  const [syncStatus, setSyncStatus] = createSignal<SyncStatus | null>(null);
  const [activeResultTab, setActiveResultTab] =
    createSignal<ResultTab>("meaning");
  const [themeMode, setThemeMode] = createSignal<ThemeMode>("light");
  const [backgroundOpacity, setBackgroundOpacity] = createSignal(0.94);
  const authRequired = createMemo(
    () => syncAuthStatus() === null || !syncAuthStatus()!.signedIn,
  );
  let activeRequestId: number | null = null;
  let windowMode: "auth" | "popup" | null = null;

  async function startTransform(shortcut: string, capture: CaptureMetadata) {
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
    await invoke("hide_main_window");
  }

  function startWindowDrag(event: MouseEvent) {
    if (event.button !== 0) {
      return;
    }

    event.preventDefault();
    void getCurrentWindow()
      .startDragging()
      .catch(() => undefined);
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

  async function startGoogleSignIn() {
    const started = await invoke<GoogleSignInStart>("start_google_sign_in");
    await openUrl(started.authUrl);
  }

  async function applyWindowMode(requiredAuth: boolean) {
    const nextMode = requiredAuth ? "auth" : "popup";
    if (windowMode === nextMode) {
      return;
    }
    windowMode = nextMode;

    const window = getCurrentWindow();
    if (requiredAuth) {
      await window.setMinSize(AUTH_MIN_SIZE);
      await window.setSize(AUTH_WINDOW_SIZE);
      await window.center();
      await window.show();
      return;
    }

    await window.setMinSize(POPUP_MIN_SIZE);
    await window.setSize(POPUP_WINDOW_SIZE);
    await window.center();
  }

  onMount(() => {
    let cleanupCapture: (() => void) | undefined;
    let cleanupTransform: (() => void) | undefined;
    let cleanupSettings: (() => void) | undefined;

    void invoke<ProviderSettings>("get_provider_settings").then((settings) => {
      setProviderSettings(settings);
      setBackgroundOpacity(settings.backgroundOpacity);
      setThemeMode(settings.theme);
    });

    void invoke<SyncAuthStatus>("get_sync_auth_status").then(setSyncAuthStatus);
    void invoke<SyncStatus>("get_sync_status").then(setSyncStatus);

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
            headword:
              payload.provider === "deep-l" ? "" : payload.selectedTextPreview,
          },
          mode:
            payload.provider === "deep-l" ? "text-translation" : "word-study",
          sourceText: payload.selectedText ?? null,
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
          mode: current.kind === "streaming" ? current.mode : "word-study",
          sourceText: current.kind === "streaming" ? current.sourceText : null,
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

    void listen<SyncAuthStatus>("lexi:sync-auth", (event) => {
      setSyncAuthStatus(event.payload);
      void invoke<SyncStatus>("get_sync_status").then(setSyncStatus);
    });

    void listen<AppError>("lexi:sync-auth-error", () => {
      void invoke<SyncAuthStatus>("get_sync_auth_status").then(setSyncAuthStatus);
    });

    void listen<SyncStatus>("lexi:sync-status", (event) => {
      setSyncStatus(event.payload);
    });

    void listen<SettingsUpdatedEvent>("lexi:settings-updated", (event) => {
      setProviderSettings(event.payload.settings);
      setBackgroundOpacity(event.payload.settings.backgroundOpacity);
      setThemeMode(event.payload.themeMode);
      setState((current) => ({
        ...current,
        shortcut: event.payload.settings.shortcut,
      }));
    }).then((unlisten) => {
      cleanupSettings = unlisten;
    });

    const keyHandler = (event: KeyboardEvent) => {
      if (event.defaultPrevented) {
        return;
      }

      if (isShortcutRecorderTarget(event.target)) {
        return;
      }

      if (authRequired()) {
        return;
      }

      const closeShortcut =
        providerSettings()?.closeShortcut ?? DEFAULT_CLOSE_SHORTCUT;
      if (matchesShortcutEvent(event, closeShortcut)) {
        event.preventDefault();
        void closePopup();
        return;
      }

      const pronunciationShortcut =
        providerSettings()?.pronunciationShortcut ?? DEFAULT_PRONUNCIATION_SHORTCUT;
      const current = state();
      if (matchesShortcutEvent(event, pronunciationShortcut)) {
        const headword = speakableHeadwordForState(current);
        if (headword) {
          event.preventDefault();
          speakText(headword);
        }
        return;
      }

      if (
        event.key === "Enter" &&
        current.kind === "error" &&
        current.error.retryable
      ) {
        event.preventDefault();
        retryCurrent();
      }
    };

    window.addEventListener("keydown", keyHandler, { capture: true });

    onCleanup(() => {
      cleanupCapture?.();
      cleanupTransform?.();
      cleanupSettings?.();
      window.removeEventListener("keydown", keyHandler, { capture: true });
    });
  });

  createEffect(() => {
    void applyWindowMode(authRequired()).catch(() => undefined);
  });

  return (
    <PopupView
      state={state()}
      providerSettings={providerSettings()}
      syncAuthStatus={syncAuthStatus()}
      activeResultTab={activeResultTab()}
      themeMode={themeMode()}
      backgroundOpacity={backgroundOpacity()}
      onClose={closePopup}
      onRetry={retryCurrent}
      onStartGoogleSignIn={startGoogleSignIn}
      onSetResultTab={setActiveResultTab}
      onStartWindowDrag={startWindowDrag}
    />
  );
}

export function PopupView(props: {
  state: PopupState;
  providerSettings: ProviderSettings | null;
  syncAuthStatus?: SyncAuthStatus | null;
  activeResultTab: ResultTab;
  themeMode: ThemeMode;
  backgroundOpacity?: number;
  onClose: () => void;
  onRetry: () => void;
  onStartGoogleSignIn?: () => Promise<void>;
  onSetResultTab: (tab: ResultTab) => void;
  onStartWindowDrag?: (event: MouseEvent) => void;
}) {
  const backgroundOpacity = () => props.backgroundOpacity ?? 0.94;
  const authRequired = () =>
    props.syncAuthStatus !== undefined &&
    (props.syncAuthStatus === null || !props.syncAuthStatus.signedIn);

  return (
    <main
      class={`popup-shell state-${props.state.kind} theme-${props.themeMode}`}
      classList={{ "auth-required": authRequired() }}
      style={{ "--background-opacity": backgroundOpacity().toFixed(2) }}
    >
      <Show when={!authRequired()}>
        <header class="lexi-header">
          <div
            class="window-drag-strip"
            data-tauri-drag-region=""
            aria-hidden="true"
            onMouseDown={props.onStartWindowDrag ?? (() => undefined)}
          />
          <div class="title-block">
            <div class="headword-row">
              <h1 class="headword">{headwordForState(props.state)}</h1>
              <Show when={speakableHeadwordForState(props.state)}>
                {(headword) => (
                  <HeadwordVoiceButton
                    headword={headword()}
                    shortcutLabel={
                      props.providerSettings?.pronunciationShortcut ??
                      DEFAULT_PRONUNCIATION_SHORTCUT
                    }
                  />
                )}
              </Show>
            </div>
            <InflectionLine
              headword={headwordForState(props.state)}
              inflections={inflectionsForState(props.state)}
            />
          </div>
        </header>
      </Show>

      <Show when={authRequired()}>
        <AuthGate
          settings={props.providerSettings}
          syncAuthStatus={props.syncAuthStatus ?? null}
          onStartGoogleSignIn={props.onStartGoogleSignIn}
        />
      </Show>

      <Show
        when={
          props.syncAuthStatus === undefined || props.syncAuthStatus?.signedIn
        }
      >
        <section class="lexi-body" aria-live="polite">
          <Switch>
            <Match when={props.state.kind === "idle"}>
              <EmptyState shortcut={props.state.shortcut} />
            </Match>

            <Match when={props.state.kind === "capturing"}>
              <LoadingDictionaryView label="選択テキストを確認中" />
            </Match>

            <Match when={requestingState(props.state)}>
              {(requesting) => (
                <LoadingDictionaryView
                  capture={requesting().capture}
                  label="結果を組み立て中"
                />
              )}
            </Match>

            <Match when={resultState(props.state)}>
              {(result) => (
                <ResultDisplayView
                  state={result()}
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
      </Show>
    </main>
  );
}

function HeadwordVoiceButton(props: {
  headword: string;
  shortcutLabel: string;
}) {
  return (
    <button
      class="headword-voice-button"
      type="button"
      aria-label={`${props.headword} を発音 (${props.shortcutLabel})`}
      title={`発音 (${props.shortcutLabel})`}
      onClick={(event) => {
        event.stopPropagation();
        speakText(props.headword);
      }}
    >
      <span class="headword-voice-icon" aria-hidden="true">
        <svg viewBox="0 0 24 24" width="20" height="20">
          <path
            fill="currentColor"
            d="M3 10v4h4l5 5V5L7 10H3zm13.5 2c0-1.77-1.02-3.29-2.5-4.03v8.06c1.48-.74 2.5-2.26 2.5-4.03zM14 3.23v2.06c2.89.86 5 3.54 5 6.71s-2.11 5.85-5 6.71v2.06c4.01-.91 7-4.49 7-8.77s-2.99-7.86-7-8.77z"
          />
        </svg>
      </span>
    </button>
  );
}

function AuthGate(props: {
  settings: ProviderSettings | null;
  syncAuthStatus: SyncAuthStatus | null;
  onStartGoogleSignIn?: () => Promise<void>;
}) {
  const [signingIn, setSigningIn] = createSignal(false);
  const [authError, setAuthError] = createSignal<string | null>(null);

  onMount(() => {
    let cleanupAuth: (() => void) | undefined;
    let cleanupAuthError: (() => void) | undefined;

    void listen<SyncAuthStatus>("lexi:sync-auth", () => {
      setSigningIn(false);
      setAuthError(null);
    }).then((unlisten) => {
      cleanupAuth = unlisten;
    });

    void listen<AppError>("lexi:sync-auth-error", (event) => {
      setSigningIn(false);
      setAuthError(event.payload.userMessage);
    }).then((unlisten) => {
      cleanupAuthError = unlisten;
    });

    return () => {
      cleanupAuth?.();
      cleanupAuthError?.();
    };
  });

  async function startGoogleSignIn() {
    setSigningIn(true);
    setAuthError(null);

    try {
      await props.onStartGoogleSignIn?.();
    } catch {
      setSigningIn(false);
      setAuthError("Googleログインを開始できませんでした。");
    }
  }

  return (
    <section class="auth-gate" aria-label="Googleログイン">
      <button
        class="auth-google-button"
        type="button"
        disabled={signingIn() || !props.onStartGoogleSignIn}
        onClick={startGoogleSignIn}
      >
        <Show
          when={signingIn()}
          fallback={
            <span class="auth-google-icon" aria-hidden="true">
              <svg viewBox="0 0 24 24">
                <path
                  fill="#4285f4"
                  d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"
                />
                <path
                  fill="#34a853"
                  d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"
                />
                <path
                  fill="#fbbc05"
                  d="M5.84 14.1c-.22-.66-.35-1.36-.35-2.1s.13-1.44.35-2.1V7.06H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.94l3.66-2.84z"
                />
                <path
                  fill="#ea4335"
                  d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.06L5.84 9.9C6.71 7.3 9.14 5.38 12 5.38z"
                />
              </svg>
            </span>
          }
        >
          <span class="auth-spinner" aria-hidden="true" />
        </Show>
        {signingIn() ? "ログイン中" : "Googleでログイン"}
      </button>
      <Show when={signingIn()}>
        <p class="auth-status-note">ブラウザでログインを完了してください</p>
      </Show>
      <Show when={authError()}>
        {(message) => <p class="settings-error auth-status-note">{message()}</p>}
      </Show>
    </section>
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

export function SettingsPanel(props: {
  settings: ProviderSettings;
  syncAuthStatus?: SyncAuthStatus | null;
  syncStatus?: SyncStatus | null;
  themeMode: ThemeMode;
  backgroundOpacity: number;
  onSave: (update: ProviderSettingsUpdate) => Promise<void>;
  onSignOutSync?: () => Promise<void>;
  onRetrySync?: () => Promise<void>;
  onToggleTheme: () => void;
  onSetBackgroundOpacity: (opacity: number) => void;
}) {
  const [provider, setProvider] = createSignal<ProviderKind>(
    props.settings.provider,
  );
  const [shortcut, setShortcut] = createSignal(props.settings.shortcut);
  const [closeShortcut, setCloseShortcut] = createSignal(
    props.settings.closeShortcut ?? DEFAULT_CLOSE_SHORTCUT,
  );
  const [pronunciationShortcut, setPronunciationShortcut] = createSignal(
    props.settings.pronunciationShortcut ?? DEFAULT_PRONUNCIATION_SHORTCUT,
  );
  const [model, setModel] = createSignal(props.settings.model);
  const [resultLanguage, setResultLanguage] = createSignal(
    props.settings.resultLanguage,
  );
  const [apiKey, setApiKey] = createSignal("");
  const [deeplApiKey, setDeeplApiKey] = createSignal("");
  const [models, setModels] = createSignal<ProviderModel[]>(
    ensureSelectedModel(
      FALLBACK_MODEL_OPTIONS[props.settings.provider],
      props.settings.model,
    ),
  );
  const [modelsLoading, setModelsLoading] = createSignal(false);
  const [modelsWarning, setModelsWarning] = createSignal<string | null>(null);
  const [recordingShortcut, setRecordingShortcut] = createSignal(false);
  const [recordingShortcutPreview, setRecordingShortcutPreview] =
    createSignal("");
  const [recordingCloseShortcut, setRecordingCloseShortcut] =
    createSignal(false);
  const [recordingCloseShortcutPreview, setRecordingCloseShortcutPreview] =
    createSignal("");
  const [recordingPronunciationShortcut, setRecordingPronunciationShortcut] =
    createSignal(false);
  const [recordingPronunciationShortcutPreview, setRecordingPronunciationShortcutPreview] =
    createSignal("");
  const [saving, setSaving] = createSignal(false);
  const [signingIn, setSigningIn] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  let modelLoadSequence = 0;
  const hasChanges = createMemo(
    () =>
      provider() !== props.settings.provider ||
      shortcut().trim() !== props.settings.shortcut ||
      closeShortcut().trim() !==
        (props.settings.closeShortcut ?? DEFAULT_CLOSE_SHORTCUT) ||
      pronunciationShortcut().trim() !==
        (props.settings.pronunciationShortcut ?? DEFAULT_PRONUNCIATION_SHORTCUT) ||
      props.backgroundOpacity !== props.settings.backgroundOpacity ||
      props.themeMode !== props.settings.theme ||
      model().trim() !== props.settings.model ||
      resultLanguage().trim() !== props.settings.resultLanguage ||
      apiKey().trim().length > 0 ||
      deeplApiKey().trim().length > 0,
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
        shortcut: shortcut().trim(),
        closeShortcut: closeShortcut().trim(),
        pronunciationShortcut: pronunciationShortcut().trim(),
        backgroundOpacity: props.backgroundOpacity,
        theme: props.themeMode,
        provider: provider(),
        model: model().trim(),
        resultLanguage: resultLanguage().trim(),
        promptMode: "word-study",
        apiKey: apiKey().trim().length > 0 ? apiKey().trim() : null,
        deeplApiKey:
          deeplApiKey().trim().length > 0 ? deeplApiKey().trim() : null,
      });
    } catch (caught) {
      setError(normalizeAppError(caught).userMessage);
    } finally {
      setSaving(false);
    }
  }

  async function signOutSync() {
    setSigningIn(true);
    setError(null);

    try {
      await props.onSignOutSync?.();
    } catch (caught) {
      setError(normalizeAppError(caught).userMessage);
    } finally {
      setSigningIn(false);
    }
  }

  return (
    <aside
      class="settings-panel"
      role="dialog"
      aria-modal="true"
      aria-label="Settings"
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
          <span>Background opacity</span>
          <div class="range-field">
            <input
              type="range"
              min="0"
              max="1"
              step="0.05"
              value={props.backgroundOpacity}
              onInput={(event) =>
                props.onSetBackgroundOpacity(Number(event.currentTarget.value))
              }
            />
            <output>{Math.round(props.backgroundOpacity * 100)}%</output>
          </div>
        </label>

        <div class="settings-field">
          <span>Capture shortcut</span>
          <button
            class="button shortcut-recorder"
            data-shortcut-recorder=""
            type="button"
            aria-pressed={recordingShortcut()}
            onClick={() => {
              setRecordingShortcut(true);
              setRecordingShortcutPreview("");
            }}
            onBlur={() => {
              setRecordingShortcut(false);
              setRecordingShortcutPreview("");
            }}
            onKeyDown={(event) => {
              if (!recordingShortcut()) {
                return;
              }

              event.preventDefault();
              event.stopPropagation();

              if (event.key === "Escape") {
                setRecordingShortcut(false);
                setRecordingShortcutPreview("");
                return;
              }

              setRecordingShortcutPreview(
                shortcutPreviewFromKeyboardEvent(event),
              );
              const nextShortcut = shortcutFromKeyboardEvent(event);
              if (nextShortcut) {
                setShortcut(nextShortcut);
                setRecordingShortcut(false);
                setRecordingShortcutPreview("");
              }
            }}
            onKeyUp={(event) => {
              if (recordingShortcut()) {
                setRecordingShortcutPreview(
                  shortcutPreviewFromKeyboardEvent(event),
                );
              }
            }}
          >
            <ShortcutKeySequence
              shortcut={
                recordingShortcut()
                  ? recordingShortcutPreview() || "Press shortcut"
                  : shortcut()
              }
              recording={recordingShortcut()}
            />
          </button>
        </div>

        <div class="settings-field">
          <span>Close shortcut</span>
          <button
            class="button shortcut-recorder"
            data-shortcut-recorder=""
            type="button"
            aria-pressed={recordingCloseShortcut()}
            onClick={() => {
              setRecordingCloseShortcut(true);
              setRecordingCloseShortcutPreview("");
            }}
            onBlur={() => {
              setRecordingCloseShortcut(false);
              setRecordingCloseShortcutPreview("");
            }}
            onKeyDown={(event) => {
              if (!recordingCloseShortcut()) {
                return;
              }

              event.preventDefault();
              event.stopPropagation();

              setRecordingCloseShortcutPreview(
                shortcutPreviewFromKeyboardEvent(event),
              );
              const nextShortcut = shortcutFromKeyboardEvent(event, {
                requireModifier: false,
              });
              if (nextShortcut) {
                setCloseShortcut(nextShortcut);
                setRecordingCloseShortcut(false);
                setRecordingCloseShortcutPreview("");
              }
            }}
            onKeyUp={(event) => {
              if (recordingCloseShortcut()) {
                setRecordingCloseShortcutPreview(
                  shortcutPreviewFromKeyboardEvent(event),
                );
              }
            }}
          >
            <ShortcutKeySequence
              shortcut={
                recordingCloseShortcut()
                  ? recordingCloseShortcutPreview() || "Press shortcut"
                  : closeShortcut()
              }
              recording={recordingCloseShortcut()}
            />
          </button>
        </div>

        <div class="settings-field">
          <span>Pronunciation shortcut</span>
          <button
            class="button shortcut-recorder"
            data-shortcut-recorder=""
            type="button"
            aria-pressed={recordingPronunciationShortcut()}
            onClick={() => {
              setRecordingPronunciationShortcut(true);
              setRecordingPronunciationShortcutPreview("");
            }}
            onBlur={() => {
              setRecordingPronunciationShortcut(false);
              setRecordingPronunciationShortcutPreview("");
            }}
            onKeyDown={(event) => {
              if (!recordingPronunciationShortcut()) {
                return;
              }

              event.preventDefault();
              event.stopPropagation();

              if (event.key === "Escape") {
                setRecordingPronunciationShortcut(false);
                setRecordingPronunciationShortcutPreview("");
                return;
              }

              setRecordingPronunciationShortcutPreview(
                shortcutPreviewFromKeyboardEvent(event),
              );
              const nextShortcut = shortcutFromKeyboardEvent(event);
              if (nextShortcut) {
                setPronunciationShortcut(nextShortcut);
                setRecordingPronunciationShortcut(false);
                setRecordingPronunciationShortcutPreview("");
              }
            }}
            onKeyUp={(event) => {
              if (recordingPronunciationShortcut()) {
                setRecordingPronunciationShortcutPreview(
                  shortcutPreviewFromKeyboardEvent(event),
                );
              }
            }}
          >
            <ShortcutKeySequence
              shortcut={
                recordingPronunciationShortcut()
                  ? recordingPronunciationShortcutPreview() || "Press shortcut"
                  : pronunciationShortcut()
              }
              recording={recordingPronunciationShortcut()}
            />
          </button>
        </div>

        <label>
          <span>Word provider</span>
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
          <span>単語用APIキー</span>
          <input
            type="password"
            value={apiKey()}
            placeholder={
              props.settings.apiKeyConfigured
                ? "保存済み。変更時のみ入力"
                : "APIキー"
            }
            onInput={(event) => setApiKey(event.currentTarget.value)}
            onPaste={(event) => {
              const pasted = event.clipboardData?.getData("text") ?? "";
              if (pasted.length > 0) {
                event.preventDefault();
                setApiKey(pasted);
              }
            }}
            autocomplete="off"
          />
        </label>

        <label>
          <span>DeepL APIキー</span>
          <input
            type="password"
            value={deeplApiKey()}
            placeholder={
              props.settings.deeplApiKeyConfigured
                ? "保存済み。変更時のみ入力"
                : "DeepL APIキー"
            }
            onInput={(event) => setDeeplApiKey(event.currentTarget.value)}
            onPaste={(event) => {
              const pasted = event.clipboardData?.getData("text") ?? "";
              if (pasted.length > 0) {
                event.preventDefault();
                setDeeplApiKey(pasted);
              }
            }}
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

        <div class="settings-divider" />

        <div class="settings-field">
          <span>Google同期</span>
          <div class="sync-auth-row">
            <p class="settings-note sync-auth-status">
              {props.syncAuthStatus?.signedIn
                ? `ログイン中: ${
                    props.syncAuthStatus.userEmail ??
                    props.syncAuthStatus.userId ??
                    "Supabaseユーザー"
                  }`
                : "未ログイン"}
            </p>
            <Show when={props.syncAuthStatus?.signedIn}>
              <button
                class="button sync-auth-button"
                type="button"
                disabled={signingIn() || !props.onSignOutSync}
                onClick={signOutSync}
              >
                ログアウト
              </button>
            </Show>
          </div>
          <Show when={syncStatusLabel(props.syncStatus ?? null)}>
            {(message) => <p class="settings-note">{message()}</p>}
          </Show>
          <Show
            when={
              props.syncStatus?.lifecycle === "error" && props.onRetrySync
            }
          >
            <button
              class="button sync-auth-button"
              type="button"
              onClick={() => {
                void props.onRetrySync?.();
              }}
            >
              同期を再試行
            </button>
          </Show>
        </div>

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

function ResultDisplayView(props: {
  state:
    | Extract<PopupState, { kind: "streaming" }>
    | Extract<PopupState, { kind: "ready" }>;
  activeResultTab: ResultTab;
  onSetResultTab: (tab: ResultTab) => void;
}) {
  void props.activeResultTab;
  void props.onSetResultTab;

  return (
    <Show
      when={
        props.state.kind === "ready" &&
        props.state.result.mode === "text-translation"
          ? props.state.result
          : null
      }
      fallback={
        <Show
          when={
            props.state.kind === "streaming" &&
            props.state.mode === "text-translation"
          }
          fallback={
            <DictionaryBody
              result={
                props.state.kind === "ready" &&
                props.state.result.mode === "word-study"
                  ? props.state.result
                  : props.state.kind === "streaming"
                    ? props.state.partial
                    : emptyPartialResult()
              }
              streaming={props.state.kind === "streaming"}
            />
          }
        >
          <LoadingTextTranslationView
            sourceText={
              props.state.kind === "streaming"
                ? (props.state.sourceText ?? "")
                : ""
            }
          />
        </Show>
      }
    >
      {(result) => <TextTranslationBody result={result()} />}
    </Show>
  );
}

function LoadingDictionaryView(props: {
  capture?: CaptureMetadata;
  label: string;
}) {
  const busyLabel = () =>
    props.capture
      ? `${props.label}: ${props.capture.characterCount} characters captured.`
      : props.label;

  return (
    <div
      class="dictionary-layout streaming"
      aria-busy="true"
      aria-label={busyLabel()}
    >
      <SkeletonDictionaryBody />
    </div>
  );
}

function DictionaryBody(props: { result: ResultLike; streaming?: boolean }) {
  return (
    <div class="dictionary-layout" classList={{ streaming: props.streaming }}>
      <Show
        when={props.result.nuance}
        fallback={<SkeletonBlock class="nuance-skeleton" />}
      >
        {(nuance) => <p class="nuance content-reveal">{nuance()}</p>}
      </Show>

      <section class="section" aria-labelledby="translations-title">
        <h2 class="section-title" id="translations-title">
          Translations
        </h2>
        <div class="translation-list">
          <Show
            when={props.result.translations.length > 0}
            fallback={<TranslationSkeletonList />}
          >
            <Index each={props.result.translations}>
              {(translation) => (
                <article class="translation-row content-reveal">
                  <div>
                    <Show when={translation().note}>
                      {(note) => (
                        <span class="pos-icon" aria-label={note()}>
                          {partOfSpeechMark(note())}
                        </span>
                      )}
                    </Show>
                  </div>
                  <div>
                    <div class="translation-head">
                      <span class="translation-text">{translation().text}</span>
                      <Show
                        when={
                          translation().senseKind === "inflection" &&
                          translation().baseWord
                        }
                      >
                        {(baseWord) => (
                          <span class="inflection-sense-label">
                            {baseWord()} の活用
                          </span>
                        )}
                      </Show>
                    </div>
                    <p class="example">
                      <HighlightedExample
                        sentence={translation().example.sentence}
                        target={props.result.headword ?? ""}
                      />
                      <span class="example-ja">
                        {translation().example.japanese}
                      </span>
                    </p>
                  </div>
                </article>
              )}
            </Index>
          </Show>
        </div>
      </section>

      <Show when={props.result.synonyms.length > 0}>
        <section class="section" aria-labelledby="synonyms-title">
          <h2 class="section-title" id="synonyms-title">
            Similar words
          </h2>
          <RelatedWordList words={props.result.synonyms} />
        </section>
      </Show>

      <Show when={props.result.idioms.length > 0}>
        <section class="section" aria-labelledby="idioms-title">
          <h2 class="section-title" id="idioms-title">
            Idioms
          </h2>
          <IdiomList idioms={props.result.idioms} />
        </section>
      </Show>

      <Show when={props.result.warnings.length > 0}>
        <p class="warning-line">{props.result.warnings[0]}</p>
      </Show>
    </div>
  );
}

function SkeletonDictionaryBody() {
  return (
    <>
      <SkeletonBlock class="nuance-skeleton" />
      <section class="section" aria-labelledby="translations-loading-title">
        <h2 class="section-title" id="translations-loading-title">
          Translations
        </h2>
        <div class="translation-list">
          <TranslationSkeletonList />
        </div>
      </section>
    </>
  );
}

function LoadingTextTranslationView(props: { sourceText: string }) {
  return (
    <div
      class="text-translation-layout streaming"
      aria-busy="true"
      aria-label="翻訳中"
    >
      <textarea
        class="translation-field translation-source-text content-reveal"
        aria-label="原文"
        readOnly
        value={props.sourceText}
      />
      <div class="translation-arrow" aria-hidden="true">
        ↓
      </div>
      <SkeletonBlock class="translation-field-skeleton translated" />
    </div>
  );
}

function TextTranslationBody(props: { result: TextTranslationResultV1 }) {
  const sourceText = () => props.result.segments[0]?.source.trim() ?? "";

  return (
    <div class="text-translation-layout">
      <Show when={sourceText().length > 0}>
        <textarea
          class="translation-field translation-source-text content-reveal"
          aria-label="原文"
          readOnly
          value={sourceText()}
        />
        <div class="translation-arrow" aria-hidden="true">
          ↓
        </div>
      </Show>
      <section class="text-translation-main" aria-label="翻訳結果">
        <textarea
          class="translation-field translated-text content-reveal"
          aria-label="日本語訳"
          lang={props.result.resultLanguage}
          readOnly
          value={props.result.translatedText}
        />
      </section>

      <Show when={props.result.warnings.length > 0}>
        <p class="warning-line">{props.result.warnings[0]}</p>
      </Show>
    </div>
  );
}

function TranslationSkeletonList() {
  return (
    <>
      <TranslationSkeletonRow />
      <TranslationSkeletonRow compact />
    </>
  );
}

function TranslationSkeletonRow(props: { compact?: boolean }) {
  return (
    <div class="translation-row skeleton-row" aria-hidden="true">
      <SkeletonBlock class="pos-skeleton" />
      <div class="skeleton-copy">
        <SkeletonBlock class={props.compact ? "line-sm" : "line-md"} />
        <SkeletonBlock class="line-full" />
        <SkeletonBlock class="line-short" />
      </div>
    </div>
  );
}

function InflectionLine(props: {
  headword: string;
  inflections: Inflection[];
}) {
  const plural = createMemo(
    () => props.inflections.find((item) => item.kind === "plural")?.form,
  );
  const verbForms = createMemo(() =>
    inflectionVerbForms(props.headword, props.inflections),
  );

  return (
    <Show when={plural() || verbForms().length > 1}>
      <p class="inflection-line" aria-label="Irregular inflections">
        <Show
          when={plural()}
          fallback={<VerbInflectionForms forms={verbForms()} />}
        >
          {(form) => (
            <span class="inflection-plural">
              <span class="plural-icon" aria-hidden="true" />
              <span>{form()}</span>
            </span>
          )}
        </Show>
      </p>
    </Show>
  );
}

function VerbInflectionForms(props: { forms: string[] }) {
  return (
    <span class="inflection-verb">
      <For each={props.forms}>
        {(form, index) => (
          <>
            <Show when={index() > 0}>
              <span class="verb-flow-icon" aria-hidden="true" />
            </Show>
            <span>{form}</span>
          </>
        )}
      </For>
    </span>
  );
}

function SkeletonBlock(props: { class?: string }) {
  return <span class={`skeleton-block ${props.class ?? ""}`} />;
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
  let searchFrom = 0;

  while (searchFrom < sentence.length) {
    const index = lowerSentence.indexOf(lowerNeedle, searchFrom);
    if (index < 0) {
      segments.push({ text: sentence.slice(cursor), highlighted: false });
      break;
    }

    const end = index + needle.length;
    if (!isTargetBoundary(sentence, index, end, needle)) {
      searchFrom = index + 1;
      continue;
    }

    if (index > cursor) {
      segments.push({
        text: sentence.slice(cursor, index),
        highlighted: false,
      });
    }
    segments.push({ text: sentence.slice(index, end), highlighted: true });
    cursor = end;
    searchFrom = end;
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

function inflectionVerbForms(
  headword: string,
  inflections: Inflection[],
): string[] {
  const base = headword.trim();
  if (base.length === 0 || inflections.length === 0) {
    return [];
  }

  const past = inflections.find((item) => item.kind === "past")?.form;
  const pastParticiple = inflections.find(
    (item) => item.kind === "pastParticiple",
  )?.form;

  return [base, past, pastParticiple].filter((form): form is string =>
    Boolean(form),
  );
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
  let detailShell: HTMLDivElement | undefined;
  const detailMaxHeight = () =>
    open() && detailShell ? `${detailShell.scrollHeight}px` : "0px";

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
      <div
        class="synonym-detail-shell"
        aria-hidden={!open()}
        ref={(element) => {
          detailShell = element;
        }}
        style={{ "max-height": detailMaxHeight() }}
      >
        <div class="synonym-detail-content">
          <p>{props.word.usageComparison}</p>
        </div>
      </div>
    </article>
  );
}

function IdiomList(props: { idioms: Idiom[] }) {
  return (
    <div class="idiom-list">
      <For each={props.idioms}>{(idiom) => <IdiomItem idiom={idiom} />}</For>
    </div>
  );
}

function IdiomItem(props: { idiom: Idiom }) {
  return (
    <article class="idiom-row content-reveal">
      <div class="idiom-head">
        <span class="idiom-term">{props.idiom.idiom}</span>
        <span class="idiom-ja">{props.idiom.japanese}</span>
      </div>
      <p class="idiom-example">{props.idiom.example}</p>
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

function resultState(
  state: PopupState,
):
  | Extract<PopupState, { kind: "streaming" }>
  | Extract<PopupState, { kind: "ready" }>
  | null {
  return state.kind === "streaming" || state.kind === "ready" ? state : null;
}

function errorState(
  state: PopupState,
): Extract<PopupState, { kind: "error" }> | null {
  return state.kind === "error" ? state : null;
}

function titleForState(state: PopupState): string {
  switch (state.kind) {
    case "capturing":
      return "";
    case "requesting":
      return "";
    case "streaming":
      return "";
    case "ready":
      return state.result.mode === "word-study" ? state.result.headword : "";
    case "error":
      return "Error";
    case "idle":
      return "";
  }
}

export function speakableHeadwordForState(state: PopupState): string | null {
  if (state.kind === "ready" && state.result.mode === "word-study") {
    const headword = state.result.headword.trim();
    return headword.length > 0 ? headword : null;
  }

  if (state.kind === "streaming" && state.mode === "word-study") {
    const headword = (state.partial.headword ?? "").trim();
    return headword.length > 0 ? headword : null;
  }

  return null;
}

function headwordForState(state: PopupState): string {
  if (state.kind === "streaming") {
    if (state.mode === "text-translation") {
      return "";
    }
    return state.partial.headword ?? "";
  }

  return titleForState(state);
}

function inflectionsForState(state: PopupState): Inflection[] {
  if (state.kind === "streaming") {
    return state.partial.inflections;
  }

  if (state.kind === "ready") {
    return state.result.mode === "word-study" ? state.result.inflections : [];
  }

  return [];
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
    inflections: [],
    translations: [],
    nuance: null,
    synonyms: [],
    idioms: [],
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

function ShortcutKeySequence(props: { shortcut: string; recording: boolean }) {
  const parts = createMemo(() =>
    props.shortcut
      .split("+")
      .map((part) => part.trim())
      .filter((part) => part.length > 0),
  );

  return (
    <span class="shortcut-key-sequence">
      <For each={parts()}>
        {(part, index) => (
          <>
            <Show when={index() > 0}>
              <span class="shortcut-plus" aria-hidden="true">
                +
              </span>
            </Show>
            <span
              class="shortcut-keycap"
              classList={{ pending: props.recording && part === "..." }}
            >
              {part}
            </span>
          </>
        )}
      </For>
    </span>
  );
}

function shortcutFromKeyboardEvent(
  event: KeyboardEvent,
  options: { requireModifier?: boolean } = {},
): string | null {
  const key = shortcutKeyLabel(event);
  if (!key) {
    return null;
  }

  const parts = shortcutModifierLabels(event);
  if ((options.requireModifier ?? true) && parts.length === 0) {
    return null;
  }

  parts.push(key);
  return parts.join("+");
}

function shortcutPreviewFromKeyboardEvent(event: KeyboardEvent): string {
  const parts = shortcutModifierLabels(event);
  const key = shortcutKeyLabel(event);

  if (key) {
    parts.push(key);
  } else if (parts.length > 0) {
    parts.push("...");
  }

  return parts.join("+");
}

function shortcutKeyLabel(event: KeyboardEvent): string | null {
  const key = event.key;
  if (key.length === 1 && /^[a-z0-9]$/i.test(key)) {
    return key.toUpperCase();
  }
  if (key === " ") {
    return "Space";
  }
  if (key.length === 1 && key !== "+") {
    return key;
  }
  if (key === "+") {
    return "Plus";
  }

  if (key === "Escape" || key === "Esc") {
    return "Escape";
  }
  if (key === "Tab") {
    return "Tab";
  }
  if (key === "Enter") {
    return "Enter";
  }
  if (key === "Backspace") {
    return "Backspace";
  }
  if (/^F([1-9]|1[0-2])$/.test(key)) {
    return key.toUpperCase();
  }

  return null;
}

function shortcutModifierLabels(event: KeyboardEvent): string[] {
  const parts = [];
  if (event.ctrlKey) {
    parts.push("Ctrl");
  }
  if (event.altKey) {
    parts.push("Alt");
  }
  if (event.shiftKey) {
    parts.push("Shift");
  }
  if (event.metaKey) {
    parts.push("Super");
  }

  return parts;
}

function matchesShortcutEvent(event: KeyboardEvent, shortcut: string): boolean {
  return (
    shortcutFromKeyboardEvent(event, { requireModifier: false }) === shortcut
  );
}

function isShortcutRecorderTarget(target: EventTarget | null): boolean {
  return (
    target instanceof Element &&
    Boolean(target.closest("[data-shortcut-recorder]"))
  );
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
