import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { defaultLemmatizer } from "./lemmatizer";

const probes = JSON.parse(
  readFileSync(new URL("../../../../fixtures/lemma/probes.json", import.meta.url), "utf8"),
) as { category: string; surface: string; lemma: string }[];
describe("S1 bounded synthetic measurement", () => {
  it("AC2/AC4: 36 probes reproduce measured first-candidate 33/36 and any-candidate 36/36", () => {
    const lemmatizer = defaultLemmatizer();
    expect(probes.filter((p) => lemmatizer.lemmas(p.surface)[0] === p.lemma)).toHaveLength(33);
    expect(probes.filter((p) => lemmatizer.lemmas(p.surface).includes(p.lemma))).toHaveLength(36);
    expect(probes.filter((p) => p.surface === p.lemma)).toHaveLength(8);
  });
});
