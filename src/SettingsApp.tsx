import { Show, createSignal, onMount } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  SettingsPanel,
  type ProviderSettings,
  type ProviderSettingsUpdate,
  type SettingsUpdatedEvent,
  type SyncAuthStatus,
  type ThemeMode,
} from "./App";
import "./App.css";

import {
  DEFAULT_CLOSE_SHORTCUT,
  DEFAULT_PRONUNCIATION_SHORTCUT,
} from "./App";

function buildSettingsUpdate(
  settings: ProviderSettings,
  overrides: Partial<ProviderSettingsUpdate> = {},
): ProviderSettingsUpdate {
  return {
    shortcut: settings.shortcut,
    closeShortcut: settings.closeShortcut ?? DEFAULT_CLOSE_SHORTCUT,
    pronunciationShortcut:
      settings.pronunciationShortcut ?? DEFAULT_PRONUNCIATION_SHORTCUT,
    backgroundOpacity: settings.backgroundOpacity,
    theme: settings.theme,
    provider: settings.provider,
    model: settings.model,
    resultLanguage: settings.resultLanguage,
    promptMode: settings.promptMode,
    apiKey: null,
    deeplApiKey: null,
    ...overrides,
  };
}

export function SettingsView(props: {
  settings: ProviderSettings;
  syncAuthStatus: SyncAuthStatus | null;
  themeMode: ThemeMode;
  backgroundOpacity: number;
  onSave: (update: ProviderSettingsUpdate) => Promise<void>;
  onSignOutSync?: () => Promise<void>;
  onToggleTheme: () => void;
  onSetBackgroundOpacity: (opacity: number) => void;
}) {
  return (
    <main class={`settings-shell theme-${props.themeMode}`}>
      <SettingsPanel
        settings={props.settings}
        syncAuthStatus={props.syncAuthStatus}
        themeMode={props.themeMode}
        backgroundOpacity={props.backgroundOpacity}
        onSave={props.onSave}
        onSignOutSync={props.onSignOutSync}
        onToggleTheme={props.onToggleTheme}
        onSetBackgroundOpacity={props.onSetBackgroundOpacity}
      />
    </main>
  );
}

export default function SettingsApp() {
  const [providerSettings, setProviderSettings] =
    createSignal<ProviderSettings | null>(null);
  const [syncAuthStatus, setSyncAuthStatus] =
    createSignal<SyncAuthStatus | null>(null);
  const [themeMode, setThemeMode] = createSignal<ThemeMode>("light");
  const [backgroundOpacity, setBackgroundOpacity] = createSignal(0.94);

  async function broadcastSettingsUpdate(saved: ProviderSettings) {
    setThemeMode(saved.theme);
    await emit("lexi:settings-updated", {
      settings: saved,
      themeMode: saved.theme,
    } satisfies SettingsUpdatedEvent);
  }

  async function saveProviderSettings(update: ProviderSettingsUpdate) {
    const saved = await invoke<ProviderSettings>("update_provider_settings", {
      update,
    });
    setProviderSettings(saved);
    setBackgroundOpacity(saved.backgroundOpacity);
    await broadcastSettingsUpdate(saved);
    await getCurrentWindow().close();
  }

  async function toggleTheme() {
    const settings = providerSettings();
    if (!settings) {
      return;
    }

    const nextTheme: ThemeMode = themeMode() === "dark" ? "light" : "dark";
    setThemeMode(nextTheme);

    const saved = await invoke<ProviderSettings>("update_provider_settings", {
      update: buildSettingsUpdate(settings, {
        theme: nextTheme,
        backgroundOpacity: backgroundOpacity(),
      }),
    });
    setProviderSettings(saved);
    setBackgroundOpacity(saved.backgroundOpacity);
    await broadcastSettingsUpdate(saved);
  }

  async function signOutSync() {
    await invoke("sign_out_sync");
    const status = await invoke<SyncAuthStatus>("get_sync_auth_status");
    setSyncAuthStatus(status);
  }

  onMount(() => {
    let cleanupSyncAuth: (() => void) | undefined;

    void invoke<ProviderSettings>("get_provider_settings").then((settings) => {
      setProviderSettings(settings);
      setBackgroundOpacity(settings.backgroundOpacity);
      setThemeMode(settings.theme);
    });

    void invoke<SyncAuthStatus>("get_sync_auth_status").then(setSyncAuthStatus);

    void listen<SyncAuthStatus>("lexi:sync-auth", (event) => {
      setSyncAuthStatus(event.payload);
    }).then((unlisten) => {
      cleanupSyncAuth = unlisten;
    });

    return () => {
      cleanupSyncAuth?.();
    };
  });

  return (
    <Show when={providerSettings()}>
      {(settings) => (
        <SettingsView
          settings={settings()}
          syncAuthStatus={syncAuthStatus()}
          themeMode={themeMode()}
          backgroundOpacity={backgroundOpacity()}
          onSave={saveProviderSettings}
          onSignOutSync={signOutSync}
          onToggleTheme={() => {
            void toggleTheme();
          }}
          onSetBackgroundOpacity={setBackgroundOpacity}
        />
      )}
    </Show>
  );
}
