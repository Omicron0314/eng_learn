import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

export function violations(file: string, source: string) {
  const errors: string[] = [];
  const core = file.replaceAll("\\", "/").includes("packages/core/") && !file.endsWith(".test.ts");
  if (core) {
    if (
      /(?:from\s*|import\s*\(|require\s*\()\s*["'](?:node:)?(?:fs(?:\/promises)?|net|http|https|child_process|tls|dgram|dns|better-sqlite3|@eng-learn\/(?:db|providers))(?:["'/])/.test(
        source,
      )
    )
      errors.push("core IO import");
    if (/\b(?:fetch|Date\.now|Math\.random)\s*\(/.test(source))
      errors.push("core nondeterministic IO");
  }
  const modelPattern = new RegExp(
    `\\b(${["gpt", "claude", "gemini", "deepseek", "qwen", "glm"].join("|")})-[\\w.]+`,
    "i",
  );
  if (modelPattern.test(source)) errors.push("hardcoded model name");
  return errors;
}
export function checkBoundaries(root: string) {
  const errors: string[] = [];
  function walk(dir: string) {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (
        [
          "node_modules",
          ".git",
          ".next",
          "data",
          "docs",
          "results",
          "test-results",
          "playwright-report",
        ].includes(entry.name)
      )
        continue;
      const file = path.join(dir, entry.name);
      if (entry.isDirectory()) walk(file);
      else if (/\.(?:ts|tsx|mjs|md)$/.test(entry.name))
        for (const error of violations(path.relative(root, file), readFileSync(file, "utf8")))
          errors.push(`${path.relative(root, file)}: ${error}`);
    }
  }
  walk(root);
  return errors;
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const errors = checkBoundaries(process.argv[2] ?? process.cwd());
  if (errors.length) {
    console.error(errors.join("\n"));
    process.exitCode = 1;
  } else console.log("Architecture boundaries passed");
}
