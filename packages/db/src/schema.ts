import {
  index,
  integer,
  primaryKey,
  real,
  sqliteTable,
  text,
  uniqueIndex,
} from "drizzle-orm/sqlite-core";

export const users = sqliteTable("users", {
  id: text("id").primaryKey(),
  createdAt: integer("created_at").notNull(),
  updatedAt: integer("updated_at").notNull(),
});

export const settings = sqliteTable("settings", {
  userId: text("user_id")
    .primaryKey()
    .references(() => users.id),
  json: text("json").notNull(),
  updatedAt: integer("updated_at").notNull(),
});

export const providerConfig = sqliteTable("provider_config", {
  userId: text("user_id")
    .primaryKey()
    .references(() => users.id),
  json: text("json").notNull(),
  updatedAt: integer("updated_at").notNull(),
});

export const lexemes = sqliteTable(
  "lexemes",
  {
    id: text("id").primaryKey(),
    lemma: text("lemma").notNull(),
    pos: text("pos").notNull().default(""),
    createdAt: integer("created_at").notNull(),
  },
  (t) => [uniqueIndex("lexemes_lemma_pos").on(t.lemma, t.pos)],
);

export const cards = sqliteTable(
  "cards",
  {
    id: text("id").primaryKey(),
    userId: text("user_id")
      .notNull()
      .references(() => users.id),
    lexemeId: text("lexeme_id")
      .notNull()
      .references(() => lexemes.id),
    front: text("front").notNull(),
    backJson: text("back_json").notNull(),
    deck: text("deck").notNull(),
    suspended: integer("suspended", { mode: "boolean" }).notNull().default(false),
    origin: text("origin").notNull(),
    due: integer("due").notNull(),
    stability: real("stability").notNull(),
    difficulty: real("difficulty").notNull(),
    state: text("state").notNull(),
    reps: integer("reps").notNull(),
    lapses: integer("lapses").notNull(),
    lastReview: integer("last_review"),
    elapsedDays: real("elapsed_days").notNull(),
    scheduledDays: real("scheduled_days").notNull(),
    learningSteps: integer("learning_steps").notNull(),
    createdAt: integer("created_at").notNull(),
    updatedAt: integer("updated_at").notNull(),
  },
  (t) => [
    uniqueIndex("cards_user_lexeme").on(t.userId, t.lexemeId),
    index("cards_user_due").on(t.userId, t.due),
  ],
);

export const reviewLogs = sqliteTable(
  "review_logs",
  {
    id: text("id").primaryKey(),
    userId: text("user_id")
      .notNull()
      .references(() => users.id),
    cardId: text("card_id")
      .notNull()
      .references(() => cards.id),
    rating: text("rating").notNull(),
    stateBefore: text("state_before").notNull(),
    dueBefore: integer("due_before").notNull(),
    reviewedAt: integer("reviewed_at").notNull(),
    durationMs: integer("duration_ms").notNull(),
  },
  (t) => [
    index("review_logs_user_time").on(t.userId, t.reviewedAt),
    index("review_logs_card").on(t.cardId),
  ],
);

export const events = sqliteTable(
  "events",
  {
    id: text("id").primaryKey(),
    userId: text("user_id")
      .notNull()
      .references(() => users.id),
    type: text("type").notNull(),
    payloadJson: text("payload_json").notNull(),
    ts: integer("ts").notNull(),
  },
  (t) => [index("events_user_type_ts").on(t.userId, t.type, t.ts)],
);

export const articles = sqliteTable(
  "articles",
  {
    id: text("id").primaryKey(),
    userId: text("user_id")
      .notNull()
      .references(() => users.id),
    kind: text("kind").notNull(),
    title: text("title").notNull(),
    body: text("body").notNull(),
    genre: text("genre").notNull(),
    levelTier: text("level_tier").notNull(),
    promptVersion: text("prompt_version").notNull(),
    model: text("model").notNull(),
    source: text("source").notNull(),
    license: text("license"),
    analysisJson: text("analysis_json").notNull(),
    createdAt: integer("created_at").notNull(),
    updatedAt: integer("updated_at").notNull(),
  },
  (t) => [index("articles_user_created").on(t.userId, t.createdAt)],
);

export const articleTargets = sqliteTable(
  "article_targets",
  {
    articleId: text("article_id")
      .notNull()
      .references(() => articles.id),
    lexemeId: text("lexeme_id")
      .notNull()
      .references(() => lexemes.id),
    role: text("role").notNull(),
    occurrences: integer("occurrences").notNull(),
  },
  (t) => [primaryKey({ columns: [t.articleId, t.lexemeId] })],
);

export const audios = sqliteTable("audios", {
  id: text("id").primaryKey(),
  articleId: text("article_id")
    .notNull()
    .references(() => articles.id),
  provider: text("provider").notNull(),
  voice: text("voice").notNull(),
  blobKey: text("blob_key").notNull(),
  durationS: real("duration_s"),
  textHash: text("text_hash").notNull().unique(),
  chars: integer("chars").notNull(),
  createdAt: integer("created_at").notNull(),
});

export const inbox = sqliteTable(
  "inbox",
  {
    id: text("id").primaryKey(),
    userId: text("user_id")
      .notNull()
      .references(() => users.id),
    lexemeId: text("lexeme_id")
      .notNull()
      .references(() => lexemes.id),
    contextSentence: text("context_sentence").notNull(),
    articleId: text("article_id").references(() => articles.id),
    status: text("status").notNull(),
    createdAt: integer("created_at").notNull(),
    updatedAt: integer("updated_at").notNull(),
  },
  (t) => [index("inbox_user_status").on(t.userId, t.status)],
);

export const glossCache = sqliteTable(
  "gloss_cache",
  {
    lexemeId: text("lexeme_id")
      .notNull()
      .references(() => lexemes.id),
    contextHash: text("context_hash").notNull(),
    glossJson: text("gloss_json").notNull(),
    createdAt: integer("created_at").notNull(),
  },
  (t) => [primaryKey({ columns: [t.lexemeId, t.contextHash] })],
);

export const llmCalls = sqliteTable(
  "llm_calls",
  {
    id: text("id").primaryKey(),
    userId: text("user_id")
      .notNull()
      .references(() => users.id),
    purpose: text("purpose").notNull(),
    provider: text("provider").notNull(),
    model: text("model").notNull(),
    tokensIn: integer("tokens_in").notNull(),
    tokensOut: integer("tokens_out").notNull(),
    costUsd: real("cost_usd"),
    latencyMs: integer("latency_ms").notNull(),
    status: text("status").notNull(),
    error: text("error"),
    ts: integer("ts").notNull(),
  },
  (t) => [index("llm_calls_user_ts").on(t.userId, t.ts)],
);

export const wordLists = sqliteTable("word_lists", {
  id: text("id").primaryKey(),
  userId: text("user_id")
    .notNull()
    .references(() => users.id),
  name: text("name").notNull(),
  kind: text("kind").notNull(),
  sourceFile: text("source_file").notNull(),
  createdAt: integer("created_at").notNull(),
  updatedAt: integer("updated_at").notNull(),
});

export const wordListItems = sqliteTable(
  "word_list_items",
  {
    id: text("id").primaryKey(),
    listId: text("list_id")
      .notNull()
      .references(() => wordLists.id),
    userId: text("user_id")
      .notNull()
      .references(() => users.id),
    lexemeId: text("lexeme_id")
      .notNull()
      .references(() => lexemes.id),
    raw: text("raw").notNull(),
    createdAt: integer("created_at").notNull(),
  },
  (t) => [uniqueIndex("word_list_items_list_lexeme").on(t.listId, t.lexemeId)],
);

export const lexemeMarks = sqliteTable(
  "lexeme_marks",
  {
    userId: text("user_id")
      .notNull()
      .references(() => users.id),
    lexemeId: text("lexeme_id")
      .notNull()
      .references(() => lexemes.id),
    status: text("status").notNull(),
    markedAt: integer("marked_at").notNull(),
    updatedAt: integer("updated_at").notNull(),
  },
  (t) => [primaryKey({ columns: [t.userId, t.lexemeId] })],
);
