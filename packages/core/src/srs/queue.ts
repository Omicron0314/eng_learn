import type { SrsCard } from "./srs";

export const CARD_ORIGINS = ["import", "lookup", "inbox", "autoAdd"] as const;
export type CardOrigin = (typeof CARD_ORIGINS)[number];

export type QueueCard = {
  id: string;
  card: SrsCard;
  suspended: boolean;
  origin: CardOrigin;
  createdAt: Date;
};

export type QueueInput = {
  cards: QueueCard[];
  now: Date;
  dailyNewLimit: number;
  dailyReviewLimit: number;
  newIntroducedToday: number;
  reviewsDoneToday: number;
};

export type Queue = { learning: string[]; review: string[]; new: string[] };

const byDue = (a: QueueCard, b: QueueCard) =>
  a.card.due.getTime() - b.card.due.getTime() || a.id.localeCompare(b.id);
const byCreated = (a: QueueCard, b: QueueCard) =>
  a.createdAt.getTime() - b.createdAt.getTime() || a.id.localeCompare(b.id);

export function buildQueue(input: QueueInput): Queue {
  const now = input.now.getTime();
  const active = input.cards.filter((c) => !c.suspended);

  const learning = active
    .filter(
      (c) =>
        (c.card.state === "learning" || c.card.state === "relearning") &&
        c.card.due.getTime() <= now,
    )
    .sort(byDue);

  const reviewSlots = Math.max(0, input.dailyReviewLimit - input.reviewsDoneToday);
  const review = active
    .filter((c) => c.card.state === "review" && c.card.due.getTime() <= now)
    .sort(byDue)
    .slice(0, reviewSlots);

  const newSlots = Math.max(0, input.dailyNewLimit - input.newIntroducedToday);
  const newCards = active.filter((c) => c.card.state === "new");
  const prioritized = newCards.filter((c) => c.origin !== "import").sort(byCreated);
  const imported = newCards.filter((c) => c.origin === "import").sort(byCreated);
  const fresh = [...prioritized, ...imported].slice(0, newSlots);

  return {
    learning: learning.map((c) => c.id),
    review: review.map((c) => c.id),
    new: fresh.map((c) => c.id),
  };
}

/** Next card to show: learning first, then reviews, then new cards. */
export function nextInQueue(q: Queue): string | undefined {
  return q.learning[0] ?? q.review[0] ?? q.new[0];
}
