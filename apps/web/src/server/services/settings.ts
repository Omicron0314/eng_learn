import {
  type Settings,
  SettingsSchema,
  type StoredLLMConfig,
  StoredLLMConfigSchema,
  type StoredProviderConfig,
  type StoredTTSConfig,
  StoredTTSConfigSchema,
} from "@eng-learn/core";
import { AppError } from "@eng-learn/providers";
import { z } from "zod";
import type { AppDeps } from "../deps";
import { getTtsUsage } from "./audio";
import { validate } from "./shared";

/** Sanitized provider config for API clients; never includes API keys. */
export type ProviderConfigView = {
  llm:
    | { provider: "openai-compatible"; baseURL: string; model: string; hasApiKey: boolean }
    | { provider: "fake"; model: string }
    | null;
  tts:
    | { provider: "none" }
    | { provider: "fake"; model: string; voice: string }
    | {
        provider: "openai-compatible";
        baseURL: string;
        model: string;
        voice: string;
        hasApiKey: boolean;
      }
    | null;
};

function providerConfigView(stored: StoredProviderConfig): ProviderConfigView {
  return {
    llm:
      stored.llm?.provider === "openai-compatible"
        ? {
            provider: "openai-compatible",
            baseURL: stored.llm.baseURL,
            model: stored.llm.model,
            hasApiKey: true,
          }
        : stored.llm
          ? { provider: "fake", model: stored.llm.model }
          : null,
    tts:
      stored.tts?.provider === "openai-compatible"
        ? {
            provider: "openai-compatible",
            baseURL: stored.tts.baseURL,
            model: stored.tts.model,
            voice: stored.tts.voice,
            hasApiKey: true,
          }
        : stored.tts?.provider === "fake"
          ? { provider: "fake", model: stored.tts.model, voice: stored.tts.voice }
          : stored.tts
            ? { provider: "none" }
            : null,
  };
}

export function getSettingsView(deps: AppDeps) {
  return {
    settings: deps.settings(),
    providers: deps.providerStatus(),
    providerConfig: providerConfigView(deps.providerConfig()),
    ttsUsage: getTtsUsage(deps),
  };
}

export function updateSettings(deps: AppDeps, patch: Partial<Settings>) {
  validate(
    z
      .record(z.string(), z.unknown())
      .refine(
        (value) => Object.keys(value).every((key) => Object.hasOwn(SettingsSchema.shape, key)),
        "未知设置项",
      ),
    patch,
  );
  const before = deps.settings();
  const next = validate(SettingsSchema, { ...before, ...patch });
  const changed = (Object.keys(before) as (keyof Settings)[]).filter(
    (key) => before[key] !== next[key],
  );
  if (changed.length)
    deps.tx((repos) => {
      const now = deps.clock().getTime();
      repos.settings.save(deps.userId, next, now);
      repos.events.append(deps.userId, "settings_update", { changed }, now);
    });
  return next;
}

const httpUrl = z.url({ protocol: /^https?$/ });
const optionalSecret = z
  .string()
  .trim()
  .max(300)
  .optional()
  .transform((v) => (v ? v : undefined));
const optionalName = z
  .string()
  .trim()
  .max(200)
  .optional()
  .transform((v) => (v ? v : undefined));

const llmInputSchema = z.discriminatedUnion("provider", [
  z
    .object({
      provider: z.literal("openai-compatible"),
      baseURL: httpUrl,
      model: z.string().trim().min(1).max(200),
      apiKey: optionalSecret,
    })
    .strict(),
  z.object({ provider: z.literal("fake"), model: optionalName }).strict(),
]);
const ttsInputSchema = z.discriminatedUnion("provider", [
  z.object({ provider: z.literal("none") }).strict(),
  z.object({ provider: z.literal("fake"), model: optionalName, voice: optionalName }).strict(),
  z
    .object({
      provider: z.literal("openai-compatible"),
      baseURL: httpUrl,
      model: z.string().trim().min(1).max(200),
      voice: z.string().trim().min(1).max(200),
      apiKey: optionalSecret,
    })
    .strict(),
]);
const providerPatchSchema = z
  .object({ llm: llmInputSchema.optional(), tts: ttsInputSchema.optional() })
  .strict()
  .refine((v) => v.llm !== undefined || v.tts !== undefined, "没有需要保存的 provider 配置");

/** An empty apiKey input keeps the stored key for the same provider type. */
function mergeLlm(
  input: z.infer<typeof llmInputSchema>,
  before?: StoredLLMConfig,
): StoredLLMConfig {
  if (input.provider === "fake")
    return StoredLLMConfigSchema.parse({
      provider: "fake",
      ...(input.model ? { model: input.model } : {}),
    });
  const apiKey =
    input.apiKey ?? (before?.provider === "openai-compatible" ? before.apiKey : undefined);
  if (!apiKey) throw new AppError("VALIDATION_FAILED", "请先填写 LLM API 密钥", 400);
  return StoredLLMConfigSchema.parse({
    provider: "openai-compatible",
    baseURL: input.baseURL,
    apiKey,
    model: input.model,
  });
}
function mergeTts(
  input: z.infer<typeof ttsInputSchema>,
  before?: StoredTTSConfig,
): StoredTTSConfig {
  if (input.provider === "none") return { provider: "none" };
  if (input.provider === "fake")
    return StoredTTSConfigSchema.parse({
      provider: "fake",
      ...(input.model ? { model: input.model } : {}),
      ...(input.voice ? { voice: input.voice } : {}),
    });
  const apiKey =
    input.apiKey ?? (before?.provider === "openai-compatible" ? before.apiKey : undefined);
  if (!apiKey) throw new AppError("VALIDATION_FAILED", "请先填写 TTS API 密钥", 400);
  return StoredTTSConfigSchema.parse({
    provider: "openai-compatible",
    baseURL: input.baseURL,
    apiKey,
    model: input.model,
    voice: input.voice,
  });
}

export function updateProviderConfig(deps: AppDeps, patch: unknown) {
  const input = validate(providerPatchSchema, patch);
  const before = deps.providerConfig();
  const next: StoredProviderConfig = { ...before };
  const changed: string[] = [];
  if (input.llm) {
    const merged = mergeLlm(input.llm, before.llm);
    if (JSON.stringify(merged) !== JSON.stringify(before.llm)) {
      next.llm = merged;
      changed.push("provider.llm");
    }
  }
  if (input.tts) {
    const merged = mergeTts(input.tts, before.tts);
    if (JSON.stringify(merged) !== JSON.stringify(before.tts)) {
      next.tts = merged;
      changed.push("provider.tts");
    }
  }
  if (changed.length)
    deps.tx((repos) => {
      const now = deps.clock().getTime();
      repos.providerConfig.save(deps.userId, next, now);
      // Key names only: secret values must never enter the append-only event log.
      repos.events.append(deps.userId, "settings_update", { changed }, now);
    });
  return { providers: deps.providerStatus(), providerConfig: providerConfigView(next) };
}

export const settingsService = { getSettingsView, update: updateSettings, updateProviderConfig };
