export type TokenKind = "word" | "number" | "punct";

export type Token = {
  surface: string;
  start: number;
  end: number;
  kind: TokenKind;
  sentenceIndex: number;
  isSentenceStart: boolean;
};

const TOKEN_RE =
  /(\$?\d[\d,.:]*[A-Za-z%]*)|([A-Za-z]+(?:['’][A-Za-z]+)*(?:-[A-Za-z]+(?:['’][A-Za-z]+)*)*)|([^\s])/g;

const ABBREVIATIONS = new Set(["mr", "mrs", "ms", "dr", "st", "prof", "jr", "sr", "vs", "etc"]);
const SENTENCE_END = new Set([".", "?", "!", "…"]);
const CLOSING = new Set(['"', "”", "'", "’", ")", "]"]);
const OPENING_QUOTE = new Set(["“"]);

export function normalizeApostrophes(s: string): string {
  return s.replace(/[’‘]/g, "'");
}

export function tokenize(text: string): Token[] {
  const tokens: Token[] = [];
  let sentenceIndex = 0;
  let awaitingStart = true;
  let pendingBreak = false;
  let inQuote = false;
  let lastEnd = 0;

  for (const m of text.matchAll(TOKEN_RE)) {
    const start = m.index ?? 0;
    const surface = m[0];
    const end = start + surface.length;
    const kind: TokenKind = m[1] ? "number" : m[2] ? "word" : "punct";

    const gap = text.slice(lastEnd, start);
    if (pendingBreak && !(kind === "punct" && CLOSING.has(surface))) {
      sentenceIndex++;
      awaitingStart = true;
      pendingBreak = false;
    }
    if (/\n/.test(gap)) {
      inQuote = false;
      if (tokens.length > 0 && !awaitingStart) {
        sentenceIndex++;
        awaitingStart = true;
      }
    }

    let isSentenceStart = false;
    if (kind === "word") {
      isSentenceStart = awaitingStart;
      awaitingStart = false;
    } else if (kind === "number") {
      awaitingStart = false;
    } else if (surface === '"') {
      inQuote = !inQuote;
      if (inQuote) awaitingStart = true;
    } else if (surface === ":" || OPENING_QUOTE.has(surface)) {
      awaitingStart = true;
    } else if (SENTENCE_END.has(surface)) {
      const prev = tokens[tokens.length - 1];
      const isAbbrev =
        surface === "." && prev?.kind === "word" && ABBREVIATIONS.has(prev.surface.toLowerCase());
      if (!isAbbrev) pendingBreak = true;
    }

    tokens.push({ surface, start, end, kind, sentenceIndex, isSentenceStart });
    lastEnd = end;
  }
  return tokens;
}

/** Text of the sentence containing the character offset. */
export function sentenceAt(text: string, offset: number): string {
  const tokens = tokenize(text);
  if (tokens.length === 0) return "";
  let target = tokens.find((t) => offset >= t.start && offset < t.end);
  if (!target) target = tokens.filter((t) => t.start <= offset).at(-1) ?? tokens[0];
  if (!target) return "";
  const idx = target.sentenceIndex;
  const inSentence = tokens.filter((t) => t.sentenceIndex === idx);
  const first = inSentence[0];
  const last = inSentence.at(-1);
  if (!first || !last) return "";
  return text.slice(first.start, last.end).trim();
}

/** Split text into sentences using the same rules as `tokenize`. */
export function splitSentences(text: string): string[] {
  const tokens = tokenize(text);
  const out: string[] = [];
  let current: Token[] = [];
  for (const t of tokens) {
    if (current.length > 0 && current[0]?.sentenceIndex !== t.sentenceIndex) {
      out.push(text.slice(current[0]?.start, current.at(-1)?.end).trim());
      current = [];
    }
    current.push(t);
  }
  if (current.length > 0) out.push(text.slice(current[0]?.start, current.at(-1)?.end).trim());
  return out;
}
