import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { newSrsCard } from "@eng-learn/core";
import { describe, expect, it } from "vitest";
import { createAppDeps } from "../apps/web/src/server/deps";

function run(script: string, args: string[], env?: Record<string, string>) {
  return spawnSync(process.execPath, ["--import", "tsx", script, ...args], {
    encoding: "utf8",
    timeout: 20000,
    env: { ...process.env, ...env },
  });
}
describe("runnable tool scripts", () => {
  it("AC5: boundary CLI returns nonzero for forbidden core IO", () => {
    const root = mkdtempSync(path.join(tmpdir(), "boundary-test-"));
    try {
      mkdirSync(path.join(root, "packages/core/src"), { recursive: true });
      writeFileSync(path.join(root, "packages/core/src/bad.ts"), 'import fs from "node:fs"');
      const result = run("tools/check-boundaries.ts", [root]);
      expect(result.status).toBe(1);
      expect(result.stderr).toContain("core IO import");
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
  it("S3 AC2: fake evaluation completes 30 groups and prints all metrics", () => {
    const result = run("tools/eval/generation.ts", ["--fake"], {
      EVAL_BASELINE_PATH: "fixtures/wordlist/baseline-sample.txt",
      EVAL_MAX_COST_USD: "1",
    });
    expect(result.status, result.stderr).toBe(0);
    expect(result.stdout).toContain("30/30 groups");
    expect(result.stdout).toContain("firstJsonValidRate");
    expect(result.stdout).toContain("targetTwiceRate");
    expect(result.stdout).toContain("actualCostUsd");
  });
  it("S3 AC3: real evaluation without prices refuses before any network call", () => {
    const result = run("tools/eval/generation.ts", ["--allow-paid"], {
      LLM_PROVIDER: "fake",
      LLM_PRICE_INPUT_PER_MTOK: "",
      LLM_PRICE_OUTPUT_PER_MTOK: "",
    });
    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain("positive LLM_PRICE");
  });
  it("T6.2 AC1/2/6: CLI reports seeded group rates and only the first later review", () => {
    const dir = mkdtempSync(path.join(tmpdir(), "experiment-cli-"));
    const database = path.join(dir, "seed.sqlite");
    const now = new Date("2026-10-01T00:00:00Z");
    const deps = createAppDeps(
      { LLM_PROVIDER: "fake", TTS_PROVIDER: "none", DATABASE_PATH: database },
      { clock: () => now },
    );
    try {
      deps.tx((repos) => {
        const articleId = repos.articles.insert({
          userId: deps.userId,
          title: "Original test text",
          body: "We learn together.",
          kind: "generated",
          genre: "story",
          levelTier: "medium",
          promptVersion: "article.v1",
          model: deps.llm.model,
          source: "llm",
          license: null,
          analysisJson: "{}",
          now: now.getTime(),
        });
        for (let i = 0; i < 15; i++) {
          const role = i < 10 ? "exposed" : "control";
          const lexemeId = repos.lexemes.upsert(`sample-${i}`, "", now.getTime());
          const cardId = repos.cards.insert({
            userId: deps.userId,
            lexemeId,
            front: `sample-${i}`,
            back: { meaningSource: "import" },
            deck: "test",
            origin: "import",
            srs: newSrsCard(now),
            now: now.getTime(),
          });
          repos.articles.insertTargets([
            { articleId, lexemeId, role, occurrences: role === "exposed" ? 2 : 0 },
          ]);
          for (const offset of [1, 2])
            repos.reviewLogs.insert({
              userId: deps.userId,
              cardId,
              rating: offset === 2 || [0, 1, 10, 11].includes(i) ? "again" : "good",
              stateBefore: "review",
              dueBefore: now.getTime(),
              reviewedAt: now.getTime() + offset * 86_400_000,
              durationMs: 1000,
            });
        }
      });
    } finally {
      deps.close();
    }
    try {
      const result = run("tools/report/experiment.ts", [
        "--db",
        database,
        "--window",
        "7",
        "--json",
      ]);
      expect(result.status, result.stderr).toBe(0);
      const report = JSON.parse(result.stdout);
      expect(report.exposed).toEqual({ n: 10, again: 2, againRate: 0.2, noReview: 0 });
      expect(report.control).toEqual({ n: 5, again: 2, againRate: 0.4, noReview: 0 });
      expect(report.diff).toBeCloseTo(-0.2);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
  it("T6.2 AC5/6: empty experiment JSON is parseable with null rates", () => {
    const result = run("tools/report/experiment.ts", ["--db", ":memory:", "--json"]);
    expect(result.status, result.stderr).toBe(0);
    expect(JSON.parse(result.stdout)).toMatchObject({
      exposed: { n: 0, againRate: null },
      control: { n: 0, againRate: null },
      diff: null,
    });
  });
});
