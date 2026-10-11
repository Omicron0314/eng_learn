import { afterEach, describe, expect, it, vi } from "vitest";
import { sessionToken } from "./auth";
import type { AppDeps } from "./deps";
import { seedArticle, setup } from "./services/test-support";

const holder = vi.hoisted(() => ({ deps: undefined as AppDeps | undefined }));
vi.mock("./runtime", () => ({
  getDeps: () => {
    if (!holder.deps) throw new Error("test deps not initialized");
    return holder.deps;
  },
}));

import { GET, POST } from "../app/api/[...path]/route";

afterEach(() => {
  vi.unstubAllEnvs();
});
function call(path: string, body?: unknown, headers: Record<string, string> = {}) {
  const request = new Request(`http://localhost/api/${path}`, {
    method: body === undefined ? "GET" : "POST",
    headers: {
      ...(body instanceof FormData ? {} : { "Content-Type": "application/json" }),
      ...headers,
    },
    ...(body === undefined ? {} : { body: body instanceof FormData ? body : JSON.stringify(body) }),
  });
  return (body === undefined ? GET : POST)(request, {
    params: Promise.resolve({ path: path.split("?")[0]?.split("/") ?? [] }),
  });
}
function initialize() {
  const state = setup();
  holder.deps = state.deps;
  vi.stubEnv("APP_PASSWORD", "");
  return state;
}

describe("API routes against memory SQLite and fakes", () => {
  it("keeps HTTPS proxy sessions Secure with explicitly configured public origin", async () => {
    initialize();
    vi.stubEnv("APP_ORIGIN", "https://learning.example.invalid");
    const response = await call(
      "auth",
      { password: "test-password" },
      { origin: "https://learning.example.invalid", host: "internal:3000" },
    );
    expect(response.status).toBe(200);
    expect(response.headers.get("set-cookie")).toContain("Secure");
  });
  it("enforces .apkg 64 MiB and CSV/TXT 20 MiB file limits at multipart boundary", async () => {
    initialize();
    for (const fileName of ["large.csv", "large.txt", "large.apkg"]) {
      const form = new FormData();
      form.set("deck", "test");
      form.set("name", "baseline");
      form.set("kind", "baseline");
      const limit = fileName.endsWith(".apkg") ? 64 : 20;
      form.set("file", new File([new Uint8Array(limit * 1024 * 1024 + 1)], fileName));
      const response = await call(
        fileName.endsWith(".txt") ? "import/wordlist" : "import/cards",
        form,
      );
      expect(response.status, fileName).toBe(413);
    }
    const form = new FormData();
    form.set("deck", "test");
    form.set("file", new File([new Uint8Array(21 * 1024 * 1024)], "allowed-size.apkg"));
    const response = await call("import/cards", form);
    // Not a ZIP, but >20 MiB must reach format validation rather than a size rejection.
    expect(response.status).toBe(400);
  });
  it("protects pages' transport including audio, rejects cross-origin writes and issues httpOnly cookies", async () => {
    initialize();
    vi.stubEnv("APP_PASSWORD", "test-password");
    expect((await call("today")).status).toBe(401);
    expect((await call("audio/audio/invalid.wav")).status).toBe(401);
    expect(
      (await call("auth", { password: "test-password" }, { origin: "https://elsewhere.test" }))
        .status,
    ).toBe(403);
    const response = await call("auth", { password: "test-password" });
    expect(response.status).toBe(200);
    expect(response.headers.get("set-cookie")).toContain("HttpOnly");
    expect(response.headers.get("set-cookie")).toContain("SameSite=strict");
    expect(
      (await call("today", undefined, { cookie: `eng-session=${sessionToken("test-password")}` }))
        .status,
    ).toBe(200);
  });
  it("imports, reviews and generates through validated API inputs with corresponding events", async () => {
    const { deps } = initialize();
    const form = new FormData();
    form.set("deck", "test");
    form.set("file", new File(["word,meaning\nlearn,学习\nwalk,走路\nhelp,帮助"], "words.csv"));
    const imported = await call("import/cards", form);
    expect(await imported.json()).toMatchObject({ added: 3 });
    const queue = await (await call("review/queue")).json();
    const cardId = queue.new[0];
    expect((await call("review/rate", { cardId, rating: "invalid", durationMs: 0 })).status).toBe(
      400,
    );
    for (const id of queue.new)
      expect(
        (await call("review/rate", { cardId: id, rating: "easy", durationMs: 30 })).status,
      ).toBe(200);
    const generated = await (await call("articles", { tier: "medium", genre: "story" })).json();
    const article = await (await call(`articles/${generated.articleId}`)).json();
    expect(article.body.length).toBeGreaterThan(1);
    expect(deps.repos.events.listByType(deps.userId, "import")).toHaveLength(1);
    expect(deps.repos.events.listByType(deps.userId, "review")).toHaveLength(3);
    expect(deps.repos.events.listByType(deps.userId, "article_generate")).toHaveLength(1);
    expect(deps.repos.reviewLogs.listAll(deps.userId)).toHaveLength(3);
    expect((await call("cards?offset=-1")).status).toBe(400);
    expect((await call("settings", { unknown: 4 })).status).toBe(400);
    expect((await call("settings", { dailyNewLimit: 201 })).status).toBe(400);
  });
  it("serves audio ranges, enforces size before parsing and deduplicates reading sessions", async () => {
    const { deps } = initialize();
    const articleId = seedArticle(deps);
    const source = await (await call(`articles/${articleId}/audio`, {})).json();
    const response = await call(source.url.replace("/api/", ""), undefined, {
      range: "bytes=0-99",
    });
    expect(response.status).toBe(206);
    expect((await response.arrayBuffer()).byteLength).toBe(100);
    const openId = await (
      await call(`articles/${articleId}/events`, { type: "article_open" })
    ).json();
    for (let i = 0; i < 2; i++)
      await call(`articles/${articleId}/events`, {
        type: "article_complete",
        openId,
        via: "button",
      });
    expect(deps.repos.events.listByType(deps.userId, "article_complete")).toHaveLength(1);
    const invalid = await call("import/cards", {}, { "content-length": String(65 * 1024 * 1024) });
    expect(invalid.status).toBe(413);
  });
});
