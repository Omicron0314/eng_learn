import { type DayConfig, dayKey, type TimeRange } from "../time";

export type Difficulty = "too_hard" | "just_right" | "too_easy";

export function median(values: number[]): number | null {
  if (values.length === 0) return null;
  const s = [...values].sort((a, b) => a - b);
  const mid = Math.floor(s.length / 2);
  return s.length % 2 ? (s[mid] as number) : ((s[mid - 1] as number) + (s[mid] as number)) / 2;
}

export function countInRange<T>(items: T[], ts: (x: T) => number, range: TimeRange): number {
  return items.filter((x) => ts(x) >= range.start && ts(x) < range.end).length;
}

/** Number of distinct learning days with activity in each week range. */
export function activeDaysPerWeek(
  eventTimes: number[],
  weeks: TimeRange[],
  cfg: DayConfig,
): number[] {
  return weeks.map((w) => {
    const days = new Set(
      eventTimes.filter((t) => t >= w.start && t < w.end).map((t) => dayKey(t, cfg)),
    );
    return days.size;
  });
}

export function completionRate(opened: Set<string>, completed: Set<string>): number | null {
  if (opened.size === 0) return null;
  let done = 0;
  for (const id of opened) if (completed.has(id)) done++;
  return done / opened.size;
}

/** Latest difficulty value per (article, after), then counted. */
export function difficultyDistribution(
  events: { articleId: string; after: string; value: Difficulty; ts: number }[],
): { distribution: Record<Difficulty, number>; justRightRate: number | null } {
  const latest = new Map<string, { value: Difficulty; ts: number }>();
  for (const e of events) {
    const key = `${e.articleId}:${e.after}`;
    const prev = latest.get(key);
    if (!prev || e.ts >= prev.ts) latest.set(key, { value: e.value, ts: e.ts });
  }
  const distribution: Record<Difficulty, number> = { too_hard: 0, just_right: 0, too_easy: 0 };
  for (const v of latest.values()) distribution[v.value]++;
  const total = latest.size;
  return { distribution, justRightRate: total === 0 ? null : distribution.just_right / total };
}
