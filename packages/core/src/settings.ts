import { z } from "zod";

export const LEVELS = ["A2", "B1", "B2", "C1"] as const;
export const TIERS = ["easy", "medium", "hard"] as const;
export const LOOKUP_MODES = ["markOnly", "autoAdd"] as const;

export type Tier = (typeof TIERS)[number];
export type LookupMode = (typeof LOOKUP_MODES)[number];

function isValidTimezone(tz: string): boolean {
  try {
    new Intl.DateTimeFormat("en-US", { timeZone: tz });
    return true;
  } catch {
    return false;
  }
}

export const SettingsSchema = z
  .object({
    level: z.enum(LEVELS).default("B1"),
    dailyNewLimit: z.number().int().min(0).max(200).default(20),
    dailyReviewLimit: z.number().int().min(0).max(2000).default(200),
    requestRetention: z.number().min(0.7).max(0.99).default(0.9),
    tier: z.enum(TIERS).default("medium"),
    articleMinWords: z.number().int().min(100).max(800).default(300),
    articleMaxWords: z.number().int().min(100).max(1000).default(400),
    targetCount: z.number().int().min(1).max(12).default(8),
    controlRatio: z.number().min(0).max(0.5).default(0.2),
    generationMaxRetries: z.number().int().min(0).max(3).default(1),
    lookupMode: z.enum(LOOKUP_MODES).default("markOnly"),
    ttsDailyCharLimit: z.number().int().min(0).max(200_000).default(20_000),
    timezone: z.string().refine(isValidTimezone, "invalid timezone").default("Asia/Shanghai"),
    dayStartHour: z.number().int().min(0).max(23).default(4),
  })
  .refine((s) => s.articleMaxWords >= s.articleMinWords, {
    message: "articleMaxWords must be >= articleMinWords",
    path: ["articleMaxWords"],
  });

export type Settings = z.infer<typeof SettingsSchema>;

export const DEFAULT_SETTINGS: Settings = SettingsSchema.parse({});

export const LEVEL_LABELS: Record<Settings["level"], string> = {
  A2: "A2(约初中水平)",
  B1: "B1(约中国大学英语四级水平)",
  B2: "B2(约六级水平)",
  C1: "C1(高级)",
};
