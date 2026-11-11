import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterAll, describe, expect, it } from "vitest";
import { z } from "zod";
import { LocalFileBlob } from "./blob";
import {
  llmStatus,
  loadLLMConfig,
  loadTTSConfig,
  resolveLLMConfig,
  resolveTTSConfig,
  ttsStatus,
} from "./config";
import {
  ConfigError,
  JsonModeUnsupportedError,
  LLMOutputInvalidError,
  ProviderError,
} from "./errors";
import { createTTS } from "./factory";
import { OpenAICompatibleLLM } from "./llm/openai-compatible";
import { completeJson } from "./llm/structured";
import type { LLMCallRecord, LLMProvider } from "./llm/types";
import { FakeLLM } from "./testing/fake-llm";
import { FakeTTS, wavDuration } from "./testing/fake-tts";
import { OpenAICompatibleTTS } from "./tts/openai-compatible";

const schema = z.object({ title: z.string().min(1) });
const req = { purpose: "article" as const, messages: [{ role: "user" as const, content: "hi" }] };

function run(llm: LLMProvider, price?: { inputPerMTok: number; outputPerMTok: number }) {
  const calls: LLMCallRecord[] = [];
  const p = completeJson(llm, req, schema, {
    onCall: (r) => calls.push(r),
    shape: '{"title": string}',
    ...(price ? { price } : {}),
  });
  return { p, calls };
}

describe("config", () => {
  it("AC1: lists all missing LLM vars", () => {
    try {
      loadLLMConfig({ LLM_BASE_URL: "http://x" });
      expect.unreachable();
    } catch (e) {
      expect(e).toBeInstanceOf(ConfigError);
      expect((e as ConfigError).missing).toEqual(["LLM_API_KEY", "LLM_MODEL"]);
      expect((e as Error).message).toContain("LLM_API_KEY, LLM_MODEL");
    }
    expect(() => loadLLMConfig({ LLM_PROVIDER: "nope" })).toThrow(ConfigError);
  });

  it("AC12: TTS provider selection", () => {
    expect(createTTS(loadTTSConfig({}))).toBeNull();
    expect(
      createTTS(
        loadTTSConfig({
          TTS_PROVIDER: "openai-compatible",
          TTS_BASE_URL: "https://tts.example.com",
          TTS_API_KEY: "k",
          TTS_MODEL: "m",
          TTS_VOICE: "v",
        }),
      ),
    ).toBeInstanceOf(OpenAICompatibleTTS);
    expect(() => loadTTSConfig({ TTS_PROVIDER: "nope" })).toThrow(ConfigError);
  });

  it("settings-page config takes precedence over env and reports status", () => {
    const stored = {
      provider: "openai-compatible" as const,
      baseURL: "https://stored.example.com/v1",
      apiKey: "sk-stored",
      model: "stored-model",
    };
    const env = {
      LLM_BASE_URL: "https://env.example.com",
      LLM_API_KEY: "sk-env",
      LLM_MODEL: "env-model",
      TTS_PROVIDER: "openai-compatible",
      TTS_BASE_URL: "https://tts.example.com",
      TTS_API_KEY: "k",
      TTS_MODEL: "m",
      TTS_VOICE: "v",
    };
    expect(resolveLLMConfig(env, stored)).toMatchObject({
      baseURL: "https://stored.example.com/v1",
      model: "stored-model",
    });
    expect(resolveLLMConfig(env)).toMatchObject({ model: "env-model" });
    expect(llmStatus(env, stored)).toMatchObject({ configured: true, source: "settings" });
    expect(llmStatus(env)).toMatchObject({ configured: true, source: "env", model: "env-model" });
    expect(llmStatus({})).toMatchObject({ configured: false, source: null });
    expect(resolveTTSConfig(env, { provider: "none" })).toEqual({ provider: "none" });
    expect(resolveTTSConfig(env)).toMatchObject({ provider: "openai-compatible" });
    expect(
      ttsStatus(
        {},
        {
          provider: "openai-compatible",
          baseURL: "https://x.co",
          apiKey: "k",
          model: "m",
          voice: "v",
        },
      ),
    ).toEqual({
      source: "settings",
      provider: "openai-compatible",
    });
  });

  it("resolveLLMConfig without settings or env fails with a user-facing message", () => {
    expect(() => resolveLLMConfig({})).toThrow(/设置页/);
    expect(llmStatus({}).missing).toEqual(["端点", "密钥", "模型"]);
  });
});

describe("OpenAICompatibleLLM", () => {
  it("AC2: sends configured model and json mode to base URL", async () => {
    let captured: { url: string; body: Record<string, unknown> } | undefined;
    const fakeFetch = (async (url: string, init: RequestInit) => {
      captured = { url: String(url), body: JSON.parse(String(init.body)) };
      return new Response(
        JSON.stringify({
          id: "1",
          object: "chat.completion",
          created: 0,
          model: "served-model",
          choices: [
            { index: 0, finish_reason: "stop", message: { role: "assistant", content: "{}" } },
          ],
          usage: { prompt_tokens: 11, completion_tokens: 7, total_tokens: 18 },
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      );
    }) as unknown as typeof fetch;
    const llm = new OpenAICompatibleLLM({
      baseURL: "http://llm.local/v1",
      apiKey: "k",
      model: "configured-model",
      fetch: fakeFetch,
    });
    const res = await llm.complete({ ...req, jsonMode: true });
    expect(captured?.url).toBe("http://llm.local/v1/chat/completions");
    expect(captured?.body.model).toBe("configured-model");
    expect(captured?.body.response_format).toEqual({ type: "json_object" });
    expect(res).toMatchObject({ model: "served-model", tokensIn: 11, tokensOut: 7, text: "{}" });
  });
});

describe("completeJson", () => {
  it("AC3: valid first reply", async () => {
    const { p, calls } = run(new FakeLLM({ script: ['{"title":"A"}'] }));
    expect((await p).value).toEqual({ title: "A" });
    expect(calls.map((c) => c.status)).toEqual(["ok"]);
  });

  it("AC4: repair after invalid JSON", async () => {
    const llm = new FakeLLM({ script: ["not json", '{"title":"B"}'] });
    const { p, calls } = run(llm);
    expect((await p).attempts).toBe(2);
    expect(calls.map((c) => c.status)).toEqual(["invalid_output", "ok"]);
    const second = llm.requests[1]?.messages ?? [];
    expect(second.some((m) => m.role === "assistant" && m.content === "not json")).toBe(true);
    expect(second.at(-1)?.content).toContain("could not be used");
  });

  it("AC5: throws after two invalid replies", async () => {
    const { p, calls } = run(new FakeLLM({ script: ["x", "y"] }));
    await expect(p).rejects.toBeInstanceOf(LLMOutputInvalidError);
    expect(calls.map((c) => c.status)).toEqual(["invalid_output", "invalid_output"]);
  });

  it("AC6: schema mismatch uses the same repair path", async () => {
    const { p, calls } = run(new FakeLLM({ script: ['{"name":"x"}', '{"title":"ok"}'] }));
    await p;
    expect(calls[0]?.error).toContain("schema mismatch");
  });

  it("AC7: falls back when JSON mode is unsupported", async () => {
    const llm = new FakeLLM({ script: [new JsonModeUnsupportedError(), '{"title":"C"}'] });
    const { p, calls } = run(llm);
    const r = await p;
    expect(calls.map((c) => c.status)).toEqual(["error", "ok"]);
    expect(r.jsonModeUsed).toBe(false);
    expect(llm.requests[1]?.jsonMode).toBe(false);
  });

  it("AC8: provider errors are recorded and rethrown without repair", async () => {
    const llm = new FakeLLM({ script: [new ProviderError("boom")] });
    const { p, calls } = run(llm);
    await expect(p).rejects.toBeInstanceOf(ProviderError);
    expect(calls.map((c) => c.status)).toEqual(["error"]);
    expect(llm.calls).toBe(1);
  });

  it("AC9: cost from configured prices", async () => {
    const { p, calls } = run(new FakeLLM({ script: ['{"title":"A"}'] }), {
      inputPerMTok: 1,
      outputPerMTok: 2,
    });
    await p;
    const c = calls[0];
    expect(c?.costUsd).toBeCloseTo(((c?.tokensIn ?? 0) * 1 + (c?.tokensOut ?? 0) * 2) / 1e6);
    const { p: p2, calls: calls2 } = run(new FakeLLM({ script: ['{"title":"A"}'] }));
    await p2;
    expect(calls2[0]?.costUsd).toBeNull();
  });
});

describe("blob and tts", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "blob-"));
  afterAll(() => rmSync(dir, { recursive: true, force: true }));

  it("AC10: local blob roundtrip and key safety", async () => {
    const store = new LocalFileBlob(dir);
    await store.put("audio/a.wav", new Uint8Array([1, 2, 3]), "audio/wav");
    expect(await store.get("audio/a.wav")).toEqual({
      data: new Uint8Array([1, 2, 3]),
      contentType: "audio/wav",
    });
    for (const bad of ["../x", "/etc/passwd", "a\\b"]) {
      await expect(store.put(bad, new Uint8Array(), "x")).rejects.toThrow();
    }
  });

  it("AC11: test-double TTS is deterministic with parseable duration", async () => {
    const tts = new FakeTTS();
    const a = await tts.synthesize({ text: "hello world" });
    const b = await tts.synthesize({ text: "hello world" });
    expect(a.audio).toEqual(b.audio);
    expect(wavDuration(a.audio)).toBeCloseTo(a.durationS ?? 0, 2);
  });
});
