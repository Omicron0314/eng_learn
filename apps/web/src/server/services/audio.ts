import { createHash } from "node:crypto";
import { dayRange } from "@eng-learn/core";
import { AppError, ProviderError } from "@eng-learn/providers";
import { z } from "zod";
import type { AppDeps } from "../deps";
import { nonempty, requireArticle, singleFlight, validate } from "./shared";

export type AudioResult =
  | { mode: "server"; audioId: string; url: string; durationS: number | null; cacheHit: boolean }
  | { mode: "browser"; text: string };
export function audioTextHash(input: {
  provider: string;
  model: string;
  voice: string;
  text: string;
}) {
  return createHash("sha256")
    .update(JSON.stringify([input.provider, input.model, input.voice, input.text]))
    .digest("hex");
}
export function getTtsUsage(deps: AppDeps) {
  const range = dayRange(deps.clock(), deps.settings());
  const rowsUsage = deps.repos.audios.charsInRange(range);
  // Reservations survive partial/provider/blob/cache failures. Legacy successes without
  // a reservation still count; linked successes must not count the same work twice.
  const reservedUsage = deps.repos.events
    .listByType(deps.userId, "audio_reserve", range)
    .reduce((n, e) => n + e.payload.chars, 0);
  const loggedUsage =
    reservedUsage +
    deps.repos.events
      .listByType(deps.userId, "audio_generate", range)
      .filter((e) => !e.payload.reservationId)
      .reduce((n, e) => n + e.payload.chars, 0);
  return { usedChars: Math.max(rowsUsage, loggedUsage), limit: deps.settings().ttsDailyCharLimit };
}
export function checkTtsBudget(deps: AppDeps, chars: number) {
  const { usedChars, limit } = getTtsUsage(deps);
  if (limit === 0 || usedChars + chars > limit)
    throw new AppError(
      "TTS_DAILY_LIMIT_EXCEEDED",
      `今日已用 ${usedChars} 字符，本次需要 ${chars}，上限 ${limit}，可改用浏览器朗读`,
      429,
    );
}
// Serialize budget checks across different hashes, in addition to same-hash deduplication.
const audioQueues = new WeakMap<AppDeps, Promise<unknown>>();
export async function ensureAudio(deps: AppDeps, articleId: string): Promise<AudioResult> {
  const article = requireArticle(deps, articleId);
  const text = `${article.title}\n\n${article.body}`;
  const tts = deps.tts;
  if (!tts) return { mode: "browser", text };
  const hash = audioTextHash({ provider: tts.provider, model: tts.model, voice: tts.voice, text });
  return singleFlight(deps, `audio:${hash}`, () => {
    const previous = audioQueues.get(deps) ?? Promise.resolve();
    const result = previous
      .catch(() => undefined)
      .then(async (): Promise<AudioResult> => {
        const existing = deps.repos.audios.getByHash(hash);
        if (existing && (await deps.blobs.has(existing.blobKey)))
          return {
            mode: "server",
            audioId: existing.id,
            url: `/api/audio/${existing.blobKey}`,
            durationS: existing.durationS,
            cacheHit: true,
          };
        // Persist the full conservative charge before any paid work; never refund
        // ambiguous failures (including a later chunk failure). No cache transaction
        // can roll this back. A failed reservation prevents the provider call.
        const reservationId = deps.tx((repos) => {
          checkTtsBudget({ ...deps, repos }, text.length);
          return repos.events.append(
            deps.userId,
            "audio_reserve",
            { articleId, chars: text.length },
            deps.clock().getTime(),
          );
        });
        let synthesized: Awaited<ReturnType<typeof tts.synthesize>>;
        try {
          synthesized = await tts.synthesize({ text });
        } catch {
          throw new ProviderError("语音合成失败，请稍后重试");
        }
        const ext = synthesized.mimeType === "audio/wav" ? "wav" : "mp3";
        const blobKey = `audio/${hash}.${ext}`;
        await deps.blobs.put(blobKey, synthesized.audio, synthesized.mimeType);
        const audioId = deps.tx((repos) => {
          const now = deps.clock().getTime();
          const id = repos.audios.upsert({
            articleId,
            provider: tts.provider,
            voice: tts.voice,
            blobKey,
            durationS: synthesized.durationS,
            textHash: hash,
            chars: text.length,
            createdAt: now,
          });
          repos.events.append(
            deps.userId,
            "audio_generate",
            { articleId, audioId: id, chars: text.length, reservationId },
            now,
          );
          return id;
        });
        return {
          mode: "server",
          audioId,
          url: `/api/audio/${blobKey}`,
          durationS: synthesized.durationS,
          cacheHit: false,
        };
      });
    audioQueues.set(deps, result);
    void result
      .finally(() => {
        if (audioQueues.get(deps) === result) audioQueues.delete(deps);
      })
      .catch(() => undefined);
    return result;
  });
}
export function recordAudioPlay(
  deps: AppDeps,
  input: { articleId: string; rate: number; via: "server" | "browser" },
) {
  input = validate(
    z.object({
      articleId: nonempty,
      rate: z.number().min(0.75).max(1.25),
      via: z.enum(["server", "browser"]),
    }),
    input,
  );
  requireArticle(deps, input.articleId);
  deps.tx((repos) => repos.events.append(deps.userId, "audio_play", input, deps.clock().getTime()));
}
/** Transport-neutral byte response for the next phase's Range-capable route. */
export async function readAudio(deps: AppDeps, key: string, range?: string) {
  if (!/^audio\/[a-f0-9]{64}\.(wav|mp3)$/.test(key))
    throw new AppError("NOT_FOUND", "音频不存在", 404);
  const blob = await deps.blobs.get(key);
  if (!blob) throw new AppError("NOT_FOUND", "音频不存在", 404);
  const headers: Record<string, string> = {
    "Content-Type": blob.contentType,
    "Accept-Ranges": "bytes",
  };
  if (!range)
    return {
      status: 200,
      headers: { ...headers, "Content-Length": String(blob.data.length) },
      data: blob.data,
    };
  const m = range.match(/^bytes=(\d*)-(\d*)$/);
  if (!m || (!m[1] && !m[2]))
    return {
      status: 416,
      headers: { ...headers, "Content-Range": `bytes */${blob.data.length}` },
      data: new Uint8Array(),
    };
  const start = m[1] ? Number(m[1]) : Math.max(0, blob.data.length - Number(m[2]));
  const end = m[1] && m[2] ? Math.min(Number(m[2]), blob.data.length - 1) : blob.data.length - 1;
  if (start >= blob.data.length || start > end || (!m[1] && Number(m[2]) === 0))
    return {
      status: 416,
      headers: { ...headers, "Content-Range": `bytes */${blob.data.length}` },
      data: new Uint8Array(),
    };
  const data = blob.data.slice(start, end + 1);
  return {
    status: 206,
    headers: {
      ...headers,
      "Content-Length": String(data.length),
      "Content-Range": `bytes ${start}-${end}/${blob.data.length}`,
    },
    data,
  };
}
export const audioService = {
  ensureAudio,
  readAudio,
  recordAudioPlay,
  getTtsUsage,
  checkTtsBudget,
};
