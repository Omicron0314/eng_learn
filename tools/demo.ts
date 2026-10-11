import { spawn } from "node:child_process";

console.log("DEMO MODE: fake LLM/TTS, no paid calls. Text is demonstration only.");
const child = spawn("pnpm", ["--filter", "@eng-learn/web", "dev", ...process.argv.slice(2)], {
  stdio: "inherit",
  env: {
    ...process.env,
    LLM_PROVIDER: "fake",
    TTS_PROVIDER: process.env.TTS_PROVIDER === "none" ? "none" : "fake",
    DATABASE_PATH: process.env.DATABASE_PATH || "../../data/demo.sqlite",
    BLOB_DIR: process.env.BLOB_DIR || "../../data/demo-blobs",
  },
});
child.on("exit", (code) => {
  process.exitCode = code ?? 1;
});
for (const signal of ["SIGINT", "SIGTERM"] as const) process.on(signal, () => child.kill(signal));
