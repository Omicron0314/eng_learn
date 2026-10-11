import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { parseArgs } from "node:util";
import {
  analyzeText,
  defaultLemmatizer,
  median,
  parseWordListLine,
  renderArticlePrompt,
  tokenize,
} from "@eng-learn/core";
import {
  completeJson,
  createLLM,
  demoResponder,
  FakeLLM,
  type LLMCallRecord,
  loadLLMConfig,
  loadPrompt,
  MemoryBlob,
} from "@eng-learn/providers";
import { z } from "zod";
import { createAppDeps } from "../../apps/web/src/server/deps";
import { costGuard } from "./budget";

const { values } = parseArgs({
  options: { fake: { type: "boolean" }, "allow-paid": { type: "boolean" } },
  strict: true,
});
if (!values.fake && !values["allow-paid"])
  throw new Error(
    "Manual paid evaluation only: pass --allow-paid after reviewing prices and EVAL_MAX_COST_USD. Use --fake for offline validation.",
  );
const config = loadLLMConfig(values.fake ? { LLM_PROVIDER: "fake" } : process.env);
if (
  !values.fake &&
  (config.provider === "fake" ||
    !config.price ||
    config.price.inputPerMTok <= 0 ||
    config.price.outputPerMTok <= 0)
)
  throw new Error(
    "Real evaluation requires a real provider and positive LLM_PRICE_INPUT_PER_MTOK / LLM_PRICE_OUTPUT_PER_MTOK.",
  );
const maxCost = z.coerce
  .number()
  .positive()
  .max(1)
  .parse(process.env.EVAL_MAX_COST_USD ?? 1);
const provider = values.fake
  ? new FakeLLM({ responder: (req) => demoResponder({ ...req, purpose: "article" }) })
  : createLLM(config);
const guard = costGuard(provider, config.price ?? { inputPerMTok: 0, outputPerMTok: 0 }, maxCost);
const samples = z
  .array(
    z.object({
      targets: z.array(z.string()).length(8),
      genre: z.enum(["story", "dialogue", "monologue"]),
    }),
  )
  .length(30)
  .parse(JSON.parse(readFileSync("fixtures/eval/targets.json", "utf8")));
const baselinePath = process.env.EVAL_BASELINE_PATH || "fixtures/wordlist/baseline-sample.txt";
const known = new Set(
  readFileSync(baselinePath, "utf8")
    .split(/\r?\n/)
    .flatMap((line) => {
      const entry = parseWordListLine(line);
      return entry && !entry.isPhrase ? [entry.headword, ...entry.variants] : [];
    }),
);
const schema = z.object({
  title: z.string().min(1),
  body: z.string().min(1),
  names: z.array(z.string()),
});
const deps = createAppDeps(
  values.fake
    ? { LLM_PROVIDER: "fake", TTS_PROVIDER: "none", DATABASE_PATH: ":memory:" }
    : { ...process.env, TTS_PROVIDER: "none" },
  { llm: guard.provider, tts: null, blobs: new MemoryBlob() },
);
const records: LLMCallRecord[] = [];
const rows: {
  genre: string;
  firstJsonValid: boolean;
  valid: boolean;
  coverage: number | null;
  unknownRatio: number | null;
  targetTwiceRate: number;
  inLengthRange: boolean;
  latencyMs: number;
}[] = [];
let stopped = false;
try {
  for (const sample of samples) {
    const before = records.length;
    try {
      const result = await completeJson(
        guard.provider,
        {
          purpose: "eval",
          messages: renderArticlePrompt(loadPrompt("article.v1"), {
            ...sample,
            level: "B1",
            minWords: 300,
            maxWords: 400,
          }),
        },
        schema,
        {
          shape: '{"title":string,"body":string,"names":string[]}',
          ...(config.price ? { price: config.price } : {}),
          onCall: (record) => {
            records.push(record);
            deps.repos.llmCalls.insert({
              ...record,
              userId: deps.userId,
              ts: deps.clock().getTime(),
            });
          },
          repairTemplate: loadPrompt("json-repair.v1"),
        },
      );
      const analysis = analyzeText(
        result.value.body,
        { has: (lemma) => known.has(lemma) || sample.targets.includes(lemma) },
        { lemmatizer: defaultLemmatizer(), targets: sample.targets, names: result.value.names },
      );
      const wordCount = tokenize(result.value.body).filter((t) => t.kind === "word").length;
      rows.push({
        genre: sample.genre,
        firstJsonValid: records[before]?.status === "ok",
        valid: true,
        coverage: analysis.coverage,
        unknownRatio: analysis.unknownRatio,
        targetTwiceRate:
          sample.targets.filter((target) => (analysis.targetOccurrences[target] ?? 0) >= 2).length /
          8,
        inLengthRange: wordCount >= 300 && wordCount <= 400,
        latencyMs: records.slice(before).reduce((n, r) => n + r.latencyMs, 0),
      });
    } catch (error) {
      if ((error as Error).message === "EVAL_BUDGET_EXHAUSTED") {
        stopped = true;
        break;
      }
      rows.push({
        genre: sample.genre,
        firstJsonValid: false,
        valid: false,
        coverage: null,
        unknownRatio: null,
        targetTwiceRate: 0,
        inLengthRange: false,
        latencyMs: 0,
      });
    }
  }
  const ratio = (test: (row: (typeof rows)[number]) => boolean) =>
    rows.length ? rows.filter(test).length / rows.length : null;
  const coverages = rows
    .flatMap((r) => (r.coverage === null ? [] : [r.coverage]))
    .sort((a, b) => a - b);
  const result = {
    mode: values.fake ? "fake — not real quality evidence" : "real",
    model: provider.model,
    promptVersion: "article.v1",
    baselineSource: baselinePath,
    completed: rows.length,
    budgetStopped: stopped,
    maxCostUsd: maxCost,
    reservedCostUpperBoundUsd: guard.reserved(),
    metrics: {
      firstJsonValidRate: ratio((r) => r.firstJsonValid),
      repairedJsonValidRate: ratio((r) => r.valid),
      coverageMedian: median(coverages),
      coverageP10: coverages[Math.floor(coverages.length * 0.1)] ?? null,
      unknownRatioMedian: median(
        rows.flatMap((r) => (r.unknownRatio === null ? [] : [r.unknownRatio])),
      ),
      targetTwiceRate: rows.length
        ? rows.reduce((n, r) => n + r.targetTwiceRate, 0) / rows.length
        : null,
      inLengthRangeRate: ratio((r) => r.inLengthRange),
      latencyMsMean: rows.length ? rows.reduce((n, r) => n + r.latencyMs, 0) / rows.length : null,
      tokensIn: records.reduce((n, r) => n + r.tokensIn, 0),
      tokensOut: records.reduce((n, r) => n + r.tokensOut, 0),
      actualCostUsd: records.reduce((n, r) => n + (r.costUsd ?? 0), 0),
    },
    rows,
    calls: records,
  };
  mkdirSync("tools/eval/results", { recursive: true });
  const file = `tools/eval/results/${new Date().toISOString().replaceAll(":", "-")}.json`;
  writeFileSync(file, JSON.stringify(result, null, 2));
  console.table(result.metrics);
  console.log(
    `${result.mode}\n${rows.length}/30 groups; reserved upper bound $${guard.reserved().toFixed(4)}\n${file}`,
  );
} finally {
  deps.close();
}
