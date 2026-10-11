/** RFC 4180 CSV parser. Returns rows with their 1-based starting line numbers. */
export function parseCsv(text: string): { cells: string[]; line: number }[] {
  const src = text.replace(/^\uFEFF/, "");
  const rows: { cells: string[]; line: number }[] = [];
  let cells: string[] = [];
  let cell = "";
  let inQuotes = false;
  let line = 1;
  let rowLine = 1;

  for (let i = 0; i < src.length; i++) {
    const ch = src[i];
    if (inQuotes) {
      if (ch === '"') {
        if (src[i + 1] === '"') {
          cell += '"';
          i++;
        } else {
          inQuotes = false;
        }
      } else {
        if (ch === "\n") line++;
        cell += ch;
      }
      continue;
    }
    if (ch === '"' && cell === "") {
      inQuotes = true;
    } else if (ch === ",") {
      cells.push(cell);
      cell = "";
    } else if (ch === "\n" || ch === "\r") {
      if (ch === "\r" && src[i + 1] === "\n") i++;
      cells.push(cell);
      rows.push({ cells, line: rowLine });
      cells = [];
      cell = "";
      line++;
      rowLine = line;
    } else {
      cell += ch;
    }
  }
  if (cell !== "" || cells.length > 0) {
    cells.push(cell);
    rows.push({ cells, line: rowLine });
  }
  return rows.filter((r) => r.cells.some((c) => c.trim() !== ""));
}

export type CardImportRow = { word: string; meaning?: string; example?: string; line: number };

export type CardCsvResult = {
  rows: CardImportRow[];
  errors: { line: number; reason: string }[];
  duplicates: number;
};

export function normalizeHeadword(word: string): string {
  return word.trim().replace(/\s+/g, " ").toLowerCase();
}

export function parseCardCsv(text: string): CardCsvResult {
  const parsed = parseCsv(text);
  const rows: CardImportRow[] = [];
  const errors: { line: number; reason: string }[] = [];
  const seen = new Set<string>();
  let duplicates = 0;

  parsed.forEach((r, i) => {
    const [word = "", meaning = "", example = ""] = r.cells.map((c) => c.trim());
    if (i === 0 && word.toLowerCase() === "word") return;
    if (!word) {
      errors.push({ line: r.line, reason: "empty word" });
      return;
    }
    const key = normalizeHeadword(word);
    if (seen.has(key)) {
      duplicates++;
      return;
    }
    seen.add(key);
    rows.push({
      word,
      line: r.line,
      ...(meaning ? { meaning } : {}),
      ...(example ? { example } : {}),
    });
  });
  return { rows, errors, duplicates };
}

/** Throws if bytes are not valid UTF-8. */
export function decodeUtf8(bytes: Uint8Array): string {
  return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
}
