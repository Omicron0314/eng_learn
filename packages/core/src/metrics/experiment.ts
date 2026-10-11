import type { Rating } from "../srs/srs";

export type ExperimentSample = {
  articleId: string;
  lexemeId: string;
  role: "exposed" | "control";
  articleCreatedAt: number;
};

export type FirstReview = { rating: Rating; reviewedAt: number } | null;

export type GroupResult = { n: number; again: number; againRate: number | null; noReview: number };

export type ExperimentResult = { exposed: GroupResult; control: GroupResult; diff: number | null };

export function computeExperiment(input: {
  samples: ExperimentSample[];
  firstReview: (s: ExperimentSample) => FirstReview;
  windowDays: number;
}): ExperimentResult {
  const windowMs = input.windowDays * 86_400_000;
  const groups: Record<"exposed" | "control", GroupResult> = {
    exposed: { n: 0, again: 0, againRate: null, noReview: 0 },
    control: { n: 0, again: 0, againRate: null, noReview: 0 },
  };
  for (const s of input.samples) {
    const g = groups[s.role];
    const r = input.firstReview(s);
    if (!r || r.reviewedAt <= s.articleCreatedAt || r.reviewedAt > s.articleCreatedAt + windowMs) {
      g.noReview++;
      continue;
    }
    g.n++;
    if (r.rating === "again") g.again++;
  }
  for (const g of Object.values(groups)) g.againRate = g.n === 0 ? null : g.again / g.n;
  const diff =
    groups.exposed.againRate === null || groups.control.againRate === null
      ? null
      : groups.exposed.againRate - groups.control.againRate;
  return { ...groups, diff };
}
