import type { CardRow } from "@eng-learn/db";
import {
  AppError,
  completeJson,
  type LLMMessage,
  type LLMPurpose,
  loadPrompt,
  ProviderError,
} from "@eng-learn/providers";
import { z } from "zod";
import type { AppDeps } from "../deps";

export function validate<T>(schema: z.ZodType<T>, input: unknown): T {
  const parsed = schema.safeParse(input);
  if (!parsed.success)
    throw new AppError(
      "VALIDATION_FAILED",
      parsed.error.issues.map((i) => `${i.path.join(".")}: ${i.message}`).join("; "),
      400,
    );
  return parsed.data;
}
export const nonempty = z.string().trim().min(1).max(1000);
export function requireCard(deps: AppDeps, id: string, repos = deps.repos): CardRow {
  const card = repos.cards.get(id);
  if (!card || card.userId !== deps.userId) throw new AppError("NOT_FOUND", "词卡不存在", 404);
  return card;
}
export function requireArticle(deps: AppDeps, id: string) {
  const article = deps.repos.articles.get(id);
  if (!article || article.userId !== deps.userId)
    throw new AppError("NOT_FOUND", "文章不存在", 404);
  return article;
}
export function optionalArticle(deps: AppDeps, id?: string) {
  if (id) requireArticle(deps, id);
}
export async function jsonCompletion<T>(
  deps: AppDeps,
  purpose: LLMPurpose,
  messages: LLMMessage[],
  schema: z.ZodType<T>,
  shape: string,
  sanitizeRepairContext?: (text: string) => string,
) {
  try {
    return await completeJson(deps.llm, { purpose, messages }, schema, {
      shape,
      ...(sanitizeRepairContext ? { sanitizeRepairContext } : {}),
      ...(deps.price ? { price: deps.price } : {}),
      repairTemplate: loadPrompt("json-repair.v1"),
      onCall: (record) =>
        deps.repos.llmCalls.insert({ ...record, userId: deps.userId, ts: deps.clock().getTime() }),
    });
  } catch (error) {
    if (error instanceof AppError) throw error;
    // Never return provider exceptions or credentials to API clients.
    throw new ProviderError("AI 服务调用失败，请稍后重试");
  }
}

/** Per-dependency in-flight deduplication; rejected promises are always evicted. */
const pending = new WeakMap<AppDeps, Map<string, Promise<unknown>>>();
export function singleFlight<T>(deps: AppDeps, key: string, fn: () => Promise<T>): Promise<T> {
  let map = pending.get(deps);
  if (!map) {
    map = new Map();
    pending.set(deps, map);
  }
  const previous = map.get(key);
  if (previous) return previous as Promise<T>;
  const result = fn().finally(() => map?.delete(key));
  map.set(key, result);
  return result;
}
