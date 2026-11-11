import type { LLMProvider, LLMRequest, LLMResponse } from "../llm/types";

export type FakeReply = string | Error | ((req: LLMRequest) => string);

/**
 * Test double. Replies come from the script in order; when the script is exhausted
 * the responder is used (if any), otherwise it throws.
 */
export class FakeLLM implements LLMProvider {
  readonly provider = "fake";
  readonly model: string;
  readonly requests: LLMRequest[] = [];
  private readonly script: FakeReply[];
  private readonly responder: ((req: LLMRequest) => string) | undefined;

  constructor(
    opts: { script?: FakeReply[]; responder?: (req: LLMRequest) => string; model?: string } = {},
  ) {
    this.script = [...(opts.script ?? [])];
    this.responder = opts.responder;
    this.model = opts.model ?? "fake-llm";
  }

  get calls(): number {
    return this.requests.length;
  }

  push(...replies: FakeReply[]): void {
    this.script.push(...replies);
  }

  async complete(req: LLMRequest): Promise<LLMResponse> {
    this.requests.push(structuredClone(req));
    const next = this.script.shift();
    let text: string;
    if (next === undefined) {
      if (!this.responder) throw new Error("FakeLLM: no scripted reply left");
      text = this.responder(req);
    } else if (next instanceof Error) {
      throw next;
    } else {
      text = typeof next === "function" ? next(req) : next;
    }
    const tokensIn = req.messages.reduce((n, m) => n + Math.ceil(m.content.length / 4), 0);
    return {
      text,
      model: this.model,
      tokensIn,
      tokensOut: Math.ceil(text.length / 4),
      latencyMs: 1,
    };
  }
}
