import { afterEach, describe, expect, it, vi } from "vitest";

async function config(ci: string) {
  vi.stubEnv("CI", ci);
  vi.resetModules();
  return (await import("../apps/web/playwright.config")).default;
}

afterEach(() => {
  vi.unstubAllEnvs();
  vi.resetModules();
});

describe("T0.1 AC6: E2E server configuration", () => {
  it("uses the existing production build in CI instead of cold dev compilation", async () => {
    const result = await config("true");
    const [mock, app] = result.webServer as {
      command?: string;
      env?: Record<string, string>;
    }[];
    expect(mock).toMatchObject({
      command: "pnpm exec tsx ../../tools/mock-providers.ts",
      reuseExistingServer: false,
    });
    expect(app).toMatchObject({
      command: "pnpm exec tsx ../../tools/start-e2e.ts",
      reuseExistingServer: false,
      env: {
        LLM_BASE_URL: "http://127.0.0.1:3200/v1",
        TTS_PROVIDER: "openai-compatible",
        DATABASE_PATH: ":memory:",
      },
    });
    expect(result.use?.baseURL).toBe("http://localhost:3100");
  });

  it("keeps the local dev server workflow without requiring a build", async () => {
    const result = await config("");
    const [, app] = result.webServer as { command?: string; reuseExistingServer?: boolean }[];
    expect(app).toMatchObject({
      command: "pnpm dev --port 3100",
      reuseExistingServer: false,
    });
  });
});
