import { type BlobStore, LocalFileBlob } from "./blob";
import type { LLMConfig, TTSConfig } from "./config";
import { OpenAICompatibleLLM } from "./llm/openai-compatible";
import type { LLMProvider } from "./llm/types";
import { OpenAICompatibleTTS } from "./tts/openai-compatible";
import type { TTSProvider } from "./tts/types";

export function createLLM(cfg: LLMConfig): LLMProvider {
  return new OpenAICompatibleLLM({ baseURL: cfg.baseURL, apiKey: cfg.apiKey, model: cfg.model });
}

export function createTTS(cfg: TTSConfig): TTSProvider | null {
  if (cfg.provider === "none") return null;
  return new OpenAICompatibleTTS(cfg);
}

export function createBlobStore(dir: string): BlobStore {
  return new LocalFileBlob(dir);
}
