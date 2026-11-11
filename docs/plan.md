# 词间重构总纲

当前基线为 Rust + GPUI + `gpui-kit-material` 桌面端实现，界面与交互以 [cijian-m3.html](cijian-m3.html) 执行末尾脚本后的画板为准（[neko.html](neko.html) 仅作视觉参考）。当前实施进度与待收口项见 [IMPLEMENTATION](IMPLEMENTATION.md)。

## 1. 文档导航与职责

**权威优先级**：用户最新要求 > `cijian-m3.html` 执行脚本后的设计 > 当前规格文档 > 旧版 Web 实现与历史归档。

| 文档 | 负责内容 |
| --- | --- |
| [PRD](PRD.md) | 产品目标、核心学习体验与功能边界 |
| [UI-SPEC](UI-SPEC.md) | 页面布局、交互状态与设计画板映射 |
| [DESIGN-SYSTEM](DESIGN-SYSTEM.md) | 色彩角色、排版尺寸、组件映射与可访问性 |
| [SPEC-CORE](SPEC-CORE.md) | 默认配置、Anki/FSRS 调度、选词生成与统计口径（唯一维护处） |
| [ARCHITECTURE](ARCHITECTURE.md) | Rust Workspace 分层、模块职责与应用接口 |
| [DATA_MODEL](DATA_MODEL.md) | SQLite 表结构、事件目录与事务边界 |
| [PLATFORMS](PLATFORMS.md) | 平台支持范围、能力接口与降级策略 |
| [MIGRATION](MIGRATION.md) | 旧版 Web 数据迁入、逻辑备份与恢复方案 |
| [BACKLOG](BACKLOG.md) / [specs](specs/README.md) | 任务依赖、状态（唯一维护处）与分项规格 |
| [IMPLEMENTATION](IMPLEMENTATION.md) | 当前已落地实现与待补代码缺口 |
| [TESTING](TESTING.md) | 测试分层、本机与 CI 分工、验收命令 |
| [ROADMAP](ROADMAP.md) | 阶段划分与远期规划 |
| [DECISIONS](DECISIONS.md) | 关键产品与技术架构决策（Dxx） |
| [licenses](licenses.md) | 字体、词表与第三方资源许可 |

## 2. 产品与学习链路

词间是以本地数据为中心的英语学习桌面应用。主导航固定为：**今日、复习、文章、词库、生词箱、统计**；设置独立置于导航底部（窄屏置于顶部）。

完整学习闭环：
1. **词库导入**：导入自有词书（CSV/TXT/APKG）或含双口音录音的词包（`.cijianpack`）。
2. **间隔复习**：按 Anki 步进与 ease 调度复习词卡，FSRS 评估记忆可提取率，支持英/美发音点播与换卡自动朗读。
3. **文章生成与阅读/先听**：在文章工作区按目标词与已知词集合生成短文，支持语境查词、朗读与难度/不自然句反馈。
4. **生词整理与统计**：在生词箱批量加入复习或忽略，在今日与统计页查看热力图、覆盖率趋势与对照实验。

数据保存在本地设备；在线 LLM 为可配置能力，整篇朗读使用本机 Kokoro 模型（D64），未配置 AI 或未装模型都不阻塞离线导入、复习与已有音频播放。

## 3. 技术栈与交付边界（统一说明）

- **技术栈**：纯 Rust 领域层（`cijian-domain`）+ 应用服务（`cijian-app`）+ SQLite 存储（`cijian-storage`）+ GPUI / `gpui-kit-material` 界面（`cijian-ui`）。
- **当前交付范围**：仅交付 **Linux 桌面端**（任务 `R00–R17`、`R22`）。
- **远期搁置（不排期、不阻塞交付）**：Web/WASM（`R18`）、Android/iOS 原生（`R19–R20`）、跨端同步（`R21`），相关草案已归档至 `docs/archive/`（见 [D47](DECISIONS.md)）。
- **不在验收范围内**：Windows/macOS、真实中文 IME、触摸、读屏、人工听感、付费真实模型生成质量（见 [D54](DECISIONS.md)）。
- **验证原则**：本机运行用于秒级开发反馈，干净环境的 Linux CI 用于产出最终验收证据（见 [D56](DECISIONS.md)）。`prompts/*.v1.md` 为运行时资产，未经版本化评测不改动语义。
