import { Match, Switch, onCleanup, onMount, createSignal } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { AppError } from "./lib/errors";
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

export type PopupState =
  | { kind: "idle"; shortcut: string }
  | { kind: "capturing"; shortcut: string }
  | {
      kind: "captured";
      shortcut: string;
      captureMethod: string;
      sourceProcess: string | null;
      sourceWindowTitle: string | null;
      characterCount: number;
      multiline: boolean;
    }
  | {
      kind: "error";
      shortcut: string;
      error: AppError;
      selectionErrorCode: string;
      captureMethod: string | null;
      sourceProcess: string | null;
      sourceWindowTitle: string | null;
    };

function App() {
  const [state, setState] = createSignal<PopupState>({
    kind: "idle",
    shortcut: "Ctrl+Shift+X",
  });

  async function closePopup() {
    await getCurrentWindow().hide();
  }

  onMount(() => {
    let cleanup: (() => void) | undefined;

    void invoke<ShortcutStatus>("get_shortcut_status").then((status) => {
      if (status.registrationError) {
        setState({
          kind: "error",
          shortcut: status.shortcut,
          error: status.registrationError,
          selectionErrorCode: "ShortcutRegistrationFailed",
          captureMethod: null,
          sourceProcess: null,
          sourceWindowTitle: null,
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
          kind: "captured",
          shortcut: payload.shortcut,
          captureMethod: payload.captureMethod,
          sourceProcess: payload.sourceProcess,
          sourceWindowTitle: payload.sourceWindowTitle,
          characterCount: payload.characterCount,
          multiline: payload.multiline,
        });
        return;
      }

      setState({
        kind: "error",
        shortcut: payload.shortcut,
        error: payload.error,
        selectionErrorCode: payload.selectionErrorCode,
        captureMethod: payload.captureMethod,
        sourceProcess: payload.sourceProcess,
        sourceWindowTitle: payload.sourceWindowTitle,
      });
    }).then((unlisten) => {
      cleanup = unlisten;
    });

    onCleanup(() => cleanup?.());
  });

  return <PopupView state={state()} onClose={closePopup} />;
}

export function PopupView(props: { state: PopupState; onClose: () => void }) {
  return (
    <main class="popup-shell">
      <header class="popup-header">
        <div>
          <p class="eyebrow">Lexi</p>
          <h1>{titleForState(props.state)}</h1>
        </div>
        <button class="icon-button" type="button" aria-label="Close" onClick={props.onClose}>
          x
        </button>
      </header>

      <section class="popup-body">
        <Switch>
          <Match when={props.state.kind === "idle"}>
            <p class="status-text">Waiting for {props.state.shortcut}</p>
          </Match>

          <Match when={props.state.kind === "capturing"}>
            <div class="spinner" aria-hidden="true" />
            <p class="status-text">Reading selected text...</p>
          </Match>

          <Match when={capturedState(props.state)}>
            {(captured) => (
              <div class="summary">
                <dl>
                  <div>
                    <dt>Characters</dt>
                    <dd>{captured().characterCount}</dd>
                  </div>
                  <div>
                    <dt>Source</dt>
                    <dd>{captured().sourceProcess ?? "Unknown"}</dd>
                  </div>
                  <div>
                    <dt>Method</dt>
                    <dd>{captured().captureMethod}</dd>
                  </div>
                  <div>
                    <dt>Multiline</dt>
                    <dd>{captured().multiline ? "Yes" : "No"}</dd>
                  </div>
                </dl>
                <p class="muted">
                  Selection was captured. LLM transformation is planned for Phase 5.
                </p>
              </div>
            )}
          </Match>

          <Match when={errorState(props.state)}>
            {(failed) => (
              <div class="error-panel">
                <p>{failed().error.userMessage}</p>
                <details>
                  <summary>Details</summary>
                  <dl class="diagnostics">
                    <div>
                      <dt>Code</dt>
                      <dd>{failed().selectionErrorCode}</dd>
                    </div>
                    <div>
                      <dt>App</dt>
                      <dd>{failed().sourceProcess ?? "Unknown"}</dd>
                    </div>
                    <div>
                      <dt>Method</dt>
                      <dd>{failed().captureMethod ?? "Unknown"}</dd>
                    </div>
                    <div>
                      <dt>Window</dt>
                      <dd>{failed().sourceWindowTitle ?? "Unknown"}</dd>
                    </div>
                  </dl>
                  <p>{failed().error.diagnosticMessage}</p>
                </details>
              </div>
            )}
          </Match>
        </Switch>
      </section>
    </main>
  );
}

function capturedState(state: PopupState): Extract<PopupState, { kind: "captured" }> | null {
  return state.kind === "captured" ? state : null;
}

function errorState(state: PopupState): Extract<PopupState, { kind: "error" }> | null {
  return state.kind === "error" ? state : null;
}

function titleForState(state: PopupState): string {
  switch (state.kind) {
    case "capturing":
      return "Capturing";
    case "captured":
      return "Captured";
    case "error":
      return "Needs attention";
    case "idle":
      return "Ready";
  }
}

export default App;
