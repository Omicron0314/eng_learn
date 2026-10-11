import { describe, expect, it } from "vitest";
import { loadTTSConfig } from "../config";
import { createTTS } from "../factory";
import { OpenAICompatibleTTS, splitSpeechText } from "./openai-compatible";

describe("T5.1 OpenAI-compatible adapter", () => {
  it("AC6/AC8: configured endpoint/model/voice and MP3 chunks are mapped without network", async () => {
    const model = "configured-model";
    const voice = "configured-voice";
    const requests: { url: string; body: Record<string, unknown> }[] = [];
    const adapter = new OpenAICompatibleTTS({
      baseURL: "https://example.invalid/v1",
      apiKey: "test-only",
      model,
      voice,
      fetch: async (url, options) => {
        requests.push({ url: String(url), body: JSON.parse(String(options?.body)) });
        return new Response(new Uint8Array([requests.length]), {
          headers: { "content-type": "audio/mpeg" },
        });
      },
    });
    const text = "I go home. ".repeat(800);
    const result = await adapter.synthesize({ text });
    expect(requests.length).toBeGreaterThan(1);
    expect(
      requests.every(
        (r) =>
          r.url === "https://example.invalid/v1/audio/speech" &&
          r.body.model === model &&
          r.body.voice === voice &&
          r.body.response_format === "mp3",
      ),
    ).toBe(true);
    expect(requests.map((r) => r.body.input).join("")).toBe(text);
    expect(result).toEqual({
      audio: new Uint8Array([1, 2, 3]),
      mimeType: "audio/mpeg",
      durationS: null,
    });
  });
  it("AC6: missing required configuration fails, browser remains default", () => {
    expect(createTTS(loadTTSConfig({}))).toBeNull();
    expect(() => loadTTSConfig({ TTS_PROVIDER: "openai-compatible" })).toThrow(
      /TTS_BASE_URL.*TTS_API_KEY.*TTS_MODEL.*TTS_VOICE/,
    );
    expect(
      createTTS(
        loadTTSConfig({
          TTS_PROVIDER: "openai-compatible",
          TTS_BASE_URL: "https://example.invalid",
          TTS_API_KEY: "test-only",
          TTS_MODEL: "configured-model",
          TTS_VOICE: "configured-voice",
        }),
      ),
    ).toBeInstanceOf(OpenAICompatibleTTS);
  });
  it("AC8: provider errors are sanitized and not automatically retried", async () => {
    let calls = 0;
    const adapter = new OpenAICompatibleTTS({
      baseURL: "https://example.invalid",
      apiKey: "test-only",
      model: "configured-model",
      voice: "configured-voice",
      fetch: async () => {
        calls++;
        return new Response(
          JSON.stringify({ error: { message: "sensitive diagnostic", type: "server_error" } }),
          { status: 500, headers: { "content-type": "application/json" } },
        );
      },
    });
    await expect(adapter.synthesize({ text: "hello" })).rejects.toMatchObject({
      code: "PROVIDER_ERROR",
      message: expect.not.stringContaining("sensitive"),
    });
    expect(calls).toBe(1);
  });
  it("AC6: chunking preserves all text including unicode surrogate pairs", () => {
    const text = `${"a".repeat(3999)}🙂${"b".repeat(4000)}`;
    const chunks = splitSpeechText(text);
    expect(chunks.join("")).toBe(text);
    expect(chunks.every((c) => c.length <= 4000)).toBe(true);
    expect(chunks[1]?.startsWith("🙂")).toBe(true);
  });
});
