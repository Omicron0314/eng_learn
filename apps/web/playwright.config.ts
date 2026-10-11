import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  workers: 1,
  timeout: 90000,
  use: { baseURL: "http://localhost:3100", trace: "retain-on-failure" },
  webServer: {
    command: "pnpm dev --port 3100",
    url: "http://localhost:3100/login",
    reuseExistingServer: false,
    timeout: 120000,
    env: {
      LLM_PROVIDER: "fake",
      TTS_PROVIDER: "fake",
      APP_PASSWORD: "test-password",
      DATABASE_PATH: ":memory:",
      BLOB_DIR: "../../data/e2e-blobs",
    },
  },
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
});
