export async function register() {
  if (process.env.NEXT_RUNTIME === "nodejs") {
    const { ConfigError, hasLlmEnv, hasTtsEnv, loadLLMConfig, loadTTSConfig } = await import(
      "@eng-learn/providers"
    );
    // Next catches hook exceptions and may keep listening. Exit on invalid startup
    // configuration so a broken deployment cannot appear ready. Provider config may
    // also live in database settings, so absent env vars are no longer fatal; only
    // partially-set or invalid env config fails fast.
    try {
      if (hasLlmEnv(process.env)) loadLLMConfig(process.env);
      if (hasTtsEnv(process.env)) loadTTSConfig(process.env);
      const { configuredAppOrigin } = await import("./server/http");
      configuredAppOrigin();
    } catch (error) {
      console.error(
        error instanceof ConfigError ? error.message : "Provider configuration invalid",
      );
      process.exit(1);
    }
  }
}
