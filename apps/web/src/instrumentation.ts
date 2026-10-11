export async function register() {
  if (process.env.NEXT_RUNTIME === "nodejs") {
    const { ConfigError, loadProviderConfig } = await import("@eng-learn/providers");
    // Next catches hook exceptions and may keep listening. Exit on invalid startup
    // configuration so a real-provider deployment cannot appear ready or fall back.
    try {
      loadProviderConfig(process.env);
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
