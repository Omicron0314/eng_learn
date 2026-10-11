import { parseArgs } from "node:util";
import { z } from "zod";
import { createAppDeps } from "../../apps/web/src/server/deps";
import { getExperiment } from "../../apps/web/src/server/services/stats";

const { values } = parseArgs({
  options: { db: { type: "string" }, window: { type: "string" }, json: { type: "boolean" } },
  strict: true,
});
const windowDays = z.coerce
  .number()
  .int()
  .min(1)
  .max(365)
  .parse(values.window ?? 7);
const deps = createAppDeps({
  ...process.env,
  LLM_PROVIDER: "fake",
  TTS_PROVIDER: "none",
  DATABASE_PATH: values.db ?? process.env.DATABASE_PATH,
});
try {
  const result = getExperiment(deps, windowDays);
  if (values.json) console.log(JSON.stringify(result));
  else {
    console.table({ exposed: result.exposed, control: result.control });
    console.log(
      `差值（出现组 − 对照组）：${result.diff ?? "暂无数据"}\n样本小，仅作参考。窗口：${windowDays} 天`,
    );
  }
} finally {
  deps.close();
}
