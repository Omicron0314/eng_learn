import { createHash } from "node:crypto";
import {
  defaultLemmatizer,
  normalizeApostrophes,
  primaryLemma,
  renderPrompt,
  sentenceAt,
  tokenize,
} from "@eng-learn/core";
import { AppError, loadPrompt } from "@eng-learn/providers";
import { z } from "zod";
import type { AppDeps } from "../deps";
import {
  jsonCompletion,
  nonempty,
  optionalArticle,
  requireArticle,
  singleFlight,
  validate,
} from "./shared";

export const GlossSchema = z.object({
  lemma: nonempty,
  pos: z.string().max(50),
  meaningZh: z.string().trim().min(1).max(30),
  meaningEn: z.string().max(200).optional(),
  ipa: z.string().max(200).optional(),
});
export type Gloss = z.infer<typeof GlossSchema>;
export function contextHash(sentence: string) {
  return createHash("sha256")
    .update(normalizeApostrophes(sentence).trim().replace(/\s+/g, " "))
    .digest("hex");
}
export async function getGloss(
  deps: AppDeps,
  input: { lemma: string; surface: string; sentence: string; articleId?: string | undefined },
) {
  input = validate(
    z.object({
      lemma: nonempty,
      surface: nonempty,
      sentence: z.string().trim().min(1).max(10_000),
      articleId: nonempty.optional(),
    }),
    input,
  );
  optionalArticle(deps, input.articleId);
  const lemma = primaryLemma(input.lemma, defaultLemmatizer());
  const hash = contextHash(input.sentence);
  // Log the click even if AI fails; lexeme creation and its event are atomic.
  const { lexemeId, cached, cacheHit } = deps.tx((repos) => {
    const now = deps.clock().getTime();
    const lexemeId = repos.lexemes.upsert(lemma, "", now);
    const stored = repos.glossCache.get(lexemeId, hash);
    const cached = stored ? GlossSchema.parse(stored) : undefined;
    const cacheHit = !!cached;
    repos.events.append(deps.userId, "lookup", { ...input, lemma, lexemeId, cacheHit }, now);
    return { lexemeId, cached, cacheHit };
  });
  const gloss = cached
    ? cached
    : await singleFlight(deps, `gloss:${lexemeId}:${hash}`, async () => {
        const result = await jsonCompletion(
          deps,
          "gloss",
          renderPrompt(loadPrompt("gloss.v1"), {
            surface: input.surface,
            lemma,
            sentence: input.sentence,
          }),
          GlossSchema,
          '{"lemma":string,"pos":string,"meaningZh":string,"meaningEn":string,"ipa":string}',
        );
        deps.repos.glossCache.put(lexemeId, hash, result.value, deps.clock().getTime());
        return result.value;
      });
  return { lexemeId, gloss, cacheHit };
}
/** Derive surface, sentence and lemma from authoritative article offsets. */
export function lookupArticleWord(deps: AppDeps, input: { articleId: string; start: number }) {
  validate(z.object({ articleId: nonempty, start: z.number().int().min(0) }), input);
  const article = requireArticle(deps, input.articleId);
  const token = tokenize(article.body).find((t) => t.start === input.start && t.kind === "word");
  if (!token) throw new AppError("VALIDATION_FAILED", "所选位置不是单词", 400);
  return getGloss(deps, {
    articleId: input.articleId,
    surface: token.surface,
    lemma: token.surface,
    sentence: sentenceAt(article.body, input.start),
  });
}
export const glossService = { getGloss, lookupArticleWord };
