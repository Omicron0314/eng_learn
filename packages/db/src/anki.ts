import Database from "better-sqlite3";

/** Read an external Anki collection without applying app migrations or importing scheduling. */
export function readAnkiNoteFields(bytes: Uint8Array): { fields: string[]; tags: string[] }[] {
  const db = new Database(Buffer.from(bytes), { readonly: true });
  try {
    const rows = db.prepare("SELECT flds, tags FROM notes ORDER BY id").all() as {
      flds: string;
      tags: string;
    }[];
    return rows.map((row) => ({
      fields: row.flds.split("\x1f"),
      tags: row.tags.trim().split(/\s+/).filter(Boolean),
    }));
  } finally {
    db.close();
  }
}
