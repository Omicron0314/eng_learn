import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const cache = new Map<string, string>();

function findPromptsDir(): string {
  if (process.env.PROMPTS_DIR) return process.env.PROMPTS_DIR;
  let dir = process.cwd();
  for (let i = 0; i < 6; i++) {
    const candidate = path.join(dir, "prompts");
    if (existsSync(path.join(candidate, "article.v1.md"))) return candidate;
    dir = path.dirname(dir);
  }
  throw new Error("prompts directory not found; set PROMPTS_DIR");
}

/** Load a versioned prompt template, e.g. loadPrompt("article.v1"). */
export function loadPrompt(name: string): string {
  const hit = cache.get(name);
  if (hit) return hit;
  const text = readFileSync(path.join(findPromptsDir(), `${name}.md`), "utf8");
  cache.set(name, text);
  return text;
}
