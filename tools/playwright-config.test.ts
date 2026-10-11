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
    expect(result.webServer).toMatchObject({
      command: "pnpm exec tsx ../../tools/start-e2e.ts",
      reuseExistingServer: false,
      env: {
        LLM_PROVIDER: "fake",
        TTS_PROVIDER: "fake",
        DATABASE_PATH: ":memory:",
      },
    });
    expect(result.use?.baseURL).toBe("http://localhost:3100");
  });

  it("keeps the local dev server workflow without requiring a build", async () => {
    const result = await config("");
    expect(result.webServer).toMatchObject({
      command: "pnpm dev --port 3100",
      reuseExistingServer: false,
    });
  });
});
