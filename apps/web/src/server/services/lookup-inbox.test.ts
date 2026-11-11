import { describe, expect, it, vi } from "vitest";
import { setSuspended } from "./card";
import { contextHash, getGloss, lookupArticleWord } from "./gloss";
import { batchInbox, listInbox } from "./inbox";
import { getKnownSet } from "./knowledge";
import { addToReview, markKnown, markUnknown } from "./mark";
import { getQueue } from "./review";
import { updateSettings } from "./settings";
import { required, seedArticle, seedCard, setup } from "./test-support";

describe("T4.1 gloss", () => {
  it("AC1–AC3/AC5: caches normalized contexts and emits one lookup per click", async () => {
    const { deps, llm } = setup();
    const input = { lemma: "apple", surface: "apple", sentence: "I’m eating an apple." };
    expect(await getGloss(deps, input)).toMatchObject({ cacheHit: false });
    expect(await getGloss(deps, { ...input, sentence: "  I'm   eating an apple. " })).toMatchObject(
      { cacheHit: true },
    );
    expect(llm.calls).toBe(1);
    expect(contextHash(input.sentence)).toBe(contextHash("  I'm   eating an apple. "));
    await getGloss(deps, { ...input, sentence: "An apple is here." });
    expect(llm.calls).toBe(2);
    const events = deps.repos.events.listByType(deps.userId, "lookup");
    expect(events.map((e) => e.payload.cacheHit)).toEqual([false, true, false]);
    expect(events[0]?.payload).toMatchObject(input);
  });
  it("AC4: malformed replies log errors without caching a gloss", async () => {
    const { deps } = setup(["bad", "bad"]);
    await expect(
      getGloss(deps, { lemma: "apple", surface: "apple", sentence: "An apple." }),
    ).rejects.toMatchObject({ code: "LLM_OUTPUT_INVALID" });
    const lexemeId = required(deps.repos.lexemes.findByLemma("apple")).id;
    expect(deps.repos.glossCache.get(lexemeId, contextHash("An apple."))).toBeUndefined();
    expect(deps.repos.events.count(deps.userId, "lookup")).toBe(1);
    expect(deps.repos.llmCalls.list(deps.userId).map((c) => c.status)).toEqual([
      "invalid_output",
      "invalid_output",
    ]);
  });
  it("AC7: computes authoritative sentence and went→go from offset", async () => {
    const { deps } = setup();
    const articleId = seedArticle(deps, "I went home.");
    const result = await lookupArticleWord(deps, { articleId, start: 2 });
    expect(result.gloss.lemma).toBe("go");
    expect(deps.repos.events.listByType(deps.userId, "lookup")[0]?.payload).toMatchObject({
      lemma: "go",
      surface: "went",
      sentence: "I went home.",
    });
    expect(() => lookupArticleWord(deps, { articleId, start: 1 })).toThrow(
      expect.objectContaining({ code: "VALIDATION_FAILED" }),
    );
  });
  it("AC1: concurrent requests share a completion and keep all lookup events", async () => {
    const { deps, llm } = setup();
    await Promise.all(
      Array.from({ length: 5 }, () =>
        getGloss(deps, { lemma: "apple", surface: "apple", sentence: "An apple." }),
      ),
    );
    expect(llm.calls).toBe(1);
    expect(deps.repos.events.count(deps.userId, "lookup")).toBe(5);
  });
});
describe("T4.2 marks", () => {
  it("AC1/AC2: markOnly never builds cards and deduplicates pending inbox", () => {
    const { deps } = setup();
    const input = { lemma: "apple", sentence: "An apple." };
    const first = markUnknown(deps, input);
    expect(markUnknown(deps, input)).toEqual(first);
    expect(deps.repos.cards.count(deps.userId)).toBe(0);
    expect(listInbox(deps)).toHaveLength(1);
    expect(deps.repos.events.count(deps.userId, "mark_unknown")).toBe(2);
  });
  it("AC3: autoAdd builds card using the same context gloss with paired events", async () => {
    const { deps } = setup();
    const input = { lemma: "apple", surface: "apple", sentence: "An apple." };
    await getGloss(deps, input);
    updateSettings(deps, { lookupMode: "autoAdd" });
    const result = markUnknown(deps, input);
    expect(deps.repos.cards.get(required(result.cardId))).toMatchObject({
      origin: "autoAdd",
      back: { meaning: "语境释义（测试）", meaningSource: "ai" },
    });
    expect(deps.repos.events.count(deps.userId, "mark_unknown")).toBe(1);
    expect(deps.repos.events.count(deps.userId, "add_to_review")).toBe(1);
  });
  it("AC4–AC6: adds preferential new card, consumes slots, restores suspended card and resolves inbox", () => {
    const { deps } = setup();
    for (let i = 0; i < 30; i++) seedCard(deps, `word${i}`);
    const input = { lemma: "apple", sentence: "An apple." };
    markUnknown(deps, input);
    const result = addToReview(deps, input);
    expect(result.created).toBe(true);
    expect(getQueue(deps).new).toHaveLength(20);
    expect(getQueue(deps).new[0]).toBe(result.cardId);
    expect(listInbox(deps)).toEqual([]);
    setSuspended(deps, { cardId: result.cardId, suspended: true });
    expect(addToReview(deps, input)).toEqual({ cardId: result.cardId, created: false });
    expect(deps.repos.cards.get(result.cardId)?.suspended).toBe(false);
  });
  it("AC7/AC8: known marks affect knowledge; failure rolls back every mutation", () => {
    const { deps } = setup();
    markKnown(deps, { lemma: "apple", sentence: "" });
    expect(getKnownSet(deps).has("apple")).toBe(true);
    const tx = deps.tx;
    deps.tx = (fn) =>
      tx((repos) => {
        vi.spyOn(repos.events, "append").mockImplementation(() => {
          throw new Error("injected");
        });
        return fn(repos);
      });
    expect(() => markUnknown(deps, { lemma: "pear", sentence: "Pear." })).toThrow("injected");
    expect(deps.repos.lexemes.findByLemma("pear")).toBeUndefined();
    expect(listInbox(deps)).toEqual([]);
    expect(deps.repos.marks.list(deps.userId)).toHaveLength(1);
  });
});
describe("T4.3 inbox", () => {
  it("AC1/AC2/AC5: batch add/ignore write one event per row, count agrees with list", () => {
    const { deps } = setup();
    const ids = ["apple", "pear", "ocean"].map((lemma) =>
      required(markUnknown(deps, { lemma, sentence: lemma }).inboxId),
    );
    expect(batchInbox(deps, { action: "add", ids: ids.slice(0, 2) })).toEqual({
      processed: 2,
      cardsCreated: 2,
    });
    expect(deps.repos.cards.listAll(deps.userId).every((c) => c.origin === "inbox")).toBe(true);
    expect(deps.repos.events.count(deps.userId, "add_to_review")).toBe(2);
    expect(listInbox(deps)).toHaveLength(1);
    expect(deps.repos.inbox.countPending(deps.userId)).toBe(1);
    expect(batchInbox(deps, { action: "ignore", ids: [required(ids[2])] })).toEqual({
      processed: 1,
      cardsCreated: 0,
    });
    expect(listInbox(deps)).toEqual([]);
    expect(deps.repos.events.count(deps.userId, "inbox_ignore")).toBe(1);
  });
  it("AC3/AC4: existing cards are reused, invalid/nonpending/duplicate ids reject entire batch", () => {
    const { deps } = setup();
    seedCard(deps, "apple");
    const a = required(markUnknown(deps, { lemma: "apple", sentence: "Apple." }).inboxId);
    const b = required(markUnknown(deps, { lemma: "pear", sentence: "Pear." }).inboxId);
    expect(() => batchInbox(deps, { action: "add", ids: [a, "missing"] })).toThrow();
    expect(() => batchInbox(deps, { action: "add", ids: [a, a] })).toThrow();
    expect(listInbox(deps)).toHaveLength(2);
    expect(deps.repos.events.count(deps.userId, "add_to_review")).toBe(0);
    expect(batchInbox(deps, { action: "add", ids: [a] })).toEqual({
      processed: 1,
      cardsCreated: 0,
    });
    expect(() => batchInbox(deps, { action: "ignore", ids: [a, b] })).toThrow();
    expect(listInbox(deps)).toHaveLength(1);
  });
});
