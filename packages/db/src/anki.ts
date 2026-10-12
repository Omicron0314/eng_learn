import Database from "better-sqlite3";

export type AnkiNote = { fields: string[]; tags: string[]; fieldNames: string[] };

/**
 * Reads notes from a legacy collection (anki2/anki21) together with the field
 * names declared by each note's model. Collections without a `col` table (or a
 * malformed `models` blob) simply yield notes with empty field names.
 */
export function readAnkiNoteFields(bytes: Uint8Array): AnkiNote[] {
  const db = new Database(Buffer.from(bytes), { readonly: true });
  try {
    const modelFields = new Map<number, string[]>();
    try {
      const col = db.prepare("SELECT models FROM col").get() as { models: string } | undefined;
      if (col) {
        const models = JSON.parse(col.models) as Record<string, { flds?: { name: string }[] }>;
        for (const [mid, model] of Object.entries(models))
          modelFields.set(
            Number(mid),
            (model.flds ?? []).map((f) => f.name),
          );
      }
    } catch {
      // Tolerate collections without model metadata; field-name mapping stays empty.
    }
    const rows = db.prepare("SELECT mid, flds, tags FROM notes ORDER BY id").all() as {
      mid: number;
      flds: string;
      tags: string;
    }[];
    return rows.map((row) => ({
      fields: row.flds.split("\x1f"),
      tags: row.tags.trim().split(/\s+/).filter(Boolean),
      fieldNames: modelFields.get(row.mid) ?? [],
    }));
  } finally {
    db.close();
  }
}
