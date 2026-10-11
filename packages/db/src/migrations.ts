import type Database from "better-sqlite3";

export type Migration = { id: string; sql: string };

/**
 * Applied in order and recorded in `_migrations`. Merged migrations must never be edited;
 * add a new entry instead.
 */
export const MIGRATIONS: Migration[] = [
  {
    id: "0001_init",
    sql: `
CREATE TABLE users (
  id TEXT PRIMARY KEY,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE TABLE settings (
  user_id TEXT PRIMARY KEY REFERENCES users(id),
  json TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE TABLE lexemes (
  id TEXT PRIMARY KEY,
  lemma TEXT NOT NULL,
  pos TEXT NOT NULL DEFAULT '',
  created_at INTEGER NOT NULL
);
CREATE UNIQUE INDEX lexemes_lemma_pos ON lexemes(lemma, pos);
CREATE TABLE cards (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id),
  lexeme_id TEXT NOT NULL REFERENCES lexemes(id),
  front TEXT NOT NULL,
  back_json TEXT NOT NULL,
  deck TEXT NOT NULL,
  suspended INTEGER NOT NULL DEFAULT 0,
  origin TEXT NOT NULL,
  due INTEGER NOT NULL,
  stability REAL NOT NULL,
  difficulty REAL NOT NULL,
  state TEXT NOT NULL,
  reps INTEGER NOT NULL,
  lapses INTEGER NOT NULL,
  last_review INTEGER,
  elapsed_days REAL NOT NULL,
  scheduled_days REAL NOT NULL,
  learning_steps INTEGER NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE UNIQUE INDEX cards_user_lexeme ON cards(user_id, lexeme_id);
CREATE INDEX cards_user_due ON cards(user_id, due);
CREATE TABLE review_logs (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id),
  card_id TEXT NOT NULL REFERENCES cards(id),
  rating TEXT NOT NULL,
  state_before TEXT NOT NULL,
  due_before INTEGER NOT NULL,
  reviewed_at INTEGER NOT NULL,
  duration_ms INTEGER NOT NULL
);
CREATE INDEX review_logs_user_time ON review_logs(user_id, reviewed_at);
CREATE INDEX review_logs_card ON review_logs(card_id);
CREATE TABLE events (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id),
  type TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  ts INTEGER NOT NULL
);
CREATE INDEX events_user_type_ts ON events(user_id, type, ts);
CREATE TABLE articles (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id),
  kind TEXT NOT NULL,
  title TEXT NOT NULL,
  body TEXT NOT NULL,
  genre TEXT NOT NULL,
  level_tier TEXT NOT NULL,
  prompt_version TEXT NOT NULL,
  model TEXT NOT NULL,
  source TEXT NOT NULL,
  license TEXT,
  analysis_json TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX articles_user_created ON articles(user_id, created_at);
CREATE TABLE article_targets (
  article_id TEXT NOT NULL REFERENCES articles(id),
  lexeme_id TEXT NOT NULL REFERENCES lexemes(id),
  role TEXT NOT NULL,
  occurrences INTEGER NOT NULL,
  PRIMARY KEY (article_id, lexeme_id)
);
CREATE TABLE audios (
  id TEXT PRIMARY KEY,
  article_id TEXT NOT NULL REFERENCES articles(id),
  provider TEXT NOT NULL,
  voice TEXT NOT NULL,
  blob_key TEXT NOT NULL,
  duration_s REAL,
  text_hash TEXT NOT NULL UNIQUE,
  chars INTEGER NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE TABLE inbox (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id),
  lexeme_id TEXT NOT NULL REFERENCES lexemes(id),
  context_sentence TEXT NOT NULL,
  article_id TEXT REFERENCES articles(id),
  status TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX inbox_user_status ON inbox(user_id, status);
CREATE TABLE gloss_cache (
  lexeme_id TEXT NOT NULL REFERENCES lexemes(id),
  context_hash TEXT NOT NULL,
  gloss_json TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY (lexeme_id, context_hash)
);
CREATE TABLE llm_calls (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id),
  purpose TEXT NOT NULL,
  provider TEXT NOT NULL,
  model TEXT NOT NULL,
  tokens_in INTEGER NOT NULL,
  tokens_out INTEGER NOT NULL,
  cost_usd REAL,
  latency_ms INTEGER NOT NULL,
  status TEXT NOT NULL,
  error TEXT,
  ts INTEGER NOT NULL
);
CREATE INDEX llm_calls_user_ts ON llm_calls(user_id, ts);
CREATE TABLE word_lists (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id),
  name TEXT NOT NULL,
  kind TEXT NOT NULL,
  source_file TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE TABLE word_list_items (
  id TEXT PRIMARY KEY,
  list_id TEXT NOT NULL REFERENCES word_lists(id),
  user_id TEXT NOT NULL REFERENCES users(id),
  lexeme_id TEXT NOT NULL REFERENCES lexemes(id),
  raw TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE UNIQUE INDEX word_list_items_list_lexeme ON word_list_items(list_id, lexeme_id);
CREATE TABLE lexeme_marks (
  user_id TEXT NOT NULL REFERENCES users(id),
  lexeme_id TEXT NOT NULL REFERENCES lexemes(id),
  status TEXT NOT NULL,
  marked_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (user_id, lexeme_id)
);
`,
  },
  {
    id: "0002_append_only_triggers",
    sql: `
CREATE TRIGGER events_no_update BEFORE UPDATE ON events BEGIN SELECT RAISE(ABORT, 'events is append-only'); END;
CREATE TRIGGER events_no_delete BEFORE DELETE ON events BEGIN SELECT RAISE(ABORT, 'events is append-only'); END;
CREATE TRIGGER review_logs_no_update BEFORE UPDATE ON review_logs BEGIN SELECT RAISE(ABORT, 'review_logs is append-only'); END;
CREATE TRIGGER review_logs_no_delete BEFORE DELETE ON review_logs BEGIN SELECT RAISE(ABORT, 'review_logs is append-only'); END;
`,
  },
  {
    id: "0003_provider_config",
    sql: `
CREATE TABLE provider_config (
  user_id TEXT PRIMARY KEY REFERENCES users(id),
  json TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);
`,
  },
];

export function runMigrations(sqlite: Database.Database): string[] {
  sqlite.exec(
    "CREATE TABLE IF NOT EXISTS _migrations (id TEXT PRIMARY KEY, applied_at INTEGER NOT NULL)",
  );
  const applied = new Set(
    (sqlite.prepare("SELECT id FROM _migrations").all() as { id: string }[]).map((r) => r.id),
  );
  const ran: string[] = [];
  for (const m of MIGRATIONS) {
    if (applied.has(m.id)) continue;
    sqlite.transaction(() => {
      sqlite.exec(m.sql);
      sqlite
        .prepare("INSERT INTO _migrations (id, applied_at) VALUES (?, ?)")
        .run(m.id, Date.now());
    })();
    ran.push(m.id);
  }
  return ran;
}
