import type { KnownSet } from "../analyzer/analyze";
import { createSrs, type SrsCard } from "../srs/srs";

export const MIN_RETRIEVABILITY = 0.8;
export const RECENT_UNKNOWN_DAYS = 14;

export type KnownInput = {
  baselineLemmas: Iterable<string>;
  cards: { lemma: string; card: SrsCard; lastReviewAt: Date | null }[];
  marks: { lemma: string; status: "known" | "unknown"; markedAt: Date }[];
  targets: string[];
  now: Date;
  requestRetention?: number;
  minRetrievability?: number;
  recentUnknownDays?: number;
};

export function buildKnownSet(input: KnownInput): KnownSet & { size: number } {
  const srs = createSrs({ requestRetention: input.requestRetention ?? 0.9 });
  const minR = input.minRetrievability ?? MIN_RETRIEVABILITY;
  const windowMs = (input.recentUnknownDays ?? RECENT_UNKNOWN_DAYS) * 86_400_000;
  const now = input.now.getTime();

  const known = new Set<string>();
  for (const l of input.baselineLemmas) known.add(l.toLowerCase());

  const lastReview = new Map<string, number>();
  for (const c of input.cards) {
    const lemma = c.lemma.toLowerCase();
    if (c.lastReviewAt) {
      lastReview.set(lemma, Math.max(lastReview.get(lemma) ?? 0, c.lastReviewAt.getTime()));
    }
    if (c.card.state === "review" && srs.retrievability(c.card, input.now) >= minR) {
      known.add(lemma);
    }
  }

  for (const m of input.marks) {
    const lemma = m.lemma.toLowerCase();
    if (m.status === "known") {
      known.add(lemma);
      continue;
    }
    const marked = m.markedAt.getTime();
    const recent = now - marked <= windowMs;
    const reviewedSince = (lastReview.get(lemma) ?? 0) > marked;
    if (recent && !reviewedSince) known.delete(lemma);
  }

  for (const t of input.targets) known.add(t.toLowerCase());

  return { has: (lemma) => known.has(lemma), size: known.size };
}
