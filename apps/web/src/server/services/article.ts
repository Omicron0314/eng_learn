import { dayRange, type Genre, type Tier, tokenize } from "@eng-learn/core";
import { AppError } from "@eng-learn/providers";
import { z } from "zod";
import type { AppDeps } from "../deps";
import { ArticleAnalysisSchema } from "./generation";
import { getQueue } from "./review";
import { nonempty, requireArticle, validate } from "./shared";

export function getArticleView(deps: AppDeps, articleId: string) {
  const article = deps.repos.articles.get(articleId);
  if (!article || article.userId !== deps.userId) return null;
  const analysis = ArticleAnalysisSchema.parse(JSON.parse(article.analysisJson));
  const tokens = tokenize(article.body);
  // Keep separators in paragraph text, and offsets/sentence indexes global to the body.
  const paragraphs: { text: string; tokens: typeof tokens }[] = [];
  let start = 0;
  const boundaries = [...article.body.matchAll(/\n\s*\n/g)].map(
    (m) => (m.index ?? 0) + m[0].length,
  );
  for (const end of [...boundaries, article.body.length]) {
    if (end === start) continue;
    paragraphs.push({
      text: article.body.slice(start, end),
      tokens: tokens.filter((t) => t.start >= start && t.start < end),
    });
    start = end;
  }
  return {
    id: article.id,
    title: article.title,
    body: article.body,
    genre: article.genre as Genre,
    tier: article.levelTier as Tier,
    coverage: analysis.coverage,
    belowTier: analysis.belowTier,
    analysis,
    createdAt: article.createdAt,
    paragraphs,
  };
}
/** Returns an open event id; pass it to completion to deduplicate per reading session. */
export function recordArticleEvent(
  deps: AppDeps,
  input: {
    articleId: string;
    type: "article_open" | "article_complete";
    via?: "button" | "scroll" | undefined;
    openId?: string | undefined;
  },
) {
  input = validate(
    z.object({
      articleId: nonempty,
      type: z.enum(["article_open", "article_complete"]),
      via: z.enum(["button", "scroll"]).optional(),
      openId: nonempty.optional(),
    }),
    input,
  );
  requireArticle(deps, input.articleId);
  return deps.tx((repos) => {
    const now = deps.clock().getTime();
    if (input.type === "article_open")
      return repos.events.append(deps.userId, "article_open", { articleId: input.articleId }, now);
    const opens = repos.events
      .listByType(deps.userId, "article_open")
      .filter((e) => e.payload.articleId === input.articleId);
    const open = input.openId ? opens.find((e) => e.id === input.openId) : opens.at(-1);
    if (input.openId && !open) throw new AppError("VALIDATION_FAILED", "阅读会话不存在", 400);
    const previous = repos.events
      .listByType(deps.userId, "article_complete")
      .find((e) => e.payload.articleId === input.articleId && e.payload.openId === open?.id);
    if (previous) return previous.id;
    return repos.events.append(
      deps.userId,
      "article_complete",
      {
        articleId: input.articleId,
        via: input.via ?? "button",
        ...(open ? { openId: open.id } : {}),
      },
      now,
    );
  });
}
export function getTodayView(deps: AppDeps) {
  const settings = deps.settings();
  const queue = getQueue(deps);
  const completed = new Set(
    deps.repos.events.listByType(deps.userId, "article_complete").map((e) => e.payload.articleId),
  );
  const articles = deps.repos.articles
    .list(deps.userId, { range: dayRange(deps.clock(), settings) })
    .map((a) => ({
      id: a.id,
      title: a.title,
      genre: a.genre as Genre,
      coverage: ArticleAnalysisSchema.parse(JSON.parse(a.analysisJson)).coverage,
      completed: completed.has(a.id),
    }));
  return {
    queue: { learning: queue.learning.length, review: queue.review.length, new: queue.new.length },
    articles,
    defaults: { tier: settings.tier },
  };
}
export const articleService = { getArticleView, recordArticleEvent };
export const todayService = { getTodayView };
