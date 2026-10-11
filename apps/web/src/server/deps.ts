import type { Settings, StoredProviderConfig } from "@eng-learn/core";
import { closeDb, createRepos, migrate, openDb, type Repos, withTx } from "@eng-learn/db";
import {
  type BlobStore,
  createBlobStore,
  createLLM,
  createTTS,
  type LLMProvider,
  type LLMStatus,
  llmStatus,
  type Price,
  readPrice,
  resolveLLMConfig,
  resolveTTSConfig,
  type TTSProvider,
  type TTSStatus,
  ttsStatus,
} from "@eng-learn/providers";

export type ProviderStatus = { llm: LLMStatus; tts: TTSStatus };
export type AppDeps = {
  repos: Repos;
  userId: string;
  tx: <T>(fn: (repos: Repos) => T) => T;
  /** Resolved per call so settings-page changes apply without a restart. */
  llmProvider: () => LLMProvider;
  ttsProvider: () => TTSProvider | null;
  blobs: BlobStore;
  clock: () => Date;
  rng: () => number;
  settings: () => Settings;
  providerConfig: () => StoredProviderConfig;
  providerStatus: () => ProviderStatus;
  price?: Price;
  close: () => void;
};

type Overrides = {
  llm?: LLMProvider;
  tts?: TTSProvider | null;
  blobs?: BlobStore;
  clock?: () => Date;
  rng?: () => number;
};

/** Provider configuration is resolved lazily; tests inject provider overrides. */
export function createAppDeps(
  env: Record<string, string | undefined>,
  overrides: Overrides = {},
): AppDeps {
  const db = openDb(env.DATABASE_PATH?.trim() || "./data/app.sqlite");
  migrate(db);
  const repos = createRepos(db);
  const clock = overrides.clock ?? (() => new Date());
  const userId = repos.users.ensureDefaultUser(clock().getTime());
  const stored = () => repos.providerConfig.get(userId);
  const price = readPrice(env);
  return {
    repos,
    userId,
    tx: (fn) => withTx(db, (tx) => fn(createRepos(tx))),
    llmProvider: overrides.llm
      ? () => overrides.llm as LLMProvider
      : () => createLLM(resolveLLMConfig(env, stored().llm)),
    ttsProvider:
      overrides.tts !== undefined
        ? () => overrides.tts ?? null
        : () => createTTS(resolveTTSConfig(env, stored().tts)),
    blobs: overrides.blobs ?? createBlobStore(env.BLOB_DIR?.trim() || "./data/blobs"),
    clock,
    rng: overrides.rng ?? Math.random,
    settings: () => repos.settings.get(userId),
    providerConfig: stored,
    providerStatus: () => ({
      llm: llmStatus(env, stored().llm),
      tts: ttsStatus(env, stored().tts),
    }),
    ...(price ? { price } : {}),
    close: () => closeDb(db),
  };
}
