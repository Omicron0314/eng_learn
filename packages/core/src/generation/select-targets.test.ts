import { describe, expect, it } from "vitest";
import { newSrsCard } from "../srs/srs";
import { seededRng, selectTargets, type TargetCandidate } from "./select-targets";

const now = new Date("2026-10-11T02:00:00Z");

function cand(id: string, p: Partial<TargetCandidate> = {}): TargetCandidate {
  return {
    lexemeId: id,
    lemma: p.lemma ?? id,
    card: p.card ?? { ...newSrsCard(now), state: "review", due: new Date(now.getTime() - 1000) },
    suspended: p.suspended ?? false,
    todayRatings: p.todayRatings ?? [],
    introducedToday: p.introducedToday ?? false,
    retrievability: p.retrievability ?? 0.5,
  };
}

const run = (
  candidates: TargetCandidate[],
  extra: Partial<Parameters<typeof selectTargets>[0]> = {},
) =>
  selectTargets({
    candidates,
    recentExposure: new Map(),
    now,
    n: 8,
    controlRatio: 0,
    rng: seededRng(1),
    ...extra,
  });

describe("selectTargets", () => {
  it("AC1: orders buckets and assigns each word to its highest bucket", () => {
    const r = run([
      cand("due"),
      cand("new", { introducedToday: true, card: { ...newSrsCard(now), state: "learning" } }),
      cand("both", { introducedToday: true, todayRatings: ["again"] }),
      cand("hard", { todayRatings: ["good", "hard"] }),
    ]);
    expect(r.map((x) => [x.lexemeId, x.bucket])).toEqual([
      ["both", 1],
      ["hard", 1],
      ["new", 2],
      ["due", 3],
    ]);
  });

  it("AC2: sorts by retrievability within a bucket", () => {
    const r = run([cand("a", { retrievability: 0.9 }), cand("b", { retrievability: 0.3 })]);
    expect(r.map((x) => x.lexemeId)).toEqual(["b", "a"]);
  });

  it("AC3: excludes words exposed 3+ times in the last 3 days", () => {
    const r = run([cand("three"), cand("two")], {
      recentExposure: new Map([
        ["three", 3],
        ["two", 2],
      ]),
    });
    expect(r.map((x) => x.lexemeId)).toEqual(["two"]);
  });

  it("AC4: 20% of 8 are controls, deterministic per seed", () => {
    const cands = Array.from({ length: 10 }, (_, i) => cand(`w${i}`, { retrievability: i / 10 }));
    const a = run(cands, { controlRatio: 0.2, rng: seededRng(42) });
    const b = run(cands, { controlRatio: 0.2, rng: seededRng(42) });
    expect(a).toHaveLength(8);
    expect(a.filter((x) => x.role === "control")).toHaveLength(2);
    expect(a).toEqual(b);
    const differs = [1, 2, 3, 4, 5].some(
      (s) =>
        JSON.stringify(run(cands, { controlRatio: 0.2, rng: seededRng(s) })) !== JSON.stringify(a),
    );
    expect(differs).toBe(true);
  });

  it("AC5: returns fewer when candidates are scarce", () => {
    expect(run([cand("a")])).toHaveLength(1);
    expect(run([])).toEqual([]);
  });

  it("AC6: skips suspended, phrases and unstudied new cards", () => {
    const r = run([
      cand("s", { suspended: true }),
      cand("p", { lemma: "be able to" }),
      cand("n", { card: newSrsCard(now) }),
    ]);
    expect(r).toEqual([]);
  });
});
