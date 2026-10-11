import { describe, expect, it } from "vitest";
import { getArticleView, getTodayView, recordArticleEvent } from "./article";
import { generateArticle, selectTargetsForUser } from "./generation";
import { rate } from "./review";
import { required, seedCard, setup } from "./test-support";

function target(deps: ReturnType<typeof setup>["deps"], lemma = "decide") {
  const id = seedCard(deps, lemma);
  rate(deps, { cardId: id, rating: "easy", durationMs: 1 });
  return id;
}
const reply = (body: string) => JSON.stringify({ title: "Today", body, names: [] });
describe("T3.2 generateArticle", () => {
  it("AC1/AC8: persists article, all targets, correct model/version, event and LLM log", async () => {
    const { deps, llm } = setup([reply("I decide. You decide.")]);
    target(deps);
    const result = await generateArticle(deps);
    expect(result).toMatchObject({ coverage: 1, belowTier: false, missingTargets: [] });
    expect(llm.calls).toBe(1);
    expect(deps.repos.articles.targets(result.articleId)).toMatchObject([
      { lemma: "decide", role: "exposed", occurrences: 2 },
    ]);
    expect(deps.repos.articles.get(result.articleId)).toMatchObject({
      promptVersion: "article.v1",
      model: llm.model,
    });
    expect(deps.repos.events.count(deps.userId, "article_generate")).toBe(1);
    expect(deps.repos.llmCalls.list(deps.userId)).toMatchObject([
      { status: "ok", model: llm.model },
    ]);
  });
  it("AC2: malformed JSON repairs and logs both calls", async () => {
    const { deps, llm } = setup(["not json", reply("We decide. We decide.")]);
    target(deps);
    await generateArticle(deps);
    expect(deps.repos.llmCalls.list(deps.userId).map((c) => c.status)).toEqual([
      "invalid_output",
      "ok",
    ]);
    expect(llm.requests[1]?.messages.some((m) => m.content.includes("not json"))).toBe(true);
  });
  it("AC3: two invalid replies leave no article, targets or generation event", async () => {
    const { deps } = setup(["bad", "bad"]);
    target(deps);
    await expect(generateArticle(deps)).rejects.toMatchObject({ code: "LLM_OUTPUT_INVALID" });
    expect(deps.repos.articles.list(deps.userId)).toEqual([]);
    expect(deps.repos.articles.samples(deps.userId)).toEqual([]);
    expect(deps.repos.events.count(deps.userId, "article_generate")).toBe(0);
    expect(deps.repos.llmCalls.list(deps.userId).map((c) => c.status)).toEqual([
      "invalid_output",
      "invalid_output",
    ]);
  });
  it("AC4: coverage retry avoids unknown words and saves better response", async () => {
    const { deps, llm } = setup([
      reply("I decide. An elephant investigates a galaxy."),
      reply("I decide. You decide."),
    ]);
    target(deps);
    const result = await generateArticle(deps);
    const second = llm.requests[1]?.messages.map((m) => m.content).join("\n") ?? "";
    expect(second).toContain("Do NOT use these words:");
    expect(second).toContain("elephant");
    expect(result.coverage).toBe(1);
    expect(deps.repos.articles.get(result.articleId)?.body).toBe("I decide. You decide.");
  });
  it("AC5: bounded coverage retries save first attempt if later attempt is worse", async () => {
    const { deps, llm } = setup([
      reply("I decide. An elephant."),
      reply("Galaxy elephant investigation."),
    ]);
    target(deps);
    const result = await generateArticle(deps);
    expect(llm.calls).toBe(2);
    expect(result.belowTier).toBe(true);
    expect(deps.repos.articles.get(result.articleId)?.body).toBe("I decide. An elephant.");
    const analysis = required(getArticleView(deps, result.articleId)).analysis;
    expect(analysis.attempts).toHaveLength(2);
    expect(analysis.missingTargets).toEqual([]);
  });
  it("AC6: missing exposed targets are reported without an extra call", async () => {
    const { deps, llm } = setup([reply("I am here. You are here.")]);
    target(deps);
    const result = await generateArticle(deps);
    expect(result.missingTargets).toEqual(["decide"]);
    expect(llm.calls).toBe(1);
    expect(deps.repos.articles.targets(result.articleId)[0]?.occurrences).toBe(0);
  });
  it("AC7: control words are absent from both prompts but counted and not automatically known", async () => {
    const { deps, llm } = setup();
    const lemmas = [
      "decide",
      "galaxy",
      "elephant",
      "investigate",
      "wander",
      "bright",
      "reflect",
      "ocean",
    ];
    // Low-quality new cards are targets, not known Review cards.
    for (const lemma of lemmas)
      rate(deps, { cardId: seedCard(deps, lemma), rating: "again", durationMs: 0 });
    const chosen = selectTargetsForUser(deps);
    const control = chosen.filter((t) => t.role === "control").map((t) => t.lemma);
    // Reset deterministic selection and make output intentionally contain every control.
    const { seededRng } = await import("@eng-learn/core");
    deps.rng = seededRng(7);
    llm.push(reply(`I decide. ${control.join(" ")}.`), reply(`I decide. ${control.join(" ")}.`));
    const result = await generateArticle(deps);
    expect(llm.calls).toBe(2);
    expect(result.coverage).toBeLessThan(1);
    for (const word of control)
      for (const request of llm.requests)
        expect(request.messages.map((m) => m.content).join("\n")).not.toContain(word);
    expect(
      deps.repos.articles
        .targets(result.articleId)
        .filter((t) => t.role === "control")
        .every((t) => t.occurrences === 1),
    ).toBe(true);
  });
  it("AC7: invalid article JSON repair redacts controls from output and parser diagnostics", async () => {
    const { deps, llm } = setup();
    for (const lemma of [
      "decide",
      "galaxy",
      "elephant",
      "investigate",
      "wander",
      "bright",
      "reflect",
      "ocean",
    ])
      rate(deps, { cardId: seedCard(deps, lemma), rating: "again", durationMs: 0 });
    const chosen = selectTargetsForUser(deps);
    const control = chosen.filter((t) => t.role === "control").map((t) => t.lemma);
    const exposed = chosen.filter((t) => t.role === "exposed").map((t) => t.lemma);
    const { seededRng } = await import("@eng-learn/core");
    deps.rng = seededRng(7);
    // Bare invalid token is also quoted in JSON.parse's diagnostic on current Node.
    llm.push(
      `${control.join(" ")} not JSON`,
      reply(`We ${exposed.join(" ")}. We ${exposed.join(" ")}.`),
    );
    await generateArticle(deps);
    expect(llm.calls).toBe(2);
    for (const word of control)
      for (const request of llm.requests)
        expect(
          request.messages
            .map((m) => m.content)
            .join("\n")
            .toLowerCase(),
        ).not.toContain(word);
    expect(deps.repos.llmCalls.list(deps.userId).map((c) => c.status)).toEqual([
      "invalid_output",
      "ok",
    ]);
  });
  it("AC9: no targets means no LLM call", async () => {
    const { deps, llm } = setup();
    seedCard(deps, "unused");
    await expect(generateArticle(deps)).rejects.toMatchObject({ code: "NO_TARGETS" });
    expect(llm.calls).toBe(0);
  });
  it("T3.1 AC7: recent exposed occurrences exclude targets; controls do not", async () => {
    const { deps } = setup([reply("We decide. We decide. We decide.")]);
    target(deps);
    const generated = await generateArticle(deps);
    expect(selectTargetsForUser(deps)).toEqual([]);
    const targets = deps.repos.articles.targets(generated.articleId);
    expect(targets[0]?.occurrences).toBe(3);
  });
  it("T3.3 AC2–AC5; T3.4 AC1/AC6: preserves paragraphs/offsets and completion per open", async () => {
    const body = "I decide.\n\n You decide!";
    const { deps } = setup([reply(body)]);
    target(deps);
    const result = await generateArticle(deps);
    const view = required(getArticleView(deps, result.articleId));
    expect(view.paragraphs.map((p) => p.text).join("")).toBe(body);
    for (const p of view.paragraphs)
      for (const token of p.tokens) expect(body.slice(token.start, token.end)).toBe(token.surface);
    const openId = recordArticleEvent(deps, { articleId: result.articleId, type: "article_open" });
    recordArticleEvent(deps, {
      articleId: result.articleId,
      type: "article_complete",
      via: "scroll",
      openId,
    });
    recordArticleEvent(deps, {
      articleId: result.articleId,
      type: "article_complete",
      via: "button",
      openId,
    });
    expect(deps.repos.events.count(deps.userId, "article_complete")).toBe(1);
    const nextOpen = recordArticleEvent(deps, {
      articleId: result.articleId,
      type: "article_open",
    });
    recordArticleEvent(deps, {
      articleId: result.articleId,
      type: "article_complete",
      openId: nextOpen,
    });
    expect(deps.repos.events.count(deps.userId, "article_complete")).toBe(2);
    expect(getTodayView(deps).articles).toMatchObject([{ id: result.articleId, completed: true }]);
    expect(getArticleView(deps, "missing")).toBeNull();
    expect(() => recordArticleEvent(deps, { articleId: "missing", type: "article_open" })).toThrow(
      expect.objectContaining({ code: "NOT_FOUND" }),
    );
  });
});
