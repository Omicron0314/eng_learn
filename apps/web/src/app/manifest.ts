import type { MetadataRoute } from "next";
export default function manifest(): MetadataRoute.Manifest {
  return {
    name: "词间 · 英语学习",
    short_name: "词间",
    description: "把单词放回生活。",
    start_url: "/today",
    display: "standalone",
    background_color: "#f7f6f1",
    theme_color: "#245c4c",
    lang: "zh-CN",
    icons: [
      { src: "/icon.svg", sizes: "any", type: "image/svg+xml", purpose: "any" },
      { src: "/icon.svg", sizes: "any", type: "image/svg+xml", purpose: "maskable" },
    ],
  };
}
