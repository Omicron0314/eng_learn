/** Deck name defaults to the uploaded file name, without its extension. */
export function deckNameFromFile(fileName: string): string {
  const base = fileName.split(/[\\/]/).pop() ?? "";
  const name = base
    .replace(/\.[^.]*$/, "")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 1000)
    .trim();
  return name || "我的词书";
}
