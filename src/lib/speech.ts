import { invoke } from "@tauri-apps/api/core";

export function speakText(text: string): void {
  const trimmed = text.trim();
  if (trimmed.length === 0) {
    return;
  }

  void invoke("speak_headword", { text: trimmed }).catch(() => undefined);
}

export function stopSpeaking(): void {
  void invoke("stop_headword_speech").catch(() => undefined);
}
