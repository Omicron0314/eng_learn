import { describe, expect, it } from "vitest";
import { dayKey, dayRange, weekRange } from "./time";

const cfg = { timezone: "Asia/Shanghai", dayStartHour: 4 };

describe("dayRange", () => {
  it("starts at 04:00 local time", () => {
    // 2026-10-11 10:00 +08:00
    const r = dayRange(new Date("2026-10-11T02:00:00Z"), cfg);
    expect(new Date(r.start).toISOString()).toBe("2026-10-10T20:00:00.000Z");
    expect(new Date(r.end).toISOString()).toBe("2026-10-11T20:00:00.000Z");
  });

  it("counts 03:59 local as the previous day", () => {
    const before = dayRange(new Date("2026-10-11T19:59:00Z"), cfg); // 03:59 on the 12th
    const after = dayRange(new Date("2026-10-11T20:00:00Z"), cfg); // 04:00 on the 12th
    expect(before.end).toBe(after.start);
    expect(dayKey(Date.parse("2026-10-11T19:59:00Z"), cfg)).toBe("2026-10-11");
    expect(dayKey(Date.parse("2026-10-11T20:00:00Z"), cfg)).toBe("2026-10-12");
  });

  it("handles DST timezones", () => {
    const r = dayRange(new Date("2026-03-08T15:00:00Z"), {
      timezone: "America/New_York",
      dayStartHour: 0,
    });
    expect(r.end - r.start).toBe(23 * 3_600_000);
  });
});

describe("weekRange", () => {
  it("starts on Monday at day start", () => {
    // 2026-10-11 is a Sunday
    const r = weekRange(new Date("2026-10-11T02:00:00Z"), cfg);
    expect(new Date(r.start).toISOString()).toBe("2026-10-04T20:00:00.000Z");
    expect(r.end - r.start).toBe(7 * 86_400_000);
  });
});
