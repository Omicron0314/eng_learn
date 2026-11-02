# 词间 · 多端英语学习 App

词间围绕「复习 → 阅读／听力 → 整理生词」组织学习。新的产品与界面基线是 [cijian-m3 设计稿](docs/cijian-m3.html)，重构采用 Rust、GPUI 与 [gpui-kit-material](https://github.com/mizorewww/gpui-kit-material)。

**当前处于文档重规划完成、原生重构尚未开始的阶段。** 仓库可运行的仍是 TypeScript / Next.js Web 版本；尚无 Cargo workspace。下文的目标架构不代表已经实现。

## 新规划入口

- [项目总纲](docs/plan.md)：范围、文档权威、实施顺序。
- [产品需求](docs/PRD.md)与[页面／交互规范](docs/UI-SPEC.md)：七个入口、完整学习流程和移动布局。
- [设计系统](docs/DESIGN-SYSTEM.md)：浅深色、排版、形状、组件与 GPUI 对应。
- [架构](docs/ARCHITECTURE.md)与[平台矩阵](docs/PLATFORMS.md)：共享 Rust 领域层、桌面、Web/WASM 与移动原生路线。
- [数据模型](docs/DATA_MODEL.md)、[核心规则](docs/SPEC-CORE.md)、[迁移方案](docs/MIGRATION.md)。
- [任务表](docs/BACKLOG.md)、[任务规格](docs/specs/README.md)、[验收方案](docs/TESTING.md)。

桌面端按 Linux → Windows / macOS 验证与交付；Web/WASM 承担浏览器和手机浏览器形态。Android / iOS 原生是目标，但上游材料不足以证明直接可用，必须先完成平台验证。多端可运行与跨端同步分别规划。

## 运行现有 Web 版本

以下命令属于现有实现，供迁移对照和维护使用。Node 24、pnpm 12.11.2：

```sh
pnpm install --frozen-lockfile
pnpm dev
# http://localhost:3000
```

在设置页配置自己的 OpenAI 兼容 LLM / TTS。配置缺失时相关在线操作报错，已有词卡复习等本地操作可用。产品不提供 fake / 演示 provider。数据库默认 `data/app.sqlite`，音频默认 `data/blobs`；不要提交数据库或密钥。现有 Web 的密钥存储与原生目标方案不同，见[迁移方案](docs/MIGRATION.md)。

```sh
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm e2e
pnpm report:experiment --db ./data/app.sqlite --window 7 --json
```

真实 AI 调用与 `pnpm eval:generation --allow-paid` 需明确授权；本次文档重规划没有运行。开发服务器日志写到 `/tmp`，避免仓库内日志写入触发 Turbopack 循环刷新。现有部署说明、命令历史和验证记录见 git 历史（提交 46f499a）中的 `docs/WEB-VALIDATION.md`，其中早期记录可能已被后续记录修正。

## 文档与资源

设计稿加载后的最终画面决定页面布局；其中示例词书、日期、数量、模型占位符不是真实产品数据。`docs/neko.html` 仅辅助理解视觉语言。两份设计稿原文件保留。

旧 M0–M6 文档与任务已归档，新任务使用 R00–R21，状态从未开始重新计算。现有代码、迁移、提示词和测试样例保留为迁移资产；许可证、词表和字体来源见 [licenses](docs/licenses.md)。
