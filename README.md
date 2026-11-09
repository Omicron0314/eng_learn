# 词间 · 多端英语学习 App

词间围绕「复习 → 阅读／听力 → 整理生词」组织学习。新的产品与界面基线是 [cijian-m3 设计稿](docs/cijian-m3.html)，重构采用 Rust、GPUI 与 [gpui-kit-material](https://github.com/mizorewww/gpui-kit-material)。

**当前已有可运行的 Rust/GPUI 桌面端与 Cargo workspace，界面按 `docs/cijian-m3.html` 重做；TypeScript / Next.js Web 版本仍保留为迁移对照。** 桌面端验收仍在进行，验收证据由 CI 产出。真实 IME、触摸与付费模型质量不在本轮范围；Web/WASM、移动原生与跨端同步为远期规划。

## 新规划入口

- [项目总纲](docs/plan.md)：范围、文档权威、实施顺序。
- [产品需求](docs/PRD.md)与[页面／交互规范](docs/UI-SPEC.md)：七个入口与完整学习流程；移动布局为远期参考。
- [设计系统](docs/DESIGN-SYSTEM.md)：浅深色、排版、形状、组件与 GPUI 对应。
- [架构](docs/ARCHITECTURE.md)与[平台矩阵](docs/PLATFORMS.md)：共享 Rust 领域层与桌面路线；Web/WASM 与移动原生只作远期设计。
- [数据模型](docs/DATA_MODEL.md)、[核心规则](docs/SPEC-CORE.md)、[迁移方案](docs/MIGRATION.md)。
- [任务表](docs/BACKLOG.md)、[任务规格](docs/specs/README.md)、[验收方案](docs/TESTING.md)。

本轮只交付 Linux 桌面，验收证据由 CI 产出；Windows／macOS 不做。Web/WASM、移动原生与跨端同步为远期规划，不排期、不阻塞桌面交付。

## 运行桌面版本

Rust workspace 使用锁定工具链。开发运行：

```sh
cargo run -p cijian-desktop
# 隔离学习库：
cargo run -p cijian-desktop -- --data-dir /tmp/cijian-dev
```

复习卡的单词发音随词书提供。导入纯词表后需要补齐英／美录音时，先关闭应用再运行维护工具（它会访问有道词典音频与条目接口，不占远程 TTS 额度）：

```sh
cargo run -p cijian-fill -- --data-dir ~/.local/share/cijian            # 补齐所有缺失词
cargo run -p cijian-fill -- --data-dir ~/.local/share/cijian --limit 20 # 先试跑
cargo run -p cijian-fill -- --data-dir ~/.local/share/cijian --dry-run  # 只列待补词
```

工具可中断续跑：每写一条即落库，重跑只处理仍缺口音的词。

构建产物统一落在仓库根 `target/`（cargo 默认目录，见 D58），与 `cargo test/clippy`、rust-analyzer 共用同一份缓存，不需要设置 `CARGO_TARGET_DIR`。

每次构建并希望启动器立即打开新版本时，使用：

```sh
tools/build-desktop.sh            # debug，默认
tools/build-desktop.sh --release  # release，交付前检查
```

脚本会构建对应 profile，并把 `~/.local/bin/cijian-desktop` 重新连接为指向该构建产物的符号链接，同时刷新 `~/.local/share/applications/cijian.desktop` 与图标缓存。启动器默认连接 debug 构建；之后只要重新构建，点击启动器就会直接运行新产物，无需再次运行脚本。`cargo clean` 后重新运行 `tools/build-desktop.sh` 重接即可。

发布/交付时使用 `tools/package-desktop.sh`：它构建 release 并刷新 `dist/cijian-desktop-linux-x86_64.tar.gz`，但不会改动启动器；启动器保持指向 debug 构建。

```sh
tools/package-desktop.sh
```

学习数据默认写入 `$XDG_DATA_HOME/org.cijian.cijian`；桌面包未签名、无自动更新，真实模型和 TTS 凭据不随包分发。

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

旧 M0–M6 文档与任务已归档；本轮任务为 R00–R17，R18–R21 属远期规划，状态从未开始重新计算。现有代码、迁移、提示词和测试样例保留为迁移资产；许可证、词表和字体来源见 [licenses](docs/licenses.md)。
