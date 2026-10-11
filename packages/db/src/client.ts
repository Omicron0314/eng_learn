import { mkdirSync } from "node:fs";
import path from "node:path";
import type { RunResult } from "better-sqlite3";
import Database from "better-sqlite3";
import { type BetterSQLite3Database, drizzle } from "drizzle-orm/better-sqlite3";
import type { BaseSQLiteDatabase } from "drizzle-orm/sqlite-core";
import { runMigrations } from "./migrations";
import * as schema from "./schema";

export type Db = BetterSQLite3Database<typeof schema> & { $client: Database.Database };
export type Conn = BaseSQLiteDatabase<"sync", RunResult, typeof schema>;

export function openDb(file: string): Db {
  if (file !== ":memory:") mkdirSync(path.dirname(path.resolve(file)), { recursive: true });
  const sqlite = new Database(file);
  sqlite.pragma("journal_mode = WAL");
  sqlite.pragma("foreign_keys = ON");
  sqlite.pragma("busy_timeout = 5000");
  return drizzle({ client: sqlite, schema }) as Db;
}

export function migrate(db: Db): string[] {
  return runMigrations(db.$client);
}

export function closeDb(db: Db): void {
  db.$client.close();
}

export function withTx<T>(db: Db, fn: (tx: Conn) => T): T {
  return db.transaction((tx) => fn(tx as unknown as Conn), { behavior: "immediate" });
}
