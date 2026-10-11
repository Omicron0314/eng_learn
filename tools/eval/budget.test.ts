import { FakeLLM } from "@eng-learn/providers";
import { expect, it } from "vitest";
import { costGuard } from "./budget";

it("reserves budget before each call and prevents overshoot including failed attempts", async () => {
  const fake = new FakeLLM({ script: ["{}", "{}"] });
  const guard = costGuard(fake, { inputPerMTok: 1, outputPerMTok: 1 }, 0.003);
  const req = { purpose: "eval" as const, messages: [{ role: "user" as const, content: "test" }] };
  await guard.provider.complete(req);
  await expect(guard.provider.complete(req)).rejects.toThrow("EVAL_BUDGET_EXHAUSTED");
  expect(fake.calls).toBe(1);
  expect(guard.reserved()).toBeLessThanOrEqual(0.003);
});
it("retains reservations on provider failure so repair or fallback cannot bypass the cap", async () => {
  const fake = new FakeLLM({ script: [new Error("simulated provider failure"), "{}"] });
  const guard = costGuard(fake, { inputPerMTok: 1, outputPerMTok: 1 }, 0.003);
  const request = {
    purpose: "eval" as const,
    messages: [{ role: "user" as const, content: "test" }],
  };
  await expect(guard.provider.complete(request)).rejects.toThrow("simulated provider failure");
  await expect(guard.provider.complete(request)).rejects.toThrow("EVAL_BUDGET_EXHAUSTED");
  expect(fake.calls).toBe(1);
});
