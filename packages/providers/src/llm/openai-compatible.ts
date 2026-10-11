import OpenAI from "openai";
import { JsonModeUnsupportedError, ProviderError } from "../errors";
import type { LLMProvider, LLMRequest, LLMResponse } from "./types";

export type OpenAICompatibleOptions = {
  baseURL: string;
  apiKey: string;
  model: string;
  fetch?: typeof fetch;
  timeoutMs?: number;
};

export class OpenAICompatibleLLM implements LLMProvider {
  readonly provider = "openai-compatible";
  readonly model: string;
  private readonly client: OpenAI;

  constructor(opts: OpenAICompatibleOptions) {
    this.model = opts.model;
    this.client = new OpenAI({
      baseURL: opts.baseURL,
      apiKey: opts.apiKey,
      timeout: opts.timeoutMs ?? 120_000,
      maxRetries: 0,
      ...(opts.fetch ? { fetch: opts.fetch } : {}),
    });
  }

  async complete(req: LLMRequest): Promise<LLMResponse> {
    const started = Date.now();
    try {
      const res = await this.client.chat.completions.create({
        model: this.model,
        messages: req.messages,
        ...(req.jsonMode ? { response_format: { type: "json_object" as const } } : {}),
        ...(req.temperature !== undefined ? { temperature: req.temperature } : {}),
        ...(req.maxTokens !== undefined ? { max_tokens: req.maxTokens } : {}),
      });
      return {
        text: res.choices[0]?.message?.content ?? "",
        model: res.model || this.model,
        tokensIn: res.usage?.prompt_tokens ?? 0,
        tokensOut: res.usage?.completion_tokens ?? 0,
        latencyMs: Date.now() - started,
      };
    } catch (err) {
      if (err instanceof OpenAI.APIError) {
        if (req.jsonMode && err.status === 400 && /response_format|json/i.test(err.message)) {
          throw new JsonModeUnsupportedError(err.message);
        }
        throw new ProviderError(
          `LLM request failed (${err.status ?? "network"}); check provider configuration or retry later`,
        );
      }
      throw new ProviderError("LLM request failed; check provider configuration or retry later");
    }
  }
}
