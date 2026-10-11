import {
  activeDaysPerWeek,
  completionRate,
  computeExperiment,
  createSrs,
  DAY_MS,
  dayKey,
  dayRange,
  difficultyDistribution,
  median,
  weekRange,
} from "@eng-learn/core";
import { z } from "zod";
import type { AppDeps } from "../deps";
import { ArticleAnalysisSchema } from "./generation";
import { validate } from "./shared";

export function getExperiment(deps: AppDeps, windowDays = 7) {
  validate(z.number().int().min(1).max(365), windowDays);
  return computeExperiment({
    samples: deps.repos.articles.samples(deps.userId),
    windowDays,
    firstReview: (sample) => {
      const card = deps.repos.cards.getByLexeme(deps.userId, sample.lexemeId);
      return card ? deps.repos.reviewLogs.firstReviewAfter(card.id, sample.articleCreatedAt) : null;
    },
  });
}
export function getStats(
  deps: AppDeps,
  input: { offset?: number | undefined; limit?: number | undefined } = {},
) {
  input = validate(
    z.object({
      offset: z.number().int().min(0).default(0),
      limit: z.number().int().min(1).max(100).default(50),
    }),
    input,
  );
  const now = deps.clock();
  const settings = deps.settings();
  const day = dayRange(now, settings);
  const week = weekRange(now, settings);
  const count = (range: typeof day) => ({
    newLearned: deps.repos.reviewLogs.countNewIntroduced(deps.userId, range),
    reviews: deps.repos.reviewLogs.countReviews(deps.userId, range),
  });
  const logs = deps.repos.reviewLogs.listAll(deps.userId);
  const first = new Map<string, number>();
  for (const log of logs) if (!first.has(log.cardId)) first.set(log.cardId, log.reviewedAt);
  const srs = createSrs(settings);
  const learned = deps.repos.cards
    .listAll(deps.userId)
    .filter((c) => c.srs.reps > 0)
    .sort((a, b) => (first.get(b.id) ?? 0) - (first.get(a.id) ?? 0) || a.id.localeCompare(b.id));
  const learnedWords = learned
    .slice(input.offset ?? 0, (input.offset ?? 0) + (input.limit ?? 50))
    .map((c) => ({
      lemma: c.lemma,
      meaning: c.back.meaning,
      state: c.srs.state,
      retrievability: srs.retrievability(c.srs, now),
    }));
  const articles = deps.repos.articles
    .list(deps.userId, { limit: Number.MAX_SAFE_INTEGER })
    .sort((a, b) => a.createdAt - b.createdAt || a.id.localeCompare(b.id));
  const analyses = articles.map((a) => ({
    ...a,
    analysis: ArticleAnalysisSchema.parse(JSON.parse(a.analysisJson)),
  }));
  const weeks = [3, 2, 1, 0].map((back) => {
    // Step from a midday instant; avoids landing before the learning-day boundary after DST.
    const date = new Date(week.start + 12 * 3_600_000 - back * 7 * DAY_MS);
    return weekRange(date, settings);
  });
  const active = activeDaysPerWeek(
    deps.repos.events.timestamps(deps.userId, {
      start: weeks[0]?.start ?? week.start,
      end: week.end,
    }),
    weeks,
    settings,
  );
  return {
    learning: { today: count(day), week: count(week) },
    learnedWords,
    learnedWordsTotal: learned.length,
    coverageTrend: analyses.map((a) => ({
      date: dayKey(a.createdAt, settings),
      coverage: a.analysis.coverage,
    })),
    unknownRatioMedian: {
      last7d: median(
        analyses
          .filter((a) => a.createdAt >= day.start - 6 * DAY_MS)
          .map((a) => a.analysis.unknownRatio),
      ),
      all: median(analyses.map((a) => a.analysis.unknownRatio)),
    },
    h1: {
      activeDaysPerWeek: weeks.map((w, i) => ({
        weekStart: dayKey(w.start, settings),
        days: active[i] ?? 0,
      })),
      completionRate: completionRate(
        new Set(
          deps.repos.events.listByType(deps.userId, "article_open").map((e) => e.payload.articleId),
        ),
        new Set(
          deps.repos.events
            .listByType(deps.userId, "article_complete")
            .map((e) => e.payload.articleId),
        ),
      ),
    },
    h2: difficultyDistribution(
      deps.repos.events
        .listByType(deps.userId, "difficulty_feedback")
        .map((e) => ({ ...e.payload, ts: e.ts })),
    ),
    h3: getExperiment(deps),
  };
}
export const statsService = { getStats, getExperiment };
