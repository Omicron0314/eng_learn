import type { Rating, SrsCard } from "../srs/srs";

export type TargetCandidate = {
  lexemeId: string;
  lemma: string;
  card: SrsCard;
  suspended: boolean;
  todayRatings: Rating[];
  introducedToday: boolean;
  retrievability: number;
};

export type SelectedTarget = {
  lexemeId: string;
  lemma: string;
  role: "exposed" | "control";
  bucket: 1 | 2 | 3;
};

export const RECENT_EXPOSURE_DAYS = 3;
export const MAX_RECENT_EXPOSURE = 3;

function bucketOf(c: TargetCandidate, now: Date): 1 | 2 | 3 | null {
  if (c.todayRatings.some((r) => r === "again" || r === "hard")) return 1;
  if (c.introducedToday) return 2;
  if (c.card.state === "review" && c.card.due.getTime() <= now.getTime()) return 3;
  return null;
}

export function selectTargets(input: {
  candidates: TargetCandidate[];
  recentExposure: Map<string, number>;
  now: Date;
  n: number;
  controlRatio: number;
  rng: () => number;
}): SelectedTarget[] {
  const ranked = input.candidates
    .filter((c) => !c.suspended && !c.lemma.includes(" "))
    .filter((c) => (input.recentExposure.get(c.lexemeId) ?? 0) < MAX_RECENT_EXPOSURE)
    .map((c) => ({ c, bucket: bucketOf(c, input.now) }))
    .filter((x): x is { c: TargetCandidate; bucket: 1 | 2 | 3 } => x.bucket !== null)
    .sort(
      (a, b) =>
        a.bucket - b.bucket ||
        a.c.retrievability - b.c.retrievability ||
        a.c.lexemeId.localeCompare(b.c.lexemeId),
    );

  const seen = new Set<string>();
  const chosen: { c: TargetCandidate; bucket: 1 | 2 | 3 }[] = [];
  for (const x of ranked) {
    if (seen.has(x.c.lemma)) continue;
    seen.add(x.c.lemma);
    chosen.push(x);
    if (chosen.length >= input.n) break;
  }

  const controlCount = Math.round(chosen.length * input.controlRatio);
  const indexes = chosen.map((_, i) => i);
  for (let i = indexes.length - 1; i > 0; i--) {
    const j = Math.floor(input.rng() * (i + 1));
    [indexes[i], indexes[j]] = [indexes[j] as number, indexes[i] as number];
  }
  const controls = new Set(indexes.slice(0, controlCount));

  return chosen.map((x, i) => ({
    lexemeId: x.c.lexemeId,
    lemma: x.c.lemma,
    bucket: x.bucket,
    role: controls.has(i) ? "control" : "exposed",
  }));
}

/** Deterministic PRNG (mulberry32) for tests and reproducible selection. */
export function seededRng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
