"use client";
import { useState } from "react";
import { api, Notice } from "./ui";
export function Login() {
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  async function login(form: FormData) {
    setBusy(true);
    setError("");
    try {
      await api("auth", { password: String(form.get("password")) });
      window.location.assign("/today");
    } catch (err) {
      setError((err as Error).message);
      setBusy(false);
    }
  }
  return (
    <main id="main" className="login-space">
      <div className="card login-card">
        <span className="brand-mark">e.</span>
        <div className="eyebrow">A QUIET PLACE TO LEARN</div>
        <h1>欢迎回到词间。</h1>
        <p className="muted">这是你的私人学习空间。</p>
        <form action={login}>
          <label>
            访问密码
            <input
              name="password"
              type="password"
              autoComplete="current-password"
              required
              maxLength={1000}
            />
          </label>
          <Notice error={error} />
          <button type="submit" disabled={busy}>
            {busy ? "正在解锁…" : "解锁学习空间"}
          </button>
        </form>
      </div>
    </main>
  );
}
