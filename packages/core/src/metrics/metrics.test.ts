import { describe, expect, it } from "vitest";
import { computeExperiment, type ExperimentSample, type FirstReview } from "./experiment";
import { completionRate, difficultyDistribution, median } from "./stats";

const DAY = 86_400_000;
const t0 = Date.parse("2026-10-01T00:00:00Z");

function samples(role: "exposed" | "control", n: number): ExperimentSample[] {
  return Array.from({ length: n }, (_, i) => ({
    articleId: "a",
    lexemeId: `${role}${i}`,
    role,
    articleCreatedAt: t0,
  }));
}

describe("computeExperiment", () => {
  it("AC1: computes rates and diff", () => {
    const all = [...samples("exposed", 10), ...samples("control", 5)];
    const again = new Set(["exposed0", "exposed1", "control0", "control1"]);
    const r = computeExperiment({
      samples: all,
      firstReview: (s) => ({
        rating: again.has(s.lexemeId) ? "again" : "good",
        reviewedAt: t0 + DAY,
      }),
      windowDays: 7,
    });
    expect(r.exposed).toEqual({ n: 10, again: 2, againRate: 0.2, noReview: 0 });
    expect(r.control).toEqual({ n: 5, again: 2, againRate: 0.4, noReview: 0 });
    expect(r.diff).toBeCloseTo(-0.2);
  });

  it("AC3/AC4: window and pre-article reviews", () => {
    const reviews: Record<string, FirstReview> = {
      exposed0: { rating: "again", reviewedAt: t0 + 7 * DAY },
      exposed1: { rating: "again", reviewedAt: t0 + 7 * DAY + 1 },
      exposed2: { rating: "again", reviewedAt: t0 - 1 },
    };
    const r = computeExperiment({
      samples: samples("exposed", 3),
      firstReview: (s) => reviews[s.lexemeId] ?? null,
      windowDays: 7,
    });
    expect(r.exposed.n).toBe(1);
    expect(r.exposed.noReview).toBe(2);
  });

  it("AC5: empty data yields nulls", () => {
    const r = computeExperiment({ samples: [], firstReview: () => null, windowDays: 7 });
    expect(r.exposed.againRate).toBeNull();
    expect(r.diff).toBeNull();
  });
});

describe("stats helpers", () => {
  it("median", () => {
    expect(median([])).toBeNull();
    expect(median([3, 1, 2])).toBe(2);
    expect(median([4, 1, 2, 3])).toBe(2.5);
  });

  it("completion rate", () => {
    expect(completionRate(new Set(["a", "b", "c", "d"]), new Set(["a", "b", "c"]))).toBe(0.75);
    expect(completionRate(new Set(), new Set())).toBeNull();
  });

  it("difficulty keeps the latest per article and mode", () => {
    const r = difficultyDistribution([
      { articleId: "a", after: "reading", value: "too_hard", ts: 1 },
      { articleId: "a", after: "reading", value: "just_right", ts: 2 },
      { articleId: "a", after: "listening", value: "too_easy", ts: 3 },
    ]);
    expect(r.distribution).toEqual({ too_hard: 0, just_right: 1, too_easy: 1 });
    expect(r.justRightRate).toBe(0.5);
  });
});
