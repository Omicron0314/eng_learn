# 验证与验收

本文件定义实现的验证要求。Rust 工程已进入实现与本地验证，已执行证据见 [IMPLEMENTATION](IMPLEMENTATION.md)。未执行真实付费 AI 评测或用户数据迁移。旧 Web 历史证据见 git 历史（提交 46f499a），不得转写为新端通过。

## 验证分层

| 层 | 验证对象 | 必要证据 |
| --- | --- | --- |
| 文档 | 画板→规格→任务、依赖顺序、链接、现状／目标区分 | 本地链接有效、每个R任务有spec、状态一致、无循环 |
| domain | FSRS 记忆状态／Anki 调度／时间／KnownSet／选词／统计 | 固定时钟随机种子、边界样例、跨语言向量 |
| storage/app | 事务、幂等、事件、配额、迁移 | 内存库＋临时文件库、失败注入、重开、重复命令 |
| providers | JSON输出、修复、超时、预算、配置 | 本地协议stub与test-only doubles，每个请求有日志 |
| GPUI UI | 焦点、快捷键、选择、弹层、布局、任务状态 | Kit/GPUI测试harness驱动实际组件；稳定可访问名称 |
| 视觉 | 浅深色、字体、布局、状态 | 与最终设计稿画板对照的固定数据截图 |
| 平台（Linux） | 渲染后端、音频解码、文件 API、凭据服务可用性 | CI 在 Xvfb 下运行；软件渲染需注明，能力缺失记为 unknown |
| 使用质量（不在本轮 AC） | 自然度、可读性、听力与长期效果 | 另记，不作为任务完成条件 |

## 自动化重点

- 调度：四档、未来due拒绝、New/Review限额为零、learning例外、暂停与归档、旧版本重提、多窗口并发。
- 时间：学习日4点、跨年／周界、DST缺失／重叠、换时区、历史重分组。
- 文本：原文可无损拼回、UTF-8／UTF-16转换、缩写／音标／短语、词元位置、空分母。
- 生成：无候选、control不泄漏、自动轮换、非法JSON修复、覆盖率重写硬上限、取消／重启不重复请求、所有调用落日志。
- 标记：释义失败可操作、markOnly不建卡、autoAdd幂等、已暂停卡不自动恢复、批量失败整批回滚。
- 音频：相同配置命中缓存、换voice不命中、缺blob重新预留、失败不退、并发不超额、0额度仍播缓存、播放会话完成去重。
- 导入：类型／用途／目标、字段别名、同名合并、重复词不覆盖用户释义、合法行导入与DB整体回滚、资源错误可恢复。
- 统计：首页与统计对账、无样本null、reading/listening各取最后一次、实验首次评分、旧／新实验分组隔离。
- 数据：schema升级、只读旧库、WAL快照、凭据脱敏、恢复到新profile、中断不损源库、未知版本拒绝。

## UI与视觉场景

验收画板按 [UI-SPEC](UI-SPEC.md) 全表逐项勾验。至少录制以下完整链路：

1. 空库→导入100词→词库搜索／编辑／暂停→按当天20新词限制完成复习→重启仍一致。
2. 用本地协议stub生成→先听→阅读→点词三动作→批量整理→再次进复习；每步持久化与事件对账。
3. 生成／释义／音频失败、额度耗尽、取消、切词／切页、长表单未保存、词书删除确认。
4. 浅／深主题与1440×900、1024×768、412×892、360×800；100/150/200%字体和长词书名；截图由 CI 产出。

对照的是脚本重组后的设计：首页无生成器，文章页有生成器，阅读返回文章。不要把HTML查看器工具栏截进产品基准。原生截图不能用浏览器DOM截图代替；Web/WASM可使用Playwright，但canvas内操作需可访问桥／测试接口，不能假定普通DOM选择器能找到每个GPUI元素。

## 本机与 CI 的分工

核心区别不是“重要程度”，而是**信任度与反馈速度**：本机检查可以跳过（`--no-verify`）、会读到未提交改动、会依赖旧的 `target`；CI 从干净 clone 出发，只跑已提交的内容，但结果要等。因此本机负责**秒级反馈**，CI 负责**可复核的证据**（[SpecStory](https://specstory.com/learning/ci-cd/pre-commit-vs-pre-push-vs-ci)）。反过来，把全部检查推到 CI 会让反馈变成 10 分钟起，开发会绕开它（[CI/CD for beginners](https://hackmamba.io/engineering/a-simple-guide-to-ci-cd-for-total-beginners-in-2022/)）。

| 位置 | 时间预算 | 只放这些 | 不放这些 |
| --- | --- | --- | --- |
| 本机：每次改动 | 秒级 | `fmt --check`、单 crate `check`/`clippy`、单模块测试、跑应用看界面、设计迭代截图 | 全 workspace 测试、release 构建、故障注入 |
| 本机：提交前 | ≤ 5 分钟 | 受影响 crate 的完整测试、边界检查脚本 | 依赖全新解析、性能数字 |
| CI verify：每次 push/PR | 10–15 分钟 | 干净 clone 的 fmt／clippy／test、空库迁移、锁文件解析、边界检查、快速故障注入 | release 构建、打包、截图巡检 |
| CI nightly／tag | 30–45 分钟 | release 性能、打包与启动冒烟、慢测试、截图巡检 | 真实设备与人工判断项 |

不在任何一列：真实 IME／触摸／读屏、Windows／macOS、付费模型质量、人工听感。它们既不在本机也不在 CI，已从 AC 删除（[D54](DECISIONS.md)）。

三条约束：

1. **CI 的每一步必须能用一条本机命令复现**，不允许只存在于 YAML 的逻辑。否则本机无法debug，CI 一旦红了只能猜。
2. **编译时间占大头**，所以 verify 做成**一个 job** 顺序跑，不拆成五个并行 job；拆开是各自冷编译一遍，还会撞 GitHub 的 10 GB 缓存上限（[rust-cache](https://github.com/Swatinem/rust-cache)、[dependency caching](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching)）。
3. **拉取请求之间不共享缓存**，但都读基分支缓存，所以基分支也要跑一次 verify 来暖缓存。

## 本机命令（秒级反馈）

```sh
# 改完立刻跑
cargo fmt --all --check
cargo check -p cijian-storage --all-targets
cargo clippy -p cijian-storage --all-targets -- -D warnings
cargo test -p cijian-storage <filter>
python3 tools/check-rust-boundaries.py

# 动到共享层或准备提交时
cargo test --workspace

# 看界面与交互
cargo run -p cijian-desktop -- --data-dir /tmp/cijian-dev
```

旧 Web 工程维护用 `pnpm lint`、`pnpm typecheck`、`pnpm test`；改到 Web 运行行为才加 build／e2e。文档修改不重跑产品测试。

## CI 作业

**verify（每次 push/PR，目标 ≤ 15 分钟，单一 job）**

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
xvfb-run -a cargo test --workspace
python3 tools/check-rust-boundaries.py
```

空库迁移、事件回滚、中断任务状态已由 workspace 测试覆盖（`upgrades_initial_database_without_rewriting_its_migration`、`event_failure_rolls_back_settings_and_receipt`、`interrupted_jobs_keep_snapshot_without_retry`），不需要独立步骤。边界脚本内部执行 `cargo metadata --locked`，同时验证锁文件可解析。

**acceptance（非 PR 事件与每晚，目标 ≤ 45 分钟）**

release 构建与 `tools/package-desktop.sh`、Xvfb 启动冒烟（进程存活 20 秒即 timeout 124）、上传 `dist` 产物。

**wasm-consumer** 仅手动触发；WASM 属远期。

**尚未接入**（需要先补代码，测试存在前不进 CI，也不计作证据）：

- 真实 ENOSPC（`tmpfs`）与 `SIGKILL` 中断注入 —— R16
- 30 篇以上文章、50 条以上已学词的分页测试 —— R09／R14
- release 性能采样与截图巡检 —— R17（截图还缺自动驱动）

**首次远端运行尚未发生**；上面的时间与冒烟判据是目标，不是实测。

视觉截图是唯一例外：自动驱动落地前允许本机 Xvfb 截图作为视觉证据，但必须在记录里标注来源；其余证据仍以 CI 为准。

## 性能与打包

性能验收在 CI runner 上测量并记录机型与 release 基线。初始目标：1万词可搜索／分页且输入不被IO阻塞；滚动与普通交互p95帧时≤16.7ms，空闲无持续重绘；超出则带trace定位。它们是词间目标，不是本次实测，更不能把上游120fps宣传直接当结果。

## AI与发布证据

Prompt语义变化须有版本、旧／新同样例离线评测；真实评测单独显式授权后执行，按配置单价预算预留并限制底层调用。未知价格记录null，不记0。TTS语音质量、系统语音能力与付费费用不能由静音样例证明。

每任务交付记录 CI 运行结果、测试／截图位置、已知缺口。打包在 CI 构建；签名、上传商店、公开发布等另按用户授权执行。本轮只有 Linux 一个目标平台，不存在多平台勾选。
