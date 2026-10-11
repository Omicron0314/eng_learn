import {
  type FSRS,
  type Card as FsrsCard,
  Rating as FsrsRating,
  State as FsrsState,
  fsrs,
  type Grade,
  generatorParameters,
} from "ts-fsrs";

export const RATINGS = ["again", "hard", "good", "easy"] as const;
export type Rating = (typeof RATINGS)[number];
export const CARD_STATES = ["new", "learning", "review", "relearning"] as const;
export type CardStateName = (typeof CARD_STATES)[number];

export type SrsCard = {
  due: Date;
  stability: number;
  difficulty: number;
  state: CardStateName;
  reps: number;
  lapses: number;
  lastReview: Date | null;
  elapsedDays: number;
  scheduledDays: number;
  learningSteps: number;
};

const RATING_TO_FSRS: Record<Rating, Grade> = {
  again: FsrsRating.Again,
  hard: FsrsRating.Hard,
  good: FsrsRating.Good,
  easy: FsrsRating.Easy,
};

const STATE_FROM_FSRS: Record<FsrsState, CardStateName> = {
  [FsrsState.New]: "new",
  [FsrsState.Learning]: "learning",
  [FsrsState.Review]: "review",
  [FsrsState.Relearning]: "relearning",
};

const STATE_TO_FSRS: Record<CardStateName, FsrsState> = {
  new: FsrsState.New,
  learning: FsrsState.Learning,
  review: FsrsState.Review,
  relearning: FsrsState.Relearning,
};

export function newSrsCard(now: Date): SrsCard {
  return {
    due: now,
    stability: 0,
    difficulty: 0,
    state: "new",
    reps: 0,
    lapses: 0,
    lastReview: null,
    elapsedDays: 0,
    scheduledDays: 0,
    learningSteps: 0,
  };
}

function toFsrs(card: SrsCard): FsrsCard {
  return {
    due: card.due,
    stability: card.stability,
    difficulty: card.difficulty,
    elapsed_days: card.elapsedDays,
    scheduled_days: card.scheduledDays,
    reps: card.reps,
    lapses: card.lapses,
    learning_steps: card.learningSteps,
    state: STATE_TO_FSRS[card.state],
    ...(card.lastReview ? { last_review: card.lastReview } : {}),
  } as FsrsCard;
}

function fromFsrs(card: FsrsCard): SrsCard {
  return {
    due: card.due,
    stability: card.stability,
    difficulty: card.difficulty,
    elapsedDays: card.elapsed_days,
    scheduledDays: card.scheduled_days,
    reps: card.reps,
    lapses: card.lapses,
    learningSteps: card.learning_steps,
    state: STATE_FROM_FSRS[card.state],
    lastReview: card.last_review ?? null,
  };
}

export type Srs = {
  rate(
    card: SrsCard,
    rating: Rating,
    now: Date,
  ): { card: SrsCard; log: { stateBefore: CardStateName; dueBefore: Date } };
  preview(card: SrsCard, now: Date): Record<Rating, { due: Date; intervalMs: number }>;
  retrievability(card: SrsCard, now: Date): number;
};

export function createSrs(opts: { requestRetention: number }): Srs {
  const f: FSRS = fsrs(
    generatorParameters({ request_retention: opts.requestRetention, enable_fuzz: false }),
  );
  return {
    rate(card, rating, now) {
      const next = f.next(toFsrs(card), now, RATING_TO_FSRS[rating]);
      return { card: fromFsrs(next.card), log: { stateBefore: card.state, dueBefore: card.due } };
    },
    preview(card, now) {
      const out = {} as Record<Rating, { due: Date; intervalMs: number }>;
      for (const r of RATINGS) {
        const next = f.next(toFsrs(card), now, RATING_TO_FSRS[r]);
        out[r] = { due: next.card.due, intervalMs: next.card.due.getTime() - now.getTime() };
      }
      return out;
    },
    retrievability(card, now) {
      if (card.state === "new" || !card.lastReview) return 0;
      const r = f.get_retrievability(toFsrs(card), now, false);
      return typeof r === "number" ? r : Number(r);
    },
  };
}

export function formatInterval(ms: number): string {
  const min = Math.round(ms / 60_000);
  if (min < 60) return `${Math.max(1, min)}分钟`;
  const hours = Math.round(min / 60);
  if (hours < 24) return `${hours}小时`;
  const days = Math.round(hours / 24);
  if (days < 30) return `${days}天`;
  const months = Math.round(days / 30);
  if (months < 12) return `${months}个月`;
  return `${(days / 365).toFixed(1)}年`;
}
