import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { analyzeText, dayRange, defaultLemmatizer, newSrsCard } from "@eng-learn/core";
import { closeDb, createRepos, migrate, openDb, withTx } from "@eng-learn/db";
import { zipSync } from "fflate";
import { describe, expect, it, vi } from "vitest";
import { createAppDeps } from "../deps";
import { readApkgNotes, stripAnkiHtml } from "../import/apkg";
import { ensureMeaning, listCards, setSuspended, updateCardText } from "./card";
import { importCards, importWordList } from "./import";
import { getKnownSet } from "./knowledge";
import { markKnown, markUnknown } from "./mark";
import { getQueue, rate } from "./review";
import { getSettingsView, updateSettings } from "./settings";
import { bytes, required, seedCard, setup } from "./test-support";

const words = Array.from({ length: 100 }, (_, i) => `word${i}`);
describe("T1.1 imports", () => {
  it("service enforces per-format compressed upload limits before parsing", async () => {
    const { deps } = setup();
    for (const size of [20 * 1024 * 1024 + 1, 64 * 1024 * 1024])
      await expect(
        importCards(deps, { fileName: "large.apkg", bytes: new Uint8Array(size), deck: "d" }),
      ).rejects.toMatchObject({ code: "VALIDATION_FAILED" });
    await expect(
      importCards(deps, {
        fileName: "oversized.apkg",
        bytes: new Uint8Array(64 * 1024 * 1024 + 1),
        deck: "d",
      }),
    ).rejects.toMatchObject({ code: "PAYLOAD_TOO_LARGE" });
    await expect(
      importWordList(deps, {
        fileName: "large.txt",
        bytes: new Uint8Array(20 * 1024 * 1024 + 1),
        name: "base",
        kind: "baseline",
      }),
    ).rejects.toMatchObject({ code: "PAYLOAD_TOO_LARGE" });
    expect(deps.repos.events.count(deps.userId)).toBe(0);
  });
  it("AC1/AC2/AC10: imports 100 new cards atomically and preserves them on repeat", async () => {
    const { deps } = setup();
    const input = {
      fileName: "words.csv",
      bytes: bytes(`word,meaning,example\n${words.map((w) => `${w},释义,example`).join("\n")}`),
      deck: "mine",
    };
    expect(await importCards(deps, input)).toMatchObject({ added: 100, skipped: 0 });
    const before = deps.repos.cards.listAll(deps.userId);
    expect(before).toHaveLength(100);
    expect(
      before.every(
        (c) =>
          c.srs.state === "new" &&
          c.srs.due.getTime() === deps.clock().getTime() &&
          c.deck === "mine",
      ),
    ).toBe(true);
    expect(await importCards(deps, input)).toMatchObject({ added: 0, skipped: 100 });
    expect(deps.repos.cards.listAll(deps.userId)).toEqual(before);
    expect(deps.repos.events.listByType(deps.userId, "import").map((e) => e.payload.added)).toEqual(
      [100, 0],
    );
  });
  it("AC3–AC6: handles BOM, quoting, missing definitions, duplicates and row errors", async () => {
    const { deps } = setup();
    const result = await importCards(deps, {
      fileName: "words.csv",
      bytes: bytes(
        '\uFEFFword,meaning,example\nApple,"a, fruit","line\n""two"""\napple,other\npear\n,missing',
      ),
      deck: "d",
    });
    expect(result).toMatchObject({
      added: 2,
      duplicates: 1,
      errors: [{ line: 6, reason: "empty word" }],
    });
    const cards = deps.repos.cards.listAll(deps.userId);
    expect(cards[0]?.back).toMatchObject({ meaning: "a, fruit", example: 'line\n"two"' });
    expect(cards[1]?.back.meaning).toBeUndefined();
  });
  it("AC7/AC10: rejects invalid UTF-8 and oversized files without writes", async () => {
    const { deps } = setup();
    await expect(
      importCards(deps, { fileName: "bad.csv", bytes: new Uint8Array([255]), deck: "d" }),
    ).rejects.toMatchObject({ code: "VALIDATION_FAILED" });
    await expect(
      importCards(deps, {
        fileName: "large.csv",
        bytes: new Uint8Array(20 * 1024 * 1024 + 1),
        deck: "d",
      }),
    ).rejects.toMatchObject({ code: "PAYLOAD_TOO_LARGE" });
    expect(deps.repos.events.count(deps.userId)).toBe(0);
    expect(deps.repos.cards.count(deps.userId)).toBe(0);
  });
  it("AC8: imports synthetic legacy SQLite notes and ignores media/scheduling", async () => {
    const db = openDb(":memory:");
    db.$client.exec(
      "CREATE TABLE notes(id INTEGER PRIMARY KEY, flds TEXT, tags TEXT); INSERT INTO notes VALUES(1, '<b>Apple</b>' || char(31) || 'fruit<br>&amp; {{c1::red::hint}} [sound:x.mp3]<img src=x>', 'a b')",
    );
    const archive = zipSync({
      "collection.anki2": db.$client.serialize(),
      media: bytes("ignored"),
    });
    closeDb(db);
    const { deps } = setup();
    expect(
      await importCards(deps, { fileName: "synthetic.apkg", bytes: archive, deck: "anki" }),
    ).toMatchObject({ added: 1 });
    expect(deps.repos.cards.listAll(deps.userId)[0]).toMatchObject({
      front: "Apple",
      back: { meaning: "fruit\n& red" },
      srs: newSrsCard(deps.clock()),
    });
    expect(stripAnkiHtml("<script>bad</script>A&nbsp;&lt;B&gt;&#x21;")).toBe("A <B>!");
  });
  it("AC9: new-format marker wins over legacy stub", async () => {
    const archive = zipSync({
      "collection.anki2": bytes("stub"),
      "collection.anki21b": bytes("compressed"),
    });
    const { deps } = setup();
    await expect(
      importCards(deps, { fileName: "new.apkg", bytes: archive, deck: "d" }),
    ).rejects.toMatchObject({ code: "APKG_UNSUPPORTED_FORMAT" });
    await expect(readApkgNotes(bytes("not zip"))).rejects.toMatchObject({
      code: "VALIDATION_FAILED",
    });
    expect(deps.repos.events.count(deps.userId)).toBe(0);
  });
  it("AC10: event failure rolls back cards and lexemes", async () => {
    const { deps } = setup();
    const tx = deps.tx;
    deps.tx = (fn) =>
      tx((repos) => {
        vi.spyOn(repos.events, "append").mockImplementation(() => {
          throw new Error("injected event failure");
        });
        return fn(repos);
      });
    await expect(
      importCards(deps, { fileName: "a.csv", bytes: bytes("apple,fruit"), deck: "d" }),
    ).rejects.toThrow("injected");
    expect(deps.repos.cards.count(deps.userId)).toBe(0);
    expect(deps.repos.lexemes.findByLemma("apple")).toBeUndefined();
  });
});
describe("T2.2/T2.3 word lists and knowledge", () => {
  it("AC7: imported baseline feeds KnownSet and sample article analysis", async () => {
    const { deps } = setup();
    await importWordList(deps, {
      fileName: "baseline.txt",
      bytes: readFileSync(
        new URL("../../../../../fixtures/wordlist/baseline-sample.txt", import.meta.url),
      ),
      name: "baseline",
      kind: "baseline",
    });
    const text = readFileSync(
      new URL("../../../../../fixtures/analyzer/samples/import-baseline.txt", import.meta.url),
      "utf8",
    );
    const options = { lemmatizer: defaultLemmatizer() };
    const withoutBaseline = analyzeText(text, new Set(), options);
    const withBaseline = analyzeText(text, getKnownSet(deps), options);
    expect(withBaseline.coverage).toBeGreaterThan(withoutBaseline.coverage);
    expect(withBaseline.unknownLemmas).toEqual([{ lemma: "galaxy", count: 1 }]);
    expect(withBaseline.coverage).toBe((withBaseline.tokenCount - 1) / withBaseline.tokenCount);
    expect(deps.repos.cards.count(deps.userId)).toBe(0);
    expect(deps.repos.events.count(deps.userId, "import")).toBe(1);
  });
  it("AC6: mid-import failure rolls back lists, items, lexemes, cards and events", async () => {
    const { deps } = setup();
    const db = openDb(":memory:");
    migrate(db);
    deps.repos = createRepos(db);
    deps.repos.users.ensureDefaultUser(deps.clock().getTime());
    deps.tx = (fn) => withTx(db, (conn) => fn(createRepos(conn)));
    const tx = deps.tx;
    let processed = 0;
    deps.tx = (fn) =>
      tx((repos) => {
        const insert = repos.cards.insert;
        vi.spyOn(repos.cards, "insert").mockImplementation((input) => {
          const id = insert(input);
          if (++processed === 2) throw new Error("mid-import failure");
          return id;
        });
        return fn(repos);
      });
    await expect(
      importWordList(deps, {
        fileName: "study.txt",
        bytes: bytes("apple\npear"),
        name: "rollback",
        kind: "study",
      }),
    ).rejects.toThrow("mid-import failure");
    expect(processed).toBe(2);
    expect(deps.repos.wordLists.find(deps.userId, "rollback", "study")).toBeUndefined();
    expect(deps.repos.wordLists.list(deps.userId)).toEqual([]);
    expect(deps.repos.lexemes.findByLemma("apple")).toBeUndefined();
    expect(deps.repos.lexemes.findByLemma("pear")).toBeUndefined();
    expect(deps.repos.cards.count(deps.userId)).toBe(0);
    expect(deps.repos.events.count(deps.userId)).toBe(0);
    for (const table of ["word_lists", "word_list_items", "lexemes", "cards", "events"])
      expect(db.$client.prepare(`SELECT COUNT(*) AS n FROM ${table}`).get()).toEqual({ n: 0 });
    // An independent import must insert both items afresh.
    deps.tx = tx;
    const retry = await importWordList(deps, {
      fileName: "study.txt",
      bytes: bytes("apple\npear"),
      name: "rollback",
      kind: "study",
    });
    expect(deps.repos.wordLists.list(deps.userId)[0]?.n).toBe(2);
    expect(retry).toMatchObject({ cardsAdded: 2, duplicates: 0 });
    closeDb(db);
  });
  it("AC1: combined variants and phonetics normalize like separate headword rows", async () => {
    const { deps } = setup();
    await importWordList(deps, {
      fileName: "combined.txt",
      bytes: bytes("color/colour /kʌlə/ n. 颜色"),
      name: "combined",
      kind: "baseline",
    });
    expect(deps.repos.wordLists.baselineLemmas(deps.userId).sort()).toEqual(["color", "colour"]);
    await importWordList(deps, {
      fileName: "separate.txt",
      bytes: bytes("color\ncolour"),
      name: "separate",
      kind: "baseline",
    });
    expect(deps.repos.wordLists.baselineLemmas(deps.userId).sort()).toEqual(["color", "colour"]);
    expect(deps.repos.wordLists.list(deps.userId).map((row) => row.n)).toEqual([2, 2]);
  });
  it("AC2: 500 generated alphabetic baseline identifiers import without cards", async () => {
    const { deps } = setup();
    const identifiers = Array.from(
      { length: 500 },
      (_, i) =>
        `entry${String.fromCharCode(97 + Math.floor(i / 26))}${String.fromCharCode(97 + (i % 26))}`,
    );
    const result = await importWordList(deps, {
      fileName: "synthetic.txt",
      bytes: bytes(identifiers.join("\n")),
      name: "generated",
      kind: "baseline",
    });
    expect(result.parsed).toBe(500);
    expect(deps.repos.wordLists.baselineLemmas(deps.userId)).toHaveLength(500);
    expect(deps.repos.cards.count(deps.userId)).toBe(0);
    expect(deps.repos.events.count(deps.userId, "import")).toBe(1);
  });
  it("AC2–AC6: baseline, variants, phrases, merging, study cards and unknown exclusions", async () => {
    const { deps, setTime } = setup();
    const input = {
      fileName: "list.txt",
      bytes: bytes("Unit 1\nApple n. 苹果\napple\ncolour (color)\nbe able to\n???\nglasses"),
      name: "mine",
      kind: "baseline" as const,
    };
    const result = await importWordList(deps, input);
    expect(result).toMatchObject({
      wordsAdded: 5,
      parsed: 4,
      duplicates: 1,
      phrases: 1,
      cardsAdded: 0,
      skippedLines: [{ line: 6, raw: "???" }],
    });
    expect(deps.repos.wordLists.baselineLemmas(deps.userId).sort()).toEqual([
      "apple",
      "be able to",
      "color",
      "colour",
      "glasses",
    ]);
    expect(deps.repos.cards.count(deps.userId)).toBe(0);
    await expect(importWordList(deps, input)).rejects.toMatchObject({ code: "VALIDATION_FAILED" });
    expect(deps.repos.events.count(deps.userId, "import")).toBe(1);
    expect(await importWordList(deps, { ...input, mergeIfExists: true })).toMatchObject({
      duplicates: 6,
      wordsAdded: 0,
      cardsAdded: 0,
    });
    expect(await importWordList(deps, { ...input, kind: "study" })).toMatchObject({
      cardsAdded: 5,
    });
    markUnknown(deps, { lemma: "apple", sentence: "An apple." });
    expect(getKnownSet(deps).has("apple")).toBe(false);
    expect(getKnownSet(deps, { targets: ["apple"] }).has("apple")).toBe(true);
    setTime(deps.clock().getTime() + 1000);
    rate(deps, {
      cardId: required(deps.repos.cards.listAll(deps.userId).find((c) => c.lemma === "apple")).id,
      rating: "easy",
      durationMs: 1,
    });
    expect(getKnownSet(deps).has("apple")).toBe(true);
    markKnown(deps, { lemma: "novel", sentence: "" });
    expect(getKnownSet(deps).has("novel")).toBe(true);
  });
  it("AC1/AC4: quoted CSV word list meanings do not become headwords", async () => {
    const { deps } = setup();
    await importWordList(deps, {
      fileName: "a.csv",
      bytes: bytes('word,meaning\n"apple","fruit, red"\n"colour (color)",颜色'),
      name: "study",
      kind: "study",
    });
    expect(
      deps.repos.cards.listAll(deps.userId).find((c) => c.lemma === "apple")?.back.meaning,
    ).toBe("fruit, red");
    expect(deps.repos.cards.count(deps.userId)).toBe(3);
  });
});
describe("T1.2/T1.4 review", () => {
  it("mutation rejects zero/exhausted new quota and stale future submissions atomically", () => {
    const { deps } = setup();
    const cardId = seedCard(deps, "apple");
    const other = seedCard(deps, "pear");
    const rejectWithoutWrites = (id: string) => {
      const card = deps.repos.cards.get(id);
      const logs = deps.repos.reviewLogs.listAll(deps.userId);
      const events = deps.repos.events.count(deps.userId);
      expect(() => rate(deps, { cardId: id, rating: "easy", durationMs: 0 })).toThrow(
        expect.objectContaining({ code: "VALIDATION_FAILED" }),
      );
      expect(deps.repos.cards.get(id)).toEqual(card);
      expect(deps.repos.reviewLogs.listAll(deps.userId)).toEqual(logs);
      expect(deps.repos.events.count(deps.userId)).toBe(events);
    };
    updateSettings(deps, { dailyNewLimit: 0 });
    rejectWithoutWrites(cardId);
    updateSettings(deps, { dailyNewLimit: 1 });
    rate(deps, { cardId, rating: "easy", durationMs: 0 });
    rejectWithoutWrites(other);
    rejectWithoutWrites(cardId);
  });
  it("mutation enforces review quota, future learning due, but permits due learning at zero quota", () => {
    const { deps } = setup();
    const ids = [seedCard(deps, "apple"), seedCard(deps, "pear")];
    for (const cardId of ids) {
      const card = required(deps.repos.cards.get(cardId));
      deps.repos.cards.updateSrs(cardId, {
        ...card.srs,
        state: "review",
        stability: 10,
        difficulty: 5,
        reps: 1,
        lastReview: new Date(deps.clock().getTime() - 86400000),
      });
    }
    updateSettings(deps, { dailyReviewLimit: 1 });
    rate(deps, { cardId: required(ids[0]), rating: "again", durationMs: 0 });
    const before = deps.repos.cards.get(required(ids[1]));
    expect(() => rate(deps, { cardId: required(ids[1]), rating: "good", durationMs: 0 })).toThrow(
      expect.objectContaining({ code: "VALIDATION_FAILED" }),
    );
    expect(deps.repos.cards.get(required(ids[1]))).toEqual(before);
    expect(() => rate(deps, { cardId: required(ids[0]), rating: "good", durationMs: 0 })).toThrow(
      expect.objectContaining({ code: "VALIDATION_FAILED" }),
    );
    updateSettings(deps, { dailyNewLimit: 0, dailyReviewLimit: 0 });
    const learning = required(deps.repos.cards.get(required(ids[0])));
    deps.repos.cards.updateSrs(learning.id, { ...learning.srs, due: deps.clock() });
    rate(deps, { cardId: learning.id, rating: "good", durationMs: 0 });
    expect(deps.repos.reviewLogs.listAll(deps.userId)).toHaveLength(2);
    expect(deps.repos.events.count(deps.userId, "review")).toBe(2);
  });
  it("T1.4 AC7: scored cards, paired logs/events and queue counters survive file reopen", () => {
    const dir = mkdtempSync(path.join(tmpdir(), "review-reopen-"));
    const env = {
      LLM_PROVIDER: "fake",
      TTS_PROVIDER: "none",
      DATABASE_PATH: path.join(dir, "app.sqlite"),
    };
    const now = new Date("2026-10-12T02:00:00Z");
    let deps = createAppDeps(env, { clock: () => now });
    try {
      const cardId = seedCard(deps, "apple");
      seedCard(deps, "pear");
      rate(deps, { cardId, rating: "easy", durationMs: 5 });
      const card = deps.repos.cards.get(cardId);
      const logs = deps.repos.reviewLogs.listAll(deps.userId);
      const events = deps.repos.events.listByType(deps.userId, "review");
      const queue = getQueue(deps);
      const range = dayRange(now, deps.settings());
      const counts = [
        deps.repos.reviewLogs.countNewIntroduced(deps.userId, range),
        deps.repos.reviewLogs.countReviews(deps.userId, range, "review"),
      ];
      deps.close();
      deps = createAppDeps(env, { clock: () => now });
      expect(deps.repos.cards.get(cardId)).toEqual(card);
      expect(deps.repos.reviewLogs.listAll(deps.userId)).toEqual(logs);
      expect(deps.repos.events.listByType(deps.userId, "review")).toEqual(events);
      expect(logs).toHaveLength(1);
      expect(events).toHaveLength(1);
      expect(counts).toEqual([1, 0]);
      expect([
        deps.repos.reviewLogs.countNewIntroduced(deps.userId, range),
        deps.repos.reviewLogs.countReviews(deps.userId, range, "review"),
      ]).toEqual(counts);
      expect(getQueue(deps)).toEqual(queue);
    } finally {
      deps.close();
      rmSync(dir, { recursive: true, force: true });
    }
  });
  it("AC5/AC7; T1.4 AC1: ratings write exactly paired events/logs and consume new slots", async () => {
    const { deps } = setup();
    await importCards(deps, { fileName: "a.csv", bytes: bytes(words.join("\n")), deck: "d" });
    const ids = getQueue(deps).new;
    expect(ids).toHaveLength(20);
    for (const [i, id] of ids.slice(0, 10).entries())
      rate(deps, {
        cardId: id,
        rating: required((["again", "hard", "good", "easy"] as const)[i % 4]),
        durationMs: i,
      });
    expect(getQueue(deps).new).toHaveLength(10);
    const logs = deps.repos.reviewLogs.listAll(deps.userId);
    const events = deps.repos.events.listByType(deps.userId, "review");
    expect(logs).toHaveLength(10);
    expect(events).toHaveLength(10);
    logs.forEach((log, i) => {
      expect(events[i]?.payload).toMatchObject({
        cardId: log.cardId,
        rating: log.rating,
        stateBefore: log.stateBefore,
        dueBefore: log.dueBefore,
      });
    });
  });
  it("AC7; T1.4 AC2: scheduling/log writes roll back on invalid event", () => {
    const { deps } = setup();
    const cardId = seedCard(deps, "apple");
    const before = deps.repos.cards.get(cardId);
    const tx = deps.tx;
    deps.tx = (fn) =>
      tx((repos) => {
        const append = repos.events.append;
        repos.events.append = ((user, type) => append(user, type, {} as never)) as typeof append;
        return fn(repos);
      });
    expect(() => rate(deps, { cardId, rating: "easy", durationMs: 1 })).toThrow();
    expect(deps.repos.cards.get(cardId)).toEqual(before);
    expect(deps.repos.reviewLogs.listAll(deps.userId)).toEqual([]);
    expect(deps.repos.events.count(deps.userId)).toBe(0);
  });
  it("AC8/AC9: validates missing/suspended cards and counts at learning-day boundary", () => {
    const { deps, setTime } = setup();
    const cardId = seedCard(deps, "apple");
    expect(() => rate(deps, { cardId: "missing", rating: "good", durationMs: 0 })).toThrow(
      expect.objectContaining({ code: "NOT_FOUND" }),
    );
    setSuspended(deps, { cardId, suspended: true });
    expect(() => rate(deps, { cardId, rating: "good", durationMs: 0 })).toThrow(
      expect.objectContaining({ code: "VALIDATION_FAILED" }),
    );
    expect(getQueue(deps).new).toEqual([]);
    setSuspended(deps, { cardId, suspended: false });
    setTime("2026-10-12T19:59:00Z");
    rate(deps, { cardId, rating: "easy", durationMs: 0 });
    expect(
      deps.repos.reviewLogs.countNewIntroduced(deps.userId, {
        start: Date.parse("2026-10-11T20:00:00Z"),
        end: Date.parse("2026-10-12T20:00:00Z"),
      }),
    ).toBe(1);
    setTime("2026-10-12T20:00:00Z");
    const next = seedCard(deps, "pear");
    expect(getQueue(deps).new).toEqual([next]);
  });
});
describe("T1.3 cards; T1.5 settings", () => {
  const invalidSettings = [
    { level: "invalid" },
    { dailyNewLimit: 201 },
    { dailyReviewLimit: 2001 },
    { requestRetention: 0.69 },
    { tier: "invalid" },
    { articleMinWords: 99 },
    { articleMaxWords: 1001 },
    { targetCount: 0 },
    { controlRatio: 0.51 },
    { generationMaxRetries: -1 },
    { lookupMode: "invalid" },
    { ttsDailyCharLimit: 200001 },
    { timezone: "invalid-zone" },
    { dayStartHour: 24 },
  ];
  for (const patch of invalidSettings)
    it(`T1.5 AC1: rejects invalid ${Object.keys(patch)[0]} without writes`, () => {
      const { deps } = setup();
      expect(() => updateSettings(deps, patch as never)).toThrow(
        expect.objectContaining({ code: "VALIDATION_FAILED" }),
      );
      expect(deps.repos.events.count(deps.userId)).toBe(0);
    });
  it("AC3: concurrent AI definition is cached with one card edit", async () => {
    const { deps, llm } = setup();
    const id = seedCard(deps, "apple");
    await Promise.all([ensureMeaning(deps, id), ensureMeaning(deps, id)]);
    await ensureMeaning(deps, id);
    expect(llm.calls).toBe(1);
    expect(deps.repos.events.count(deps.userId, "card_edit")).toBe(1);
    expect(deps.repos.cards.get(id)?.back).toMatchObject({
      meaning: "演示释义",
      meaningSource: "ai",
    });
  });
  it("AC4: invalid AI definition logs failures but leaves card and rating usable", async () => {
    const { deps } = setup(["bad", "bad"]);
    const id = seedCard(deps, "apple");
    await expect(ensureMeaning(deps, id)).rejects.toMatchObject({ code: "LLM_OUTPUT_INVALID" });
    expect(deps.repos.cards.get(id)?.back.meaning).toBeUndefined();
    expect(deps.repos.events.count(deps.userId, "card_edit")).toBe(0);
    expect(deps.repos.llmCalls.list(deps.userId).map((c) => c.status)).toEqual([
      "invalid_output",
      "invalid_output",
    ]);
    rate(deps, { cardId: id, rating: "easy", durationMs: 0 });
  });
  it("AC5–AC7: edits/suspension events and filtered library queries", () => {
    const { deps } = setup();
    const id = seedCard(deps, "apple");
    seedCard(deps, "pear");
    updateCardText(deps, { cardId: id, meaning: "苹果", example: "Apple." });
    setSuspended(deps, { cardId: id, suspended: true });
    setSuspended(deps, { cardId: id, suspended: false });
    expect(deps.repos.events.count(deps.userId, "card_suspend")).toBe(2);
    expect(listCards(deps, { q: "APP", state: "new", deck: "test" })).toMatchObject({
      total: 1,
      rows: [{ back: { meaningSource: "user", meaning: "苹果" } }],
    });
    expect(listCards(deps, { offset: 1, limit: 1 }).rows[0]?.lemma).toBe("pear");
  });
  it("T1.5 AC1–AC3: settings validate atomically, report actual changed keys and affect queue", () => {
    const { deps } = setup();
    for (let i = 0; i < 10; i++) seedCard(deps, `word${i}`);
    expect(() => updateSettings(deps, { dailyNewLimit: -1 })).toThrow(
      expect.objectContaining({ code: "VALIDATION_FAILED" }),
    );
    expect(() => updateSettings(deps, { articleMinWords: 500, articleMaxWords: 100 })).toThrow();
    expect(deps.repos.events.count(deps.userId)).toBe(0);
    updateSettings(deps, { dailyNewLimit: 5, tier: "medium" });
    updateSettings(deps, { dailyNewLimit: 5 });
    expect(getQueue(deps).new).toHaveLength(5);
    expect(
      deps.repos.events.listByType(deps.userId, "settings_update").map((e) => e.payload.changed),
    ).toEqual([["dailyNewLimit"]]);
    expect(getSettingsView(deps).providers).not.toHaveProperty("apiKey");
  });
});
