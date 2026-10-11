"use client";
import { type Settings, sentenceAt, splitSentences } from "@eng-learn/core";
import Link from "next/link";
import { useCallback, useEffect, useRef, useState } from "react";
import type { AudioResult, getArticleView, lookupArticleWord } from "../server/services";
import { AudioPlayer } from "./AudioPlayer";
import { Modal } from "./Modal";
import { api, genreLabels, Heading, Loading, Notice, percent, tierLabels, useLoad } from "./ui";

type View = NonNullable<ReturnType<typeof getArticleView>> & {
  feedback: Partial<Record<"reading" | "listening", string>>;
};
type Lookup = Awaited<ReturnType<typeof lookupArticleWord>>;
function Feedback({
  articleId,
  after,
  selected,
  onSelected,
}: {
  articleId: string;
  after: "reading" | "listening";
  selected: string;
  onSelected: (value: string) => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  async function submit(value: string) {
    setBusy(true);
    setError("");
    try {
      await api(`articles/${articleId}/feedback`, { type: "difficulty", value, after });
      onSelected(value);
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="feedback">
      <h3>{after === "listening" ? "听完后，感觉如何？" : "读完后，感觉如何？"}</h3>
      <div className="actions">
        {[
          ["too_hard", "太难"],
          ["just_right", "刚好"],
          ["too_easy", "太简单"],
        ].map(([k, v]) => (
          <button
            type="button"
            disabled={busy}
            aria-pressed={selected === k}
            className={selected === k ? "" : "secondary"}
            key={k}
            onClick={() => k && submit(k)}
          >
            {v}
          </button>
        ))}
      </div>
      <Notice error={error} />
    </section>
  );
}
export function Article({ id }: { id: string }) {
  const { data, error, setData } = useLoad<View>(`articles/${id}`);
  const [visible, setVisible] = useState(false);
  const [source, setSource] = useState<AudioResult>();
  const [audioBusy, setAudioBusy] = useState(false);
  const [audioError, setAudioError] = useState("");
  const [listened, setListened] = useState(false);
  const [completed, setCompleted] = useState(false);
  const [eventError, setEventError] = useState("");
  const openPromise = useRef<Promise<string> | null>(null);
  const completionPending = useRef(false);
  const played = useRef(false);
  const end = useRef<HTMLDivElement>(null);
  const [selected, setSelected] = useState<{ start: number; surface: string }>();
  const [lookup, setLookup] = useState<Lookup>();
  const [lookupMode, setLookupMode] = useState<Settings["lookupMode"]>();
  const [lookupError, setLookupError] = useState("");
  const [markMessage, setMarkMessage] = useState("");
  const [markBusy, setMarkBusy] = useState(false);
  const requestId = useRef(0);
  const [flagOpen, setFlagOpen] = useState(false);
  const [flagMessage, setFlagMessage] = useState("");
  const [flagBusy, setFlagBusy] = useState(false);
  useEffect(() => {
    if (data && !openPromise.current) {
      openPromise.current = api<string>(`articles/${id}/events`, { type: "article_open" });
      openPromise.current.catch(() => setEventError("阅读记录失败，请刷新后重试。"));
    }
  }, [data, id]);
  const complete = useCallback(
    async (via: "button" | "scroll") => {
      if (completed || completionPending.current || !openPromise.current) return;
      completionPending.current = true;
      try {
        const openId = await openPromise.current;
        await api(`articles/${id}/events`, { type: "article_complete", via, openId });
        setCompleted(true);
      } catch {
        setEventError("读完记录失败，请重试。");
      } finally {
        completionPending.current = false;
      }
    },
    [id, completed],
  );
  useEffect(() => {
    if (!visible || !data || completed || !end.current) return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const observer = new IntersectionObserver(
      (entries) => {
        if (timer) clearTimeout(timer);
        if (entries[0]?.isIntersecting)
          timer = setTimeout(() => {
            void complete("scroll");
          }, 3000);
      },
      { threshold: 1 },
    );
    observer.observe(end.current);
    return () => {
      observer.disconnect();
      if (timer) clearTimeout(timer);
    };
  }, [visible, data, completed, complete]);
  async function getAudio() {
    setAudioBusy(true);
    setAudioError("");
    try {
      setSource(await api<AudioResult>(`articles/${id}/audio`, {}));
    } catch (err) {
      setAudioError((err as Error).message);
    } finally {
      setAudioBusy(false);
    }
  }
  function audioStarted(rate: number) {
    if (played.current || !source) return;
    played.current = true;
    api(`articles/${id}/play`, { rate, via: source.mode }).catch(() => {
      played.current = false;
      setEventError("播放记录失败，请再次播放。");
    });
  }
  async function select(start: number, surface: string) {
    const current = ++requestId.current;
    setSelected({ start, surface });
    setLookup(undefined);
    setLookupMode(undefined);
    setLookupError("");
    setMarkMessage("");
    try {
      const [result, view] = await Promise.all([
        api<Lookup>("gloss", { articleId: id, start }),
        api<{ settings: Settings }>("settings"),
      ]);
      if (current === requestId.current) {
        setLookup(result);
        setLookupMode(view.settings.lookupMode);
      }
    } catch {
      if (current === requestId.current) setLookupError("释义生成失败，可重试");
    }
  }
  async function mark(action: string) {
    if (!lookup || !selected || !data) return;
    const current = requestId.current;
    setMarkBusy(true);
    setLookupError("");
    try {
      const result = await api<{ created?: boolean; cardId?: string; inboxId?: string }>("marks", {
        action,
        lemma: lookup.gloss.lemma,
        sentence: sentenceAt(data.body, selected.start),
        articleId: id,
      });
      if (current === requestId.current)
        setMarkMessage(
          action === "add"
            ? result.created === false
              ? "已在复习中"
              : "已加入复习"
            : action === "unknown"
              ? result.cardId
                ? "已标记不认识，并加入复习（计入每日新词上限）"
                : "已标记不认识，已进入生词箱"
              : "已标记认识",
        );
      window.dispatchEvent(new Event("inbox-changed"));
    } catch (err) {
      if (current === requestId.current) setLookupError((err as Error).message);
    } finally {
      setMarkBusy(false);
    }
  }
  function pronounce(text: string) {
    if (!("speechSynthesis" in window)) {
      setLookupError("当前浏览器不支持朗读");
      return;
    }
    window.speechSynthesis.cancel();
    const utterance = new SpeechSynthesisUtterance(text);
    utterance.lang = "en-US";
    window.speechSynthesis.speak(utterance);
  }
  async function flag(form: FormData) {
    setFlagBusy(true);
    setEventError("");
    try {
      await api(`articles/${id}/feedback`, {
        type: "unnatural",
        sentenceIndexes: form.getAll("sentences").map(Number),
        note: String(form.get("note") ?? ""),
      });
      setFlagOpen(false);
      setFlagMessage("反馈已记录，谢谢。");
    } catch (err) {
      setEventError((err as Error).message);
    } finally {
      setFlagBusy(false);
    }
  }
  return (
    <>
      <Link href="/today" className="back-link">
        ← 回到今日
      </Link>
      <Notice error={error} />
      {!data ? (
        error ? (
          <div className="card">
            <h1>文章不存在或暂时无法读取</h1>
          </div>
        ) : (
          <Loading />
        )
      ) : (
        <>
          <Heading
            eyebrow={`${genreLabels[data.genre]} / ${tierLabels[data.tier]}`}
            title={data.title}
          >
            <span>
              覆盖率 {percent(data.coverage)} ·{" "}
              {new Date(data.createdAt).toLocaleDateString("zh-CN")} · {data.analysis.wordCount} 词
            </span>
          </Heading>
          {data.belowTier && <Notice message="比所选档位更难；覆盖率仅作参考。" />}
          <section className="card listening">
            <div className="section-title">
              <div>
                <div className="eyebrow">LISTEN FIRST</div>
                <h2>先听，再看。</h2>
              </div>
              <span className="story-symbol" aria-hidden="true">
                ♫
              </span>
            </div>
            <p className="muted">给耳朵一点时间。听完以后，再打开原文。</p>
            {!source ? (
              <>
                <button type="button" disabled={audioBusy} onClick={getAudio}>
                  {audioBusy ? "正在生成音频…" : "生成音频"}
                </button>
                <button
                  type="button"
                  className="text-button"
                  onClick={() => {
                    setSource({ mode: "browser", text: `${data.title}\n\n${data.body}` });
                    setAudioError("");
                  }}
                >
                  改用浏览器朗读
                </button>
              </>
            ) : (
              <AudioPlayer
                source={source}
                onEnded={() => setListened(true)}
                onStarted={audioStarted}
              />
            )}
            <Notice error={audioError} />
            {audioError && (
              <button
                type="button"
                className="secondary"
                onClick={() => {
                  setSource({ mode: "browser", text: `${data.title}\n\n${data.body}` });
                  setAudioError("");
                }}
              >
                改用浏览器朗读
              </button>
            )}
            {listened && (
              <Feedback
                articleId={id}
                after="listening"
                selected={data.feedback.listening ?? ""}
                onSelected={(value) =>
                  setData(
                    (current) =>
                      current && {
                        ...current,
                        feedback: { ...current.feedback, listening: value },
                      },
                  )
                }
              />
            )}
          </section>
          <section className="section">
            <div className="section-title">
              <h2>阅读原文</h2>
              <button
                type="button"
                className="secondary"
                aria-expanded={visible}
                onClick={() => setVisible((v) => !v)}
              >
                {visible ? "收起原文" : "显示原文"}
              </button>
            </div>
            {visible ? (
              <div className="card reading">
                <div className="body-text" lang="en">
                  {data.paragraphs.map((p, i) => {
                    const base = data.body.indexOf(
                      p.text,
                      i === 0
                        ? 0
                        : data.paragraphs.slice(0, i).reduce((n, v) => n + v.text.length, 0),
                    );
                    let cursor = base;
                    const nodes: React.ReactNode[] = [];
                    for (const token of p.tokens) {
                      nodes.push(data.body.slice(cursor, token.start));
                      nodes.push(
                        token.kind === "word" ? (
                          // biome-ignore lint/a11y/useSemanticElements: spec requires keyboard-accessible word spans.
                          <span
                            role="button"
                            tabIndex={0}
                            key={token.start}
                            data-start={token.start}
                            data-end={token.end}
                            data-sentence={token.sentenceIndex}
                            onClick={() => select(token.start, token.surface)}
                            onKeyDown={(e) => {
                              if (e.key === "Enter" || e.key === " ") {
                                e.preventDefault();
                                void select(token.start, token.surface);
                              }
                            }}
                          >
                            {data.body.slice(token.start, token.end)}
                          </span>
                        ) : (
                          data.body.slice(token.start, token.end)
                        ),
                      );
                      cursor = token.end;
                    }
                    nodes.push(data.body.slice(cursor, base + p.text.length));
                    return <p key={i}>{nodes}</p>;
                  })}
                </div>
                <div ref={end} className="reading-end">
                  <span>— END —</span>
                </div>
                <div className="actions">
                  <button type="button" disabled={completed} onClick={() => complete("button")}>
                    {completed ? "已读完 ✓" : "读完了"}
                  </button>
                  <button type="button" className="text-button" onClick={() => setFlagOpen(true)}>
                    标记不自然
                  </button>
                </div>
                {completed && (
                  <Feedback
                    articleId={id}
                    after="reading"
                    selected={data.feedback.reading ?? ""}
                    onSelected={(value) =>
                      setData(
                        (current) =>
                          current && {
                            ...current,
                            feedback: { ...current.feedback, reading: value },
                          },
                      )
                    }
                  />
                )}
              </div>
            ) : (
              <div className="closed-text">原文暂时收起。准备好了，就打开它。</div>
            )}
            <Notice error={eventError} message={flagMessage} />
          </section>
          {selected && (
            <Modal
              title={`语境释义 · ${selected.surface}`}
              onClose={() => {
                requestId.current++;
                setSelected(undefined);
              }}
            >
              <p className="context" lang="en">
                {sentenceAt(data.body, selected.start)}
              </p>
              {lookup ? (
                <>
                  <h3 lang="en">
                    {lookup.gloss.lemma}{" "}
                    <small>
                      {lookup.gloss.pos} {lookup.gloss.ipa}
                    </small>
                  </h3>
                  <button
                    type="button"
                    className="text-button"
                    onClick={() => pronounce(lookup.gloss.lemma)}
                  >
                    发音 ♫
                  </button>
                  <p className="meaning">{lookup.gloss.meaningZh}</p>
                  <span className="pill">
                    AI 生成 · {lookup.cacheHit ? "缓存释义" : "语境释义"}
                  </span>
                  <div className="actions lookup-actions">
                    <button
                      type="button"
                      disabled={markBusy}
                      className="secondary"
                      onClick={() => mark("known")}
                    >
                      认识
                    </button>
                    <button
                      type="button"
                      disabled={markBusy}
                      className="secondary"
                      onClick={() => mark("unknown")}
                    >
                      标记不认识
                    </button>
                    <button type="button" disabled={markBusy} onClick={() => mark("add")}>
                      加入复习
                    </button>
                  </div>
                </>
              ) : lookupError ? (
                <button type="button" onClick={() => select(selected.start, selected.surface)}>
                  重试释义
                </button>
              ) : (
                <Loading />
              )}
              <Notice error={lookupError} message={markMessage} />
              <p className="muted">
                {lookupMode === "autoAdd"
                  ? "当前模式：自动建卡。标记不认识会加入复习，并计入每日新词上限。"
                  : lookupMode === "markOnly"
                    ? "当前模式：只标记。标记不认识仅进入生词箱，不建卡。"
                    : "正在读取当前点词模式…"}
              </p>
            </Modal>
          )}
          {flagOpen && (
            <Modal title="标记不自然" onClose={() => setFlagOpen(false)}>
              <form action={flag}>
                <div className="sentence-list">
                  {splitSentences(data.body).map((sentence, i) => (
                    <label className="check" key={i}>
                      <input name="sentences" type="checkbox" value={i} />
                      <span lang="en">{sentence}</span>
                    </label>
                  ))}
                </div>
                <label>
                  备注
                  <textarea
                    name="note"
                    maxLength={200}
                    placeholder="哪里读起来不自然？（可选，最多 200 字）"
                  />
                </label>
                <button type="submit" disabled={flagBusy}>
                  提交反馈
                </button>
                <Notice error={eventError} />
              </form>
            </Modal>
          )}
        </>
      )}
    </>
  );
}
