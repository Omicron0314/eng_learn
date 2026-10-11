"use client";
import Link from "next/link";
import { useState } from "react";
import type { listInbox } from "../server/services";
import { api, Empty, Heading, Loading, Notice, useLoad } from "./ui";

type Rows = ReturnType<typeof listInbox>;
export function Inbox() {
  const { data, error, reload } = useLoad<Rows>("inbox");
  const [selected, setSelected] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [actionError, setError] = useState("");
  const [message, setMessage] = useState("");
  async function batch(action: string) {
    setBusy(true);
    setError("");
    try {
      const result = await api<{ processed: number }>("inbox/batch", { action, ids: selected });
      setSelected([]);
      setMessage(`已处理 ${result.processed} 个词`);
      reload();
      window.dispatchEvent(new Event("inbox-changed"));
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <>
      <Heading eyebrow="WORDS TO COME BACK TO" title="生词箱">
        阅读时的一个小标记，在这里慢慢整理。
      </Heading>
      <Notice error={error || actionError} message={message} />
      {!data ? (
        <Loading />
      ) : !data.length ? (
        <Empty>没有待处理的生词</Empty>
      ) : (
        <>
          <div className="inbox-toolbar">
            <label className="check">
              <input
                type="checkbox"
                checked={selected.length === data.length}
                onChange={(e) => setSelected(e.target.checked ? data.map((r) => r.id) : [])}
              />
              全选
            </label>
            <span className="muted">
              已选 {selected.length} / {data.length}
            </span>
            <button type="button" disabled={busy || !selected.length} onClick={() => batch("add")}>
              加入复习
            </button>
            <button
              type="button"
              className="secondary"
              disabled={busy || !selected.length}
              onClick={() => batch("ignore")}
            >
              忽略
            </button>
          </div>
          <div className="inbox-list">
            {data.map((row) => (
              <article className="card inbox-row" key={row.id}>
                <label className="check">
                  <input
                    aria-label={`选择 ${row.lemma}`}
                    type="checkbox"
                    checked={selected.includes(row.id)}
                    onChange={(e) =>
                      setSelected((ids) =>
                        e.target.checked ? [...ids, row.id] : ids.filter((v) => v !== row.id),
                      )
                    }
                  />
                  <strong lang="en">{row.lemma}</strong>
                </label>
                {row.gloss && (
                  <p>
                    {row.gloss.meaningZh} <span className="source">AI 生成</span>
                  </p>
                )}
                <p className="context" lang="en">
                  {row.sentence
                    .split(
                      new RegExp(`(${row.lemma.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`, "gi"),
                    )
                    .map((part, i) =>
                      part.toLowerCase() === row.lemma.toLowerCase() ? (
                        <mark key={i}>{part}</mark>
                      ) : (
                        part
                      ),
                    )}
                </p>
                <div className="section-title">
                  <span className="muted">{new Date(row.createdAt).toLocaleString("zh-CN")}</span>
                  {row.articleId && <Link href={`/article/${row.articleId}`}>回到文章 →</Link>}
                </div>
              </article>
            ))}
          </div>
        </>
      )}
    </>
  );
}
