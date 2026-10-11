import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { pickGenre, renderArticlePrompt } from "./prompt";

const template = readFileSync(
  new URL("../../../../prompts/article.v1.md", import.meta.url),
  "utf8",
);

describe("renderArticlePrompt", () => {
  const vars = {
    genre: "story" as const,
    minWords: 300,
    maxWords: 400,
    level: "B1",
    targets: ["abandon", "ability"],
  };

  it("replaces all placeholders and drops empty avoid line", () => {
    const msgs = renderArticlePrompt(template, vars);
    expect(msgs.map((m) => m.role)).toEqual(["system", "user"]);
    const all = msgs.map((m) => m.content).join("\n");
    expect(all).not.toMatch(/\{\w+\}/);
    expect(all).toContain("abandon, ability");
    expect(all).toContain("300-400");
    expect(all).not.toContain("Do NOT use");
  });

  it("includes avoid words when present", () => {
    const all = renderArticlePrompt(template, { ...vars, avoidWords: ["zeal", "quirk"] })
      .map((m) => m.content)
      .join("\n");
    expect(all).toContain("Do NOT use these words: zeal, quirk");
  });
});

describe("pickGenre", () => {
  it("picks least recently used genre", () => {
    expect(pickGenre([])).toBe("story");
    expect(pickGenre(["story"])).toBe("dialogue");
    expect(pickGenre(["dialogue", "story", "monologue"])).toBe("monologue");
    expect(pickGenre(["monologue", "story"])).toBe("dialogue");
  });
});
