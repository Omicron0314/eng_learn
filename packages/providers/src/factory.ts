import { type BlobStore, LocalFileBlob } from "./blob";
import type { LLMConfig, TTSConfig } from "./config";
import { demoResponder, FakeLLM } from "./llm/fake";
import { OpenAICompatibleLLM } from "./llm/openai-compatible";
import type { LLMProvider } from "./llm/types";
import { FakeTTS } from "./tts/fake";
import { OpenAICompatibleTTS } from "./tts/openai-compatible";
import type { TTSProvider } from "./tts/types";

export function createLLM(cfg: LLMConfig): LLMProvider {
  if (cfg.provider === "fake") return new FakeLLM({ responder: demoResponder, model: cfg.model });
  return new OpenAICompatibleLLM({ baseURL: cfg.baseURL, apiKey: cfg.apiKey, model: cfg.model });
}

export function createTTS(cfg: TTSConfig): TTSProvider | null {
  if (cfg.provider === "none") return null;
  if (cfg.provider === "openai-compatible") return new OpenAICompatibleTTS(cfg);
  return new FakeTTS(cfg.voice, cfg.model);
}

export function createBlobStore(dir: string): BlobStore {
  return new LocalFileBlob(dir);
}
