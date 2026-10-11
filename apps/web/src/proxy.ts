import { type NextRequest, NextResponse } from "next/server";
import { MULTIPART_IMPORT_MAX_BYTES } from "./import-limits";
import { authenticated, SESSION_COOKIE } from "./server/auth";
import { checkWriteOrigin, errorResponse } from "./server/http";

export function proxy(request: NextRequest) {
  try {
    checkWriteOrigin(request);
  } catch (error) {
    return errorResponse(error);
  }
  const pathname = request.nextUrl.pathname;
  const publicPath =
    pathname === "/login" ||
    pathname === "/api/auth" ||
    pathname === "/manifest.webmanifest" ||
    pathname === "/icon.svg" ||
    pathname.startsWith("/_next/");
  if (
    !publicPath &&
    !authenticated(request.cookies.get(SESSION_COOKIE)?.value, process.env.APP_PASSWORD)
  ) {
    if (pathname.startsWith("/api/"))
      return NextResponse.json(
        { error: { code: "UNAUTHORIZED", message: "请先输入访问密码" } },
        { status: 401 },
      );
    return NextResponse.redirect(new URL("/login", request.url));
  }
  if (
    pathname.startsWith("/api/import/") &&
    Number(request.headers.get("content-length")) > MULTIPART_IMPORT_MAX_BYTES
  ) {
    return NextResponse.json(
      {
        error: {
          code: "PAYLOAD_TOO_LARGE",
          message: "上传总大小不能超过 64 MiB（另允许 64 KiB 表单元数据）",
        },
      },
      { status: 413 },
    );
  }
  const response = NextResponse.next();
  response.headers.set("X-Content-Type-Options", "nosniff");
  response.headers.set("Referrer-Policy", "same-origin");
  response.headers.set("X-Frame-Options", "DENY");
  if (!publicPath) response.headers.set("Cache-Control", "no-store");
  return response;
}
export const config = { matcher: ["/((?!_next/static|_next/image|favicon.ico).*)"] };
