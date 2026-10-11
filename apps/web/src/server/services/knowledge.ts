import { buildKnownSet } from "@eng-learn/core";
import type { AppDeps } from "../deps";
export function getKnownSet(deps: AppDeps, input: { targets: string[] } = { targets: [] }) {
  return buildKnownSet({
    baselineLemmas: deps.repos.wordLists.baselineLemmas(deps.userId),
    cards: deps.repos.cards
      .listAll(deps.userId)
      .map((c) => ({ lemma: c.lemma, card: c.srs, lastReviewAt: c.srs.lastReview })),
    marks: deps.repos.marks
      .list(deps.userId)
      .map((m) => ({ ...m, markedAt: new Date(m.markedAt) })),
    targets: input.targets,
    now: deps.clock(),
    requestRetention: deps.settings().requestRetention,
  });
}
export const knowledgeService = { getKnownSet };
