import { analyzeText, defaultLemmatizer, newSrsCard, seededRng } from "@eng-learn/core";
import { MemoryBlob } from "@eng-learn/providers";
import { FakeLLM, FakeTTS, testResponder } from "@eng-learn/providers/testing";
import { afterEach } from "vitest";
import { type AppDeps, createAppDeps } from "../deps";

const opened: AppDeps[] = [];
afterEach(() => {
  for (const deps of opened.splice(0)) deps.close();
});
export function setup(script?: NonNullable<ConstructorParameters<typeof FakeLLM>[0]>["script"]) {
  let time = new Date("2026-10-12T02:00:00Z");
  const llm = new FakeLLM({ ...(script ? { script } : { responder: testResponder }) });
  const tts = new FakeTTS();
  const blobs = new MemoryBlob();
  const deps = createAppDeps(
    {
      LLM_PROVIDER: "openai-compatible",
      LLM_BASE_URL: "http://127.0.0.1:9/v1",
      LLM_API_KEY: "test",
      LLM_MODEL: "test-model",
      TTS_PROVIDER: "none",
      DATABASE_PATH: ":memory:",
    },
    { llm, tts, blobs, clock: () => time, rng: seededRng(7) },
  );
  opened.push(deps);
  return {
    deps,
    llm,
    tts,
    blobs,
    setTime: (value: string | number) => {
      time = new Date(value);
    },
  };
}
export function seedCard(deps: AppDeps, lemma: string) {
  return deps.tx((repos) => {
    const now = deps.clock();
    return repos.cards.insert({
      userId: deps.userId,
      lexemeId: repos.lexemes.upsert(lemma, "", now.getTime()),
      front: lemma,
      back: { meaningSource: "import" },
      deck: "test",
      origin: "import",
      srs: newSrsCard(now),
      now: now.getTime(),
    });
  });
}
export function seedArticle(
  deps: AppDeps,
  body = "I go home. We go home! Do you go home?",
  title = "Home",
) {
  return deps.tx((repos) =>
    repos.articles.insert({
      userId: deps.userId,
      title,
      body,
      kind: "generated",
      genre: "story",
      levelTier: "medium",
      promptVersion: "article.v1",
      model: deps.llmProvider().model,
      source: "llm",
      license: null,
      analysisJson: JSON.stringify({
        ...analyzeText(body, new Set(), { lemmatizer: defaultLemmatizer() }),
        belowTier: false,
        attempts: [],
        missingTargets: [],
        wordCount: 10,
      }),
      now: deps.clock().getTime(),
    }),
  );
}
export const bytes = (text: string) => new TextEncoder().encode(text);

export function required<T>(value: T | undefined | null): T {
  if (value === undefined || value === null) throw new Error("expected fixture value");
  return value;
}
