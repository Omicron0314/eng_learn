import type { LLMRequest } from "../llm/types";

const FILLERS = [
  "Lucy walked to school with her friend Tom on a cold morning.",
  "They talked about their plans for the weekend and laughed a lot.",
  "The sky was grey, but the two friends felt happy and warm.",
  "After class, they went to a small shop near the park to buy some bread.",
  "Tom said that he wanted to learn more about the world.",
  "Lucy smiled and told him that every day is a chance to try something new.",
];
function articleBody(targets: string[], genre: string): string {
  const lines: string[] = [];
  const words = targets.length > 0 ? targets : ["friend"];
  words.forEach((w, i) => {
    const filler = FILLERS[i % FILLERS.length] ?? "";
    if (genre.includes("dialogue")) {
      lines.push(`Tom: Do you know the word ${w}? I read the word ${w} in a book today.`);
      lines.push(`Lucy: Yes, I know it. ${filler}`);
    } else {
      lines.push(
        `${filler} Then Lucy thought about the word ${w}. She used ${w} in a short note to her mother.`,
      );
    }
  });
  lines.push("In the end, they went home and had a good dinner together.");
  return lines.join(genre.includes("dialogue") ? "\n" : " ");
}
/** Deterministic LLM used by tests only. */
export function testResponder(req: LLMRequest): string {
  const firstUser = req.messages.find((m) => m.role === "user")?.content ?? "";
  if (req.purpose === "article") {
    const targets = (firstUser.match(/Target words:\s*(.*)/)?.[1] ?? "")
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);
    const genre = firstUser.match(/Write an? (.+?) of \d+/)?.[1] ?? "story";
    return JSON.stringify({
      title: `A Day with ${targets[0] ?? "Friends"}`,
      body: articleBody(targets, genre),
      names: ["Lucy", "Tom"],
    });
  }
  if (req.purpose === "gloss") {
    const lemma = firstUser.match(/base form:\s*([^)]+)\)/)?.[1]?.trim() ?? "word";
    return JSON.stringify({
      lemma,
      pos: "n.",
      meaningZh: "语境释义（测试）",
      meaningEn: `meaning of ${lemma}`,
      ipa: `/${lemma}/`,
    });
  }
  if (req.purpose === "definition") return JSON.stringify({ meaningZh: "测试释义", pos: "" });
  return "{}";
}
