import { z } from "zod";

const id = z.string().min(1);
const rating = z.enum(["again", "hard", "good", "easy"]);
const state = z.enum(["new", "learning", "review", "relearning"]);

export const eventSchemas = {
  review: z.object({
    cardId: id,
    rating,
    stateBefore: state,
    stateAfter: state,
    dueBefore: z.number(),
    dueAfter: z.number(),
    durationMs: z.number().int().min(0),
  }),
  import: z.object({
    source: z.enum(["csv", "apkg", "txt"]),
    fileName: z.string(),
    kind: z.enum(["cards", "baseline", "study"]),
    deck: z.string().optional(),
    listId: z.string().optional(),
    added: z.number().int(),
    skipped: z.number().int(),
    duplicates: z.number().int(),
  }),
  card_edit: z.object({ cardId: id, fields: z.array(z.string()).min(1) }),
  card_suspend: z.object({ cardId: id, suspended: z.boolean() }),
  settings_update: z.object({ changed: z.array(z.string()).min(1) }),
  article_generate: z.object({
    articleId: id,
    attempts: z.number().int().min(1),
    coverage: z.number(),
    belowTier: z.boolean(),
    missingTargets: z.array(z.string()),
  }),
  article_open: z.object({ articleId: id }),
  article_complete: z.object({
    articleId: id,
    via: z.enum(["button", "scroll"]),
    openId: id.optional(),
  }),
  lookup: z.object({
    lexemeId: id,
    lemma: z.string(),
    surface: z.string(),
    sentence: z.string(),
    articleId: z.string().optional(),
    cacheHit: z.boolean(),
  }),
  mark_known: z.object({ lexemeId: id, articleId: z.string().optional() }),
  mark_unknown: z.object({
    lexemeId: id,
    articleId: z.string().optional(),
    inboxId: z.string().optional(),
    cardId: z.string().optional(),
  }),
  add_to_review: z.object({
    lexemeId: id,
    cardId: id,
    from: z.enum(["lookup", "inbox", "autoAdd"]),
    articleId: z.string().optional(),
  }),
  inbox_ignore: z.object({ inboxId: id, lexemeId: id }),
  audio_reserve: z.object({ articleId: id, chars: z.number().int().min(0) }),
  audio_generate: z.object({
    articleId: id,
    audioId: id,
    chars: z.number().int().min(0),
    reservationId: id.optional(),
  }),
  audio_play: z.object({
    articleId: id,
    rate: z.number().min(0.5).max(2),
    via: z.enum(["server", "browser"]),
  }),
  difficulty_feedback: z.object({
    articleId: id,
    value: z.enum(["too_hard", "just_right", "too_easy"]),
    after: z.enum(["reading", "listening"]),
  }),
  unnatural_flag: z.object({
    articleId: id,
    sentenceIndex: z.number().int().min(0).optional(),
    note: z.string().max(200).optional(),
  }),
} as const;

export type EventType = keyof typeof eventSchemas;
export type EventPayload<T extends EventType> = z.infer<(typeof eventSchemas)[T]>;
export const EVENT_TYPES = Object.keys(eventSchemas) as EventType[];

export function parseEventPayload<T extends EventType>(type: T, payload: unknown): EventPayload<T> {
  const schema = eventSchemas[type];
  if (!schema) throw new Error(`Unknown event type: ${String(type)}`);
  return schema.parse(payload) as EventPayload<T>;
}
