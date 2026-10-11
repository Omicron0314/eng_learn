import type { Metadata, Viewport } from "next";
import { Shell } from "../components/Shell";
import "./globals.css";
export const metadata: Metadata = {
  title: { default: "词间 · 英语学习", template: "%s · 词间" },
  description: "词表、复习、阅读与听力，在一个安静的学习空间里。",
  manifest: "/manifest.webmanifest",
  icons: { icon: "/icon.svg", apple: "/icon.svg" },
};
export const viewport: Viewport = { width: "device-width", initialScale: 1, themeColor: "#245c4c" };
export default function Layout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="zh-CN">
      <body>
        <a className="skip-link" href="#main">
          跳到主要内容
        </a>
        <Shell>{children}</Shell>
      </body>
    </html>
  );
}
