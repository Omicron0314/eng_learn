import { defineConfig } from "@playwright/test";

const mockProviderURL = "http://127.0.0.1:3200/v1";
export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  workers: 1,
  timeout: 90000,
  use: { baseURL: "http://localhost:3100", trace: "retain-on-failure" },
  webServer: [
    {
      // Test-only OpenAI-compatible endpoint; the app never ships a fake provider.
      command: "pnpm exec tsx ../../tools/mock-providers.ts",
      url: `${mockProviderURL}/models`,
      reuseExistingServer: false,
      timeout: 30000,
    },
    {
      // CI already runs pnpm build; avoid route compilation during 5-second assertions.
      command: process.env.CI ? "pnpm exec tsx ../../tools/start-e2e.ts" : "pnpm dev --port 3100",
      // Warm an API route as well: the first login POST compiles on demand in dev.
      url: "http://localhost:3100/api/today",
      reuseExistingServer: false,
      timeout: 120000,
      env: {
        LLM_BASE_URL: mockProviderURL,
        LLM_API_KEY: "test-key",
        LLM_MODEL: "mock-model",
        TTS_PROVIDER: "openai-compatible",
        TTS_BASE_URL: mockProviderURL,
        TTS_API_KEY: "test-key",
        TTS_MODEL: "mock-tts",
        TTS_VOICE: "mock-voice",
        APP_PASSWORD: "test-password",
        DATABASE_PATH: ":memory:",
        BLOB_DIR: "../../data/e2e-blobs",
      },
    },
  ],
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
});
