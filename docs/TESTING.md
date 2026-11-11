# 验证与验收规范

当前已落地的测试与验证结果见 [IMPLEMENTATION](IMPLEMENTATION.md)。

## 1. 验证分层

| 层级 | 验证对象 | 验证方式 |
| --- | --- | --- |
| 领域层 (`cijian-domain`) | Anki 调度、FSRS 记忆状态、学习日/DST、KnownSet、选词、统计 | 固定时钟与随机种子、边界单元测试、64 组跨语言向量对照 |
| 存储与应用 (`cijian-storage` / `cijian-app`) | 事务原子性、命令幂等、事件追加、配额预留、旧库迁入与备份恢复 | 内存库 + 临时文件库、并发冲突、事件失败回滚、重开校验 |
| 服务适配 (`cijian-providers`) | OpenAI 兼容协议、单次 JSON 修复、调用上限、日志脱敏 | 本地 loopback stub 与 `cijian-testing` 替身 |
| 界面组件 (`cijian-ui`) | 导航、焦点、快捷键、草稿保护、弹窗遮罩、响应式布局 | GPUI 测试 harness 驱动实际组件交互 |
| 端到端与视觉 (`cijian-desktop`) | 完整学习闭环、浅深色主题、多窗口尺寸截图、Linux 打包冒烟 | Xvfb 无头环境下的端到端测试与截图巡检 |

## 2. 本机与 CI 的分工

本机负责**秒级开发反馈**，CI 负责**干净环境的可复核证据**（见 [D56](DECISIONS.md)）：

| 场景 | 预算 | 执行内容 |
| --- | --- | --- |
| 本机日常修改 | 秒级 | `cargo fmt --all --check`、单 crate `check`/`clippy`/单测、运行桌面应用 |
| 本机提交前 | ≤ 5 分钟 | 受影响 crate 完整测试、`python3 tools/check-rust-boundaries.py` |
| CI `verify`（每次 push/PR） | ≤ 15 分钟 | 单一 job 顺序执行：`fmt`、`clippy`、`xvfb-run -a cargo test --workspace`、边界与锁文件检查 |
| CI `acceptance`（nightly/tag） | ≤ 45 分钟 | release 构建、`tools/package-desktop.sh` 打包、Xvfb 启动冒烟与截图巡检 |

## 3. 常用验证命令

```sh
# 代码格式与架构边界检查
cargo fmt --all --check
python3 tools/check-rust-boundaries.py

# 单 crate 快速检查与测试
cargo clippy -p cijian-ui --all-targets -- -D warnings
cargo test -p cijian-ui --lib

# 全工作区静态检查与测试（需图形环境或 Xvfb）
cargo clippy --workspace --all-targets -- -D warnings
xvfb-run -a cargo test --workspace

# 运行桌面端（使用隔离测试目录）
cargo run -p cijian-desktop -- --data-dir /tmp/cijian-dev
```

旧版 Web 对照工程维护使用 `pnpm lint`、`pnpm typecheck`、`pnpm test`。
