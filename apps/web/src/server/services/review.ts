import { buildQueue, createSrs, dayRange, RATINGS, type Rating } from "@eng-learn/core";
import { AppError } from "@eng-learn/providers";
import { z } from "zod";
import type { AppDeps } from "../deps";
import { nonempty, requireCard, validate } from "./shared";

export function getQueue(deps: AppDeps) {
  const now = deps.clock();
  const settings = deps.settings();
  const range = dayRange(now, settings);
  const cards = deps.repos.cards.listAll(deps.userId);
  return buildQueue({
    cards: cards.map((c) => ({
      id: c.id,
      card: c.srs,
      origin: c.origin,
      suspended: c.suspended,
      createdAt: new Date(c.createdAt),
    })),
    now,
    dailyNewLimit: settings.dailyNewLimit,
    dailyReviewLimit: settings.dailyReviewLimit,
    newIntroducedToday: deps.repos.reviewLogs.countNewIntroduced(deps.userId, range),
    reviewsDoneToday: deps.repos.reviewLogs.countReviews(deps.userId, range, "review"),
  });
}
export function getReviewCard(deps: AppDeps, cardId: string) {
  const card = requireCard(deps, cardId);
  return { ...card, preview: createSrs(deps.settings()).preview(card.srs, deps.clock()) };
}
export function rate(deps: AppDeps, input: { cardId: string; rating: Rating; durationMs: number }) {
  input = validate(
    z.object({
      cardId: nonempty,
      rating: z.enum(RATINGS),
      durationMs: z.number().int().min(0).max(86_400_000),
    }),
    input,
  );
  const now = deps.clock();
  return deps.tx((repos) => {
    const before = requireCard(deps, input.cardId, repos);
    if (before.suspended) throw new AppError("VALIDATION_FAILED", "暂停的卡片不能评分", 400);
    const queue = getQueue({
      ...deps,
      repos,
      clock: () => now,
      settings: () => repos.settings.get(deps.userId),
    });
    if (![...queue.learning, ...queue.review, ...queue.new].includes(before.id))
      throw new AppError("VALIDATION_FAILED", "卡片尚未到期或今日名额已用完，请刷新复习队列", 400);
    const result = createSrs(deps.settings()).rate(before.srs, input.rating, now);
    repos.cards.updateSrs(before.id, result.card, now.getTime());
    repos.reviewLogs.insert({
      userId: deps.userId,
      cardId: before.id,
      rating: input.rating,
      stateBefore: before.srs.state,
      dueBefore: before.srs.due.getTime(),
      reviewedAt: now.getTime(),
      durationMs: input.durationMs,
    });
    repos.events.append(
      deps.userId,
      "review",
      {
        ...input,
        stateBefore: before.srs.state,
        stateAfter: result.card.state,
        dueBefore: before.srs.due.getTime(),
        dueAfter: result.card.due.getTime(),
      },
      now.getTime(),
    );
    return result.card;
  });
}
export const reviewService = { getQueue, getReviewCard, rate };
