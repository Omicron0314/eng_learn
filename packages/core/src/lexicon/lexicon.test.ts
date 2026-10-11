import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { decodeUtf8, parseCardCsv } from "./csv";
import { parseWordList, parseWordListLine } from "./wordlist";

const cases = JSON.parse(
  readFileSync(new URL("../../../../fixtures/wordlist/cases.json", import.meta.url), "utf8"),
) as { rule: string; input: string; expected: Record<string, unknown> | null }[];

describe("parseWordListLine (S4 rules)", () => {
  for (const c of cases) {
    it(c.rule, () => {
      const r = parseWordListLine(c.input);
      if (c.expected === null) {
        expect(r).toBeNull();
      } else {
        expect(r).toMatchObject(c.expected);
      }
    });
  }
});

describe("parseWordList", () => {
  it("dedupes and reports unparsed lines", () => {
    const r = parseWordList("A\nabandon v. 放弃\nAbandon\n%%%\nability\n");
    expect(r.entries.map((e) => e.headword)).toEqual(["abandon", "ability"]);
    expect(r.duplicates).toBe(1);
    expect(r.skippedLines).toEqual([{ line: 4, raw: "%%%" }]);
  });
});

describe("parseCardCsv", () => {
  it("parses quoted fields, header and BOM", () => {
    const csv =
      '\uFEFFword,meaning,example\napple,"苹果, 一种水果","He said ""hi""\nthen left"\nbanana\n';
    const r = parseCardCsv(csv);
    expect(r.rows).toEqual([
      { word: "apple", meaning: "苹果, 一种水果", example: 'He said "hi"\nthen left', line: 2 },
      { word: "banana", line: 4 },
    ]);
  });

  it("works without header and reports duplicates and empty words", () => {
    const r = parseCardCsv("Apple,苹果\napple,苹果\n,无\npear\r\n");
    expect(r.rows.map((x) => x.word)).toEqual(["Apple", "pear"]);
    expect(r.duplicates).toBe(1);
    expect(r.errors).toEqual([{ line: 3, reason: "empty word" }]);
  });

  it("rejects non-UTF-8 input", () => {
    expect(() => decodeUtf8(new Uint8Array([0xb0, 0xa1, 0xff]))).toThrow();
  });
});
