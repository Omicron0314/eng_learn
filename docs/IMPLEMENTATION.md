# 实施现状与验证记录

本文汇总 Rust/GPUI 桌面端的当前工程状态、各模块已落地能力及剩余代码/测试缺口。

## 工程基线

- **工具链与依赖**：Rust `1.99.1`，锁定 `gpui-kit` 与 `gpui-kit-material` revisions 及 workspace 根补丁。
- **分层结构**：`cijian-domain`（纯规则）→ `cijian-ports`（契约）→ `cijian-app`（用例与命令）→ `cijian-storage` / `cijian-providers` / `cijian-platform`（适配器）→ `cijian-ui` / `cijian-desktop`（界面与桌面入口）。`tools/check-rust-boundaries.py` 强制检查领域层零 IO 与测试替身隔离。
- **构建与缓存**：所有本地命令与脚本统一共用仓库根 `target/`（见 [D58](DECISIONS.md)）。
  - `tools/build-desktop.sh`：构建（默认 debug，`--release` 可选）并将 `~/.local/bin/cijian-desktop` 符号链接指向产物，同时刷新桌面启动器。
  - `tools/package-desktop.sh`：构建 release 并生成 `dist/cijian-desktop-linux-x86_64.tar.gz`，不改动开发启动器。

## 模块实现状态与待补缺口

| 模块 / 任务 | 已落地实现 | 待补代码或测试缺口 |
| --- | --- | --- |
| 基础与工作区 (`R00–R01`) | 原生/WASM 最小消费者、锁定补丁、workspace、48dp 自定义标题栏（Linux 最小化/全屏/关闭） | CI 远端流水线首次全量运行 |
| 领域与调度 (`R02`) | 设置校验、IANA 时区学习日/DST、Anki 步进（`1m/10m`、relearn `10m`、learn-ahead `20m`、MixWithReviews）与 Anki ease 复习间隔、FSRS 记忆状态更新（64 组向量对照） | 无 |
| 存储与事务 (`R03`) | SQLite 独立迁移（`0001`–`0008`）、业务写与 `events`/`command_receipts` 同事务、revision 并发保护、append-only 日志 | 资源故障矩阵并入 CI |
| 界面壳层与设计 (`R04`) | 按 `cijian-m3.html` 重做：96dp 导航轨、28dp 面板、浅/深/跟随系统主题、Roboto Flex/Mono 静态字体实例、4 秒自动消失 Snackbar | CI 巡检截图自动产出 |
| AI 服务配置 (`R05`) | OpenAI 兼容 LLM 适配、调用日志、系统凭据库优先 + 会话内存回退、禁止隐式重试、单次 JSON 修复 | 朗读已改为本地模型（D64），设置页不再有 TTS 端点／密钥／额度 |
| 词库与导入 (`R06`) | CSV/TXT/APKG 流式导入、`.cijianpack` 词包整包校验导入、基线/学习词书、组合筛选、编辑侧栏草稿保护、删书归档预览 | **代码缺口**：超大导入批次的归一化内存尚无固定上限 |
| 复习队列 (`R07`) | 到期学习/复习/新词交错队列、正反面与 `Space`/`1–4` 快捷键、缺释义首次翻面按需生成并回填、失败不阻塞评分 | 无 |
| 词元分析与生成 (`R08–R09`) | 分词/区间/覆盖率、KnownSet、目标词与对照词抽样、生成任务持久化与取消保護、今天/历史/日期筛选与已读状态 | **测试缺口**：30 篇以上历史文章分页的自动化测试 |
| 阅读与生词箱 (`R10–R11`) | 正文 `body_hash` 绑定、点词 side/bottom sheet、认识/不认识/加入复习、阅读位置恢复、多句不自然反馈、生词箱多选原子批处理 | 无 |
| 音频与朗读 (`R12`) | 本地 Kokoro q8f16 全文合成与内容寻址缓存、英美双口音缓存、原生 WAV 播放（暂停/定位/自然结束）、WSOLA 变速不变调、安装脚本与校验 | 设置页不再提供远程 TTS 端点／密钥／字符额度；CI 只验证未安装时的降级路径 |
| 单词发音与词包 (`R22`) | `word_audio` 与英/美音标、复习卡音标点播与英美切换、换卡自动朗读、设置页“学习偏好”卡片、独立播放 sink、`cijian-fill` 补齐/打包工具 | **代码缺口**：词包音频转码压缩、导入时优先保留词书自带音标 |
| 今日与统计 (`R13–R14`) | 今日队列与周配额、13×7 学习热力图及日期详情、覆盖率趋势、7 天对照实验统计、已学词分页、阅读完成/反馈后即时刷新 | **测试缺口**：50 条以上已学词分页的自动化测试 |
| 设置与偏好 (`R15`) | 学习设置与设备偏好分离、AI 页 LLM 单卡常驻编辑与保存、独立清除密钥、后台轮询与即时偏好写入走 `Command::is_quiet()` 免闪烁 | 无 |
| 迁移与备份 (`R16`) | 旧 Web 库 WAL 只读一致性快照转换、v3 逻辑备份（可选含逐文件 SHA-256 校验的音频 blob）与隔离恢复 | **测试缺口**：真实 `ENOSPC`（`tmpfs`）与 `SIGKILL` 中断注入测试 |
| 桌面验收 (`R17`) | Linux 打包脚本、Xvfb 全功能巡检与桌面端到端测试 | **代码缺口**：截图自动驱动与 release 性能采样 |

## 本地验证证据索引

以下记录位于 `docs/evidence/rust/` 与 `docs/spikes/evidence/r00/`，供本地复核参考：

- `docs/spikes/R00-platform.md`：GPUI 消费者依赖锁定、SQLite/音频/FSRS 基础探针记录。
- `docs/evidence/rust/local-model-e2e.json`：使用 loopback stub 验证导入 → 复习 → 生成 → 阅读点词 → 反馈的完整事务与日志链路。
- `docs/evidence/rust/native-audio.json` / `native-speech.json`：本机音频输出（暂停/定位/变速/自然结束）与 `espeak-ng` 合成验证。
- `docs/evidence/rust/ui-redesign.json` / `ui-fix-tour.json` / `window-chrome.json` / `r17-desktop.json`：M3 界面重做、全功能交互巡检与窗口标题栏截图记录。
