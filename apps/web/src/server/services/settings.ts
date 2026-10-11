import { type Settings, SettingsSchema } from "@eng-learn/core";
import { z } from "zod";
import type { AppDeps } from "../deps";
import { getTtsUsage } from "./audio";
import { validate } from "./shared";

export function getSettingsView(deps: AppDeps) {
  return { settings: deps.settings(), providers: deps.providers, ttsUsage: getTtsUsage(deps) };
}
export function updateSettings(deps: AppDeps, patch: Partial<Settings>) {
  validate(
    z
      .record(z.string(), z.unknown())
      .refine(
        (value) => Object.keys(value).every((key) => Object.hasOwn(SettingsSchema.shape, key)),
        "未知设置项",
      ),
    patch,
  );
  const before = deps.settings();
  const next = validate(SettingsSchema, { ...before, ...patch });
  const changed = (Object.keys(before) as (keyof Settings)[]).filter(
    (key) => before[key] !== next[key],
  );
  if (changed.length)
    deps.tx((repos) => {
      const now = deps.clock().getTime();
      repos.settings.save(deps.userId, next, now);
      repos.events.append(deps.userId, "settings_update", { changed }, now);
    });
  return next;
}
export const settingsService = { getSettingsView, update: updateSettings };
