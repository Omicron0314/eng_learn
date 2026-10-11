import { FakeLLM, FakeTTS, MemoryBlob } from "@eng-learn/providers";
import { describe, expect, it } from "vitest";
import { createAppDeps } from "../deps";
import { getSettingsView, updateProviderConfig } from "./settings";

function setupDeps(env: Record<string, string | undefined> = {}) {
  const deps = createAppDeps(
    { DATABASE_PATH: ":memory:", ...env },
    { blobs: new MemoryBlob(), clock: () => new Date("2026-10-12T02:00:00Z") },
  );
  return deps;
}

const llmPatch = {
  llm: {
    provider: "openai-compatible",
    baseURL: "https://llm.example.com/v1",
    model: "test-model",
    apiKey: "sk-secret-123",
  },
};

describe("updateProviderConfig", () => {
  it("stores secrets server-side and never exposes them in any view", () => {
    const deps = setupDeps();
    try {
      const result = updateProviderConfig(deps, llmPatch);
      const view = getSettingsView(deps);
      for (const v of [result, view]) {
        expect(JSON.stringify(v)).not.toContain("sk-secret-123");
      }
      expect(view.providerConfig.llm).toMatchObject({
        provider: "openai-compatible",
        baseURL: "https://llm.example.com/v1",
        model: "test-model",
        hasApiKey: true,
      });
      expect(view.providers.llm).toMatchObject({
        configured: true,
        source: "settings",
        host: "llm.example.com",
      });
    } finally {
      deps.close();
    }
  });

  it("an empty apiKey keeps the stored key; a new one replaces it", () => {
    const deps = setupDeps();
    try {
      updateProviderConfig(deps, llmPatch);
      const updated = updateProviderConfig(deps, {
        llm: { ...llmPatch.llm, model: "renamed-model", apiKey: "" },
      });
      expect(updated.providerConfig.llm).toMatchObject({ model: "renamed-model" });
      expect(deps.providerConfig().llm).toMatchObject({ apiKey: "sk-secret-123" });
      updateProviderConfig(deps, { llm: { ...llmPatch.llm, apiKey: "sk-new" } });
      expect(deps.providerConfig().llm).toMatchObject({ apiKey: "sk-new" });
    } finally {
      deps.close();
    }
  });

  it("rejects a missing initial apiKey and invalid URLs without writing events", () => {
    const deps = setupDeps();
    try {
      expect(() => updateProviderConfig(deps, { llm: { ...llmPatch.llm, apiKey: "" } })).toThrow(
        /密钥/,
      );
      expect(() =>
        updateProviderConfig(deps, { llm: { ...llmPatch.llm, baseURL: "not-a-url" } }),
      ).toThrow();
      expect(deps.repos.events.count(deps.userId)).toBe(0);
    } finally {
      deps.close();
    }
  });

  it("appends a settings_update event with key names but no secret values", () => {
    const deps = setupDeps();
    try {
      updateProviderConfig(deps, {
        ...llmPatch,
        tts: {
          provider: "openai-compatible",
          baseURL: "https://tts.example.com",
          model: "m",
          voice: "v",
          apiKey: "sk-tts",
        },
      });
      const events = deps.repos.events.listByType(deps.userId, "settings_update");
      expect(events).toHaveLength(1);
      expect(events[0]?.payload.changed).toEqual(["provider.llm", "provider.tts"]);
      expect(JSON.stringify(events)).not.toContain("sk-");
    } finally {
      deps.close();
    }
  });

  it("repeated saves with identical values produce no events", () => {
    const deps = setupDeps();
    try {
      updateProviderConfig(deps, llmPatch);
      updateProviderConfig(deps, llmPatch);
      expect(deps.repos.events.count(deps.userId)).toBe(1);
    } finally {
      deps.close();
    }
  });
});

describe("lazy provider resolution", () => {
  it("settings-page config applies immediately without a restart", () => {
    const deps = setupDeps();
    try {
      expect(deps.providerStatus().llm).toMatchObject({ configured: false, source: null });
      expect(() => deps.llmProvider()).toThrow(/设置页/);
      expect(deps.ttsProvider()).toBeNull();
      updateProviderConfig(deps, { llm: { provider: "fake", model: "demo-model" } });
      const llm = deps.llmProvider();
      expect(llm).toBeInstanceOf(FakeLLM);
      expect(llm.model).toBe("demo-model");
      updateProviderConfig(deps, { tts: { provider: "fake" } });
      expect(deps.ttsProvider()).toBeInstanceOf(FakeTTS);
      updateProviderConfig(deps, { tts: { provider: "none" } });
      expect(deps.ttsProvider()).toBeNull();
    } finally {
      deps.close();
    }
  });

  it("env config is used as a fallback when the settings page has none", () => {
    const deps = setupDeps({ LLM_PROVIDER: "fake", LLM_MODEL: "env-fake" });
    try {
      expect(deps.llmProvider().model).toBe("env-fake");
      expect(deps.providerStatus().llm).toMatchObject({ configured: true, source: "env" });
      // A settings-page value wins over the environment.
      updateProviderConfig(deps, { llm: { provider: "fake", model: "settings-fake" } });
      expect(deps.llmProvider().model).toBe("settings-fake");
    } finally {
      deps.close();
    }
  });
});
