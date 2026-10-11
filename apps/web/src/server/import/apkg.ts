import { readAnkiNoteFields } from "@eng-learn/db";
import { AppError } from "@eng-learn/providers";
import { unzipSync } from "fflate";
import { APKG_IMPORT_MAX_BYTES } from "../../import-limits";

export function detectApkgFormat(entries: string[]) {
  // New exports contain a misleading compatibility stub; never import it.
  if (entries.includes("collection.anki21b")) return "anki21b";
  if (entries.includes("collection.anki21")) return "anki21";
  if (entries.includes("collection.anki2")) return "legacy-anki2";
  return "unknown";
}

export function stripAnkiHtml(field: string): string {
  return field
    .replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, "")
    .replace(/<style\b[^>]*>[\s\S]*?<\/style>/gi, "")
    .replace(/\[sound:[^\]]*\]/gi, "")
    .replace(/\{\{c\d+::(.*?)(?:::[^{}]*?)?\}\}/g, "$1")
    .replace(/<br\s*\/?\s*>|<\/(?:div|p)>/gi, "\n")
    .replace(/<[^>]*>/g, "")
    .replace(/&#(x[\da-f]+|\d+);/gi, (_, value: string) => {
      const code =
        value[0]?.toLowerCase() === "x" ? Number.parseInt(value.slice(1), 16) : Number(value);
      return code > 0 && code <= 0x10ffff ? String.fromCodePoint(code) : "�";
    })
    .replace(
      /&(amp|lt|gt|quot|apos|nbsp);/gi,
      (_, name: string) =>
        ({ amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: " " })[name.toLowerCase()] ?? "",
    )
    .trim();
}

export async function readApkgNotes(bytes: Uint8Array) {
  if (bytes.length > APKG_IMPORT_MAX_BYTES)
    throw new AppError("PAYLOAD_TOO_LARGE", "apkg 文件超过 64 MiB", 413);
  const entries: string[] = [];
  let files: Record<string, Uint8Array>;
  try {
    files = unzipSync(bytes, {
      filter: (file) => {
        entries.push(file.name);
        const collection = /^collection\.anki(?:2|21|21b)$/.test(file.name);
        if (collection && file.originalSize > 100 * 1024 * 1024)
          throw new AppError("PAYLOAD_TOO_LARGE", "解压后的词库超过 100 MB", 413);
        // Inspect only the presence of compressed new-format collections.
        return collection && file.name !== "collection.anki21b";
      },
    });
  } catch (error) {
    if (error instanceof AppError) throw error;
    throw new AppError("VALIDATION_FAILED", "无法读取 apkg 压缩包", 400);
  }
  const format = detectApkgFormat(entries);
  if (format === "anki21b")
    throw new AppError("APKG_UNSUPPORTED_FORMAT", "请勾选“支持旧版 Anki”重新导出，或改用 CSV", 400);
  const file = files[format === "anki21" ? "collection.anki21" : "collection.anki2"];
  if (!file) throw new AppError("APKG_UNSUPPORTED_FORMAT", "压缩包内没有兼容格式词库", 400);
  try {
    return readAnkiNoteFields(file).map((note) => ({
      ...note,
      fields: note.fields.map(stripAnkiHtml),
    }));
  } catch {
    throw new AppError("VALIDATION_FAILED", "无法读取 Anki 笔记字段", 400);
  }
}
