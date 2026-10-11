"use client";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState } from "react";
import type { getTodayView } from "../server/services";
import {
  api,
  Empty,
  genreLabels,
  Heading,
  Loading,
  Notice,
  percent,
  tierLabels,
  useLoad,
} from "./ui";

type TodayView = ReturnType<typeof getTodayView>;
export function Today() {
  const { data, error } = useLoad<TodayView>("today");
  const [tier, setTier] = useState("");
  const [genre, setGenre] = useState("");
  const [busy, setBusy] = useState(false);
  const [generationError, setError] = useState("");
  const router = useRouter();
  async function generate() {
    setBusy(true);
    setError("");
    try {
      const result = await api<{ articleId: string }>("articles", {
        tier: tier || data?.defaults.tier,
        ...(genre ? { genre } : {}),
      });
      router.push(`/article/${result.articleId}`);
    } catch (err) {
      setError((err as Error).message);
      setBusy(false);
    }
  }
  return (
    <>
      <Heading eyebrow="A LITTLE, EVERY DAY" title="今天，把单词放回生活。">
        从几张词卡开始，再读一段属于你的故事。
      </Heading>
      <Notice error={error} />
      {!data ? (
        <Loading />
      ) : (
        <>
          <div className="today-grid">
            <section className="card review-summary">
              <div className="eyebrow">01 / 温故知新</div>
              <h2>今日复习</h2>
              <div className="counts">
                <div>
                  <strong>{data.queue.learning + data.queue.review}</strong>
                  <span>待复习</span>
                </div>
                <div>
                  <strong>{data.queue.new}</strong>
                  <span>新词</span>
                </div>
              </div>
              <Link className="button" href="/review">
                {data.queue.new + data.queue.learning + data.queue.review
                  ? "开始复习 →"
                  : "今日复习已完成 ✓"}
              </Link>
              <p className="muted">少一点负担，多一点熟悉。</p>
            </section>
            <section className="card generation-panel">
              <div className="eyebrow">02 / 在语境里相遇</div>
              <h2>生成今日文章</h2>
              <p>复用今天的词汇，让记忆不止停留在卡片上。</p>
              <div className="fields two">
                <label>
                  难度档位
                  <select
                    value={tier || data.defaults.tier}
                    onChange={(e) => setTier(e.target.value)}
                  >
                    {Object.entries(tierLabels).map(([k, v]) => (
                      <option key={k} value={k}>
                        {v}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  体裁
                  <select value={genre} onChange={(e) => setGenre(e.target.value)}>
                    <option value="">自动轮换</option>
                    {Object.entries(genreLabels).map(([k, v]) => (
                      <option key={k} value={k}>
                        {v}
                      </option>
                    ))}
                  </select>
                </label>
              </div>
              <button type="button" disabled={busy} onClick={generate}>
                {busy ? "正在写作，请稍候…" : generationError ? "重试生成" : "生成今日文章"}
              </button>
              <Notice error={generationError} />
              <p className="muted">先复习几张卡片，才有可用的目标词。生成效果由模型决定。</p>
            </section>
          </div>
          <section className="section">
            <div className="section-title">
              <h2>今天的故事</h2>
              <span className="muted">{data.articles.length} 篇</span>
            </div>
            {data.articles.length ? (
              <div className="article-list">
                {data.articles.map((a) => (
                  <Link className="card article-link" key={a.id} href={`/article/${a.id}`}>
                    <span className="story-symbol" aria-hidden="true">
                      ▧
                    </span>
                    <div>
                      <span className="eyebrow">
                        {genreLabels[a.genre]} · 覆盖率 {percent(a.coverage)}
                      </span>
                      <h3>{a.title}</h3>
                    </div>
                    <span className="pill">{a.completed ? "已读 ✓" : "待阅读 →"}</span>
                  </Link>
                ))}
              </div>
            ) : (
              <Empty>故事还没开始。复习后，生成你的第一篇文章。</Empty>
            )}
          </section>
        </>
      )}
    </>
  );
}
