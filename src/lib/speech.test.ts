import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { speakText, stopSpeaking } from "./speech";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => undefined),
}));

describe("speech", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue(undefined);
  });

  it("invokes native headword speech for trimmed text", () => {
    speakText("  subtle  ");

    expect(invoke).toHaveBeenCalledWith("speak_headword", { text: "subtle" });
  });

  it("ignores empty text", () => {
    speakText("   ");

    expect(invoke).not.toHaveBeenCalled();
  });

  it("stops native speech", () => {
    stopSpeaking();

    expect(invoke).toHaveBeenCalledWith("stop_headword_speech");
  });
});
