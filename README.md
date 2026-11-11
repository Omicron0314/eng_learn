# 词间 · 多端英语学习 App

词间围绕「复习 → 阅读／听力 → 整理生词」组织学习。新的产品与界面基线是 [cijian-m3 设计稿](docs/cijian-m3.html)，重构采用 Rust、GPUI 与 [gpui-kit-material](https://github.com/mizorewww/gpui-kit-material)。

**当前已有可运行的 Rust/GPUI 桌面端与 Cargo workspace，界面按 `docs/cijian-m3.html` 重做；TypeScript / Next.js Web 版本仍保留为迁移对照。**

## 文档索引

- [项目总纲](docs/plan.md)：范围、文档权威、实施顺序。
- [产品需求](docs/PRD.md)与[页面／交互规范](docs/UI-SPEC.md)：七个入口与完整学习流程。
- [设计系统](docs/DESIGN-SYSTEM.md)：浅深色、排版、形状、组件与 GPUI 对应。
- [架构](docs/ARCHITECTURE.md)与[平台矩阵](docs/PLATFORMS.md)：共享 Rust 领域层与桌面路线。
- [数据模型](docs/DATA_MODEL.md)、[核心规则](docs/SPEC-CORE.md)、[迁移方案](docs/MIGRATION.md)。
- [任务表](docs/BACKLOG.md)、[实施现状](docs/IMPLEMENTATION.md)、[任务规格](docs/specs/README.md)、[验收方案](docs/TESTING.md)。

## 运行桌面版本

Rust workspace 使用锁定工具链。开发运行：

```sh
cargo run -p cijian-desktop
# 隔离学习库：
cargo run -p cijian-desktop -- --data-dir /tmp/cijian-dev
```

整篇朗读使用本机 Kokoro-82M（`model_q8f16.onnx`），不联网、不计字符额度。首次使用前安装到用户目录（需要 `uv`，脚本按固定 SHA-256 校验权重并自检英／美两个音色）：

```sh
python3 tools/install-kokoro.py
```

未安装时点击播放会提示模型不可用，其余离线功能照常。复习卡的单词发音随词书提供。导入纯词表后需要补齐英／美录音时，先关闭应用再运行维护工具（它会访问有道词典音频与条目接口）：

```sh
# 补齐已有词卡的英/美录音
cargo run -p cijian-fill -- fill --data-dir ~/.local/share/cijian
cargo run -p cijian-fill -- fill --data-dir ~/.local/share/cijian --limit 20  # 先试跑
cargo run -p cijian-fill -- fill --data-dir ~/.local/share/cijian --dry-run   # 只列待补词

# 生成可分发的词包（先补齐，再打包）
cargo run -p cijian-fill -- pack --data-dir ~/.local/share/cijian --output ~/cijian-packs/cet4.cijianpack

# 无头导入词包（应用内也可通过导入对话框选择 .cijianpack）
cargo run -p cijian-fill -- import-pack --data-dir ~/.local/share/cijian ~/cijian-packs/cet4.cijianpack
```

工具可中断续跑：每写一条即落库，重跑只处理仍缺口音的词。词包是自包含的（清单 + 词表 + 双口音录音），导入前会校验每条录音的 sha256 与容器格式，损坏的包整包拒绝。

构建产物统一落在仓库根 `target/`（cargo 默认目录，见 D58），与 `cargo test/clippy`、rust-analyzer 共用同一份缓存，不需要设置 `CARGO_TARGET_DIR`。

每次提交涉及桌面端代码后，都必须运行下面的脚本重新构建并关联启动器，否则点击启动器仍会打开旧版本的产物。脚本同时用于日常构建：

```sh
tools/build-desktop.sh            # debug，默认
tools/build-desktop.sh --release  # release，交付前检查
```

脚本会构建对应 profile，并把 `~/.local/bin/cijian-desktop` 重新连接为指向该构建产物的符号链接，同时刷新 `~/.local/share/applications/cijian.desktop` 与图标缓存。启动器默认连接 debug 构建；之后只要重新构建，点击启动器就会直接运行新产物，无需再次运行脚本。`cargo clean` 后重新运行 `tools/build-desktop.sh` 重接即可。

发布/交付时使用 `tools/package-desktop.sh`：它构建 release 并刷新 `dist/cijian-desktop-linux-x86_64.tar.gz`，但不会改动启动器；启动器保持指向 debug 构建。

```sh
tools/package-desktop.sh
```

学习数据默认写入 `$XDG_DATA_HOME/org.cijian.cijian`；桌面包未签名、无自动更新，本地朗读模型与凭据都不随包分发。

## 运行现有 Web 版本

以下命令属于现有实现，供迁移对照和维护使用。Node 24、pnpm 12.11.2：

```sh
pnpm install --frozen-lockfile
pnpm dev
# http://localhost:3000
```

在设置页配置自己的 OpenAI 兼容 LLM。整篇朗读由本机 Kokoro 模型承担，先运行 `python3 tools/install-kokoro.py`。配置缺失时相关在线操作报错，已有词卡复习等本地操作可用。产品不提供 fake / 演示 provider。数据库默认 `data/app.sqlite`，音频默认 `data/blobs`；不要提交数据库或密钥。现有 Web 的密钥存储与原生目标方案不同，见[迁移方案](docs/MIGRATION.md)。

```sh
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm e2e
pnpm report:experiment --db ./data/app.sqlite --window 7 --json
```

`pnpm eval:generation --allow-paid` 会发起真实付费调用，需明确授权后再执行。开发服务器日志建议写到 `/tmp`，避免仓库内日志写入触发 Turbopack 循环刷新。

## 设计稿与许可

设计稿 `docs/cijian-m3.html` 加载后的最终画面决定页面布局；`docs/neko.html` 仅辅助理解视觉语言。第三方许可证、词表和字体来源见 [licenses](docs/licenses.md)。
