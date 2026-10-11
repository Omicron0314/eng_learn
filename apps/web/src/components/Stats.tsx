"use client";
import { useState } from "react";
import type { getSettingsView, getStats } from "../server/services";
import { Empty, Heading, Loading, Notice, Pagination, percent, stateLabels, useLoad } from "./ui";

type View = ReturnType<typeof getStats>;
export function Stats() {
  const [offset, setOffset] = useState(0);
  const { data, error } = useLoad<View>(`stats?offset=${offset}&limit=50`);
  const { data: settings } = useLoad<ReturnType<typeof getSettingsView>>("settings");
  return (
    <>
      <Heading eyebrow="SMALL STEPS / REAL PROGRESS" title="看见积累">
        数字是线索，不是分数。慢慢观察，找到有效的学习方式。
      </Heading>
      <Notice error={error} />
      {!data ? (
        <Loading />
      ) : (
        <>
          <div className="metric-grid">
            <div className="card metric">
              <span>今日新学</span>
              <strong data-testid="today-new">{data.learning.today.newLearned}</strong>
              <small>本周 {data.learning.week.newLearned} 个</small>
            </div>
            <div className="card metric">
              <span>今日复习</span>
              <strong>{data.learning.today.reviews}</strong>
              <small>本周 {data.learning.week.reviews} 次</small>
            </div>
            <div className="card metric">
              <span>阅读完成率</span>
              <strong>{percent(data.h1.completionRate)}</strong>
              <small>目标 ≥60%</small>
            </div>
            <div className="card metric">
              <span>难度「刚好」</span>
              <strong>{percent(data.h2.justRightRate)}</strong>
              <small>目标 ≥60%</small>
            </div>
          </div>
          <section className="card section">
            <h2>文章覆盖率趋势</h2>
            {data.coverageTrend.length ? (
              <>
                <svg
                  viewBox="0 0 640 160"
                  className="trend"
                  role="img"
                  aria-label="文章覆盖率，按日期升序"
                >
                  <title>文章覆盖率趋势</title>
                  <line x1="20" y1="135" x2="620" y2="135" stroke="#ddd" />
                  <polyline
                    fill="none"
                    stroke="#245c4c"
                    strokeWidth="3"
                    points={data.coverageTrend
                      .map(
                        (p, i) =>
                          `${20 + (i / Math.max(1, data.coverageTrend.length - 1)) * 600},${135 - p.coverage * 110}`,
                      )
                      .join(" ")}
                  />
                  {data.coverageTrend.map((p, i) => (
                    <circle
                      key={i}
                      cx={20 + (i / Math.max(1, data.coverageTrend.length - 1)) * 600}
                      cy={135 - p.coverage * 110}
                      r="5"
                      fill="#245c4c"
                    >
                      <title>
                        {p.date}：{percent(p.coverage)}
                      </title>
                    </circle>
                  ))}
                </svg>
                <p className="muted">
                  {data.coverageTrend[0]?.date} → {data.coverageTrend.at(-1)?.date} · 最近 7
                  天未知词占比中位数 {percent(data.unknownRatioMedian.last7d)} · 全部{" "}
                  {percent(data.unknownRatioMedian.all)}（目标 ≤5%）
                </p>
              </>
            ) : (
              <Empty>生成文章后，这里会出现覆盖率趋势。</Empty>
            )}
          </section>
          <div className="two-columns section">
            <section className="card">
              <h2>每周活跃</h2>
              <p className="muted">目标：每周 ≥5 天 · 最近四周</p>
              {data.h1.activeDaysPerWeek.map((week) => (
                <div className="week-row" key={week.weekStart}>
                  <span>{week.weekStart}</span>
                  <progress max="7" value={week.days} aria-label={`${week.weekStart} 活跃天数`} />
                  <strong>{week.days} 天</strong>
                </div>
              ))}
            </section>
            <section className="card">
              <h2>难度反馈</h2>
              <p className="muted">每篇文章 / 阅读与听力分别取最后一次反馈</p>
              {[
                ["too_hard", "太难"],
                ["just_right", "刚好"],
                ["too_easy", "太简单"],
              ].map(([key, label]) => (
                <div className="week-row" key={key}>
                  <span>{label}</span>
                  <strong>
                    {data.h2.distribution[key as keyof typeof data.h2.distribution]} 次
                  </strong>
                </div>
              ))}
            </section>
          </div>
          <section className="card section">
            <h2>对照实验</h2>
            <p className="muted">
              文章中出现的目标词 vs 刻意留出的词，之后 7 天内首次复习的 Again
              率。样本小，仅作参考；不代表因果结论。
            </p>
            <div className="experiment-grid">
              {(["exposed", "control"] as const).map((group) => (
                <div key={group}>
                  <span className="eyebrow">
                    {group === "exposed" ? "出现组 / EXPOSED" : "对照组 / CONTROL"}
                  </span>
                  <h3>{percent(data.h3[group].againRate)}</h3>
                  <p>
                    样本 {data.h3[group].n} · Again {data.h3[group].again} · 未复习{" "}
                    {data.h3[group].noReview}
                  </p>
                </div>
              ))}
            </div>
            <p>
              出现组 − 对照组：<strong>{percent(data.h3.diff)}</strong>
            </p>
          </section>
          <section className="section">
            <div className="section-title">
              <h2>已学词汇</h2>
              <span className="muted">{data.learnedWordsTotal} 个</span>
            </div>
            {data.learnedWords.length ? (
              <div className="card table-wrap">
                <table>
                  <thead>
                    <tr>
                      <th>词 / 释义</th>
                      <th>状态</th>
                      <th>可提取率</th>
                    </tr>
                  </thead>
                  <tbody>
                    {data.learnedWords.map((word) => (
                      <tr key={word.lemma}>
                        <td>
                          <strong lang="en">{word.lemma}</strong>
                          <p>{word.meaning}</p>
                        </td>
                        <td>{stateLabels[word.state]}</td>
                        <td>{percent(word.retrievability)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : (
              <Empty>第一次复习之后，就有了第一点积累。</Empty>
            )}
            <Pagination offset={offset} total={data.learnedWordsTotal} onChange={setOffset} />
            <p className="muted">
              统计口径：{settings?.settings.timezone ?? "—"} · 每日{" "}
              {settings?.settings.dayStartHour ?? "—"}:00 开始。长期留存与真实生成质量尚待自用验证。
            </p>
          </section>
        </>
      )}
    </>
  );
}
