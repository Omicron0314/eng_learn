export type DayConfig = { timezone: string; dayStartHour: number };
export type TimeRange = { start: number; end: number };

const DAY_MS = 86_400_000;

function zonedParts(ts: number, timezone: string) {
  const fmt = new Intl.DateTimeFormat("en-US", {
    timeZone: timezone,
    hourCycle: "h23",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
  const parts = Object.fromEntries(fmt.formatToParts(new Date(ts)).map((p) => [p.type, p.value]));
  return {
    year: Number(parts.year),
    month: Number(parts.month),
    day: Number(parts.day),
    hour: Number(parts.hour),
    minute: Number(parts.minute),
    second: Number(parts.second),
  };
}

/** Offset (ms) of the timezone from UTC at the given instant. */
function tzOffset(ts: number, timezone: string): number {
  const p = zonedParts(ts, timezone);
  const asUtc = Date.UTC(p.year, p.month - 1, p.day, p.hour, p.minute, p.second);
  return asUtc - Math.floor(ts / 1000) * 1000;
}

/** UTC timestamp of a wall-clock time in the timezone. */
function zonedToUtc(y: number, m: number, d: number, hour: number, timezone: string): number {
  const guess = Date.UTC(y, m - 1, d, hour);
  const first = guess - tzOffset(guess, timezone);
  return guess - tzOffset(first, timezone);
}

/** The learning day containing `now`, as [start, end). */
export function dayRange(now: Date, cfg: DayConfig): TimeRange {
  const shifted = now.getTime() - cfg.dayStartHour * 3_600_000;
  const p = zonedParts(shifted, cfg.timezone);
  const start = zonedToUtc(p.year, p.month, p.day, cfg.dayStartHour, cfg.timezone);
  const next = new Date(Date.UTC(p.year, p.month - 1, p.day + 1));
  const end = zonedToUtc(
    next.getUTCFullYear(),
    next.getUTCMonth() + 1,
    next.getUTCDate(),
    cfg.dayStartHour,
    cfg.timezone,
  );
  return { start, end };
}

/** Learning-day key (YYYY-MM-DD) for a timestamp. */
export function dayKey(ts: number, cfg: DayConfig): string {
  const p = zonedParts(ts - cfg.dayStartHour * 3_600_000, cfg.timezone);
  return `${p.year}-${String(p.month).padStart(2, "0")}-${String(p.day).padStart(2, "0")}`;
}

/** The week (Monday start) containing `now`, as [start, end). */
export function weekRange(now: Date, cfg: DayConfig): TimeRange {
  const today = dayRange(now, cfg);
  const key = dayKey(today.start, cfg);
  const [y, m, d] = key.split("-").map(Number) as [number, number, number];
  const weekday = new Date(Date.UTC(y, m - 1, d)).getUTCDay();
  const back = (weekday + 6) % 7;
  const monday = new Date(Date.UTC(y, m - 1, d - back));
  const start = zonedToUtc(
    monday.getUTCFullYear(),
    monday.getUTCMonth() + 1,
    monday.getUTCDate(),
    cfg.dayStartHour,
    cfg.timezone,
  );
  const nextMonday = new Date(Date.UTC(y, m - 1, d - back + 7));
  const end = zonedToUtc(
    nextMonday.getUTCFullYear(),
    nextMonday.getUTCMonth() + 1,
    nextMonday.getUTCDate(),
    cfg.dayStartHour,
    cfg.timezone,
  );
  return { start, end };
}

export function daysAgo(now: Date, days: number): number {
  return now.getTime() - days * DAY_MS;
}

export { DAY_MS };
