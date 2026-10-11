"use client";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { type ReactNode, useEffect, useState } from "react";
import { api, cn } from "./ui";

const links = [
  ["today", "今日", "◈"],
  ["review", "复习", "▣"],
  ["library", "词库", "▤"],
  ["inbox", "生词箱", "◇"],
  ["stats", "统计", "↗"],
  ["settings", "设置", "⚙"],
] as const;
export function Shell({ children }: { children: ReactNode }) {
  const pathname = usePathname();
  const [pending, setPending] = useState(0);
  const [demo, setDemo] = useState(false);
  useEffect(() => {
    if (pathname === "/login") return;
    const refresh = () => {
      api<unknown[]>("inbox")
        .then((rows) => setPending(rows.length))
        .catch(() => {});
    };
    refresh();
    api<{ providers: { llm: { provider: string } } }>("settings")
      .then((view) => setDemo(view.providers.llm.provider === "fake"))
      .catch(() => {});
    window.addEventListener("inbox-changed", refresh);
    return () => window.removeEventListener("inbox-changed", refresh);
  }, [pathname]);
  if (pathname === "/login") return children;
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <Link href="/today" className="brand">
          <span className="brand-mark">e.</span>
          <span>
            词间<span className="brand-sub">WORDS INTO LIFE</span>
          </span>
        </Link>
        <div className="nav-caption">你的学习空间</div>
        <nav aria-label="主导航">
          {links.map(([href, label, icon]) => (
            <Link
              key={href}
              href={`/${href}`}
              className={cn("nav-item", pathname.startsWith(`/${href}`) && "active")}
              aria-current={pathname.startsWith(`/${href}`) ? "page" : undefined}
            >
              <span aria-hidden="true">{icon}</span>
              {label}
              {href === "inbox" && pending > 0 && <span className="badge">{pending}</span>}
            </Link>
          ))}
        </nav>
        <div className="sidebar-note">
          <span className="dot" /> 本地优先 · 慢慢积累<p>让认识的词，成为熟悉的世界。</p>
        </div>
      </aside>
      <div className="workspace">
        <div className="topbar">
          <span>一点复习，一段故事。</span>
          <span className="local-tag">SELF-USE / MVP</span>
        </div>
        <main id="main">
          {demo && (
            <div className="demo-banner">
              演示模式 · 假服务，无付费调用；文章与释义不代表真实质量。
            </div>
          )}
          {children}
        </main>
        <footer>词间 · 由你自己的词汇，构成每一天</footer>
      </div>
    </div>
  );
}
