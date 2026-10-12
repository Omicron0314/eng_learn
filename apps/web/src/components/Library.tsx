"use client";
import { useState } from "react";
import { importFileMaxBytes } from "../import-limits";
import type { listCards } from "../server/services";
import { Modal } from "./Modal";
import {
  api,
  Empty,
  Heading,
  Loading,
  Notice,
  Pagination,
  sourceLabels,
  stateLabels,
  useLoad,
} from "./ui";

type Cards = ReturnType<typeof listCards>;
export function Library() {
  const [query, setQuery] = useState("");
  const [deck, setDeck] = useState("");
  const [state, setState] = useState("");
  const [suspended, setSuspended] = useState("");
  const [offset, setOffset] = useState(0);
  const [filter, setFilter] = useState("");
  const { data, error, reload } = useLoad<Cards>(`cards?limit=50&offset=${offset}${filter}`);
  const [type, setType] = useState("cards");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [actionError, setError] = useState("");
  const [editing, setEditing] = useState<Cards["rows"][number]>();
  async function removeDeck() {
    const name = deck.trim();
    if (!name) return;
    if (
      !window.confirm(
        `确定删除牌组「${name}」中的全部词卡？已有复习记录的卡片将改为永久暂停，复习历史保留。`,
      )
    )
      return;
    setBusy(true);
    setError("");
    setMessage("");
    try {
      const result = await api<{ deleted: number; keptWithHistory: number }>("cards/delete-deck", {
        deck: name,
      });
      setMessage(
        `已删除 ${result.deleted} 张词卡` +
          (result.keptWithHistory ? `，${result.keptWithHistory} 张有复习记录，已改为暂停` : ""),
      );
      reload();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setBusy(false);
    }
  }
  async function upload(form: FormData) {
    setBusy(true);
    setError("");
    setMessage("");
    try {
      const file = form.get("file");
      if (file instanceof File && file.size > importFileMaxBytes(file.name))
        throw new Error(`文件不能超过 ${importFileMaxBytes(file.name) / (1024 * 1024)} MiB`);
      const result = await api<{
        added?: number;
        skipped?: number;
        duplicates: number;
        errors?: { line: number; reason: string }[];
        cardsAdded?: number;
        wordsAdded?: number;
        parsed?: number;
        skippedLines?: { line: number; raw: string }[];
        phrases?: number;
      }>(type === "cards" ? "import/cards" : "import/wordlist", form);
      setMessage(
        `导入完成 · ${type === "cards" ? `新增 ${result.added ?? 0} 张卡片` : `新增词条 ${result.wordsAdded ?? 0} · 新增卡片 ${result.cardsAdded ?? 0}`} · 解析 ${result.parsed ?? (result.added ?? 0) + (result.skipped ?? 0)} · 跳过 ${result.skipped ?? result.skippedLines?.length ?? 0} · 重复 ${result.duplicates}${result.phrases ? ` · 短语 ${result.phrases}` : ""}${result.errors?.length ? ` · 错误行：${result.errors.map((e) => `${e.line} (${e.reason})`).join("，")}` : ""}${result.skippedLines?.length ? ` · 跳过行：${result.skippedLines.map((e) => `${e.line} (${e.raw})`).join("，")}` : ""}`,
      );
      reload();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setBusy(false);
    }
  }
  async function suspend(card: Cards["rows"][number]) {
    setBusy(true);
    try {
      await api(`cards/${card.id}/suspend`, { suspended: !card.suspended });
      reload();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setBusy(false);
    }
  }
  async function save(form: FormData) {
    if (!editing) return;
    setBusy(true);
    try {
      await api(`cards/${editing.id}/text`, {
        meaning: String(form.get("meaning")),
        example: String(form.get("example")),
      });
      setEditing(undefined);
      reload();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <>
      <Heading eyebrow="YOUR WORDS / YOUR WORLD" title="词库">
        带上自己的词表，不需要从零开始。
      </Heading>
      <section className="card">
        <h2>导入词表</h2>
        <form action={upload}>
          <div className="fields three">
            <label>
              导入类型
              <select value={type} onChange={(e) => setType(e.target.value)}>
                <option value="cards">词卡 · CSV / 兼容 .apkg</option>
                <option value="wordlist">词表 · TXT / CSV</option>
              </select>
            </label>
            <label>
              {type === "cards" ? "牌组名称" : "词表名称"}
              <input
                name={type === "cards" ? "deck" : "name"}
                required
                maxLength={1000}
                defaultValue="我的词书"
              />
            </label>
            <label>
              导入文件
              <input
                type="file"
                name="file"
                accept={type === "cards" ? ".csv,.apkg" : ".txt,.csv"}
                required
              />
            </label>
            {type === "wordlist" && (
              <>
                <label>
                  词表用途
                  <select name="kind">
                    <option value="baseline">基线（已知，不建卡）</option>
                    <option value="study">学习词书（建卡）</option>
                  </select>
                </label>
                <label className="check">
                  <input type="checkbox" name="mergeIfExists" value="true" />
                  合并同名词表
                </label>
              </>
            )}
          </div>
          <p className="muted">
            仅本地导入，文本使用 UTF-8。CSV/TXT 最大 20 MiB；.apkg 最大 64 MiB。CSV
            列：word,meaning,example；新版 Anki 请用兼容旧版导出。
          </p>
          <button type="submit" disabled={busy}>
            开始导入
          </button>
        </form>
        <Notice error={actionError} message={message} />
      </section>
      <section className="section">
        <div className="section-title">
          <h2>我的词卡</h2>
          <span className="muted">{data?.total ?? "—"} 个词</span>
        </div>
        <form
          className="filters"
          onSubmit={(e) => {
            e.preventDefault();
            setOffset(0);
            const params = new URLSearchParams();
            if (query) params.set("q", query);
            if (deck) params.set("deck", deck);
            if (state) params.set("state", state);
            if (suspended) params.set("suspended", suspended);
            setFilter(`&${params}`);
          }}
        >
          <label>
            搜索词卡
            <input
              placeholder="输入一个词…"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </label>
          <label>
            牌组
            <input placeholder="全部牌组" value={deck} onChange={(e) => setDeck(e.target.value)} />
          </label>
          <label>
            状态
            <select value={state} onChange={(e) => setState(e.target.value)}>
              <option value="">全部状态</option>
              {Object.entries(stateLabels).map(([k, v]) => (
                <option key={k} value={k}>
                  {v}
                </option>
              ))}
            </select>
          </label>
          <label>
            暂停状态
            <select value={suspended} onChange={(e) => setSuspended(e.target.value)}>
              <option value="">全部</option>
              <option value="false">未暂停</option>
              <option value="true">已暂停</option>
            </select>
          </label>
          <button type="submit" className="secondary">
            筛选
          </button>
          {deck.trim() && (
            <button type="button" className="text-button" disabled={busy} onClick={removeDeck}>
              删除牌组「{deck.trim()}」
            </button>
          )}
        </form>
        <Notice error={error} />
        {!data ? (
          <Loading />
        ) : data.rows.length ? (
          <>
            <div className="table-wrap card">
              <table>
                <thead>
                  <tr>
                    <th>单词 / 释义</th>
                    <th>牌组</th>
                    <th>状态</th>
                    <th>下次到期</th>
                    <th>管理</th>
                  </tr>
                </thead>
                <tbody>
                  {data.rows.map((c) => (
                    <tr key={c.id}>
                      <td>
                        <strong lang="en">{c.front}</strong>
                        <p>
                          {c.back.meaning || "暂无释义"}{" "}
                          <span className="source">{sourceLabels[c.back.meaningSource]}</span>
                        </p>
                      </td>
                      <td>{c.deck}</td>
                      <td>
                        <span className="pill">
                          {c.suspended ? "已暂停" : stateLabels[c.srs.state]}
                        </span>
                      </td>
                      <td>{new Date(c.srs.due).toLocaleDateString("zh-CN")}</td>
                      <td>
                        <div className="actions">
                          <button
                            type="button"
                            className="text-button"
                            onClick={() => setEditing(c)}
                          >
                            编辑
                          </button>
                          <button
                            type="button"
                            disabled={busy}
                            className="text-button"
                            onClick={() => suspend(c)}
                          >
                            {c.suspended ? "恢复" : "暂停"}
                          </button>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <Pagination offset={offset} total={data.total} onChange={setOffset} />
          </>
        ) : (
          <Empty>还没有符合筛选条件的词卡。可以先导入一份自己的词表。</Empty>
        )}
      </section>
      {editing && (
        <Modal title={`编辑 · ${editing.front}`} onClose={() => setEditing(undefined)}>
          <form action={save}>
            <label>
              释义
              <textarea name="meaning" maxLength={1000} defaultValue={editing.back.meaning} />
            </label>
            <label>
              例句
              <textarea name="example" maxLength={5000} defaultValue={editing.back.example} />
            </label>
            <div className="actions">
              <button type="submit" disabled={busy}>
                保存词卡
              </button>
              <button type="button" className="secondary" onClick={() => setEditing(undefined)}>
                取消
              </button>
            </div>
          </form>
        </Modal>
      )}
    </>
  );
}
