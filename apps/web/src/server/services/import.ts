import {
  type CardImportRow,
  decodeUtf8,
  newSrsCard,
  normalizeHeadword,
  parseCardCsv,
  parseCsv,
  parseWordList,
  parseWordListLine,
} from "@eng-learn/core";
import type { CardBack, Repos } from "@eng-learn/db";
import { AppError } from "@eng-learn/providers";
import { z } from "zod";
import type { AppDeps } from "../deps";
import {
  APKG_FRONT_FIELDS,
  APKG_MEANING_FIELDS,
  pickApkgField,
  readApkgNotes,
} from "../import/apkg";
import { nonempty, validate } from "./shared";

export function decodeImport(bytes: Uint8Array): string {
  try {
    return decodeUtf8(bytes);
  } catch {
    throw new AppError("VALIDATION_FAILED", "请另存为 UTF-8", 400);
  }
}
function addImportedCard(
  deps: AppDeps,
  repos: Repos,
  row: CardImportRow,
  deck: string,
  now: Date,
): boolean {
  const lexemeId = repos.lexemes.upsert(normalizeHeadword(row.word), "", now.getTime());
  if (repos.cards.getByLexeme(deps.userId, lexemeId)) return false;
  const back: CardBack = {
    meaningSource: "import",
    ...(row.meaning ? { meaning: row.meaning } : {}),
    ...(row.example ? { example: row.example } : {}),
  };
  repos.cards.insert({
    userId: deps.userId,
    lexemeId,
    front: row.front ?? row.word.trim(),
    back,
    deck,
    origin: "import",
    srs: newSrsCard(now),
    now: now.getTime(),
  });
  return true;
}
export async function importCards(
  deps: AppDeps,
  input: { fileName: string; bytes: Uint8Array; deck: string },
) {
  validate(
    z.object({ fileName: nonempty, bytes: z.instanceof(Uint8Array), deck: nonempty }),
    input,
  );
  const source = input.fileName.toLowerCase().endsWith(".apkg") ? "apkg" : "csv";
  if (source === "csv" && !/\.csv$/i.test(input.fileName))
    throw new AppError("VALIDATION_FAILED", "请选择 CSV 或 apkg 文件", 400);
  let parsed: ReturnType<typeof parseCardCsv>;
  if (source === "csv") parsed = parseCardCsv(decodeImport(input.bytes));
  else {
    const notes = await readApkgNotes(input.bytes);
    const rows: CardImportRow[] = [];
    const errors: { line: number; reason: string }[] = [];
    const seen = new Set<string>();
    let duplicates = 0;
    notes.forEach((note, i) => {
      const word = note.fields[0]?.trim() ?? "";
      if (!word) {
        errors.push({ line: i + 1, reason: "empty word" });
        return;
      }
      const key = normalizeHeadword(word);
      if (seen.has(key)) {
        duplicates++;
        return;
      }
      seen.add(key);
      // Field mapping (作者决定): cards use the model's Front/Back fields by name;
      // Chinese decks map 正面 / 释义 / 中文释义. Decks without field names fall
      // back to the first/second field.
      const front = pickApkgField(note.fields, note.fieldNames, APKG_FRONT_FIELDS);
      const meaning =
        pickApkgField(note.fields, note.fieldNames, APKG_MEANING_FIELDS) ??
        note.fields[1]?.trim() ??
        "";
      rows.push({ word, ...(front ? { front } : {}), meaning, line: i + 1 });
    });
    parsed = { rows, errors, duplicates };
  }
  const now = deps.clock();
  return deps.tx((repos) => {
    let added = 0;
    for (const row of parsed.rows) if (addImportedCard(deps, repos, row, input.deck, now)) added++;
    const result = {
      added,
      skipped: parsed.rows.length - added,
      duplicates: parsed.duplicates,
      errors: parsed.errors,
    };
    repos.events.append(
      deps.userId,
      "import",
      { source, fileName: input.fileName, kind: "cards", deck: input.deck, ...result },
      now.getTime(),
    );
    return result;
  });
}
export async function importWordList(
  deps: AppDeps,
  input: {
    fileName: string;
    bytes: Uint8Array;
    name: string;
    kind: "baseline" | "study";
    mergeIfExists?: boolean;
  },
) {
  validate(
    z.object({
      fileName: nonempty,
      bytes: z.instanceof(Uint8Array),
      name: nonempty,
      kind: z.enum(["baseline", "study"]),
      mergeIfExists: z.boolean().optional(),
    }),
    input,
  );
  const text = decodeImport(input.bytes);
  const source = /\.csv$/i.test(input.fileName) ? "csv" : "txt";
  if (!/\.(csv|txt)$/i.test(input.fileName))
    throw new AppError("VALIDATION_FAILED", "请选择 CSV 或 TXT 词表", 400);
  // CSV quoting is handled before feeding normalized headwords to TXT normalization.
  const parsed =
    source === "csv"
      ? (() => {
          const entries: NonNullable<ReturnType<typeof parseWordListLine>>[] = [];
          const skippedLines: { line: number; raw: string }[] = [];
          const seen = new Set<string>();
          let duplicates = 0;
          parseCsv(text).forEach((row, i) => {
            const word = row.cells[0]?.trim() ?? "";
            if (i === 0 && word.toLowerCase() === "word") return;
            const entry = parseWordListLine(word);
            if (!entry) {
              skippedLines.push({ line: row.line, raw: row.cells.join(",") });
              return;
            }
            if (seen.has(entry.headword)) {
              duplicates++;
              return;
            }
            seen.add(entry.headword);
            if (row.cells[1]?.trim()) entry.meaning = row.cells[1].trim();
            entries.push(entry);
          });
          return { entries, skippedLines, duplicates };
        })()
      : parseWordList(text);
  const now = deps.clock();
  return deps.tx((repos) => {
    const existing = repos.wordLists.find(deps.userId, input.name, input.kind);
    if (existing && !input.mergeIfExists)
      throw new AppError("VALIDATION_FAILED", "同名词表已存在，请选择合并", 400);
    const listId =
      existing?.id ??
      repos.wordLists.create({
        userId: deps.userId,
        name: input.name,
        kind: input.kind,
        sourceFile: input.fileName,
        now: now.getTime(),
      });
    let cardsAdded = 0;
    let added = 0;
    let duplicates = parsed.duplicates;
    for (const entry of parsed.entries) {
      for (const lemma of [entry.headword, ...entry.variants]) {
        const lexemeId = repos.lexemes.upsert(lemma, "", now.getTime());
        if (
          repos.wordLists.addItem({
            userId: deps.userId,
            listId,
            lexemeId,
            raw: entry.raw,
            now: now.getTime(),
          })
        )
          added++;
        else duplicates++;
        if (
          input.kind === "study" &&
          addImportedCard(
            deps,
            repos,
            {
              word: lemma,
              ...(entry.meaning === undefined ? {} : { meaning: entry.meaning }),
              line: 0,
            },
            input.name,
            now,
          )
        )
          cardsAdded++;
      }
    }
    repos.events.append(
      deps.userId,
      "import",
      {
        source,
        fileName: input.fileName,
        kind: input.kind,
        listId,
        added,
        skipped: parsed.skippedLines.length,
        duplicates,
      },
      now.getTime(),
    );
    return {
      listId,
      wordsAdded: added,
      parsed: parsed.entries.length,
      skippedLines: parsed.skippedLines,
      duplicates,
      phrases: parsed.entries.filter((e) => e.isPhrase).length,
      cardsAdded,
    };
  });
}
