import { DAY_MS, dayRange } from "@eng-learn/core";
import { FakeTTS, OpenAICompatibleTTS } from "@eng-learn/providers";
import { describe, expect, it, vi } from "vitest";
import { recordArticleEvent } from "./article";
import { checkTtsBudget, ensureAudio, getTtsUsage, readAudio } from "./audio";
import { flagUnnatural, latestDifficulty, submitDifficulty } from "./feedback";
import { rate } from "./review";
import { updateSettings } from "./settings";
import { getExperiment, getStats } from "./stats";
import { required, seedArticle, seedCard, setup } from "./test-support";

describe("T5.1 audio", () => {
  it("AC1/AC4/AC7: concurrent/repeated requests share one synthesis, support Range reads", async () => {
    const { deps, tts } = setup();
    const articleId = seedArticle(deps);
    const results = await Promise.all(
      Array.from({ length: 5 }, () => ensureAudio(deps, articleId)),
    );
    expect(tts.calls).toBe(1);
    expect(results[0]).toEqual(results[4]);
    const cached = await ensureAudio(deps, articleId);
    expect(cached).toMatchObject({ mode: "server", cacheHit: true });
    expect(deps.repos.events.count(deps.userId, "audio_generate")).toBe(1);
    if (cached.mode !== "server") throw new Error("expected server");
    const key = cached.url.slice("/api/audio/".length);
    expect(await readAudio(deps, key, "bytes=0-99")).toMatchObject({
      status: 206,
      headers: { "Content-Length": "100", "Content-Type": "audio/wav" },
    });
    expect((await readAudio(deps, key, "bytes=-100")).data.length).toBe(100);
    expect((await readAudio(deps, key, "bytes=999999-")).status).toBe(416);
    await expect(readAudio(deps, "../secret")).rejects.toMatchObject({ code: "NOT_FOUND" });
  });
  it("AC2/AC3: configuration invalidates hash; missing blob is regenerated and charged again", async () => {
    const { deps, tts, blobs } = setup();
    const articleId = seedArticle(deps);
    const first = await ensureAudio(deps, articleId);
    if (first.mode !== "server") throw new Error("server");
    const usage = getTtsUsage(deps).usedChars;
    await blobs.delete(first.url.slice("/api/audio/".length));
    expect(await ensureAudio(deps, articleId)).toMatchObject({
      cacheHit: false,
      audioId: first.audioId,
    });
    expect(tts.calls).toBe(2);
    expect(getTtsUsage(deps).usedChars).toBe(usage * 2);
    deps.ttsProvider = () => new FakeTTS("another-configured-voice", "another-configured-model");
    const second = await ensureAudio(deps, articleId);
    expect(second).toMatchObject({ cacheHit: false });
    if (second.mode !== "server") throw new Error("server");
    expect(second.url).not.toBe(first.url);
  });
  it("AC5/AC6: browser fallback returns text with no writes", async () => {
    const { deps, tts } = setup();
    const articleId = seedArticle(deps);
    deps.ttsProvider = () => null;
    expect(await ensureAudio(deps, articleId)).toMatchObject({
      mode: "browser",
      text: expect.stringContaining("Home\n\n"),
    });
    expect(tts.calls).toBe(0);
    expect(deps.repos.events.count(deps.userId, "audio_generate")).toBe(0);
  });
});
describe("T5.3 quota", () => {
  for (const failure of ["partial synthesis", "blob", "transaction"] as const)
    it(`reserves paid work independently of ${failure} failure`, async () => {
      const { deps, tts, blobs } = setup();
      const articleId = seedArticle(deps, "body", "title");
      const chars = "title\n\nbody".length;
      updateSettings(deps, { ttsDailyCharLimit: chars });
      if (failure === "partial synthesis")
        vi.spyOn(tts, "synthesize").mockImplementation(async () => {
          // A provider may have completed a paid chunk before the next chunk fails.
          tts.calls++;
          throw new Error("second chunk failed");
        });
      if (failure === "blob") vi.spyOn(blobs, "put").mockRejectedValue(new Error("blob failed"));
      if (failure === "transaction") {
        const tx = deps.tx;
        deps.tx = (fn) =>
          tx((repos) => {
            const append = repos.events.append;
            repos.events.append = ((...args: Parameters<typeof append>) => {
              if (args[1] === "audio_generate") throw new Error("cache event failed");
              return append(...args);
            }) as typeof append;
            return fn(repos);
          });
      }
      await expect(ensureAudio(deps, articleId)).rejects.toThrow();
      expect(getTtsUsage(deps).usedChars).toBe(chars);
      expect(deps.repos.events.count(deps.userId, "audio_reserve")).toBe(1);
      expect(deps.repos.events.count(deps.userId, "audio_generate")).toBe(0);
      expect(
        deps.repos.audios.getByHash(
          (await import("./audio")).audioTextHash({
            provider: tts.provider,
            model: tts.model,
            voice: tts.voice,
            text: "title\n\nbody",
          }),
        ),
      ).toBeUndefined();
      await expect(ensureAudio(deps, articleId)).rejects.toMatchObject({
        code: "TTS_DAILY_LIMIT_EXCEEDED",
      });
      expect(tts.calls).toBe(1);
    });
  it("compatible adapter partial synthesis remains fully reserved after a later chunk fails", async () => {
    const { deps } = setup();
    let calls = 0;
    deps.ttsProvider = () =>
      new OpenAICompatibleTTS({
        baseURL: "https://example.invalid",
        apiKey: "test-only",
        model: "configured-model",
        voice: "configured-voice",
        fetch: async () => {
          calls++;
          return calls === 1
            ? new Response(new Uint8Array([1]), { headers: { "content-type": "audio/mpeg" } })
            : new Response(JSON.stringify({ error: { message: "chunk failed" } }), {
                status: 500,
                headers: { "content-type": "application/json" },
              });
        },
      });
    const body = "text ".repeat(1000);
    const articleId = seedArticle(deps, body, "title");
    const chars = `title\n\n${body}`.length;
    updateSettings(deps, { ttsDailyCharLimit: chars });
    await expect(ensureAudio(deps, articleId)).rejects.toMatchObject({ code: "PROVIDER_ERROR" });
    expect(calls).toBe(2);
    expect(getTtsUsage(deps).usedChars).toBe(chars);
    await expect(ensureAudio(deps, articleId)).rejects.toMatchObject({
      code: "TTS_DAILY_LIMIT_EXCEEDED",
    });
    expect(calls).toBe(2);
  });
  it("reservation persistence failure prevents synthesis and blob writes", async () => {
    const { deps, tts, blobs } = setup();
    const articleId = seedArticle(deps);
    const put = vi.spyOn(blobs, "put");
    deps.tx = () => {
      throw new Error("database unavailable");
    };
    await expect(ensureAudio(deps, articleId)).rejects.toThrow("database unavailable");
    expect(tts.calls).toBe(0);
    expect(put).not.toHaveBeenCalled();
    expect(getTtsUsage(deps).usedChars).toBe(0);
  });
  it("AC1/AC2/AC3/AC5: exact limit allowed, over-limit no IO, cached work still available", async () => {
    const { deps, tts } = setup();
    const articleId = seedArticle(deps);
    const article = required(deps.repos.articles.get(articleId));
    const chars = `${article.title}\n\n${article.body}`.length;
    updateSettings(deps, { ttsDailyCharLimit: chars });
    checkTtsBudget(deps, chars);
    await ensureAudio(deps, articleId);
    expect(() => checkTtsBudget(deps, 1)).toThrow(
      expect.objectContaining({ code: "TTS_DAILY_LIMIT_EXCEEDED" }),
    );
    const other = seedArticle(deps, "Another article.");
    await expect(ensureAudio(deps, other)).rejects.toMatchObject({
      code: "TTS_DAILY_LIMIT_EXCEEDED",
    });
    expect(tts.calls).toBe(1);
    updateSettings(deps, { ttsDailyCharLimit: 0 });
    expect(await ensureAudio(deps, articleId)).toMatchObject({ cacheHit: true });
    await expect(ensureAudio(deps, other)).rejects.toMatchObject({
      code: "TTS_DAILY_LIMIT_EXCEEDED",
    });
    expect(deps.repos.events.count(deps.userId, "audio_generate")).toBe(1);
  });
  it("AC4/AC7: usage resets precisely at configured day boundary", async () => {
    const { deps, setTime } = setup();
    const articleId = seedArticle(deps);
    setTime("2026-10-12T19:59:00Z");
    await ensureAudio(deps, articleId);
    expect(getTtsUsage(deps).usedChars).toBeGreaterThan(0);
    setTime("2026-10-12T20:00:00Z");
    expect(getTtsUsage(deps).usedChars).toBe(0);
  });
  it("AC2: different concurrent hashes cannot exceed budget; failed work releases lock", async () => {
    const { deps, tts } = setup();
    const a = seedArticle(deps, "a", "a");
    const b = seedArticle(deps, "b", "b");
    updateSettings(deps, { ttsDailyCharLimit: 4 });
    const result = await Promise.allSettled([ensureAudio(deps, a), ensureAudio(deps, b)]);
    expect(result.map((r) => r.status)).toEqual(["fulfilled", "rejected"]);
    expect(tts.calls).toBe(1);
    expect(getTtsUsage(deps).usedChars).toBe(4);
    updateSettings(deps, { ttsDailyCharLimit: 8 });
    await ensureAudio(deps, b);
    expect(tts.calls).toBe(2);
  });
});
describe("T6.1 feedback", () => {
  it("AC1–AC3: append-only feedback changes take latest per reading/listening", () => {
    const { deps } = setup();
    const articleId = seedArticle(deps);
    submitDifficulty(deps, { articleId, value: "too_hard", after: "reading" });
    submitDifficulty(deps, { articleId, value: "just_right", after: "reading" });
    submitDifficulty(deps, { articleId, value: "too_easy", after: "listening" });
    expect(latestDifficulty(deps, articleId)).toEqual({
      reading: "just_right",
      listening: "too_easy",
    });
    expect(deps.repos.events.count(deps.userId, "difficulty_feedback")).toBe(3);
    expect(() =>
      submitDifficulty(deps, { articleId: "missing", value: "just_right", after: "reading" }),
    ).toThrow(expect.objectContaining({ code: "NOT_FOUND" }));
  });
  it("AC4/AC5: sentence indexes validated before batch writes and notes capped", () => {
    const { deps } = setup();
    const articleId = seedArticle(deps);
    expect(
      flagUnnatural(deps, { articleId, sentenceIndexes: [0, 1, 2], note: "unnatural" }),
    ).toEqual({ events: 3 });
    expect(flagUnnatural(deps, { articleId, sentenceIndexes: [], note: "note only" })).toEqual({
      events: 1,
    });
    expect(() => flagUnnatural(deps, { articleId, sentenceIndexes: [0, 99] })).toThrow();
    expect(() =>
      flagUnnatural(deps, { articleId, sentenceIndexes: [], note: "a".repeat(201) }),
    ).toThrow();
    expect(deps.repos.events.count(deps.userId, "unnatural_flag")).toBe(4);
  });
});
describe("T6.2/T6.3 experiment and stats", () => {
  it("T6.2 AC1–AC5: real repository samples give 0.2−0.4, first post-article review only", () => {
    const { deps, setTime } = setup();
    const articleId = seedArticle(deps);
    const time = deps.clock().getTime();
    const cards = Array.from({ length: 16 }, (_, i) => seedCard(deps, `word${i}`));
    deps.tx((repos) =>
      repos.articles.insertTargets(
        cards.map((id, i) => ({
          articleId,
          lexemeId: required(repos.cards.get(id)).lexemeId,
          role: i < 10 ? "exposed" : "control",
          occurrences: i < 10 ? 2 : 0,
        })),
      ),
    );
    // At the exact creation timestamp this pre-exposure review is not eligible.
    rate(deps, { cardId: required(cards[0]), rating: "good", durationMs: 0 });
    setTime(time + DAY_MS);
    for (let i = 0; i < 15; i++)
      rate(deps, {
        cardId: required(cards[i]),
        rating: i < 2 || (i >= 10 && i < 12) ? "again" : "good",
        durationMs: 0,
      });
    setTime(time + 2 * DAY_MS);
    rate(deps, { cardId: required(cards[2]), rating: "again", durationMs: 0 });
    setTime(time + 8 * DAY_MS);
    rate(deps, { cardId: required(cards[15]), rating: "again", durationMs: 0 });
    expect(getExperiment(deps)).toEqual({
      exposed: { n: 10, again: 2, againRate: 0.2, noReview: 0 },
      control: { n: 5, again: 2, againRate: 0.4, noReview: 1 },
      diff: -0.2,
    });
    expect(getStats(deps).h3).toEqual(getExperiment(deps));
  });
  it("T6.3 AC1–AC6: logs reconcile, Monday 04:00 week boundary, completion and latest feedback", () => {
    const { deps, setTime } = setup();
    const id = seedCard(deps, "apple");
    setTime("2026-10-11T19:59:00Z");
    rate(deps, { cardId: id, rating: "easy", durationMs: 0 });
    expect(getStats(deps).learning.week.newLearned).toBe(1);
    setTime("2026-10-11T20:00:00Z");
    expect(getStats(deps).learning.week.newLearned).toBe(0);
    const next = seedCard(deps, "pear");
    rate(deps, { cardId: next, rating: "easy", durationMs: 0 });
    const articles = Array.from({ length: 4 }, () => seedArticle(deps));
    for (const [i, articleId] of articles.entries()) {
      recordArticleEvent(deps, { articleId, type: "article_open" });
      if (i < 3) recordArticleEvent(deps, { articleId, type: "article_complete" });
    }
    submitDifficulty(deps, {
      articleId: required(articles[0]),
      after: "reading",
      value: "too_hard",
    });
    submitDifficulty(deps, {
      articleId: required(articles[0]),
      after: "reading",
      value: "just_right",
    });
    const stats = getStats(deps);
    expect(stats.h1.completionRate).toBe(0.75);
    expect(stats.h2.distribution).toEqual({ too_hard: 0, just_right: 1, too_easy: 0 });
    const range = dayRange(deps.clock(), deps.settings());
    const logs = deps.repos.reviewLogs
      .listAll(deps.userId)
      .filter((l) => l.reviewedAt >= range.start && l.reviewedAt < range.end);
    expect(stats.learning.today).toEqual({
      reviews: logs.length,
      newLearned: logs.filter((l) => l.stateBefore === "new").length,
    });
    expect(stats.learnedWordsTotal).toBe(2);
    expect(stats.coverageTrend).toHaveLength(4);
  });
  it("AC3: no activity gives null metrics rather than NaN", () => {
    const { deps } = setup();
    const stats = getStats(deps);
    expect(stats.h1.completionRate).toBeNull();
    expect(stats.h2.justRightRate).toBeNull();
    expect(stats.h3.diff).toBeNull();
    expect(stats.unknownRatioMedian.all).toBeNull();
  });
});
