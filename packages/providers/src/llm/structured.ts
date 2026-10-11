import type { z } from "zod";
import type { Price } from "../config";
import { JsonModeUnsupportedError, LLMOutputInvalidError } from "../errors";
import type { LLMCallRecord, LLMMessage, LLMProvider, LLMRequest, LLMResponse } from "./types";

export const DEFAULT_REPAIR_TEMPLATE = `## user
Your previous reply could not be used: {errors}
Reply again with JSON only, matching exactly this shape: {shape}
No markdown fences, no commentary.`;

export function computeCost(tokensIn: number, tokensOut: number, price?: Price): number | null {
  if (!price) return null;
  return (tokensIn / 1e6) * price.inputPerMTok + (tokensOut / 1e6) * price.outputPerMTok;
}

function stripFences(text: string): string {
  const t = text.trim();
  const fenced = t.match(/^```(?:json)?\s*([\s\S]*?)\s*```$/i);
  return fenced ? (fenced[1] ?? "") : t;
}

function parseWith<T>(
  text: string,
  schema: z.ZodType<T>,
): { ok: true; value: T } | { ok: false; error: string } {
  let json: unknown;
  try {
    json = JSON.parse(stripFences(text));
  } catch (e) {
    return { ok: false, error: `invalid JSON (${(e as Error).message})` };
  }
  const r = schema.safeParse(json);
  if (r.success) return { ok: true, value: r.data };
  const issues = r.error.issues
    .slice(0, 5)
    .map((i) => `${i.path.join(".") || "(root)"}: ${i.message}`)
    .join("; ");
  return { ok: false, error: `schema mismatch (${issues})` };
}

function renderRepair(template: string, errors: string, shape: string): LLMMessage[] {
  const body = template
    .replace(/^##\s*user\s*$/im, "")
    .replace("{errors}", errors)
    .replace("{shape}", shape)
    .trim();
  return [{ role: "user", content: body }];
}

export type CompleteJsonOptions = {
  onCall: (r: LLMCallRecord) => void;
  price?: Price;
  /** Human-readable JSON shape included in the repair prompt. */
  shape: string;
  repairTemplate?: string;
};

export async function completeJson<T>(
  llm: LLMProvider,
  req: Omit<LLMRequest, "jsonMode">,
  schema: z.ZodType<T>,
  opts: CompleteJsonOptions,
): Promise<{ value: T; model: string; attempts: number; jsonModeUsed: boolean }> {
  let jsonMode = true;

  const call = async (messages: LLMMessage[]): Promise<LLMResponse> => {
    try {
      return await llm.complete({ ...req, messages, jsonMode });
    } catch (err) {
      if (err instanceof JsonModeUnsupportedError && jsonMode) {
        jsonMode = false;
        return call(messages);
      }
      opts.onCall({
        purpose: req.purpose,
        provider: llm.provider,
        model: llm.model,
        tokensIn: 0,
        tokensOut: 0,
        costUsd: null,
        latencyMs: 0,
        status: "error",
        error: (err as Error).message,
      });
      throw err;
    }
  };

  const record = (res: LLMResponse, status: LLMCallRecord["status"], error?: string) =>
    opts.onCall({
      purpose: req.purpose,
      provider: llm.provider,
      model: res.model,
      tokensIn: res.tokensIn,
      tokensOut: res.tokensOut,
      costUsd: computeCost(res.tokensIn, res.tokensOut, opts.price),
      latencyMs: res.latencyMs,
      status,
      ...(error ? { error } : {}),
    });

  const first = await call(req.messages);
  const firstParsed = parseWith(first.text, schema);
  if (firstParsed.ok) {
    record(first, "ok");
    return { value: firstParsed.value, model: first.model, attempts: 1, jsonModeUsed: jsonMode };
  }
  record(first, "invalid_output", firstParsed.error);

  const repairMessages: LLMMessage[] = [
    ...req.messages,
    { role: "assistant", content: first.text },
    ...renderRepair(opts.repairTemplate ?? DEFAULT_REPAIR_TEMPLATE, firstParsed.error, opts.shape),
  ];
  const second = await call(repairMessages);
  const secondParsed = parseWith(second.text, schema);
  if (secondParsed.ok) {
    record(second, "ok");
    return { value: secondParsed.value, model: second.model, attempts: 2, jsonModeUsed: jsonMode };
  }
  record(second, "invalid_output", secondParsed.error);
  throw new LLMOutputInvalidError(`LLM output invalid after repair: ${secondParsed.error}`);
}
