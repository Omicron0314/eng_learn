import { describe, expect, it } from "vitest";
import { deckNameFromFile } from "./deck-name";

describe("deckNameFromFile", () => {
  it("uses the file name without its extension", () => {
    expect(deckNameFromFile("words.csv")).toBe("words");
    expect(deckNameFromFile("我的词书.apkg")).toBe("我的词书");
    expect(deckNameFromFile("2024 CET-6.txt")).toBe("2024 CET-6");
    expect(deckNameFromFile("my.words.csv")).toBe("my.words");
  });
  it("strips directories and collapses whitespace", () => {
    expect(deckNameFromFile("C:\\Users\\me\\Deck  One.CSV")).toBe("Deck One");
    expect(deckNameFromFile("/tmp/ spaced .apkg")).toBe("spaced");
  });
  it("falls back to a default name when nothing remains", () => {
    expect(deckNameFromFile("")).toBe("我的词书");
    expect(deckNameFromFile(".csv")).toBe("我的词书");
    expect(deckNameFromFile("   ")).toBe("我的词书");
  });
  it("keeps names within the API limit", () => {
    expect(deckNameFromFile(`${"a".repeat(2000)}.txt`)).toHaveLength(1000);
  });
});
