import { ConfigError } from "./errors";

export type Price = { inputPerMTok: number; outputPerMTok: number };

export type LLMConfig =
  | { provider: "openai-compatible"; baseURL: string; apiKey: string; model: string; price?: Price }
  | { provider: "fake"; model: string; price?: Price };

export type TTSConfig = { provider: "none" } | { provider: "fake"; voice: string; model: string };

export type ProviderConfig = {
  llm: LLMConfig;
  tts: TTSConfig;
  blobDir: string;
};

type Env = Record<string, string | undefined>;

function readPrice(env: Env): Price | undefined {
  const i = env.LLM_PRICE_INPUT_PER_MTOK;
  const o = env.LLM_PRICE_OUTPUT_PER_MTOK;
  if (!i || !o) return undefined;
  const inputPerMTok = Number(i);
  const outputPerMTok = Number(o);
  if (!Number.isFinite(inputPerMTok) || !Number.isFinite(outputPerMTok)) {
    throw new ConfigError("LLM_PRICE_INPUT_PER_MTOK / LLM_PRICE_OUTPUT_PER_MTOK must be numbers");
  }
  return { inputPerMTok, outputPerMTok };
}

export function loadLLMConfig(env: Env): LLMConfig {
  const provider = env.LLM_PROVIDER?.trim() || "openai-compatible";
  const price = readPrice(env);
  if (provider === "fake") {
    return {
      provider: "fake",
      model: env.LLM_MODEL?.trim() || "fake-llm",
      ...(price ? { price } : {}),
    };
  }
  if (provider !== "openai-compatible") {
    throw new ConfigError(`Unknown LLM_PROVIDER: ${provider}`);
  }
  const required = ["LLM_BASE_URL", "LLM_API_KEY", "LLM_MODEL"] as const;
  const missing = required.filter((k) => !env[k]?.trim());
  if (missing.length > 0) {
    throw new ConfigError(`Missing LLM configuration: ${missing.join(", ")}`, [...missing]);
  }
  return {
    provider: "openai-compatible",
    baseURL: (env.LLM_BASE_URL ?? "").trim(),
    apiKey: (env.LLM_API_KEY ?? "").trim(),
    model: (env.LLM_MODEL ?? "").trim(),
    ...(price ? { price } : {}),
  };
}

export function loadTTSConfig(env: Env): TTSConfig {
  const provider = env.TTS_PROVIDER?.trim() || "none";
  if (provider === "none") return { provider: "none" };
  if (provider === "fake") {
    return {
      provider: "fake",
      voice: env.TTS_VOICE?.trim() || "fake-voice",
      model: env.TTS_MODEL?.trim() || "fake-tts",
    };
  }
  throw new ConfigError(`Unknown TTS_PROVIDER: ${provider}`);
}

/** Missing LLM variables without throwing (for status displays). */
export function llmConfigStatus(env: Env): {
  provider: string;
  host: string | null;
  model: string | null;
  missing: string[];
} {
  const provider = env.LLM_PROVIDER?.trim() || "openai-compatible";
  let host: string | null = null;
  try {
    host = env.LLM_BASE_URL ? new URL(env.LLM_BASE_URL).host : null;
  } catch {
    host = null;
  }
  const missing =
    provider === "fake"
      ? []
      : (["LLM_BASE_URL", "LLM_API_KEY", "LLM_MODEL"] as const).filter((k) => !env[k]?.trim());
  return { provider, host, model: env.LLM_MODEL?.trim() || null, missing: [...missing] };
}

export function loadProviderConfig(env: Env): ProviderConfig {
  return {
    llm: loadLLMConfig(env),
    tts: loadTTSConfig(env),
    blobDir: env.BLOB_DIR?.trim() || "./data/blobs",
  };
}
