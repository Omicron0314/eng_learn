import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: ["packages/*/src/**/*.test.ts", "apps/web/src/**/*.test.ts", "tools/**/*.test.ts"],
    environment: "node",
    testTimeout: 20_000,
  },
});
