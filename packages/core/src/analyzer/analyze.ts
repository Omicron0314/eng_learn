import {
  CONTRACTION_SUFFIXES,
  FUNCTION_WORDS,
  IRREGULAR_CONTRACTIONS,
} from "../lexicon/function-words";
import type { Lemmatizer } from "../lexicon/lemmatizer";
import { normalizeApostrophes, type Token, tokenize } from "../lexicon/tokenize";

export interface KnownSet {
  has(lemma: string): boolean;
}

export type AnalysisResult = {
  tokenCount: number;
  coverage: number;
  typeCoverage: number;
  unknownRatio: number;
  unknownLemmas: { lemma: string; count: number }[];
  targetOccurrences: Record<string, number>;
  names: string[];
};

export type AnalyzeOptions = {
  lemmatizer: Lemmatizer;
  targets?: string[];
  names?: string[];
};

/** Split a contraction or possessive into its stem and the implied extra word. */
export function splitContraction(word: string): { stem: string; extra?: string } {
  const w = normalizeApostrophes(word.toLowerCase());
  const irregular = IRREGULAR_CONTRACTIONS[w];
  if (irregular) return { stem: irregular, extra: "not" };
  for (const [suffix, full] of Object.entries(CONTRACTION_SUFFIXES)) {
    if (w.endsWith(suffix) && w.length > suffix.length) {
      return { stem: w.slice(0, -suffix.length), extra: full };
    }
  }
  return { stem: w };
}

/** Candidate lemmas for a word token (lowercased, contractions handled). */
export function lemmaCandidates(surface: string, lemmatizer: Lemmatizer): string[] {
  const { stem } = splitContraction(surface);
  return lemmatizer.lemmas(stem);
}

export function primaryLemma(surface: string, lemmatizer: Lemmatizer): string {
  return lemmaCandidates(surface, lemmatizer)[0] ?? surface.toLowerCase();
}

function isNameToken(token: Token, names: Set<string>): boolean {
  const lower = normalizeApostrophes(token.surface.toLowerCase());
  const base = splitContraction(lower).stem;
  if (names.has(lower) || names.has(base)) return true;
  if (base === "i") return false;
  return !token.isSentenceStart && /^[A-Z]/.test(token.surface);
}

export function analyzeText(text: string, known: KnownSet, opts: AnalyzeOptions): AnalysisResult {
  const targets = (opts.targets ?? []).map((t) => t.toLowerCase());
  const targetSet = new Set(targets);
  const names = new Set((opts.names ?? []).map((n) => normalizeApostrophes(n.toLowerCase())));
  const targetOccurrences: Record<string, number> = Object.fromEntries(targets.map((t) => [t, 0]));

  const isKnownLemma = (lemma: string) =>
    FUNCTION_WORDS.has(lemma) || targetSet.has(lemma) || known.has(lemma);

  let tokenCount = 0;
  let knownCount = 0;
  const unknownCounts = new Map<string, number>();
  const typeKnown = new Map<string, boolean>();
  const skippedNames = new Set<string>();

  for (const token of tokenize(text)) {
    if (token.kind !== "word") continue;
    if (isNameToken(token, names)) {
      skippedNames.add(token.surface.replace(/['’]s$/i, ""));
      continue;
    }
    tokenCount++;
    const candidates = lemmaCandidates(token.surface, opts.lemmatizer);
    const primary = candidates[0] ?? token.surface.toLowerCase();

    for (const t of targets)
      if (candidates.includes(t)) targetOccurrences[t] = (targetOccurrences[t] ?? 0) + 1;

    let isKnown = candidates.some(isKnownLemma);
    if (!isKnown && primary.includes("-")) {
      isKnown = primary
        .split("-")
        .filter(Boolean)
        .every((part) => opts.lemmatizer.lemmas(part).some(isKnownLemma));
    }

    if (isKnown) knownCount++;
    else unknownCounts.set(primary, (unknownCounts.get(primary) ?? 0) + 1);
    typeKnown.set(primary, (typeKnown.get(primary) ?? false) || isKnown);
  }

  const coverage = tokenCount === 0 ? 1 : knownCount / tokenCount;
  const types = [...typeKnown.values()];
  const typeCoverage = types.length === 0 ? 1 : types.filter(Boolean).length / types.length;
  const unknownLemmas = [...unknownCounts.entries()]
    .map(([lemma, count]) => ({ lemma, count }))
    .sort((a, b) => b.count - a.count || a.lemma.localeCompare(b.lemma));

  return {
    tokenCount,
    coverage,
    typeCoverage,
    unknownRatio: 1 - coverage,
    unknownLemmas,
    targetOccurrences,
    names: [...skippedNames],
  };
}

export function setKnown(words: Iterable<string>): KnownSet {
  const s = new Set([...words].map((w) => w.toLowerCase()));
  return { has: (lemma) => s.has(lemma) };
}

export function countWords(text: string): number {
  return tokenize(text).filter((t) => t.kind === "word").length;
}
