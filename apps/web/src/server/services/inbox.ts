import { AppError } from "@eng-learn/providers";
import { z } from "zod";
import type { AppDeps } from "../deps";
import { contextHash, GlossSchema } from "./gloss";
import { addReviewInTx } from "./mark";
import { nonempty, validate } from "./shared";

export function listInbox(deps: AppDeps) {
  return deps.repos.inbox.listPending(deps.userId).map(({ item, lemma }) => {
    const cached = deps.repos.glossCache.get(item.lexemeId, contextHash(item.contextSentence));
    return {
      id: item.id,
      lemma,
      sentence: item.contextSentence,
      articleId: item.articleId,
      createdAt: item.createdAt,
      ...(cached ? { gloss: GlossSchema.parse(cached) } : {}),
    };
  });
}
export function batchInbox(deps: AppDeps, input: { action: "add" | "ignore"; ids: string[] }) {
  input = validate(
    z.object({
      action: z.enum(["add", "ignore"]),
      ids: z
        .array(nonempty)
        .min(1)
        .max(1000)
        .refine((ids) => new Set(ids).size === ids.length, "重复的条目"),
    }),
    input,
  );
  return deps.tx((repos) => {
    const rows = repos.inbox.getMany(input.ids);
    if (
      rows.length !== input.ids.length ||
      rows.some((r) => r.userId !== deps.userId || r.status !== "pending")
    )
      throw new AppError("VALIDATION_FAILED", "请选择仍待处理的条目", 400);
    let cardsCreated = 0;
    const now = deps.clock().getTime();
    for (const row of rows) {
      if (input.action === "ignore") {
        repos.inbox.setStatus([row.id], "ignored", now);
        repos.events.append(
          deps.userId,
          "inbox_ignore",
          { inboxId: row.id, lexemeId: row.lexemeId },
          now,
        );
      } else {
        const lemma = repos.lexemes.get(row.lexemeId)?.lemma;
        if (!lemma) throw new AppError("NOT_FOUND", "词元不存在", 404);
        if (
          addReviewInTx(
            deps,
            repos,
            {
              lemma,
              sentence: row.contextSentence,
              ...(row.articleId ? { articleId: row.articleId } : {}),
            },
            "inbox",
          ).created
        )
          cardsCreated++;
      }
    }
    return { processed: rows.length, cardsCreated };
  });
}
export const inboxService = { listInbox, batchInbox };
