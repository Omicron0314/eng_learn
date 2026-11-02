// Original deterministic fixtures; no user collection is read.
import { createRequire } from "node:module";
const require = createRequire(new URL("../../packages/core/package.json", import.meta.url));
const { fsrs, generatorParameters, createEmptyCard, State } = require("ts-fsrs");
const now = new Date("2026-11-02T04:00:00.000Z");
const parameters = generatorParameters({ request_retention: 0.9, enable_fuzz: false });
const scheduler = fsrs(parameters);
const cases = [
  { name: "new", card: createEmptyCard(now) },
  ...[State.Learning, State.Review, State.Relearning].map(state => ({
    name: State[state].toLowerCase(),
    card: {
      ...createEmptyCard(now), state, stability: 7, difficulty: 5,
      reps: 4, lapses: state === State.Relearning ? 1 : 0,
      elapsed_days: 7, scheduled_days: 7, learning_steps: 0,
      last_review: new Date(now.getTime() - 7 * 86400000),
    },
  })),
  ...[0, 1, 30, 365].flatMap(days => [State.Learning, State.Review, State.Relearning].map(state => ({
    name: `${State[state].toLowerCase()}-${days}d`,
    card: { ...createEmptyCard(now), state, stability: 17.1234, difficulty: 7.1234,
      reps: 12, lapses: 2, elapsed_days: days, scheduled_days: 17,
      learning_steps: state === State.Learning ? 1 : 0,
      last_review: new Date(now.getTime() - days * 86400000) },
  }))),
];
console.log(JSON.stringify({
  source: "ts-fsrs 5.4.2", now, parameters,
  cases: cases.map(({ name, card }) => ({
    name, card,
    ratings: [1, 2, 3, 4].map(rating => ({ rating, ...scheduler.next(card, now, rating) })),
  })),
}, null, 2));
