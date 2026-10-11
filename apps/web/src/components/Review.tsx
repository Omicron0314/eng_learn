"use client";
import Link from "next/link";
import { useCallback, useEffect, useRef, useState } from "react";
import type { getReviewCard } from "../server/services";
import { api, Heading, Loading, Notice, sourceLabels } from "./ui";

type Card = ReturnType<typeof getReviewCard>;
const choices = [
  ["again", "Again", "重来"],
  ["hard", "Hard", "困难"],
  ["good", "Good", "记得"],
  ["easy", "Easy", "轻松"],
] as const;
function interval(ms: number) {
  const minutes = Math.round(ms / 60000);
  return minutes < 60
    ? `${Math.max(1, minutes)} 分钟`
    : minutes < 1440
      ? `${Math.round(minutes / 60)} 小时`
      : `${Math.round(minutes / 1440)} 天`;
}
export function Review() {
  const [card, setCard] = useState<Card>();
  const [remaining, setRemaining] = useState<number>();
  const [answer, setAnswer] = useState(false);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [meaningBusy, setMeaningBusy] = useState(false);
  const started = useRef(Date.now());
  const lock = useRef(false);
  const next = useCallback(async () => {
    const queue = await api<{ learning: string[]; review: string[]; new: string[] }>(
      "review/queue",
    );
    const ids = [...queue.learning, ...queue.review, ...queue.new];
    setRemaining(ids.length);
    setAnswer(false);
    setCard(ids[0] ? await api<Card>(`cards/${ids[0]}`) : undefined);
    started.current = Date.now();
  }, []);
  useEffect(() => {
    next().catch((err) => setError(err.message));
  }, [next]);
  const show = useCallback(async () => {
    if (!card || answer) return;
    setAnswer(true);
    if (!card.back.meaning) {
      setMeaningBusy(true);
      try {
        const result = await api<{ meaning: string; source: "ai" | "import" | "user" }>(
          `cards/${card.id}/meaning`,
          {},
        );
        setCard((c) =>
          c && c.id === card.id
            ? { ...c, back: { ...c.back, meaning: result.meaning, meaningSource: result.source } }
            : c,
        );
      } catch {
        setError("释义生成失败，仍可评分；稍后可在词库编辑。");
      } finally {
        setMeaningBusy(false);
      }
    }
  }, [card, answer]);
  const rate = useCallback(
    async (rating: string) => {
      if (!card || !answer || lock.current) return;
      lock.current = true;
      setBusy(true);
      setError("");
      try {
        await api("review/rate", {
          cardId: card.id,
          rating,
          durationMs: Math.min(86400000, Date.now() - started.current),
        });
        await next();
      } catch (err) {
        setError((err as Error).message);
      } finally {
        lock.current = false;
        setBusy(false);
      }
    },
    [card, answer, next],
  );
  useEffect(() => {
    const handle = (event: KeyboardEvent) => {
      if ((event.target as HTMLElement).matches("input,textarea,select,button,a")) return;
      if (event.code === "Space") {
        event.preventDefault();
        void show();
      }
      const choice = choices[Number(event.key) - 1];
      if (choice && answer) {
        event.preventDefault();
        void rate(choice[0]);
      }
    };
    window.addEventListener("keydown", handle);
    return () => window.removeEventListener("keydown", handle);
  }, [show, rate, answer]);
  return (
    <>
      <Heading eyebrow="RECALL / REMEMBER" title="每一次想起，都算数。">
        空格显示答案，数字 1–4 评分。按真实记忆来选择。
      </Heading>
      <Notice error={error} />
      {remaining === undefined ? (
        <Loading />
      ) : !card ? (
        <div className="card completion">
          <span className="completion-mark">✓</span>
          <h2>今日已完成</h2>
          <p>把词卡合上，去故事里再次遇见它们。</p>
          <Link className="button" href="/today">
            回到今日 →
          </Link>
        </div>
      ) : (
        <>
          <div className="review-meta">
            <span>今日队列 · 还剩 {remaining} 张</span>
            <span className="pill">{card.deck}</span>
          </div>
          <section className="card flashcard">
            <span className="eyebrow">试着想起它的意思</span>
            <h2 lang="en">{card.front}</h2>
            {answer ? (
              <div className="answer">
                <p className="meaning">
                  {meaningBusy ? "正在生成释义…" : card.back.meaning || "暂无释义"}
                </p>
                <span className="pill">{sourceLabels[card.back.meaningSource]}</span>
                {card.back.example && (
                  <p lang="en" className="example">
                    {card.back.example}
                  </p>
                )}
              </div>
            ) : (
              <button type="button" className="secondary" onClick={show}>
                显示答案
              </button>
            )}
          </section>
          {answer && (
            <div className="ratings">
              {choices.map(([rating, en, zh], i) => (
                <button
                  type="button"
                  key={rating}
                  disabled={busy}
                  className={`rating ${rating}`}
                  onClick={() => rate(rating)}
                >
                  {i + 1} · {en}
                  <span>
                    {zh} · {interval(card.preview[rating].intervalMs)}
                  </span>
                </button>
              ))}
            </div>
          )}
        </>
      )}
    </>
  );
}
