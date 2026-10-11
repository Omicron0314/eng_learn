import { z } from "zod";

/**
 * Provider configuration stored in the database (settings page), as opposed to
 * environment variables. Section absent = fall back to environment variables.
 * API keys live here and must never be returned to API clients.
 */
const httpUrl = z.url({ protocol: /^https?$/ });
const secret = z.string().trim().min(1).max(300);
const name = z.string().trim().min(1).max(200);

export const StoredLLMConfigSchema = z.discriminatedUnion("provider", [
  z.object({
    provider: z.literal("openai-compatible"),
    baseURL: httpUrl,
    apiKey: secret,
    model: name,
  }),
  z.object({ provider: z.literal("fake"), model: name.default("fake-llm") }),
]);

export const StoredTTSConfigSchema = z.discriminatedUnion("provider", [
  z.object({ provider: z.literal("none") }),
  z.object({
    provider: z.literal("fake"),
    model: name.default("fake-tts"),
    voice: name.default("fake-voice"),
  }),
  z.object({
    provider: z.literal("openai-compatible"),
    baseURL: httpUrl,
    apiKey: secret,
    model: name,
    voice: name,
  }),
]);

export const StoredProviderConfigSchema = z.object({
  llm: StoredLLMConfigSchema.optional(),
  tts: StoredTTSConfigSchema.optional(),
});

export type StoredLLMConfig = z.infer<typeof StoredLLMConfigSchema>;
export type StoredTTSConfig = z.infer<typeof StoredTTSConfigSchema>;
export type StoredProviderConfig = z.infer<typeof StoredProviderConfigSchema>;
