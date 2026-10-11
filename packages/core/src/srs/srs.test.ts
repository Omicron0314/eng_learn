import { createEmptyCard, fsrs, generatorParameters, Rating as R } from "ts-fsrs";
import { describe, expect, it } from "vitest";
import { buildQueue, type QueueCard } from "./queue";
import { createSrs, newSrsCard, RATINGS, type SrsCard } from "./srs";

const now = new Date("2026-10-11T02:00:00Z");
const srs = createSrs({ requestRetention: 0.9 });

function reviewCard(): SrsCard {
  let card = newSrsCard(new Date("2026-09-01T02:00:00Z"));
  let t = new Date("2026-09-01T02:00:00Z");
  for (let i = 0; i < 3; i++) {
    card = srs.rate(card, "good", t).card;
    t = card.due;
  }
  return card;
}

describe("srs.rate", () => {
  it("AC1: new card due order Again <= Hard <= Good <= Easy and matches ts-fsrs", () => {
    const card = newSrsCard(now);
    const dues = RATINGS.map((r) => srs.rate(card, r, now).card.due.getTime());
    for (let i = 1; i < dues.length; i++) expect(dues[i]).toBeGreaterThanOrEqual(dues[i - 1] ?? 0);

    const f = fsrs(generatorParameters({ request_retention: 0.9, enable_fuzz: false }));
    const ref = f.repeat(createEmptyCard(now), now);
    expect(dues[0]).toBe(ref[R.Again].card.due.getTime());
    expect(dues[1]).toBe(ref[R.Hard].card.due.getTime());
    expect(dues[2]).toBe(ref[R.Good].card.due.getTime());
    expect(dues[3]).toBe(ref[R.Easy].card.due.getTime());
  });

  it("AC2: review card ordering and lapse on Again", () => {
    const card = reviewCard();
    expect(card.state).toBe("review");
    const at = card.due;
    const dues = RATINGS.map((r) => srs.rate(card, r, at).card.due.getTime());
    for (let i = 1; i < dues.length; i++) expect(dues[i]).toBeGreaterThanOrEqual(dues[i - 1] ?? 0);
    const again = srs.rate(card, "again", at);
    expect(again.card.lapses).toBe(card.lapses + 1);
    expect(again.card.state).toBe("relearning");
    expect(again.log.stateBefore).toBe("review");
  });

  it("AC3: retrievability is 0 for new cards and decreases over time", () => {
    expect(srs.retrievability(newSrsCard(now), now)).toBe(0);
    const card = reviewCard();
    const last = card.lastReview ?? now;
    const r1 = srs.retrievability(card, new Date(last.getTime() + 60_000));
    const r2 = srs.retrievability(card, new Date(last.getTime() + 10 * 86_400_000));
    const r3 = srs.retrievability(card, new Date(last.getTime() + 60 * 86_400_000));
    expect(r1).toBeGreaterThan(0.95);
    expect(r2).toBeLessThan(r1);
    expect(r3).toBeLessThan(r2);
  });

  it("preview returns all four ratings", () => {
    const p = srs.preview(newSrsCard(now), now);
    expect(Object.keys(p)).toEqual(["again", "hard", "good", "easy"]);
  });
});

function qc(
  id: string,
  partial: Partial<Omit<QueueCard, "card">> & { card?: Partial<SrsCard> } = {},
): QueueCard {
  return {
    id,
    suspended: partial.suspended ?? false,
    origin: partial.origin ?? "import",
    createdAt: partial.createdAt ?? new Date(now.getTime() - 1000),
    card: { ...newSrsCard(new Date(now.getTime() - 1000)), ...partial.card },
  };
}

describe("buildQueue", () => {
  const base = {
    now,
    dailyNewLimit: 20,
    dailyReviewLimit: 200,
    newIntroducedToday: 0,
    reviewsDoneToday: 0,
  };

  it("AC4: excludes suspended; learning before review before new", () => {
    const q = buildQueue({
      ...base,
      cards: [
        qc("n1"),
        qc("r1", { card: { state: "review", due: new Date(now.getTime() - 5000) } }),
        qc("l1", { card: { state: "learning", due: new Date(now.getTime() - 1000) } }),
        qc("s1", { suspended: true }),
        qc("future", { card: { state: "review", due: new Date(now.getTime() + 86_400_000) } }),
      ],
    });
    expect(q).toEqual({ learning: ["l1"], review: ["r1"], new: ["n1"] });
  });

  it("AC5: respects remaining new and review slots", () => {
    const cards = Array.from({ length: 30 }, (_, i) => qc(`n${String(i).padStart(2, "0")}`));
    const reviews = Array.from({ length: 10 }, (_, i) =>
      qc(`r${i}`, { card: { state: "review", due: new Date(now.getTime() - 1000) } }),
    );
    const q = buildQueue({
      ...base,
      cards: [...cards, ...reviews],
      newIntroducedToday: 5,
      dailyReviewLimit: 8,
      reviewsDoneToday: 2,
    });
    expect(q.new).toHaveLength(15);
    expect(q.review).toHaveLength(6);
  });

  it("AC6: lookup cards take new slots first", () => {
    const imported = Array.from({ length: 30 }, (_, i) =>
      qc(`i${String(i).padStart(2, "0")}`, { createdAt: new Date(now.getTime() - 100_000 + i) }),
    );
    const lookups = ["a", "b", "c"].map((id, i) =>
      qc(`look-${id}`, { origin: "lookup", createdAt: new Date(now.getTime() - 10 + i) }),
    );
    const q = buildQueue({ ...base, cards: [...imported, ...lookups] });
    expect(q.new).toHaveLength(20);
    expect(q.new.slice(0, 3)).toEqual(["look-a", "look-b", "look-c"]);
  });
});
