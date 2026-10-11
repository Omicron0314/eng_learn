export type WordListEntry = {
  headword: string;
  variants: string[];
  pos?: string;
  meaning?: string;
  isPhrase: boolean;
  raw: string;
};

const POS_RE =
  /^(?:n|v|vt|vi|adj|a|adv|ad|prep|conj|pron|art|num|int|interj|aux|abbr|modal\s*v)\.?(?:\s*(?:&|\/|,)\s*(?:n|v|vt|vi|adj|a|adv|ad|prep|conj|pron|art|num|int|interj|aux|abbr)\.?)*$/i;

const LETTER_PHRASE = /^[A-Za-z][A-Za-z'’.\- ]*$/;

function stripPhonetics(s: string): string {
  // Protect an attached variant headword before pairing phonetic delimiters.
  // This also preserves the existing support for IPA attached without whitespace.
  const variants = s.match(/^[A-Za-z'’.-]+(?:\/[A-Za-z'’.-]+)+(?=\s|$)/)?.[0];
  const rest = variants ? s.slice(variants.length) : s;
  return (variants ?? "") + rest.replace(/\/[^/]*\//g, " ").replace(/\[[^\]]*\]/g, " ");
}

/**
 * Parse one line of a user word list (TXT or CSV).
 * Returns null for blank lines and headings such as "A" or "Unit 1".
 */
export function parseWordListLine(line: string): WordListEntry | null {
  const raw = line.replace(/^\uFEFF/, "").trim();
  if (!raw) return null;
  if (/^#/.test(raw)) return null;

  let rest = raw
    .replace(/^\d+\s*[.、)）:]\s*/, "")
    .replace(/^[-*•]\s+/, "")
    .trim();

  // CSV / TSV: first cell is the word, second (optional) is the meaning
  let meaningFromColumns: string | undefined;
  const sep = rest.includes("\t") ? "\t" : /^[A-Za-z][A-Za-z'’ .\-()/]*,/.test(rest) ? "," : null;
  if (sep) {
    const [first = "", ...others] = rest.split(sep);
    rest = first.trim();
    const m = others.join(sep).trim().replace(/^"|"$/g, "");
    if (m) meaningFromColumns = m;
  }

  rest = stripPhonetics(rest).replace(/\s+/g, " ").trim();

  // Leading English part ends where CJK or a part-of-speech tag starts
  const cjkIdx = rest.search(/[\u3000-\u9fff\uff00-\uffef]/);
  let english = cjkIdx >= 0 ? rest.slice(0, cjkIdx) : rest;
  let meaning = cjkIdx >= 0 ? rest.slice(cjkIdx).trim() : undefined;

  const words = english.trim().split(" ");
  let pos: string | undefined;
  const posIdx = words.findIndex((w, i) => i > 0 && POS_RE.test(w));
  if (posIdx > 0) {
    const tail = words.slice(posIdx);
    english = words.slice(0, posIdx).join(" ");
    const posWords: string[] = [];
    while (tail.length && POS_RE.test(tail[0] ?? "")) posWords.push(tail.shift() ?? "");
    pos = posWords.join(" ");
    const tailText = tail.join(" ").trim();
    if (tailText) meaning = meaning ? `${tailText} ${meaning}` : tailText;
  }
  english = english.trim();
  if (meaningFromColumns)
    meaning = meaning ? `${meaning} ${meaningFromColumns}` : meaningFromColumns;

  // Variants: "a (an)", "colour (color)", "color/colour"
  const variants: string[] = [];
  english = english.replace(/\(([^)]*)\)/g, (_, inner: string) => {
    for (const v of inner.split(/[,/]/)) if (v.trim()) variants.push(v.trim());
    return " ";
  });
  english = english.replace(/\s+/g, " ").trim();
  if (/^[A-Za-z'’.-]+\/[A-Za-z'’./-]+$/.test(english)) {
    const [head = "", ...vs] = english.split("/");
    english = head;
    variants.push(...vs);
  }

  if (!english || !LETTER_PHRASE.test(english)) return null;
  // Headings: a single capital letter, or "Unit 3"-style lines
  if (/^[A-Z]$/.test(english) && !meaning && !pos) return null;
  if (/^(unit|lesson|chapter|list|part)\b/i.test(english) && /\d/.test(raw)) return null;

  const headword = english.replace(/[’]/g, "'").toLowerCase();
  const cleanVariants = [
    ...new Set(variants.map((v) => v.replace(/[’]/g, "'").toLowerCase())),
  ].filter((v) => v !== headword && LETTER_PHRASE.test(v));

  return {
    headword,
    variants: cleanVariants,
    isPhrase: headword.includes(" "),
    raw,
    ...(pos ? { pos } : {}),
    ...(meaning ? { meaning: meaning.trim() } : {}),
  };
}

export type ParsedWordList = {
  entries: WordListEntry[];
  skippedLines: { line: number; raw: string }[];
  duplicates: number;
};

export function parseWordList(text: string): ParsedWordList {
  const entries: WordListEntry[] = [];
  const skippedLines: { line: number; raw: string }[] = [];
  const seen = new Set<string>();
  let duplicates = 0;
  text
    .replace(/^\uFEFF/, "")
    .split(/\r?\n/)
    .forEach((line, i) => {
      if (!line.trim()) return;
      if (i === 0 && /^word\s*(,|\t|$)/i.test(line.trim())) return;
      const entry = parseWordListLine(line);
      if (!entry) {
        if (
          !/^[A-Z]$/.test(line.trim()) &&
          !/^(unit|lesson|chapter|list|part)\b/i.test(line.trim())
        )
          skippedLines.push({ line: i + 1, raw: line.trim() });
        return;
      }
      if (seen.has(entry.headword)) {
        duplicates++;
        return;
      }
      seen.add(entry.headword);
      entries.push(entry);
    });
  return { entries, skippedLines, duplicates };
}
