import { type Difficulty, splitSentences } from "@eng-learn/core";
import { AppError } from "@eng-learn/providers";
import { z } from "zod";
import type { AppDeps } from "../deps";
import { nonempty, requireArticle, validate } from "./shared";

export function submitDifficulty(
  deps: AppDeps,
  input: { articleId: string; value: Difficulty; after: "reading" | "listening" },
) {
  input = validate(
    z.object({
      articleId: nonempty,
      value: z.enum(["too_hard", "just_right", "too_easy"]),
      after: z.enum(["reading", "listening"]),
    }),
    input,
  );
  requireArticle(deps, input.articleId);
  deps.tx((repos) =>
    repos.events.append(deps.userId, "difficulty_feedback", input, deps.clock().getTime()),
  );
}
export function flagUnnatural(
  deps: AppDeps,
  input: { articleId: string; sentenceIndexes: number[]; note?: string | undefined },
) {
  input = validate(
    z.object({
      articleId: nonempty,
      sentenceIndexes: z.array(z.number().int().min(0)).max(1000),
      note: z.string().max(200).optional(),
    }),
    input,
  );
  const article = requireArticle(deps, input.articleId);
  const total = splitSentences(article.body).length;
  const indexes = [...new Set(input.sentenceIndexes)];
  if (indexes.some((i) => i >= total)) throw new AppError("VALIDATION_FAILED", "句子序号越界", 400);
  return deps.tx((repos) => {
    const now = deps.clock().getTime();
    for (const index of indexes.length ? indexes : [undefined])
      repos.events.append(
        deps.userId,
        "unnatural_flag",
        { articleId: input.articleId, sentenceIndex: index, note: input.note },
        now,
      );
    return { events: indexes.length || 1 };
  });
}
export function latestDifficulty(deps: AppDeps, articleId: string) {
  requireArticle(deps, articleId);
  const latest: Partial<Record<"reading" | "listening", Difficulty>> = {};
  for (const e of deps.repos.events.listByType(deps.userId, "difficulty_feedback"))
    if (e.payload.articleId === articleId) latest[e.payload.after] = e.payload.value;
  return latest;
}
export const feedbackService = { submitDifficulty, flagUnnatural, latestDifficulty };
