import OpenAI from "openai";
import { ProviderError } from "../errors";
import type { TTSProvider, TTSResult } from "./types";

/** Keep requests under common compatible endpoint limits; MP3 frames concatenate directly. */
export function splitSpeechText(text: string, maxChars = 4000): string[] {
  const chunks: string[] = [];
  let remaining = text;
  while (remaining.length > maxChars) {
    const prefix = remaining.slice(0, maxChars);
    const boundary = Math.max(prefix.lastIndexOf("\n"), prefix.lastIndexOf(" "));
    let end = boundary > maxChars / 2 ? boundary + 1 : maxChars;
    if (/^[\uDC00-\uDFFF]$/.test(remaining[end] ?? "")) end--;
    chunks.push(remaining.slice(0, end));
    remaining = remaining.slice(end);
  }
  if (remaining) chunks.push(remaining);
  return chunks;
}
export class OpenAICompatibleTTS implements TTSProvider {
  readonly provider = "openai-compatible";
  readonly model: string;
  readonly voice: string;
  private readonly client: OpenAI;
  constructor(opts: {
    baseURL: string;
    apiKey: string;
    model: string;
    voice: string;
    fetch?: typeof fetch;
    timeoutMs?: number;
  }) {
    this.model = opts.model;
    this.voice = opts.voice;
    this.client = new OpenAI({
      baseURL: opts.baseURL,
      apiKey: opts.apiKey,
      timeout: opts.timeoutMs ?? 120_000,
      maxRetries: 0,
      ...(opts.fetch ? { fetch: opts.fetch } : {}),
    });
  }
  async synthesize(req: { text: string }): Promise<TTSResult> {
    try {
      const chunks: Uint8Array[] = [];
      let mimeType = "audio/mpeg";
      for (const input of splitSpeechText(req.text)) {
        const response = await this.client.audio.speech.create({
          model: this.model,
          voice: this.voice,
          input,
          response_format: "mp3",
        });
        const contentType = response.headers.get("content-type")?.split(";")[0]?.trim();
        if (contentType === "audio/wav" || contentType === "audio/x-wav") mimeType = "audio/wav";
        chunks.push(new Uint8Array(await response.arrayBuffer()));
      }
      const audio = new Uint8Array(chunks.reduce((n, c) => n + c.length, 0));
      let offset = 0;
      for (const chunk of chunks) {
        audio.set(chunk, offset);
        offset += chunk.length;
      }
      return { audio, mimeType, durationS: null };
    } catch {
      throw new ProviderError("TTS request failed; check provider configuration or retry later");
    }
  }
}
