import { SettingsSchema } from "@eng-learn/core";
import { AppError } from "@eng-learn/providers";
import { NextResponse } from "next/server";
import { z } from "zod";
import { importFileMaxBytes, MULTIPART_IMPORT_MAX_BYTES } from "../../../import-limits";
import {
  passwordMatches,
  requestAuthenticated,
  SESSION_COOKIE,
  sessionToken,
} from "../../../server/auth";
import {
  boundedRequest,
  checkWriteOrigin,
  errorResponse,
  jsonInput,
  requestOrigin,
} from "../../../server/http";
import { getDeps } from "../../../server/runtime";
import * as services from "../../../server/services";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";
const id = z.string().uuid();
const tiers = z.enum(["easy", "medium", "hard"]);
const ratings = z.enum(["again", "hard", "good", "easy"]);
const markSchema = z
  .object({
    action: z.enum(["known", "unknown", "add"]),
    lemma: z.string().min(1).max(1000),
    sentence: z.string().min(1).max(10000),
    articleId: id.optional(),
  })
  .strict();
const querySchema = z
  .object({
    q: z.string().max(1000).optional(),
    deck: z.string().min(1).max(1000).optional(),
    state: z.enum(["new", "learning", "review", "relearning"]).optional(),
    suspended: z
      .enum(["true", "false"])
      .transform((x) => x === "true")
      .optional(),
    offset: z.coerce.number().int().min(0).optional(),
    limit: z.coerce.number().int().min(1).max(100).optional(),
  })
  .strict();
// A bounded, process-local slowdown protects the single-user gate against repeated guesses.
const attempts = new Map<string, { count: number; until: number }>();
async function handle(request: Request, context: { params: Promise<{ path: string[] }> }) {
  try {
    checkWriteOrigin(request);
    const { path } = await context.params;
    const key = path.join("/");
    const method = request.method;
    if (key === "auth" && method === "POST") {
      const { password } = await jsonInput(
        request,
        z.object({ password: z.string().max(1000) }).strict(),
      );
      const address = "single-user-gate";
      const previous = attempts.get(address);
      if (previous && previous.count >= 10 && previous.until > Date.now())
        throw new AppError("RATE_LIMITED", "尝试过多，请一分钟后重试", 429);
      if (process.env.APP_PASSWORD && !passwordMatches(password, process.env.APP_PASSWORD)) {
        if (attempts.size > 1000) attempts.clear();
        attempts.set(address, {
          count: previous && previous.until > Date.now() ? previous.count + 1 : 1,
          until: Date.now() + 60000,
        });
        throw new AppError("UNAUTHORIZED", "密码不正确", 401);
      }
      attempts.delete(address);
      const response = NextResponse.json({ ok: true });
      response.cookies.set(SESSION_COOKIE, sessionToken(process.env.APP_PASSWORD ?? ""), {
        httpOnly: true,
        sameSite: "strict",
        secure: new URL(requestOrigin(request)).protocol === "https:",
        path: "/",
        maxAge: 604800,
      });
      return response;
    }
    if (!requestAuthenticated(request)) throw new AppError("UNAUTHORIZED", "请先输入访问密码", 401);
    if (key === "auth" && method === "DELETE") {
      const response = NextResponse.json({ ok: true });
      response.cookies.delete(SESSION_COOKIE);
      return response;
    }
    const deps = getDeps();
    let result: unknown;
    if (method === "GET") {
      const params = Object.fromEntries(new URL(request.url).searchParams);
      if (key === "today") result = services.getTodayView(deps);
      else if (key === "review/queue") result = services.getQueue(deps);
      else if (key === "cards") result = services.listCards(deps, querySchema.parse(params));
      else if (key === "inbox") result = services.listInbox(deps);
      else if (key === "settings") result = services.getSettingsView(deps);
      else if (key === "stats")
        result = services.getStats(
          deps,
          z
            .object({
              offset: z.coerce.number().int().min(0).optional(),
              limit: z.coerce.number().int().min(1).max(100).optional(),
            })
            .strict()
            .parse(params),
        );
      else if (path[0] === "cards" && path.length === 2)
        result = services.getReviewCard(deps, id.parse(path[1]));
      else if (path[0] === "articles" && path.length === 2) {
        const articleId = id.parse(path[1]);
        const article = services.getArticleView(deps, articleId);
        if (!article) throw new AppError("NOT_FOUND", "文章不存在", 404);
        result = { ...article, feedback: services.latestDifficulty(deps, articleId) };
      } else if (path[0] === "audio") {
        const audio = await services.readAudio(
          deps,
          path.slice(1).join("/"),
          request.headers.get("range") ?? undefined,
        );
        return new Response(new Uint8Array(audio.data), {
          status: audio.status,
          headers: { ...audio.headers, "Cache-Control": "private, no-store" },
        });
      } else throw new AppError("NOT_FOUND", "接口不存在", 404);
    } else if (method === "POST" || method === "PATCH") {
      if (key === "import/cards" || key === "import/wordlist") {
        const bounded = await boundedRequest(request, MULTIPART_IMPORT_MAX_BYTES);
        let form: FormData;
        try {
          form = await bounded.formData();
        } catch {
          throw new AppError("VALIDATION_FAILED", "上传格式无效", 400);
        }
        const file = form.get("file");
        if (!(file instanceof File)) throw new AppError("VALIDATION_FAILED", "请选择文件", 400);
        if (file.size > importFileMaxBytes(file.name))
          throw new AppError(
            "PAYLOAD_TOO_LARGE",
            `文件不能超过 ${importFileMaxBytes(file.name) / (1024 * 1024)} MiB`,
            413,
          );
        const base = {
          fileName: z.string().min(1).max(1000).parse(file.name),
          bytes: new Uint8Array(await file.arrayBuffer()),
        };
        if (key === "import/cards")
          result = await services.importCards(deps, {
            ...base,
            deck: z.string().trim().min(1).max(1000).parse(form.get("deck")),
          });
        else
          result = await services.importWordList(deps, {
            ...base,
            name: z.string().trim().min(1).max(1000).parse(form.get("name")),
            kind: z.enum(["baseline", "study"]).parse(form.get("kind")),
            mergeIfExists:
              z
                .enum(["true", "false"])
                .default("false")
                .parse(form.get("mergeIfExists") ?? undefined) === "true",
          });
      } else if (key === "review/rate")
        result = services.rate(
          deps,
          await jsonInput(
            request,
            z
              .object({
                cardId: id,
                rating: ratings,
                durationMs: z.number().int().min(0).max(86400000),
              })
              .strict(),
          ),
        );
      else if (key === "articles")
        result = await services.generateArticle(
          deps,
          await jsonInput(
            request,
            z
              .object({
                tier: tiers.optional(),
                genre: z.enum(["story", "dialogue", "monologue"]).optional(),
              })
              .strict(),
          ),
        );
      else if (key === "gloss")
        result = await services.lookupArticleWord(
          deps,
          await jsonInput(
            request,
            z.object({ articleId: id, start: z.number().int().min(0) }).strict(),
          ),
        );
      else if (key === "marks") {
        const input = await jsonInput(request, markSchema);
        result =
          input.action === "known"
            ? services.markKnown(deps, input)
            : input.action === "unknown"
              ? services.markUnknown(deps, input)
              : services.addToReview(deps, input);
      } else if (key === "inbox/batch")
        result = services.batchInbox(
          deps,
          await jsonInput(
            request,
            z
              .object({ action: z.enum(["add", "ignore"]), ids: z.array(id).min(1).max(1000) })
              .strict(),
          ),
        );
      else if (key === "settings/providers")
        result = services.updateProviderConfig(deps, await jsonInput(request, z.unknown()));
      else if (key === "settings") {
        const patch = await jsonInput(
          request,
          z
            .object(
              Object.fromEntries(
                Object.entries(SettingsSchema.shape).map(([k, v]) => [
                  k,
                  v.removeDefault().optional(),
                ]),
              ),
            )
            .strict(),
        );
        result = services.updateSettings(deps, patch);
      } else if (key === "cards/delete-deck") {
        result = services.deleteDeck(
          deps,
          await jsonInput(request, z.object({ deck: z.string().min(1).max(1000) }).strict()),
        );
      } else if (path[0] === "cards" && path.length === 3) {
        const cardId = id.parse(path[1]);
        if (path[2] === "meaning") result = await services.ensureMeaning(deps, cardId);
        else if (path[2] === "text")
          result = services.updateCardText(deps, {
            cardId,
            ...(await jsonInput(
              request,
              z
                .object({
                  meaning: z.string().max(1000).optional(),
                  example: z.string().max(5000).optional(),
                })
                .strict(),
            )),
          });
        else if (path[2] === "suspend")
          result = services.setSuspended(deps, {
            cardId,
            ...(await jsonInput(request, z.object({ suspended: z.boolean() }).strict())),
          });
        else throw new AppError("NOT_FOUND", "接口不存在", 404);
      } else if (path[0] === "articles" && path.length === 3) {
        const articleId = id.parse(path[1]);
        if (path[2] === "audio") {
          await jsonInput(request, z.object({}).strict());
          result = await services.ensureAudio(deps, articleId);
        } else if (path[2] === "events")
          result = services.recordArticleEvent(deps, {
            articleId,
            ...(await jsonInput(
              request,
              z
                .object({
                  type: z.enum(["article_open", "article_complete"]),
                  via: z.enum(["button", "scroll"]).optional(),
                  openId: id.optional(),
                })
                .strict(),
            )),
          });
        else if (path[2] === "play")
          result = services.recordAudioPlay(deps, {
            articleId,
            ...(await jsonInput(
              request,
              z
                .object({
                  rate: z.number().min(0.75).max(1.25),
                  via: z.enum(["server", "browser"]),
                })
                .strict(),
            )),
          });
        else if (path[2] === "feedback") {
          const input = await jsonInput(
            request,
            z.discriminatedUnion("type", [
              z
                .object({
                  type: z.literal("difficulty"),
                  value: z.enum(["too_hard", "just_right", "too_easy"]),
                  after: z.enum(["reading", "listening"]),
                })
                .strict(),
              z
                .object({
                  type: z.literal("unnatural"),
                  sentenceIndexes: z.array(z.number().int().min(0)).max(1000),
                  note: z.string().max(200).optional(),
                })
                .strict(),
            ]),
          );
          result =
            input.type === "difficulty"
              ? services.submitDifficulty(deps, { articleId, ...input })
              : services.flagUnnatural(deps, { articleId, ...input });
        } else throw new AppError("NOT_FOUND", "接口不存在", 404);
      } else throw new AppError("NOT_FOUND", "接口不存在", 404);
    } else throw new AppError("METHOD_NOT_ALLOWED", "不支持此请求方式", 405);
    return Response.json(result ?? { ok: true }, { headers: { "Cache-Control": "no-store" } });
  } catch (error) {
    if (error instanceof z.ZodError)
      return errorResponse(new AppError("VALIDATION_FAILED", "请求参数无效，请检查输入", 400));
    return errorResponse(error);
  }
}
export const GET = handle;
export const POST = handle;
export const PATCH = handle;
export const DELETE = handle;
