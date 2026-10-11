import type { LLMProvider, LLMRequest, LLMResponse } from "./types";

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

const FILLERS = [
  "Lucy walked to school with her friend Tom on a cold morning.",
  "They talked about their plans for the weekend and laughed a lot.",
  "The sky was grey, but the two friends felt happy and warm.",
  "After class, they went to a small shop near the park to buy some bread.",
  "Tom said that he wanted to learn more about the world.",
  "Lucy smiled and told him that every day is a chance to try something new.",
];

function articleBody(targets: string[], genre: string): string {
  const lines: string[] = [];
  const words = targets.length > 0 ? targets : ["friend"];
  words.forEach((w, i) => {
    const filler = FILLERS[i % FILLERS.length] ?? "";
    if (genre.includes("dialogue")) {
      lines.push(`Tom: Do you know the word ${w}? I read the word ${w} in a book today.`);
      lines.push(`Lucy: Yes, I know it. ${filler}`);
    } else {
      lines.push(
        `${filler} Then Lucy thought about the word ${w}. She used ${w} in a short note to her mother.`,
      );
    }
  });
  lines.push("In the end, they went home and had a good dinner together.");
  return lines.join(genre.includes("dialogue") ? "\n" : " ");
}

/** Deterministic responder used when LLM_PROVIDER=fake (local demo and e2e tests). */
export function demoResponder(req: LLMRequest): string {
  const firstUser = req.messages.find((m) => m.role === "user")?.content ?? "";
  if (req.purpose === "article") {
    const targets = (firstUser.match(/Target words:\s*(.*)/)?.[1] ?? "")
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);
    const genre = firstUser.match(/Write an? (.+?) of \d+/)?.[1] ?? "story";
    return JSON.stringify({
      title: `A Day with ${targets[0] ?? "Friends"}`,
      body: articleBody(targets, genre),
      names: ["Lucy", "Tom"],
    });
  }
  if (req.purpose === "gloss") {
    const lemma = firstUser.match(/base form:\s*([^)]+)\)/)?.[1]?.trim() ?? "word";
    return JSON.stringify({
      lemma,
      pos: "n.",
      meaningZh: `${lemma} 的语境释义(演示)`,
      meaningEn: `meaning of ${lemma}`,
      ipa: `/${lemma}/`,
    });
  }
  if (req.purpose === "definition") {
    const word = firstUser.match(/Word:\s*(.+)/)?.[1]?.trim() ?? "word";
    return JSON.stringify({ meaningZh: `${word}(演示释义)`, pos: "" });
  }
  return "{}";
}
