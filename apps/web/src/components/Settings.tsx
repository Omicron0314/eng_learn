"use client";
import { LEVEL_LABELS, type Settings as SettingsType } from "@eng-learn/core";
import { useState } from "react";
import type { getSettingsView } from "../server/services";
import { api, Heading, Loading, Notice, tierLabels, useLoad } from "./ui";

type View = ReturnType<typeof getSettingsView>;
const numeric = [
  ["dailyNewLimit", "每日新词上限", 0, 200, 1],
  ["dailyReviewLimit", "每日复习上限", 0, 2000, 1],
  ["requestRetention", "目标保持率", 0.7, 0.99, 0.01],
  ["articleMinWords", "文章最少词数", 100, 800, 1],
  ["articleMaxWords", "文章最多词数", 100, 1000, 1],
  ["ttsDailyCharLimit", "每日语音字符上限", 0, 200000, 1],
  ["dayStartHour", "学习日开始小时", 0, 23, 1],
] as const;
const advanced = [
  ["targetCount", "目标词数量", 1, 12, 1],
  ["controlRatio", "对照词比例", 0, 0.5, 0.01],
  ["generationMaxRetries", "覆盖率重试上限", 0, 3, 1],
] as const;
export function Settings() {
  const { data, error, setData } = useLoad<View>("settings");
  const [busy, setBusy] = useState(false);
  const [saveError, setError] = useState("");
  const [message, setMessage] = useState("");
  async function save(form: FormData) {
    if (!data) return;
    setBusy(true);
    setError("");
    setMessage("");
    const patch: Record<string, unknown> = {};
    for (const [k, v] of form)
      patch[k] = typeof data.settings[k as keyof SettingsType] === "number" ? Number(v) : String(v);
    try {
      const settings = await api<SettingsType>("settings", patch);
      setData({ ...data, settings });
      setMessage("设置已保存");
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setBusy(false);
    }
  }
  function numbers(fields: typeof numeric | typeof advanced) {
    return fields.map(([key, label, min, max, step]) => (
      <label key={key}>
        {label}
        <input
          type="number"
          name={key}
          min={min}
          max={max}
          step={step}
          required
          defaultValue={data?.settings[key]}
        />
      </label>
    ));
  }
  return (
    <>
      <Heading eyebrow="MAKE IT YOURS" title="按自己的节奏">
        合理的上限，让学习更轻一点。
      </Heading>
      <Notice error={error} />
      {!data ? (
        <Loading />
      ) : (
        <>
          <section className="card provider-status">
            <div>
              <h2>服务状态</h2>
              <p>
                LLM ·{" "}
                {data.providers.llm.provider === "fake"
                  ? "演示模式（假服务，不代表真实生成质量）"
                  : data.providers.llm.provider}
              </p>
              <p className="muted">
                {data.providers.llm.host ?? "未配置端点"} ·{" "}
                {data.providers.llm.model ?? "无模型配置"}
              </p>
              {data.providers.llm.missing.length > 0 && (
                <Notice error={`缺少：${data.providers.llm.missing.join(", ")}`} />
              )}
            </div>
            <div>
              <p>
                TTS ·{" "}
                {data.providers.tts.provider === "none"
                  ? "使用浏览器朗读"
                  : data.providers.tts.provider}
              </p>
              <p className="muted">
                今日已用 {data.ttsUsage.usedChars} / {data.settings.ttsDailyCharLimit} 字符
              </p>
              <p className="muted">密钥只在服务端环境变量中设置；此页不会发起测试调用。</p>
            </div>
          </section>
          <form action={save} className="card section">
            <h2>学习与阅读</h2>
            <div className="fields three">
              <label>
                英语等级
                <select name="level" defaultValue={data.settings.level}>
                  {Object.entries(LEVEL_LABELS).map(([key, label]) => (
                    <option value={key} key={key}>
                      {label}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                默认难度
                <select name="tier" defaultValue={data.settings.tier}>
                  {Object.entries(tierLabels).map(([key, label]) => (
                    <option value={key} key={key}>
                      {label}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                点词模式
                <select name="lookupMode" defaultValue={data.settings.lookupMode}>
                  <option value="markOnly">只标记 → 生词箱</option>
                  <option value="autoAdd">标记不认识 → 自动建卡</option>
                </select>
              </label>
              {numbers(numeric)}
              <label>
                时区
                <input required name="timezone" defaultValue={data.settings.timezone} />
              </label>
            </div>
            <p className="muted">
              更改时区或日界会改变今天与历史统计的计算口径。语音上限为 0 时禁止服务端合成。
            </p>
            <details>
              <summary>高级 · 实验参数</summary>
              <div className="fields three">{numbers(advanced)}</div>
            </details>
            <Notice error={saveError} message={message} />
            <button type="submit" disabled={busy}>
              保存设置
            </button>
          </form>
          <section className="section">
            <button
              type="button"
              className="text-button"
              onClick={async () => {
                await api("auth", {}, "DELETE");
                window.location.assign("/login");
              }}
            >
              锁定 / 退出
            </button>
          </section>
        </>
      )}
    </>
  );
}
