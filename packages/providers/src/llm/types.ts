export type LLMPurpose = "article" | "gloss" | "definition" | "eval";
export type LLMMessage = { role: "system" | "user" | "assistant"; content: string };

export type LLMRequest = {
  purpose: LLMPurpose;
  messages: LLMMessage[];
  jsonMode?: boolean;
  temperature?: number;
  maxTokens?: number;
};

export type LLMResponse = {
  text: string;
  model: string;
  tokensIn: number;
  tokensOut: number;
  latencyMs: number;
};

export interface LLMProvider {
  readonly provider: string;
  readonly model: string;
  complete(req: LLMRequest): Promise<LLMResponse>;
}

export type LLMCallRecord = {
  purpose: LLMPurpose;
  provider: string;
  model: string;
  tokensIn: number;
  tokensOut: number;
  costUsd: number | null;
  latencyMs: number;
  status: "ok" | "invalid_output" | "error";
  error?: string;
};
