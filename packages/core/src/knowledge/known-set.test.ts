import { describe, expect, it } from "vitest";
import { createSrs, newSrsCard, type SrsCard } from "../srs/srs";
import { buildKnownSet } from "./known-set";

const now = new Date("2026-10-11T02:00:00Z");
const DAY = 86_400_000;
const srs = createSrs({ requestRetention: 0.9 });

function reviewCardAt(r: number): SrsCard {
  let card = newSrsCard(new Date(now.getTime() - 200 * DAY));
  let t = card.due;
  for (let i = 0; i < 3; i++) {
    card = srs.rate(card, "good", t).card;
    t = card.due;
  }
  // find a "now"-relative last review so that retrievability(now) ~= r
  const last = card.lastReview ?? t;
  let lo = 0;
  let hi = 2000;
  for (let i = 0; i < 60; i++) {
    const mid = (lo + hi) / 2;
    const shifted = { ...card, lastReview: new Date(now.getTime() - mid * DAY), due: now };
    if (srs.retrievability(shifted, now) > r) lo = mid;
    else hi = mid;
  }
  void last;
  return { ...card, lastReview: new Date(now.getTime() - lo * DAY), due: now };
}

const base = { baselineLemmas: [] as string[], cards: [], marks: [], targets: [], now };

describe("buildKnownSet", () => {
  it("AC1: baseline words are known, others unknown", () => {
    const k = buildKnownSet({ ...base, baselineLemmas: ["apple"] });
    expect(k.has("apple")).toBe(true);
    expect(k.has("pear")).toBe(false);
  });

  it("AC2: review cards known only above retrievability 0.8", () => {
    const high = reviewCardAt(0.81);
    const low = reviewCardAt(0.79);
    expect(srs.retrievability(high, now)).toBeGreaterThanOrEqual(0.8);
    expect(srs.retrievability(low, now)).toBeLessThan(0.8);
    const k = buildKnownSet({
      ...base,
      cards: [
        { lemma: "high", card: high, lastReviewAt: high.lastReview },
        { lemma: "low", card: low, lastReviewAt: low.lastReview },
        { lemma: "learn", card: { ...high, state: "learning" }, lastReviewAt: high.lastReview },
      ],
    });
    expect(k.has("high")).toBe(true);
    expect(k.has("low")).toBe(false);
    expect(k.has("learn")).toBe(false);
  });

  it("AC3: words marked known are known", () => {
    expect(
      buildKnownSet({ ...base, marks: [{ lemma: "zeal", status: "known", markedAt: now }] }).has(
        "zeal",
      ),
    ).toBe(true);
  });

  it("AC4: recent unknown marks exclude until reviewed or expired", () => {
    const marked = new Date(now.getTime() - 3 * DAY);
    const mark = { lemma: "apple", status: "unknown" as const, markedAt: marked };
    const card = newSrsCard(now);
    expect(buildKnownSet({ ...base, baselineLemmas: ["apple"], marks: [mark] }).has("apple")).toBe(
      false,
    );
    const reviewedAfter = buildKnownSet({
      ...base,
      baselineLemmas: ["apple"],
      marks: [mark],
      cards: [{ lemma: "apple", card, lastReviewAt: new Date(marked.getTime() + DAY) }],
    });
    expect(reviewedAfter.has("apple")).toBe(true);
    const old = buildKnownSet({
      ...base,
      baselineLemmas: ["apple"],
      marks: [{ ...mark, markedAt: new Date(now.getTime() - 15 * DAY) }],
    });
    expect(old.has("apple")).toBe(true);
  });

  it("AC5: targets are always known", () => {
    const k = buildKnownSet({
      ...base,
      targets: ["apple"],
      marks: [{ lemma: "apple", status: "unknown", markedAt: now }],
    });
    expect(k.has("apple")).toBe(true);
  });
});
