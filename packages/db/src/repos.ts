import {
  type CardOrigin,
  type CardStateName,
  DEFAULT_SETTINGS,
  type Rating,
  type Settings,
  SettingsSchema,
  type SrsCard,
  type StoredProviderConfig,
  StoredProviderConfigSchema,
  type TimeRange,
} from "@eng-learn/core";
import { and, asc, count, desc, eq, gte, inArray, like, lt, or, sql, sum } from "drizzle-orm";
import { z } from "zod";
import type { Conn } from "./client";
import { type EventPayload, type EventType, parseEventPayload } from "./events";
import { newId } from "./id";
import * as t from "./schema";

export const DEFAULT_USER_ID = "00000000-0000-7000-8000-000000000001";

export const CardBackSchema = z.object({
  meaning: z.string().optional(),
  example: z.string().optional(),
  meaningSource: z.enum(["import", "ai", "user"]),
  pos: z.string().optional(),
});
export type CardBack = z.infer<typeof CardBackSchema>;

export type CardRow = {
  id: string;
  userId: string;
  lexemeId: string;
  lemma: string;
  front: string;
  back: CardBack;
  deck: string;
  suspended: boolean;
  origin: CardOrigin;
  srs: SrsCard;
  createdAt: number;
  updatedAt: number;
};

type CardSelect = typeof t.cards.$inferSelect;

function toSrs(r: CardSelect): SrsCard {
  return {
    due: new Date(r.due),
    stability: r.stability,
    difficulty: r.difficulty,
    state: r.state as CardStateName,
    reps: r.reps,
    lapses: r.lapses,
    lastReview: r.lastReview === null ? null : new Date(r.lastReview),
    elapsedDays: r.elapsedDays,
    scheduledDays: r.scheduledDays,
    learningSteps: r.learningSteps,
  };
}

function srsColumns(s: SrsCard) {
  return {
    due: s.due.getTime(),
    stability: s.stability,
    difficulty: s.difficulty,
    state: s.state,
    reps: s.reps,
    lapses: s.lapses,
    lastReview: s.lastReview ? s.lastReview.getTime() : null,
    elapsedDays: s.elapsedDays,
    scheduledDays: s.scheduledDays,
    learningSteps: s.learningSteps,
  };
}

function toCardRow(r: CardSelect, lemma: string): CardRow {
  return {
    id: r.id,
    userId: r.userId,
    lexemeId: r.lexemeId,
    lemma,
    front: r.front,
    back: CardBackSchema.parse(JSON.parse(r.backJson)),
    deck: r.deck,
    suspended: r.suspended,
    origin: r.origin as CardOrigin,
    srs: toSrs(r),
    createdAt: r.createdAt,
    updatedAt: r.updatedAt,
  };
}

const inRange = (col: Parameters<typeof gte>[0], range: TimeRange) =>
  and(gte(col, range.start), lt(col, range.end));

export function createRepos(db: Conn) {
  const users = {
    ensureDefaultUser(now = Date.now()): string {
      db.insert(t.users)
        .values({ id: DEFAULT_USER_ID, createdAt: now, updatedAt: now })
        .onConflictDoNothing()
        .run();
      return DEFAULT_USER_ID;
    },
  };

  const settings = {
    get(userId: string): Settings {
      const row = db.select().from(t.settings).where(eq(t.settings.userId, userId)).get();
      if (!row) return DEFAULT_SETTINGS;
      const parsed = SettingsSchema.safeParse({ ...DEFAULT_SETTINGS, ...JSON.parse(row.json) });
      return parsed.success ? parsed.data : DEFAULT_SETTINGS;
    },
    save(userId: string, value: Settings, now = Date.now()): void {
      const json = JSON.stringify(SettingsSchema.parse(value));
      db.insert(t.settings)
        .values({ userId, json, updatedAt: now })
        .onConflictDoUpdate({ target: t.settings.userId, set: { json, updatedAt: now } })
        .run();
    },
  };

  const providerConfig = {
    get(userId: string): StoredProviderConfig {
      const row = db
        .select()
        .from(t.providerConfig)
        .where(eq(t.providerConfig.userId, userId))
        .get();
      if (!row) return {};
      const parsed = StoredProviderConfigSchema.safeParse(JSON.parse(row.json));
      return parsed.success ? parsed.data : {};
    },
    save(userId: string, value: StoredProviderConfig, now = Date.now()): void {
      const json = JSON.stringify(StoredProviderConfigSchema.parse(value));
      db.insert(t.providerConfig)
        .values({ userId, json, updatedAt: now })
        .onConflictDoUpdate({ target: t.providerConfig.userId, set: { json, updatedAt: now } })
        .run();
    },
  };

  const events = {
    append<T extends EventType>(
      userId: string,
      type: T,
      payload: EventPayload<T>,
      ts = Date.now(),
    ) {
      const valid = parseEventPayload(type, payload);
      const id = newId(ts);
      db.insert(t.events)
        .values({ id, userId, type, payloadJson: JSON.stringify(valid), ts })
        .run();
      return id;
    },
    listByType<T extends EventType>(userId: string, type: T, range?: TimeRange) {
      const rows = db
        .select()
        .from(t.events)
        .where(
          and(
            eq(t.events.userId, userId),
            eq(t.events.type, type),
            range ? inRange(t.events.ts, range) : undefined,
          ),
        )
        .orderBy(asc(t.events.ts), asc(t.events.id))
        .all();
      return rows.map((r) => ({
        id: r.id,
        ts: r.ts,
        payload: parseEventPayload(type, JSON.parse(r.payloadJson)),
      }));
    },
    timestamps(userId: string, range: TimeRange): number[] {
      return db
        .select({ ts: t.events.ts })
        .from(t.events)
        .where(and(eq(t.events.userId, userId), inRange(t.events.ts, range)))
        .all()
        .map((r) => r.ts);
    },
    count(userId: string, type?: EventType): number {
      return (
        db
          .select({ n: count() })
          .from(t.events)
          .where(and(eq(t.events.userId, userId), type ? eq(t.events.type, type) : undefined))
          .get()?.n ?? 0
      );
    },
  };

  const lexemes = {
    upsert(lemma: string, pos = "", now = Date.now()): string {
      const l = lemma.trim().replace(/\s+/g, " ").toLowerCase();
      const p = pos.trim();
      const existing = db
        .select({ id: t.lexemes.id })
        .from(t.lexemes)
        .where(and(eq(t.lexemes.lemma, l), eq(t.lexemes.pos, p)))
        .get();
      if (existing) return existing.id;
      const id = newId(now);
      db.insert(t.lexemes).values({ id, lemma: l, pos: p, createdAt: now }).run();
      return id;
    },
    findByLemma(lemma: string, pos = "") {
      return db
        .select()
        .from(t.lexemes)
        .where(and(eq(t.lexemes.lemma, lemma.trim().toLowerCase()), eq(t.lexemes.pos, pos)))
        .get();
    },
    get(id: string) {
      return db.select().from(t.lexemes).where(eq(t.lexemes.id, id)).get();
    },
  };

  const cardSelect = () =>
    db
      .select({ card: t.cards, lemma: t.lexemes.lemma })
      .from(t.cards)
      .innerJoin(t.lexemes, eq(t.cards.lexemeId, t.lexemes.id));

  const cards = {
    insert(input: {
      userId: string;
      lexemeId: string;
      front: string;
      back: CardBack;
      deck: string;
      origin: CardOrigin;
      srs: SrsCard;
      now?: number;
    }): string {
      const now = input.now ?? Date.now();
      const id = newId(now);
      db.insert(t.cards)
        .values({
          id,
          userId: input.userId,
          lexemeId: input.lexemeId,
          front: input.front,
          backJson: JSON.stringify(CardBackSchema.parse(input.back)),
          deck: input.deck,
          suspended: false,
          origin: input.origin,
          ...srsColumns(input.srs),
          createdAt: now,
          updatedAt: now,
        })
        .run();
      return id;
    },
    get(id: string): CardRow | undefined {
      const r = cardSelect().where(eq(t.cards.id, id)).get();
      return r ? toCardRow(r.card, r.lemma) : undefined;
    },
    getByLexeme(userId: string, lexemeId: string): CardRow | undefined {
      const r = cardSelect()
        .where(and(eq(t.cards.userId, userId), eq(t.cards.lexemeId, lexemeId)))
        .get();
      return r ? toCardRow(r.card, r.lemma) : undefined;
    },
    listAll(userId: string): CardRow[] {
      return cardSelect()
        .where(eq(t.cards.userId, userId))
        .all()
        .map((r) => toCardRow(r.card, r.lemma));
    },
    updateSrs(id: string, s: SrsCard, now = Date.now()) {
      db.update(t.cards)
        .set({ ...srsColumns(s), updatedAt: now })
        .where(eq(t.cards.id, id))
        .run();
    },
    updateBack(id: string, back: CardBack, now = Date.now()) {
      db.update(t.cards)
        .set({ backJson: JSON.stringify(CardBackSchema.parse(back)), updatedAt: now })
        .where(eq(t.cards.id, id))
        .run();
    },
    setSuspended(id: string, suspended: boolean, now = Date.now()) {
      db.update(t.cards).set({ suspended, updatedAt: now }).where(eq(t.cards.id, id)).run();
    },
    search(
      userId: string,
      f: {
        q?: string | undefined;
        deck?: string | undefined;
        state?: string | undefined;
        suspended?: boolean | undefined;
        offset: number;
        limit: number;
      },
    ): { rows: CardRow[]; total: number } {
      const where = and(
        eq(t.cards.userId, userId),
        f.q
          ? or(like(t.lexemes.lemma, `%${f.q.toLowerCase()}%`), like(t.cards.front, `%${f.q}%`))
          : undefined,
        f.deck ? eq(t.cards.deck, f.deck) : undefined,
        f.state ? eq(t.cards.state, f.state) : undefined,
        f.suspended !== undefined ? eq(t.cards.suspended, f.suspended) : undefined,
      );
      const total =
        db
          .select({ n: count() })
          .from(t.cards)
          .innerJoin(t.lexemes, eq(t.cards.lexemeId, t.lexemes.id))
          .where(where)
          .get()?.n ?? 0;
      const rows = cardSelect()
        .where(where)
        .orderBy(asc(t.cards.createdAt), asc(t.cards.id))
        .limit(f.limit)
        .offset(f.offset)
        .all()
        .map((r) => toCardRow(r.card, r.lemma));
      return { rows, total };
    },
    decks(userId: string): { deck: string; n: number }[] {
      return db
        .select({ deck: t.cards.deck, n: count() })
        .from(t.cards)
        .where(eq(t.cards.userId, userId))
        .groupBy(t.cards.deck)
        .all();
    },
    count(userId: string): number {
      return (
        db.select({ n: count() }).from(t.cards).where(eq(t.cards.userId, userId)).get()?.n ?? 0
      );
    },
    idsByDeck(userId: string, deck: string): string[] {
      return db
        .select({ id: t.cards.id })
        .from(t.cards)
        .where(and(eq(t.cards.userId, userId), eq(t.cards.deck, deck)))
        .all()
        .map((r) => r.id);
    },
    /** Hard delete. review_logs hold an FK to cards, so callers must not delete logged cards. */
    delete(id: string): void {
      db.delete(t.cards).where(eq(t.cards.id, id)).run();
    },
  };

  const reviewLogs = {
    countByCard(cardId: string): number {
      return (
        db.select({ n: count() }).from(t.reviewLogs).where(eq(t.reviewLogs.cardId, cardId)).get()
          ?.n ?? 0
      );
    },
    insert(input: {
      userId: string;
      cardId: string;
      rating: Rating;
      stateBefore: CardStateName;
      dueBefore: number;
      reviewedAt: number;
      durationMs: number;
    }): string {
      const id = newId(input.reviewedAt);
      db.insert(t.reviewLogs)
        .values({ id, ...input })
        .run();
      return id;
    },
    countNewIntroduced(userId: string, range: TimeRange): number {
      return (
        db
          .select({ n: count() })
          .from(t.reviewLogs)
          .where(
            and(
              eq(t.reviewLogs.userId, userId),
              eq(t.reviewLogs.stateBefore, "new"),
              inRange(t.reviewLogs.reviewedAt, range),
            ),
          )
          .get()?.n ?? 0
      );
    },
    countReviews(userId: string, range: TimeRange, stateBefore?: CardStateName): number {
      return (
        db
          .select({ n: count() })
          .from(t.reviewLogs)
          .where(
            and(
              eq(t.reviewLogs.userId, userId),
              inRange(t.reviewLogs.reviewedAt, range),
              stateBefore ? eq(t.reviewLogs.stateBefore, stateBefore) : undefined,
            ),
          )
          .get()?.n ?? 0
      );
    },
    listBetween(userId: string, range: TimeRange) {
      return db
        .select()
        .from(t.reviewLogs)
        .where(and(eq(t.reviewLogs.userId, userId), inRange(t.reviewLogs.reviewedAt, range)))
        .orderBy(asc(t.reviewLogs.reviewedAt), asc(t.reviewLogs.id))
        .all()
        .map((r) => ({
          ...r,
          rating: r.rating as Rating,
          stateBefore: r.stateBefore as CardStateName,
        }));
    },
    listAll(userId: string) {
      return reviewLogs.listBetween(userId, { start: 0, end: Number.MAX_SAFE_INTEGER });
    },
    firstReviewAfter(cardId: string, ts: number): { rating: Rating; reviewedAt: number } | null {
      const r = db
        .select({ rating: t.reviewLogs.rating, reviewedAt: t.reviewLogs.reviewedAt })
        .from(t.reviewLogs)
        .where(and(eq(t.reviewLogs.cardId, cardId), sql`${t.reviewLogs.reviewedAt} > ${ts}`))
        .orderBy(asc(t.reviewLogs.reviewedAt), asc(t.reviewLogs.id))
        .limit(1)
        .get();
      return r ? { rating: r.rating as Rating, reviewedAt: r.reviewedAt } : null;
    },
  };

  const articles = {
    insert(
      input: Omit<typeof t.articles.$inferInsert, "id" | "createdAt" | "updatedAt"> & {
        now?: number;
      },
    ): string {
      const now = input.now ?? Date.now();
      const id = newId(now);
      const { now: _n, ...rest } = input;
      db.insert(t.articles)
        .values({ ...rest, id, createdAt: now, updatedAt: now })
        .run();
      return id;
    },
    get(id: string) {
      return db.select().from(t.articles).where(eq(t.articles.id, id)).get();
    },
    list(userId: string, opts: { range?: TimeRange; limit?: number } = {}) {
      return db
        .select()
        .from(t.articles)
        .where(
          and(
            eq(t.articles.userId, userId),
            opts.range ? inRange(t.articles.createdAt, opts.range) : undefined,
          ),
        )
        .orderBy(desc(t.articles.createdAt))
        .limit(opts.limit ?? 1000)
        .all();
    },
    insertTargets(
      rows: {
        articleId: string;
        lexemeId: string;
        role: "exposed" | "control";
        occurrences: number;
      }[],
    ) {
      if (rows.length > 0) db.insert(t.articleTargets).values(rows).run();
    },
    targets(articleId: string) {
      return db
        .select({
          lexemeId: t.articleTargets.lexemeId,
          role: t.articleTargets.role,
          occurrences: t.articleTargets.occurrences,
          lemma: t.lexemes.lemma,
        })
        .from(t.articleTargets)
        .innerJoin(t.lexemes, eq(t.articleTargets.lexemeId, t.lexemes.id))
        .where(eq(t.articleTargets.articleId, articleId))
        .all();
    },
    /** lexemeId -> total exposed occurrences in articles created since `since`. */
    recentExposure(userId: string, since: number): Map<string, number> {
      const rows = db
        .select({ lexemeId: t.articleTargets.lexemeId, n: sum(t.articleTargets.occurrences) })
        .from(t.articleTargets)
        .innerJoin(t.articles, eq(t.articleTargets.articleId, t.articles.id))
        .where(
          and(
            eq(t.articles.userId, userId),
            eq(t.articleTargets.role, "exposed"),
            gte(t.articles.createdAt, since),
          ),
        )
        .groupBy(t.articleTargets.lexemeId)
        .all();
      return new Map(rows.map((r) => [r.lexemeId, Number(r.n ?? 0)]));
    },
    samples(userId: string) {
      return db
        .select({
          articleId: t.articleTargets.articleId,
          lexemeId: t.articleTargets.lexemeId,
          role: t.articleTargets.role,
          articleCreatedAt: t.articles.createdAt,
        })
        .from(t.articleTargets)
        .innerJoin(t.articles, eq(t.articleTargets.articleId, t.articles.id))
        .where(eq(t.articles.userId, userId))
        .all()
        .map((r) => ({ ...r, role: r.role as "exposed" | "control" }));
    },
  };

  const audios = {
    getByHash(textHash: string) {
      return db.select().from(t.audios).where(eq(t.audios.textHash, textHash)).get();
    },
    upsert(input: Omit<typeof t.audios.$inferInsert, "id"> & { id?: string }): string {
      const existing = audios.getByHash(input.textHash);
      if (existing) {
        db.update(t.audios)
          .set({
            blobKey: input.blobKey,
            durationS: input.durationS,
            createdAt: input.createdAt,
            chars: input.chars,
          })
          .where(eq(t.audios.id, existing.id))
          .run();
        return existing.id;
      }
      const id = input.id ?? newId(input.createdAt);
      db.insert(t.audios)
        .values({ ...input, id })
        .run();
      return id;
    },
    charsInRange(range: TimeRange): number {
      const r = db
        .select({ n: sum(t.audios.chars) })
        .from(t.audios)
        .where(inRange(t.audios.createdAt, range))
        .get();
      return Number(r?.n ?? 0);
    },
  };

  const inboxRepo = {
    insert(input: {
      userId: string;
      lexemeId: string;
      contextSentence: string;
      articleId?: string | null;
      now?: number;
    }) {
      const now = input.now ?? Date.now();
      const id = newId(now);
      db.insert(t.inbox)
        .values({
          id,
          userId: input.userId,
          lexemeId: input.lexemeId,
          contextSentence: input.contextSentence,
          articleId: input.articleId ?? null,
          status: "pending",
          createdAt: now,
          updatedAt: now,
        })
        .run();
      return id;
    },
    findPending(userId: string, lexemeId: string) {
      return db
        .select()
        .from(t.inbox)
        .where(
          and(
            eq(t.inbox.userId, userId),
            eq(t.inbox.lexemeId, lexemeId),
            eq(t.inbox.status, "pending"),
          ),
        )
        .get();
    },
    listPending(userId: string) {
      return db
        .select({ item: t.inbox, lemma: t.lexemes.lemma })
        .from(t.inbox)
        .innerJoin(t.lexemes, eq(t.inbox.lexemeId, t.lexemes.id))
        .where(and(eq(t.inbox.userId, userId), eq(t.inbox.status, "pending")))
        .orderBy(desc(t.inbox.createdAt))
        .all();
    },
    getMany(ids: string[]) {
      if (ids.length === 0) return [];
      return db.select().from(t.inbox).where(inArray(t.inbox.id, ids)).all();
    },
    setStatus(ids: string[], status: "pending" | "added" | "ignored", now = Date.now()) {
      if (ids.length === 0) return;
      db.update(t.inbox).set({ status, updatedAt: now }).where(inArray(t.inbox.id, ids)).run();
    },
    countPending(userId: string): number {
      return (
        db
          .select({ n: count() })
          .from(t.inbox)
          .where(and(eq(t.inbox.userId, userId), eq(t.inbox.status, "pending")))
          .get()?.n ?? 0
      );
    },
  };

  const glossCache = {
    get(lexemeId: string, contextHash: string) {
      const r = db
        .select()
        .from(t.glossCache)
        .where(and(eq(t.glossCache.lexemeId, lexemeId), eq(t.glossCache.contextHash, contextHash)))
        .get();
      return r ? JSON.parse(r.glossJson) : undefined;
    },
    latestFor(lexemeId: string) {
      const r = db
        .select()
        .from(t.glossCache)
        .where(eq(t.glossCache.lexemeId, lexemeId))
        .orderBy(desc(t.glossCache.createdAt))
        .limit(1)
        .get();
      return r ? JSON.parse(r.glossJson) : undefined;
    },
    put(lexemeId: string, contextHash: string, gloss: unknown, now = Date.now()) {
      db.insert(t.glossCache)
        .values({ lexemeId, contextHash, glossJson: JSON.stringify(gloss), createdAt: now })
        .onConflictDoNothing()
        .run();
    },
  };

  const llmCalls = {
    insert(input: Omit<typeof t.llmCalls.$inferInsert, "id">) {
      db.insert(t.llmCalls)
        .values({ ...input, id: newId(input.ts) })
        .run();
    },
    list(userId: string) {
      return db
        .select()
        .from(t.llmCalls)
        .where(eq(t.llmCalls.userId, userId))
        .orderBy(asc(t.llmCalls.ts))
        .all();
    },
  };

  const wordLists = {
    find(userId: string, name: string, kind: string) {
      return db
        .select()
        .from(t.wordLists)
        .where(
          and(
            eq(t.wordLists.userId, userId),
            eq(t.wordLists.name, name),
            eq(t.wordLists.kind, kind),
          ),
        )
        .get();
    },
    create(input: {
      userId: string;
      name: string;
      kind: string;
      sourceFile: string;
      now?: number;
    }): string {
      const now = input.now ?? Date.now();
      const id = newId(now);
      db.insert(t.wordLists)
        .values({
          id,
          userId: input.userId,
          name: input.name,
          kind: input.kind,
          sourceFile: input.sourceFile,
          createdAt: now,
          updatedAt: now,
        })
        .run();
      return id;
    },
    addItem(input: {
      listId: string;
      userId: string;
      lexemeId: string;
      raw: string;
      now?: number;
    }): boolean {
      const now = input.now ?? Date.now();
      const r = db
        .insert(t.wordListItems)
        .values({
          id: newId(now),
          listId: input.listId,
          userId: input.userId,
          lexemeId: input.lexemeId,
          raw: input.raw,
          createdAt: now,
        })
        .onConflictDoNothing()
        .run();
      return r.changes > 0;
    },
    baselineLemmas(userId: string): string[] {
      return db
        .selectDistinct({ lemma: t.lexemes.lemma })
        .from(t.wordListItems)
        .innerJoin(t.wordLists, eq(t.wordListItems.listId, t.wordLists.id))
        .innerJoin(t.lexemes, eq(t.wordListItems.lexemeId, t.lexemes.id))
        .where(and(eq(t.wordLists.userId, userId), eq(t.wordLists.kind, "baseline")))
        .all()
        .map((r) => r.lemma);
    },
    list(userId: string) {
      return db
        .select({ list: t.wordLists, n: count(t.wordListItems.id) })
        .from(t.wordLists)
        .leftJoin(t.wordListItems, eq(t.wordListItems.listId, t.wordLists.id))
        .where(eq(t.wordLists.userId, userId))
        .groupBy(t.wordLists.id)
        .all();
    },
  };

  const marks = {
    upsert(userId: string, lexemeId: string, status: "known" | "unknown", now = Date.now()) {
      db.insert(t.lexemeMarks)
        .values({ userId, lexemeId, status, markedAt: now, updatedAt: now })
        .onConflictDoUpdate({
          target: [t.lexemeMarks.userId, t.lexemeMarks.lexemeId],
          set: { status, markedAt: now, updatedAt: now },
        })
        .run();
    },
    list(userId: string) {
      return db
        .select({
          lemma: t.lexemes.lemma,
          status: t.lexemeMarks.status,
          markedAt: t.lexemeMarks.markedAt,
        })
        .from(t.lexemeMarks)
        .innerJoin(t.lexemes, eq(t.lexemeMarks.lexemeId, t.lexemes.id))
        .where(eq(t.lexemeMarks.userId, userId))
        .all()
        .map((r) => ({ ...r, status: r.status as "known" | "unknown" }));
    },
  };

  return {
    users,
    settings,
    providerConfig,
    events,
    lexemes,
    cards,
    reviewLogs,
    articles,
    audios,
    inbox: inboxRepo,
    glossCache,
    llmCalls,
    wordLists,
    marks,
  };
}

export type Repos = ReturnType<typeof createRepos>;
