import { type CardOrigin, newSrsCard, normalizeHeadword } from "@eng-learn/core";
import type { Repos } from "@eng-learn/db";
import { z } from "zod";
import type { AppDeps } from "../deps";
import { contextHash, GlossSchema } from "./gloss";
import { nonempty, optionalArticle, validate } from "./shared";

export type MarkInput = { lemma: string; sentence: string; articleId?: string | undefined };
export const MarkInputSchema = z.object({
  lemma: nonempty,
  sentence: z.string().max(10_000),
  articleId: nonempty.optional(),
});
function checked(deps: AppDeps, input: MarkInput) {
  const value = validate(MarkInputSchema, input);
  optionalArticle(deps, value.articleId);
  return { ...value, lemma: normalizeHeadword(value.lemma) };
}
export function addReviewInTx(
  deps: AppDeps,
  repos: Repos,
  input: MarkInput,
  from: Exclude<CardOrigin, "import">,
) {
  const now = deps.clock();
  const lexemeId = repos.lexemes.upsert(input.lemma, "", now.getTime());
  const existing = repos.cards.getByLexeme(deps.userId, lexemeId);
  const cached = repos.glossCache.get(lexemeId, contextHash(input.sentence));
  const gloss = cached ? GlossSchema.parse(cached) : undefined;
  const cardId =
    existing?.id ??
    repos.cards.insert({
      userId: deps.userId,
      lexemeId,
      front: input.lemma,
      back: {
        meaningSource: "ai",
        ...(gloss ? { meaning: gloss.meaningZh, pos: gloss.pos } : {}),
        ...(input.sentence ? { example: input.sentence } : {}),
      },
      deck: "阅读生词",
      origin: from,
      srs: newSrsCard(now),
      now: now.getTime(),
    });
  if (existing?.suspended) repos.cards.setSuspended(existing.id, false, now.getTime());
  const pending = repos.inbox.findPending(deps.userId, lexemeId);
  if (pending) repos.inbox.setStatus([pending.id], "added", now.getTime());
  repos.events.append(
    deps.userId,
    "add_to_review",
    { lexemeId, cardId, from, articleId: input.articleId },
    now.getTime(),
  );
  return { cardId, created: !existing };
}
export function markKnown(deps: AppDeps, input: MarkInput) {
  input = checked(deps, input);
  deps.tx((repos) => {
    const now = deps.clock().getTime();
    const lexemeId = repos.lexemes.upsert(input.lemma, "", now);
    repos.marks.upsert(deps.userId, lexemeId, "known", now);
    repos.events.append(deps.userId, "mark_known", { lexemeId, articleId: input.articleId }, now);
  });
}
export function markUnknown(
  deps: AppDeps,
  input: MarkInput,
): { inboxId?: string; cardId?: string } {
  input = checked(deps, input);
  return deps.tx((repos) => {
    const now = deps.clock().getTime();
    const lexemeId = repos.lexemes.upsert(input.lemma, "", now);
    repos.marks.upsert(deps.userId, lexemeId, "unknown", now);
    if (deps.settings().lookupMode === "autoAdd") {
      const result = addReviewInTx(deps, repos, input, "autoAdd");
      repos.events.append(
        deps.userId,
        "mark_unknown",
        { lexemeId, articleId: input.articleId, cardId: result.cardId },
        now,
      );
      return { cardId: result.cardId };
    }
    const inboxId =
      repos.inbox.findPending(deps.userId, lexemeId)?.id ??
      repos.inbox.insert({
        userId: deps.userId,
        lexemeId,
        contextSentence: input.sentence,
        ...(input.articleId ? { articleId: input.articleId } : {}),
        now,
      });
    repos.events.append(
      deps.userId,
      "mark_unknown",
      { lexemeId, articleId: input.articleId, inboxId },
      now,
    );
    return { inboxId };
  });
}
export function addToReview(deps: AppDeps, input: MarkInput) {
  input = checked(deps, input);
  return deps.tx((repos) => addReviewInTx(deps, repos, input, "lookup"));
}
export const markService = { markKnown, markUnknown, addToReview };
