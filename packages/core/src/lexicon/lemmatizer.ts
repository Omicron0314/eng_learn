/// <reference path="../types/wink-lemmatizer.d.ts" />
import wink from "wink-lemmatizer";

export interface Lemmatizer {
  /** Candidate lemmas for a lowercase word, most likely first. Never empty. */
  lemmas(word: string): string[];
}

/** Plural-looking words whose surface form is the usual dictionary headword. */
const KEEP_SURFACE = new Set([
  "data",
  "media",
  "news",
  "glasses",
  "clothes",
  "goods",
  "means",
  "series",
  "species",
  "politics",
  "physics",
  "mathematics",
  "economics",
  "people",
  "police",
  "trousers",
  "jeans",
  "scissors",
  "thanks",
  "congratulations",
  "headquarters",
  "boss",
]);

export function createWinkLemmatizer(): Lemmatizer {
  return {
    lemmas(word: string): string[] {
      const w = word.toLowerCase();
      if (!/^[a-z]/.test(w)) return [w];
      const verb = wink.verb(w);
      const noun = wink.noun(w);
      const adjective = wink.adjective(w);
      const ordered: string[] = [];
      if (KEEP_SURFACE.has(w)) ordered.push(w);
      for (const c of [verb, noun, adjective]) if (c !== w) ordered.push(c);
      ordered.push(w);
      return [...new Set(ordered)];
    },
  };
}

let shared: Lemmatizer | undefined;

export function defaultLemmatizer(): Lemmatizer {
  shared ??= createWinkLemmatizer();
  return shared;
}
