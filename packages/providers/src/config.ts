import type { StoredLLMConfig, StoredTTSConfig } from "@eng-learn/core";
import { z } from "zod";
import { ConfigError } from "./errors";

function validateEndpoint(value: string | undefined, key: string) {
  const parsed = z.url({ protocol: /^https?$/ }).safeParse(value?.trim());
  if (!parsed.success) throw new ConfigError(`${key} must be an HTTP(S) URL`);
}

export type Price = { inputPerMTok: number; outputPerMTok: number };

export type LLMConfig = {
  provider: "openai-compatible";
  baseURL: string;
  apiKey: string;
  model: string;
  price?: Price;
};

export type TTSConfig =
  | { provider: "none" }
  | {
      provider: "openai-compatible";
      baseURL: string;
      apiKey: string;
      voice: string;
      model: string;
    };

export type ProviderConfig = {
  llm: LLMConfig;
  tts: TTSConfig;
  blobDir: string;
};

type Env = Record<string, string | undefined>;

function readPriceInternal(env: Env): Price | undefined {
  const i = env.LLM_PRICE_INPUT_PER_MTOK;
  const o = env.LLM_PRICE_OUTPUT_PER_MTOK;
  if (!i || !o) return undefined;
  const inputPerMTok = Number(i);
  const outputPerMTok = Number(o);
  if (
    !Number.isFinite(inputPerMTok) ||
    !Number.isFinite(outputPerMTok) ||
    inputPerMTok < 0 ||
    outputPerMTok < 0
  ) {
    throw new ConfigError("LLM_PRICE_INPUT_PER_MTOK / LLM_PRICE_OUTPUT_PER_MTOK must be numbers");
  }
  return { inputPerMTok, outputPerMTok };
}

export function loadLLMConfig(env: Env): LLMConfig {
  const provider = env.LLM_PROVIDER?.trim() || "openai-compatible";
  const price = readPriceInternal(env);
  if (provider !== "openai-compatible") {
    throw new ConfigError(`Unknown LLM_PROVIDER: ${provider}`);
  }
  const required = ["LLM_BASE_URL", "LLM_API_KEY", "LLM_MODEL"] as const;
  const missing = required.filter((k) => !env[k]?.trim());
  if (missing.length > 0) {
    throw new ConfigError(`Missing LLM configuration: ${missing.join(", ")}`, [...missing]);
  }
  validateEndpoint(env.LLM_BASE_URL, "LLM_BASE_URL");
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
  if (provider === "openai-compatible") {
    const required = ["TTS_BASE_URL", "TTS_API_KEY", "TTS_MODEL", "TTS_VOICE"] as const;
    const missing = required.filter((k) => !env[k]?.trim());
    if (missing.length)
      throw new ConfigError(`Missing TTS configuration: ${missing.join(", ")}`, [...missing]);
    validateEndpoint(env.TTS_BASE_URL, "TTS_BASE_URL");
    return {
      provider,
      baseURL: (env.TTS_BASE_URL ?? "").trim(),
      apiKey: (env.TTS_API_KEY ?? "").trim(),
      model: (env.TTS_MODEL ?? "").trim(),
      voice: (env.TTS_VOICE ?? "").trim(),
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
  const missing = (["LLM_BASE_URL", "LLM_API_KEY", "LLM_MODEL"] as const).filter(
    (k) => !env[k]?.trim(),
  );
  return { provider, host, model: env.LLM_MODEL?.trim() || null, missing: [...missing] };
}

export function loadProviderConfig(env: Env): ProviderConfig {
  return {
    llm: loadLLMConfig(env),
    tts: loadTTSConfig(env),
    blobDir: env.BLOB_DIR?.trim() || "./data/blobs",
  };
}

// --- Database-stored (settings page) configuration ---------------------------

export type ProviderSource = "settings" | "env";

function hostOf(url: string | undefined): string | null {
  if (!url) return null;
  try {
    return new URL(url).host;
  } catch {
    return null;
  }
}

const LLM_ENV_KEYS = ["LLM_PROVIDER", "LLM_BASE_URL", "LLM_API_KEY", "LLM_MODEL"] as const;
/** True when any LLM environment variable is set (even partially). */
export function hasLlmEnv(env: Env): boolean {
  return LLM_ENV_KEYS.some((k) => Boolean(env[k]?.trim()));
}
/** True when a TTS provider is explicitly selected via the environment. */
export function hasTtsEnv(env: Env): boolean {
  return Boolean(env.TTS_PROVIDER?.trim());
}

export function readPrice(env: Env): Price | undefined {
  return readPriceInternal(env);
}

function llmConfigFromStored(stored: StoredLLMConfig, price?: Price): LLMConfig {
  return {
    provider: "openai-compatible",
    baseURL: stored.baseURL,
    apiKey: stored.apiKey,
    model: stored.model,
    ...(price ? { price } : {}),
  };
}

function ttsConfigFromStored(stored: StoredTTSConfig): TTSConfig {
  if (stored.provider === "none") return { provider: "none" };
  return {
    provider: "openai-compatible",
    baseURL: stored.baseURL,
    apiKey: stored.apiKey,
    model: stored.model,
    voice: stored.voice,
  };
}

/**
 * Effective LLM config: settings-page (database) values take precedence over the
 * environment. Throws ConfigError with a user-facing message when neither exists.
 */
export function resolveLLMConfig(env: Env, stored?: StoredLLMConfig): LLMConfig {
  if (stored) return llmConfigFromStored(stored, readPrice(env));
  if (!hasLlmEnv(env)) throw new ConfigError("未配置 LLM：请在设置页填写服务商端点、密钥与模型");
  return loadLLMConfig(env);
}

/** Effective TTS config; defaults to browser speech when nothing is configured. */
export function resolveTTSConfig(env: Env, stored?: StoredTTSConfig): TTSConfig {
  if (stored) return ttsConfigFromStored(stored);
  return loadTTSConfig(env);
}

export type LLMStatus = {
  configured: boolean;
  source: ProviderSource | null;
  provider: string;
  host: string | null;
  model: string | null;
  missing: string[];
};

export function llmStatus(env: Env, stored?: StoredLLMConfig): LLMStatus {
  if (stored) {
    return {
      configured: true,
      source: "settings",
      provider: stored.provider,
      host: hostOf(stored.baseURL),
      model: stored.model,
      missing: [],
    };
  }
  if (!hasLlmEnv(env)) {
    return {
      configured: false,
      source: null,
      provider: "openai-compatible",
      host: null,
      model: null,
      missing: ["端点", "密钥", "模型"],
    };
  }
  const s = llmConfigStatus(env);
  return {
    configured: s.missing.length === 0 && s.host !== null,
    source: "env",
    provider: s.provider,
    host: s.host,
    model: s.model,
    missing: s.missing,
  };
}

export type TTSStatus = { source: ProviderSource | null; provider: string };

export function ttsStatus(env: Env, stored?: StoredTTSConfig): TTSStatus {
  if (stored) return { source: "settings", provider: stored.provider };
  if (hasTtsEnv(env)) {
    try {
      return { source: "env", provider: loadTTSConfig(env).provider };
    } catch {
      return { source: "env", provider: "配置无效" };
    }
  }
  return { source: null, provider: "none" };
}
