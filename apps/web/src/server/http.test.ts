import { afterEach, describe, expect, it, vi } from "vitest";
import { authenticated, passwordMatches, sessionToken } from "./auth";
import { boundedRequest, checkWriteOrigin, errorResponse } from "./http";

afterEach(() => vi.unstubAllEnvs());
describe("HTTP security", () => {
  it("uses direct Host, not standalone bind address or forged forwarded authority", () => {
    vi.stubEnv("APP_ORIGIN", "");
    const req = (origin: string) =>
      new Request("http://0.0.0.0:3000/api/auth", {
        method: "POST",
        headers: {
          host: "localhost:3000",
          origin,
          "x-forwarded-host": "elsewhere.test",
          "x-forwarded-proto": "https",
        },
      });
    expect(() => checkWriteOrigin(req("http://localhost:3000"))).not.toThrow();
    expect(() => checkWriteOrigin(req("https://elsewhere.test"))).toThrow();
  });
  it("explicit validated public origin supports HTTPS reverse proxies", () => {
    vi.stubEnv("APP_ORIGIN", "https://learning.example.invalid");
    const req = (origin: string) =>
      new Request("http://0.0.0.0:3000/api/auth", {
        method: "POST",
        headers: { host: "internal:3000", origin },
      });
    expect(() => checkWriteOrigin(req("https://learning.example.invalid"))).not.toThrow();
    expect(() => checkWriteOrigin(req("http://internal:3000"))).toThrow();
    for (const invalid of [
      "ftp://host",
      "https://u:p@host",
      "https://host/path",
      "https://host?x=1",
      "null",
    ]) {
      vi.stubEnv("APP_ORIGIN", invalid);
      expect(() => checkWriteOrigin(req("https://learning.example.invalid"))).toThrow(/APP_ORIGIN/);
    }
  });
  it("password and signed cookie verification reject tampering", () => {
    expect(passwordMatches("incorrect", "test-password")).toBe(false);
    expect(passwordMatches("test-password", "test-password")).toBe(true);
    expect(authenticated(sessionToken("test-password"), "test-password")).toBe(true);
    expect(authenticated("a", "test-password")).toBe(false);
    expect(authenticated(undefined, undefined)).toBe(true);
  });
  it("rejects cross-origin writes and allows same-origin requests", () => {
    expect(() =>
      checkWriteOrigin(
        new Request("http://localhost/api/test", {
          method: "POST",
          headers: { origin: "https://elsewhere.test" },
        }),
      ),
    ).toThrow();
    expect(() =>
      checkWriteOrigin(
        new Request("http://localhost/api/test", {
          method: "POST",
          headers: { origin: "http://localhost" },
        }),
      ),
    ).not.toThrow();
    expect(() =>
      checkWriteOrigin(
        new Request("http://localhost/api/test", {
          method: "POST",
          headers: { "sec-fetch-site": "cross-site" },
        }),
      ),
    ).toThrow();
  });
  it("caps streamed request bodies even without Content-Length", async () => {
    await expect(
      boundedRequest(new Request("http://localhost", { method: "POST", body: "12345" }), 4),
    ).rejects.toThrow();
    const request = await boundedRequest(
      new Request("http://localhost", { method: "POST", body: "1234" }),
      4,
    );
    expect(await request.text()).toBe("1234");
  });
  it("never leaks unexpected errors", async () => {
    const response = errorResponse(new Error("private credential"));
    expect(response.status).toBe(500);
    expect(await response.text()).not.toContain("credential");
  });
});
