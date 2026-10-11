import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { newSrsCard } from "@eng-learn/core";
import { getTableConfig, type SQLiteTable } from "drizzle-orm/sqlite-core";
import { afterAll, describe, expect, it } from "vitest";
import { closeDb, migrate, openDb, withTx } from "./client";
import { newId } from "./id";
import { createRepos, DEFAULT_USER_ID } from "./repos";
import * as schema from "./schema";

function freshDb() {
  const db = openDb(":memory:");
  migrate(db);
  const repos = createRepos(db);
  repos.users.ensureDefaultUser();
  return { db, repos };
}

describe("migrations", () => {
  it("AC1: migrate is idempotent", () => {
    const db = openDb(":memory:");
    expect(migrate(db)).toEqual(["0001_init", "0002_append_only_triggers", "0003_provider_config"]);
    expect(migrate(db)).toEqual([]);
  });

  it("AC2: every Drizzle table and column exists in the database", () => {
    const { db } = freshDb();
    for (const table of Object.values(schema) as SQLiteTable[]) {
      const cfg = getTableConfig(table);
      const cols = (
        db.$client.prepare(`PRAGMA table_info(${cfg.name})`).all() as { name: string }[]
      ).map((c) => c.name);
      expect(cols.sort(), cfg.name).toEqual(cfg.columns.map((c) => c.name).sort());
    }
  });
});

describe("constraints", () => {
  it("AC3: lexeme upsert is case and whitespace insensitive", () => {
    const { repos } = freshDb();
    const a = repos.lexemes.upsert(" Run ");
    const b = repos.lexemes.upsert("run");
    expect(a).toBe(b);
    expect(repos.lexemes.upsert("run", "v")).not.toBe(a);
  });

  it("AC4: one card per word and foreign keys enforced", () => {
    const { repos } = freshDb();
    const lex = repos.lexemes.upsert("apple");
    const card = {
      userId: DEFAULT_USER_ID,
      lexemeId: lex,
      front: "apple",
      back: { meaningSource: "import" as const },
      deck: "d",
      origin: "import" as const,
      srs: newSrsCard(new Date()),
    };
    repos.cards.insert(card);
    expect(() => repos.cards.insert(card)).toThrow();
    expect(() => repos.cards.insert({ ...card, lexemeId: "missing" })).toThrow();
  });

  it("AC5: events and review_logs are append-only", () => {
    const { db, repos } = freshDb();
    repos.events.append(DEFAULT_USER_ID, "article_open", { articleId: "x" });
    const cardId = repos.cards.insert({
      userId: DEFAULT_USER_ID,
      lexemeId: repos.lexemes.upsert("pear"),
      front: "pear",
      back: { meaningSource: "import" },
      deck: "d",
      origin: "import",
      srs: newSrsCard(new Date()),
    });
    repos.reviewLogs.insert({
      userId: DEFAULT_USER_ID,
      cardId,
      rating: "good",
      stateBefore: "new",
      dueBefore: 0,
      reviewedAt: Date.now(),
      durationMs: 1,
    });
    expect(() => db.$client.exec("UPDATE events SET type = 'y'")).toThrow(/append-only/);
    expect(() => db.$client.exec("DELETE FROM events")).toThrow(/append-only/);
    expect(() => db.$client.exec("UPDATE review_logs SET rating = 'again'")).toThrow(/append-only/);
    expect(() => db.$client.exec("DELETE FROM review_logs")).toThrow(/append-only/);
  });

  it("AC6: failed transactions roll back events and rows", () => {
    const { db, repos } = freshDb();
    expect(() =>
      withTx(db, (tx) => {
        const r = createRepos(tx);
        r.lexemes.upsert("ghost");
        r.events.append(DEFAULT_USER_ID, "article_open", { articleId: "x" });
        throw new Error("boom");
      }),
    ).toThrow("boom");
    expect(repos.events.count(DEFAULT_USER_ID)).toBe(0);
    expect(repos.lexemes.findByLemma("ghost")).toBeUndefined();
  });

  it("invalid event payloads are rejected", () => {
    const { repos } = freshDb();
    // @ts-expect-error payload missing articleId
    expect(() => repos.events.append(DEFAULT_USER_ID, "article_open", {})).toThrow();
  });
});

describe("newId", () => {
  it("AC7: produces ordered UUIDv7 values", () => {
    const now = Date.now();
    const ids = Array.from({ length: 1000 }, () => newId(now));
    for (const id of ids)
      expect(id).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
    expect([...ids].sort()).toEqual(ids);
    expect(new Set(ids).size).toBe(1000);
  });
});

describe("settings", () => {
  it("AC8: defaults, validation and persistence", () => {
    const { repos } = freshDb();
    const s = repos.settings.get(DEFAULT_USER_ID);
    expect(s.dailyNewLimit).toBe(20);
    expect(() => repos.settings.save(DEFAULT_USER_ID, { ...s, dailyNewLimit: -1 })).toThrow();
    repos.settings.save(DEFAULT_USER_ID, { ...s, dailyNewLimit: 5 });
    expect(repos.settings.get(DEFAULT_USER_ID).dailyNewLimit).toBe(5);
  });
});

describe("providerConfig", () => {
  it("round-trips stored provider config and rejects invalid values", () => {
    const { repos } = freshDb();
    expect(repos.providerConfig.get(DEFAULT_USER_ID)).toEqual({});
    const value = {
      llm: {
        provider: "openai-compatible" as const,
        baseURL: "https://llm.example.com/v1",
        apiKey: "sk-test-only",
        model: "test-model",
      },
      tts: { provider: "none" as const },
    };
    repos.providerConfig.save(DEFAULT_USER_ID, value);
    expect(repos.providerConfig.get(DEFAULT_USER_ID)).toEqual(value);
    expect(() =>
      repos.providerConfig.save(DEFAULT_USER_ID, {
        llm: { ...value.llm, baseURL: "not-a-url" },
      }),
    ).toThrow();
  });
});

describe("file persistence", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "db-"));
  afterAll(() => rmSync(dir, { recursive: true, force: true }));

  it("AC9: data survives reopen", () => {
    const file = path.join(dir, "app.sqlite");
    const db1 = openDb(file);
    migrate(db1);
    const r1 = createRepos(db1);
    r1.users.ensureDefaultUser();
    r1.lexemes.upsert("persist");
    closeDb(db1);
    const db2 = openDb(file);
    migrate(db2);
    expect(createRepos(db2).lexemes.findByLemma("persist")).toBeDefined();
    closeDb(db2);
  });
});
