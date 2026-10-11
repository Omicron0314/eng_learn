import { describe, expect, it } from "vitest";
import { violations } from "./check-boundaries";

describe("architecture boundaries", () => {
  it("AC5: rejects IO imports, nondeterminism and configured model literals", () => {
    expect(violations("packages/core/src/bad.ts", 'import fs from "node:fs"')).toContain(
      "core IO import",
    );
    expect(violations("packages/core/src/bad.ts", "Math.random()")).toContain(
      "core nondeterministic IO",
    );
    expect(violations("prompts/bad.md", `model: ${["gpt", "example"].join("-")}`)).toContain(
      "hardcoded model name",
    );
    expect(violations("packages/core/src/good.ts", "export const n = 1")).toEqual([]);
  });
});
