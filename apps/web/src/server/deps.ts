import type { Settings } from "@eng-learn/core";
import { closeDb, createRepos, migrate, openDb, type Repos, withTx } from "@eng-learn/db";
import {
  type BlobStore,
  createBlobStore,
  createLLM,
  createTTS,
  type LLMProvider,
  llmConfigStatus,
  loadProviderConfig,
  type Price,
  type TTSProvider,
} from "@eng-learn/providers";

export type ProviderStatus = {
  llm: ReturnType<typeof llmConfigStatus>;
  tts: { provider: string };
};
export type AppDeps = {
  repos: Repos;
  userId: string;
  tx: <T>(fn: (repos: Repos) => T) => T;
  llm: LLMProvider;
  tts: TTSProvider | null;
  blobs: BlobStore;
  clock: () => Date;
  rng: () => number;
  settings: () => Settings;
  providers: ProviderStatus;
  price?: Price;
  close: () => void;
};

/** Configuration is validated before opening a persistent database. Tests inject overrides. */
export function createAppDeps(
  env: Record<string, string | undefined>,
  overrides: Partial<Pick<AppDeps, "llm" | "tts" | "blobs" | "clock" | "rng">> = {},
): AppDeps {
  const cfg = loadProviderConfig(env);
  const db = openDb(env.DATABASE_PATH?.trim() || "./data/app.sqlite");
  migrate(db);
  const repos = createRepos(db);
  const clock = overrides.clock ?? (() => new Date());
  const userId = repos.users.ensureDefaultUser(clock().getTime());
  return {
    repos,
    userId,
    tx: (fn) => withTx(db, (tx) => fn(createRepos(tx))),
    llm: overrides.llm ?? createLLM(cfg.llm),
    tts: overrides.tts === undefined ? createTTS(cfg.tts) : overrides.tts,
    blobs: overrides.blobs ?? createBlobStore(cfg.blobDir),
    clock,
    rng: overrides.rng ?? Math.random,
    settings: () => repos.settings.get(userId),
    providers: { llm: llmConfigStatus(env), tts: { provider: cfg.tts.provider } },
    ...(cfg.llm.price ? { price: cfg.llm.price } : {}),
    close: () => closeDb(db),
  };
}
