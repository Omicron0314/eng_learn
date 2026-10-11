import type { Tier } from "../settings";

export type ChatMessage = { role: "system" | "user" | "assistant"; content: string };

export const GENRES = ["story", "dialogue", "monologue"] as const;
export type Genre = (typeof GENRES)[number];

export const GENRE_LABELS: Record<Genre, string> = {
  story: "短故事",
  dialogue: "对话",
  monologue: "独白",
};

export const GENRE_PROMPT_NAMES: Record<Genre, string> = {
  story: "short story",
  dialogue: "dialogue between two people",
  monologue: "monologue",
};

export const TIER_MIN_COVERAGE: Record<Tier, number> = { easy: 0.98, medium: 0.95, hard: 0.9 };
export const TIER_LABELS: Record<Tier, string> = { easy: "轻松", medium: "适中", hard: "挑战" };

/**
 * Render a prompt template. Templates use `## system` and `## user` sections and
 * `{name}` placeholders. Lines whose placeholders all render empty are dropped.
 */
export function renderPrompt(
  template: string,
  vars: Record<string, string | number | undefined>,
): ChatMessage[] {
  const sections: { role: "system" | "user"; lines: string[] }[] = [];
  for (const line of template.split(/\r?\n/)) {
    const header = line.match(/^##\s*(system|user)\s*$/i);
    if (header) {
      sections.push({ role: (header[1] ?? "user").toLowerCase() as "system" | "user", lines: [] });
      continue;
    }
    if (sections.length === 0) continue;
    const placeholders = [...line.matchAll(/\{(\w+)\}/g)].map((m) => m[1] ?? "");
    if (placeholders.length > 0 && placeholders.every((p) => !String(vars[p] ?? "").trim())) {
      continue;
    }
    const rendered = line.replace(/\{(\w+)\}/g, (_, name: string) => String(vars[name] ?? ""));
    sections.at(-1)?.lines.push(rendered);
  }
  return sections.map((s) => ({ role: s.role, content: s.lines.join("\n").trim() }));
}

export type ArticlePromptVars = {
  genre: Genre;
  minWords: number;
  maxWords: number;
  level: string;
  targets: string[];
  avoidWords?: string[];
};

export function renderArticlePrompt(template: string, v: ArticlePromptVars): ChatMessage[] {
  return renderPrompt(template, {
    genre: GENRE_PROMPT_NAMES[v.genre],
    minWords: v.minWords,
    maxWords: v.maxWords,
    level: v.level,
    targets: v.targets.join(", "),
    avoidWords: (v.avoidWords ?? []).join(", "),
  });
}

/** Least recently used genre given the genres of recent articles, newest first. */
export function pickGenre(recentNewestFirst: Genre[]): Genre {
  let best: Genre = GENRES[0];
  let bestIdx = -1;
  for (const g of GENRES) {
    const idx = recentNewestFirst.indexOf(g);
    const age = idx === -1 ? Number.POSITIVE_INFINITY : idx;
    if (age > bestIdx) {
      best = g;
      bestIdx = age;
    }
  }
  return best;
}
