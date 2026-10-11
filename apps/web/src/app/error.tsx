"use client";
export default function ErrorPage({ reset }: { reset: () => void }) {
  return (
    <section className="card">
      <h1>暂时无法打开</h1>
      <p>请检查配置后重试。详细配置状态见设置页。</p>
      <button type="button" onClick={reset}>
        重试
      </button>
    </section>
  );
}
