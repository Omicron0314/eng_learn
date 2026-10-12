import { describe, expect, it } from "vitest";
import { type EventType, eventSchemas, parseEventPayload } from "./events";

const valid: Record<EventType, unknown> = {
  review: {
    cardId: "c",
    rating: "good",
    stateBefore: "new",
    stateAfter: "learning",
    dueBefore: 0,
    dueAfter: 1,
    durationMs: 10,
  },
  import: { source: "csv", fileName: "x.csv", kind: "cards", added: 1, skipped: 0, duplicates: 0 },
  card_edit: { cardId: "c", fields: ["meaning"] },
  card_suspend: { cardId: "c", suspended: true },
  deck_delete: { deck: "d", deleted: 2, keptWithHistory: 1 },
  settings_update: { changed: ["dailyNewLimit"] },
  article_generate: {
    articleId: "a",
    attempts: 1,
    coverage: 1,
    belowTier: false,
    missingTargets: [],
  },
  article_open: { articleId: "a" },
  article_complete: { articleId: "a", via: "button", openId: "open" },
  lookup: { lexemeId: "l", lemma: "go", surface: "went", sentence: "I went.", cacheHit: false },
  mark_known: { lexemeId: "l" },
  mark_unknown: { lexemeId: "l", inboxId: "i" },
  add_to_review: { lexemeId: "l", cardId: "c", from: "lookup" },
  inbox_ignore: { inboxId: "i", lexemeId: "l" },
  audio_reserve: { articleId: "a", chars: 3 },
  audio_generate: { articleId: "a", audioId: "audio", chars: 3 },
  audio_play: { articleId: "a", rate: 1, via: "browser" },
  difficulty_feedback: { articleId: "a", value: "just_right", after: "reading" },
  unnatural_flag: { articleId: "a", sentenceIndex: 0, note: "x" },
};
describe("T1.4 typed events", () => {
  for (const type of Object.keys(eventSchemas) as EventType[]) {
    it(`AC3: ${type} accepts valid payload and rejects invalid payload`, () => {
      expect(parseEventPayload(type, valid[type])).toEqual(valid[type]);
      expect(() => parseEventPayload(type, {})).toThrow();
      expect(() =>
        parseEventPayload(type, {
          ...(valid[type] as object),
          ...(type === "review"
            ? { rating: "invalid" }
            : type === "import"
              ? { source: "invalid" }
              : type === "deck_delete"
                ? { deleted: -1 }
                : { articleId: 1, lexemeId: 1, cardId: 1, changed: [], fields: [] }),
        }),
      ).toThrow();
    });
  }
});
