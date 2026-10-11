"use client";
import { type ReactNode, useEffect, useState } from "react";
export function cn(...values: (string | false | undefined)[]) {
  return values.filter(Boolean).join(" ");
}
export async function api<T>(path: string, body?: unknown, method = "POST"): Promise<T> {
  const response = await fetch(
    `/api/${path}`,
    body === undefined
      ? { cache: "no-store" }
      : {
          method,
          headers: body instanceof FormData ? {} : { "Content-Type": "application/json" },
          body: body instanceof FormData ? body : JSON.stringify(body),
        },
  );
  const data = await response.json();
  if (!response.ok) {
    if (response.status === 401 && path !== "auth") window.location.assign("/login");
    throw new Error(data.error?.message ?? "操作失败，请重试");
  }
  return data as T;
}
export function useLoad<T>(path: string) {
  const [data, setData] = useState<T>();
  const [error, setError] = useState("");
  const [version, setVersion] = useState(0);
  useEffect(() => {
    void version;
    let active = true;
    setError("");
    api<T>(path)
      .then((value) => {
        if (active) setData(value);
      })
      .catch((err) => {
        if (active) setError(err.message);
      });
    return () => {
      active = false;
    };
  }, [path, version]);
  return { data, error, setData, reload: () => setVersion((v) => v + 1) };
}
export function Notice({ error, message }: { error?: string; message?: string }) {
  return error ? (
    <div className="notice error" role="alert">
      {error}
    </div>
  ) : message ? (
    <div className="notice" role="status">
      {message}
    </div>
  ) : null;
}
export function Loading() {
  return (
    <div className="loading" role="status">
      正在整理你的学习空间…
    </div>
  );
}
export function Heading({
  eyebrow,
  title,
  children,
}: {
  eyebrow: string;
  title: string;
  children?: ReactNode;
}) {
  return (
    <header className="page-heading">
      <div className="eyebrow">{eyebrow}</div>
      <h1>{title}</h1>
      {children && <p>{children}</p>}
    </header>
  );
}
export function Empty({ children }: { children: ReactNode }) {
  return (
    <div className="empty">
      <span aria-hidden="true">✧</span>
      <p>{children}</p>
    </div>
  );
}
export function Pagination({
  offset,
  total,
  onChange,
}: {
  offset: number;
  total: number;
  onChange: (v: number) => void;
}) {
  return (
    <div className="pagination">
      <button
        type="button"
        className="secondary"
        disabled={offset === 0}
        onClick={() => onChange(Math.max(0, offset - 50))}
      >
        上一页
      </button>
      <span>{total ? `${offset + 1}–${Math.min(offset + 50, total)} / ${total}` : "0 / 0"}</span>
      <button
        type="submit"
        className="secondary"
        disabled={offset + 50 >= total}
        onClick={() => onChange(offset + 50)}
      >
        下一页
      </button>
    </div>
  );
}
export const tierLabels = { easy: "轻松 · ≥98%", medium: "适中 · 95–98%", hard: "挑战 · 90–95%" };
export const genreLabels = { story: "短故事", dialogue: "对话", monologue: "独白" };
export const stateLabels = {
  new: "新词",
  learning: "学习中",
  review: "复习",
  relearning: "重新学习",
};
export const sourceLabels = { import: "导入", ai: "AI 生成", user: "用户编辑" };
export function percent(value: number | null) {
  return value === null ? "—" : `${(value * 100).toFixed(1)}%`;
}
