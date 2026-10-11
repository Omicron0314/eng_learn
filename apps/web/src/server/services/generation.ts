import {
  type AnalysisResult,
  analyzeText,
  countWords,
  createSrs,
  dayRange,
  daysAgo,
  defaultLemmatizer,
  GENRES,
  type Genre,
  pickGenre,
  renderArticlePrompt,
  selectTargets,
  TIER_MIN_COVERAGE,
  TIERS,
  type Tier,
} from "@eng-learn/core";
import { AppError, loadPrompt } from "@eng-learn/providers";
import { z } from "zod";
import type { AppDeps } from "../deps";
import { getKnownSet } from "./knowledge";
import { jsonCompletion, validate } from "./shared";

export const ArticleAnalysisSchema = z.object({
  tokenCount: z.number().int().min(0),
  coverage: z.number().min(0).max(1),
  typeCoverage: z.number().min(0).max(1),
  unknownRatio: z.number().min(0).max(1),
  unknownLemmas: z.array(z.object({ lemma: z.string(), count: z.number().int().min(0) })),
  targetOccurrences: z.record(z.string(), z.number().int().min(0)),
  names: z.array(z.string()),
  wordCount: z.number().int().min(0),
  attempts: z.array(
    z.object({ coverage: z.number().min(0).max(1), avoidWords: z.array(z.string()) }),
  ),
  missingTargets: z.array(z.string()),
  belowTier: z.boolean(),
});
export type ArticleAnalysis = z.infer<typeof ArticleAnalysisSchema>;
export function selectTargetsForUser(deps: AppDeps) {
  const now = deps.clock();
  const settings = deps.settings();
  const srs = createSrs(settings);
  const logs = deps.repos.reviewLogs.listBetween(deps.userId, dayRange(now, settings));
  return selectTargets({
    candidates: deps.repos.cards.listAll(deps.userId).map((card) => {
      const reviews = logs.filter((log) => log.cardId === card.id);
      return {
        lexemeId: card.lexemeId,
        lemma: card.lemma,
        card: card.srs,
        suspended: card.suspended,
        todayRatings: reviews.map((r) => r.rating),
        introducedToday: reviews.some((r) => r.stateBefore === "new"),
        retrievability: srs.retrievability(card.srs, now),
      };
    }),
    recentExposure: deps.repos.articles.recentExposure(deps.userId, daysAgo(now, 3)),
    now,
    n: settings.targetCount,
    controlRatio: settings.controlRatio,
    rng: deps.rng,
  });
}
const articleSchema = z.object({
  title: z.string().trim().min(1).max(1000),
  body: z.string().trim().min(1).max(100_000),
  names: z.array(z.string()).max(100),
});
export async function generateArticle(
  deps: AppDeps,
  input: { tier?: Tier | undefined; genre?: Genre | undefined } = {},
) {
  input = validate(
    z.object({ tier: z.enum(TIERS).optional(), genre: z.enum(GENRES).optional() }),
    input,
  );
  const selected = selectTargetsForUser(deps);
  if (!selected.length)
    throw new AppError("NO_TARGETS", "今天还没有可用的目标词，先去复习几张卡片", 400);
  const settings = deps.settings();
  const tier = input.tier ?? settings.tier;
  const genre =
    input.genre ?? pickGenre(deps.repos.articles.list(deps.userId).map((a) => a.genre as Genre));
  const exposed = selected.filter((t) => t.role === "exposed").map((t) => t.lemma);
  const allTargets = selected.map((t) => t.lemma);
  const controls = selected.filter((t) => t.role === "control").map((t) => t.lemma);
  const sanitizeRepairContext = (text: string) => {
    for (const word of controls)
      text = text.replace(new RegExp(word.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "gi"), "[…]");
    return text;
  };
  const known = getKnownSet(deps, { targets: exposed });
  const lemmatizer = defaultLemmatizer();
  const attempts: ArticleAnalysis["attempts"] = [];
  let avoidWords: string[] = [];
  let best:
    | { value: z.infer<typeof articleSchema>; model: string; analysis: AnalysisResult }
    | undefined;
  for (let attempt = 0; attempt <= settings.generationMaxRetries; attempt++) {
    const result = await jsonCompletion(
      deps,
      "article",
      renderArticlePrompt(loadPrompt("article.v1"), {
        genre,
        minWords: settings.articleMinWords,
        maxWords: settings.articleMaxWords,
        level: settings.level,
        targets: exposed,
        avoidWords,
      }),
      articleSchema,
      '{"title":string,"body":string,"names":string[]}',
      sanitizeRepairContext,
    );
    const analysis = analyzeText(result.value.body, known, {
      lemmatizer,
      targets: exposed,
      names: result.value.names,
    });
    // Control words must be counted, but never automatically treated as known.
    analysis.targetOccurrences = analyzeText(result.value.body, known, {
      lemmatizer,
      targets: allTargets,
      names: result.value.names,
    }).targetOccurrences;
    attempts.push({ coverage: analysis.coverage, avoidWords: [...avoidWords] });
    if (!best || analysis.coverage > best.analysis.coverage)
      best = { value: result.value, model: result.model, analysis };
    if (analysis.coverage >= TIER_MIN_COVERAGE[tier]) break;
    // Do not leak control words into a retry prompt through the avoidance list.
    avoidWords = analysis.unknownLemmas
      .filter((w) => !allTargets.includes(w.lemma))
      .slice(0, 10)
      .map((w) => w.lemma);
  }
  if (!best) throw new Error("generation produced no attempts");
  const { value, model, analysis } = best;
  const missingTargets = exposed.filter((t) => !analysis.targetOccurrences[t]);
  const belowTier = analysis.coverage < TIER_MIN_COVERAGE[tier];
  const stored: ArticleAnalysis = {
    ...analysis,
    wordCount: countWords(value.body),
    attempts,
    missingTargets,
    belowTier,
  };
  return deps.tx((repos) => {
    const now = deps.clock().getTime();
    const articleId = repos.articles.insert({
      userId: deps.userId,
      kind: "generated",
      title: value.title,
      body: value.body,
      genre,
      levelTier: tier,
      promptVersion: "article.v1",
      model,
      source: "llm",
      license: null,
      analysisJson: JSON.stringify(stored),
      now,
    });
    repos.articles.insertTargets(
      selected.map((t) => ({
        articleId,
        lexemeId: t.lexemeId,
        role: t.role,
        occurrences: analysis.targetOccurrences[t.lemma] ?? 0,
      })),
    );
    repos.events.append(
      deps.userId,
      "article_generate",
      {
        articleId,
        attempts: attempts.length,
        coverage: analysis.coverage,
        belowTier,
        missingTargets,
      },
      now,
    );
    return { articleId, coverage: analysis.coverage, belowTier, missingTargets };
  });
}
export const generationService = { selectTargetsForUser, generateArticle };
