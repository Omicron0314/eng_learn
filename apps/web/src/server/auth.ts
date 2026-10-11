import { createHash, createHmac, timingSafeEqual } from "node:crypto";

export const SESSION_COOKIE = "eng-session";
function digest(value: string) {
  return createHash("sha256").update(value).digest();
}
export function passwordMatches(input: string, expected: string) {
  return timingSafeEqual(digest(input), digest(expected));
}
export function sessionToken(password: string) {
  return createHmac("sha256", password).update("session").digest("hex");
}
export function authenticated(cookie: string | undefined, password: string | undefined) {
  return !password || (!!cookie && passwordMatches(cookie, sessionToken(password)));
}
export function requestAuthenticated(request: Request) {
  const value = request.headers
    .get("cookie")
    ?.split(";")
    .map((x) => x.trim())
    .find((x) => x.startsWith(`${SESSION_COOKIE}=`))
    ?.slice(SESSION_COOKIE.length + 1);
  return authenticated(value, process.env.APP_PASSWORD);
}
