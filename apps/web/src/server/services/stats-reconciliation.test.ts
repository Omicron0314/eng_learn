import { dayRange, weekRange } from "@eng-learn/core";
import { closeDb, createRepos, migrate, openDb, withTx } from "@eng-learn/db";
import { expect, it } from "vitest";
import { rate } from "./review";
import { getStats } from "./stats";
import { seedCard, setup } from "./test-support";

it("T6.3 AC1/AC2: service counts equal independent SQL counts at week/day boundaries", () => {
  const db = openDb(":memory:");
  migrate(db);
  const { deps, setTime } = setup();
  deps.repos = createRepos(db);
  deps.repos.users.ensureDefaultUser(deps.clock().getTime());
  deps.tx = (fn) => withTx(db, (tx) => fn(createRepos(tx)));
  deps.settings = () => deps.repos.settings.get(deps.userId);
  try {
    const a = seedCard(deps, "apple");
    setTime("2026-10-11T19:59:00Z");
    rate(deps, { cardId: a, rating: "easy", durationMs: 1 });
    const b = seedCard(deps, "pear");
    setTime("2026-10-11T20:00:00Z");
    rate(deps, { cardId: b, rating: "again", durationMs: 1 });
    setTime("2026-10-11T20:10:00Z");
    rate(deps, { cardId: b, rating: "easy", durationMs: 1 });
    const stats = getStats(deps);
    for (const [period, range] of [
      ["today", dayRange(deps.clock(), deps.settings())],
      ["week", weekRange(deps.clock(), deps.settings())],
    ] as const) {
      const row = db.$client
        .prepare(
          "SELECT COUNT(*) AS reviews, SUM(CASE WHEN state_before = 'new' THEN 1 ELSE 0 END) AS newLearned FROM review_logs WHERE user_id = ? AND reviewed_at >= ? AND reviewed_at < ?",
        )
        .get(deps.userId, range.start, range.end);
      expect(stats.learning[period]).toEqual(row);
    }
  } finally {
    closeDb(db);
  }
});
