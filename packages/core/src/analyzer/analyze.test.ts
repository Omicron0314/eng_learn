import { describe, expect, it } from "vitest";
import { createWinkLemmatizer } from "../lexicon/lemmatizer";
import { sentenceAt, splitSentences, tokenize } from "../lexicon/tokenize";
import { analyzeText, setKnown } from "./analyze";

const lemmatizer = createWinkLemmatizer();

describe("tokenize", () => {
  it("keeps contractions and hyphenated words as one token", () => {
    const words = tokenize("I don't think they've left a well-known place.")
      .filter((t) => t.kind === "word")
      .map((t) => t.surface);
    expect(words).toEqual(["I", "don't", "think", "they've", "left", "a", "well-known", "place"]);
  });

  it("marks numbers and punctuation", () => {
    const kinds = tokenize("In 1990s, $5 cost 3rd.").map((t) => `${t.surface}:${t.kind}`);
    expect(kinds).toContain("1990s:number");
    expect(kinds).toContain("$5:number");
    expect(kinds).toContain("3rd:number");
    expect(kinds).toContain(",:punct");
  });

  it("tracks sentence starts", () => {
    const t = tokenize('He stopped. "Wait," Anna said. Mr. Lee came!');
    const starts = t.filter((x) => x.isSentenceStart).map((x) => x.surface);
    expect(starts).toEqual(["He", "Wait", "Mr"]);
    expect(t.find((x) => x.surface === "Lee")?.sentenceIndex).toBe(
      t.find((x) => x.surface === "Mr")?.sentenceIndex,
    );
  });

  it("treats dialogue speaker lines as sentences", () => {
    const t = tokenize("Anna: Hello there.\nBen: Hi!");
    expect(t.filter((x) => x.isSentenceStart).map((x) => x.surface)).toEqual([
      "Anna",
      "Hello",
      "Ben",
      "Hi",
    ]);
  });
});

describe("sentenceAt", () => {
  const text = 'The sun rose. Did you see it? Yes! It was "great." Then we left.';
  it("returns the sentence around an offset", () => {
    expect(sentenceAt(text, 1)).toBe("The sun rose.");
    expect(sentenceAt(text, text.indexOf("see"))).toBe("Did you see it?");
    expect(sentenceAt(text, text.indexOf("great"))).toBe('It was "great."');
    expect(sentenceAt(text, text.length - 2)).toBe("Then we left.");
  });
  it("splits sentences", () => {
    expect(splitSentences(text)).toHaveLength(5);
  });
});

describe("analyzeText", () => {
  it("AC1: contractions count once and are known", () => {
    const r = analyzeText("I don't think they've left.", setKnown(["think", "leave"]), {
      lemmatizer,
    });
    expect(r.tokenCount).toBe(5);
    expect(r.coverage).toBe(1);
    expect(lemmatizer.lemmas("left")).toEqual(expect.arrayContaining(["leave", "left"]));
  });

  it("AC2: skips mid-sentence proper nouns", () => {
    const r = analyzeText("Yesterday Tom met Lucy in Paris.", setKnown(["yesterday", "meet"]), {
      lemmatizer,
    });
    expect(r.names.sort()).toEqual(["Lucy", "Paris", "Tom"]);
    expect(r.tokenCount).toBe(3);
    expect(r.coverage).toBe(1);
  });

  it("AC3: skips LLM-provided names even at sentence start", () => {
    const r = analyzeText("Mia smiled.", setKnown(["smile"]), { lemmatizer, names: ["Mia"] });
    expect(r.names).toEqual(["Mia"]);
    expect(r.tokenCount).toBe(1);
  });

  it("AC4: plurals and tenses map to base forms", () => {
    const r = analyzeText(
      "The children went home. She studies hard. His house is bigger.",
      setKnown(["child", "go", "home", "study", "hard", "house", "big"]),
      { lemmatizer },
    );
    expect(r.unknownLemmas).toEqual([]);
    expect(r.coverage).toBe(1);
  });

  it("AC5: numbers and punctuation are not counted", () => {
    const r = analyzeText("In 2020, 3 cats came.", setKnown(["cat", "come"]), { lemmatizer });
    expect(r.tokenCount).toBe(3);
  });

  it("AC6: target occurrences are counted by lemma", () => {
    const r = analyzeText("I decided. She decides. They will decide.", setKnown([]), {
      lemmatizer,
      targets: ["decide", "absent"],
    });
    expect(r.targetOccurrences).toEqual({ decide: 3, absent: 0 });
  });

  it("AC7: token and type coverage", () => {
    const r = analyzeText("Cats chase mice. Cats chase dogs.", setKnown(["cat", "chase"]), {
      lemmatizer,
    });
    expect(r.tokenCount).toBe(6);
    expect(r.coverage).toBeCloseTo(4 / 6);
    expect(r.typeCoverage).toBeCloseTo(2 / 4);
    expect(r.unknownLemmas.map((u) => u.lemma).sort()).toEqual(["dog", "mouse"]);
  });

  it("hyphenated words are known when all parts are known", () => {
    const r = analyzeText("A well-known song.", setKnown(["well", "know", "song"]), {
      lemmatizer,
    });
    expect(r.coverage).toBe(1);
  });

  it("empty text has full coverage", () => {
    expect(analyzeText("", setKnown([]), { lemmatizer }).coverage).toBe(1);
  });
});
