import { AppError, ConfigError } from "@eng-learn/providers";
import type { z } from "zod";

export function errorResponse(error: unknown) {
  const known = error instanceof AppError;
  return Response.json(
    {
      error: {
        code: known ? error.code : "INTERNAL_ERROR",
        message: known ? error.message : "操作失败，请稍后重试",
      },
    },
    { status: known ? error.status : 500, headers: { "Cache-Control": "no-store" } },
  );
}
/** Explicit public authority for reverse proxies; never infer it from forwarded headers. */
export function configuredAppOrigin(value = process.env.APP_ORIGIN): string | undefined {
  if (!value?.trim()) return undefined;
  try {
    if (!/^https?:\/\/[^/?#\\]+\/?$/i.test(value.trim())) throw new Error("not an origin");
    const url = new URL(value.trim());
    if (
      !["http:", "https:"].includes(url.protocol) ||
      url.username ||
      url.password ||
      url.pathname !== "/" ||
      url.search ||
      url.hash
    )
      throw new Error("not an origin");
    return url.origin;
  } catch {
    throw new ConfigError(
      "APP_ORIGIN must be an http(s) origin without credentials, path, query or fragment",
    );
  }
}
export function requestOrigin(request: Request): string {
  const configured = configuredAppOrigin();
  if (configured) return configured;
  const url = new URL(request.url);
  const host = request.headers.get("host");
  if (!host) return url.origin;
  // Host is the direct HTTP authority, unlike arbitrary forwarding metadata.
  // Reject malformed authority rather than allowing URL path/credential parsing.
  try {
    if (!/^(?:\[[a-f\d:]+\]|[a-z\d.-]+)(?::\d{1,5})?$/i.test(host))
      throw new Error("invalid authority");
    return new URL(`${url.protocol}//${host}`).origin;
  } catch {
    throw new AppError("FORBIDDEN", "请求 Host 无效", 403);
  }
}
export function checkWriteOrigin(request: Request) {
  if (["GET", "HEAD", "OPTIONS"].includes(request.method)) return;
  const origin = request.headers.get("origin");
  if (
    request.headers.get("sec-fetch-site") === "cross-site" ||
    (origin && origin !== requestOrigin(request))
  )
    throw new AppError("FORBIDDEN", "拒绝跨站写入请求", 403);
}
/** Count actual bytes, not just the untrusted Content-Length, before parsing multipart. */
export async function boundedRequest(request: Request, maxBytes: number) {
  const declared = Number(request.headers.get("content-length"));
  if (declared > maxBytes) throw new AppError("PAYLOAD_TOO_LARGE", "上传文件过大", 413);
  const reader = request.body?.getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;
  if (reader) {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      total += value.length;
      if (total > maxBytes) {
        await reader.cancel();
        throw new AppError("PAYLOAD_TOO_LARGE", "上传文件过大", 413);
      }
      chunks.push(value);
    }
  }
  const bytes = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.length;
  }
  return new Request(request.url, {
    method: request.method,
    headers: request.headers,
    body: bytes,
  });
}
export async function jsonInput<T>(request: Request, schema: z.ZodType<T>) {
  try {
    const bounded = await boundedRequest(request, 64 * 1024);
    return schema.parse(await bounded.json());
  } catch (error) {
    if (error instanceof AppError) throw error;
    throw new AppError("VALIDATION_FAILED", "请求参数无效，请检查输入", 400);
  }
}
