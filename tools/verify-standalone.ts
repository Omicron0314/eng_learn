import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtempSync, rmSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import path from "node:path";

// Run after pnpm build. This regression starts the real standalone deployment,
// never Docker or a paid provider, and uses only temporary local data.
const root = process.cwd();
const dir = mkdtempSync(path.join(tmpdir(), "standalone-origin-"));
async function availablePort() {
  const server = createServer();
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  assert(address && typeof address !== "string");
  const port = address.port;
  await new Promise<void>((resolve, reject) =>
    server.close((error) => (error ? reject(error) : resolve())),
  );
  return port;
}
async function verify(publicOrigin?: string) {
  const port = await availablePort();
  const baseURL = `http://localhost:${port}`;
  const origin = publicOrigin ?? baseURL;
  const child = spawn(
    process.execPath,
    [path.join(root, "apps/web/.next/standalone/apps/web/server.js")],
    {
      cwd: root,
      env: {
        PATH: process.env.PATH ?? "",
        NODE_ENV: "production",
        HOSTNAME: "0.0.0.0",
        PORT: String(port),
        LLM_PROVIDER: "fake",
        TTS_PROVIDER: "none",
        APP_PASSWORD: "test-password",
        APP_ORIGIN: publicOrigin ?? "",
        DATABASE_PATH: ":memory:",
        BLOB_DIR: dir,
        PROMPTS_DIR: path.join(root, "prompts"),
      },
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  let output = "";
  child.stdout.on("data", (chunk) => {
    output += String(chunk);
  });
  child.stderr.on("data", (chunk) => {
    output += String(chunk);
  });
  try {
    let ready = false;
    for (let i = 0; i < 200; i++) {
      if (child.exitCode !== null) throw new Error(`Standalone exited: ${output}`);
      try {
        if ((await fetch(`${baseURL}/api/today`)).status === 401) {
          ready = true;
          break;
        }
      } catch {
        /* Poll only the local listener while startup finishes. */
      }
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    assert(ready, output);
    const headers = { origin, "content-type": "application/json" };
    const login = await fetch(`${baseURL}/api/auth`, {
      method: "POST",
      headers,
      body: JSON.stringify({ password: "test-password" }),
    });
    assert.equal(login.status, 200, await login.text());
    const cookie = login.headers.get("set-cookie");
    assert(cookie);
    if (publicOrigin) assert.match(cookie, /Secure/);
    const writes = { ...headers, cookie: cookie.split(";")[0] ?? "" };
    const settings = await fetch(`${baseURL}/api/settings`, {
      method: "POST",
      headers: writes,
      body: JSON.stringify({ dailyNewLimit: 5 }),
    });
    assert.equal(settings.status, 200, await settings.text());
    const forbidden = await fetch(`${baseURL}/api/settings`, {
      method: "POST",
      headers: { ...writes, origin: "https://elsewhere.test" },
      body: "{}",
    });
    assert.equal(forbidden.status, 403);
    const form = new FormData();
    form.set("deck", "transport-test");
    form.set("file", new File([new Uint8Array(21 * 1024 * 1024)], "allowed-size.apkg"));
    const upload = await fetch(`${baseURL}/api/import/cards`, {
      method: "POST",
      headers: { origin, cookie: writes.cookie },
      body: form,
    });
    assert.equal(upload.status, 400, await upload.text()); // Invalid ZIP, not a size rejection.
    console.log(
      `PASS HOSTNAME=0.0.0.0 ${publicOrigin ? "HTTPS APP_ORIGIN + Secure cookie" : "direct browser Origin"}: login/write=200, cross-site=403, >20 MiB apkg reaches format validation`,
    );
  } finally {
    if (child.exitCode === null && child.signalCode === null) {
      const stopped = once(child, "exit");
      child.kill("SIGTERM");
      await stopped;
    }
  }
}
try {
  await verify();
  await verify("https://learning.example.invalid");
} finally {
  rmSync(dir, { recursive: true, force: true });
}
