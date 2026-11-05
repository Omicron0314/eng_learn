# 重构任务总表

任务以依赖顺序实施。旧M0–M6的完成状态见 git 历史（提交 46f499a），不计入这里。用户已授权实际重构和原子提交；消费者验证见 [R00执行记录](spikes/R00-platform.md)，当前 Rust 产品实现及未验收项见 [实施证据](IMPLEMENTATION.md)。

**2026-11-05 范围修订**：本轮只交付桌面端，任务范围为 R00–R17。R18（Web/WASM）、R19–R20（移动原生）、R21（跨端同步）降为远期规划，既不排期、也不作为当前验收条件；降级不表示这些端已可用（见 [D47](DECISIONS.md)）。

| ID | 任务规格 | 依赖 | 阶段 | 状态 |
| --- | --- | --- | --- | --- |
| R00 | [GPUI消费者与平台能力验证](specs/R00-platform-spike.md) | — | G0 | 待CI（[证据与缺口](spikes/R00-platform.md)；实机项已从 AC 删除，见 D54） |
| R01 | [Rust工作区、工具链与CI](specs/R01-rust-workspace.md) | R00 | G1 | 待CI（[实施证据](IMPLEMENTATION.md)；fmt/clippy/test 流水线全绿后收口） |
| R02 | [领域类型、时间与FSRS](specs/R02-domain-srs-time.md) | R01 | G1 | 待CI（领域规则与固定向量已实现；跨平台回归降为 Linux 向量，等 CI 全绿） |
| R03 | [存储、事务与事件](specs/R03-storage-events.md) | R02 | G1 | 待CI（事务／事件／幂等／迁移已实现；资源故障矩阵进 CI） |
| R04 | [Material设计系统与应用壳](specs/R04-material-shell.md) | R01 | G1 | 待CI（六入口、主题、焦点与 360/1000dp 布局有测试；截图与无障碍断言由 CI 或标注来源的本机 Xvfb 产出） |
| R05 | [Provider接口与AI设置](specs/R05-providers-ai-settings.md) | R03, R04 | G1 | 待CI：adapter/调用日志、系统凭据优先与会话回退、真实来源提示已接入；真实模型质量与跨系统凭据从 AC 删除 |
| R06 | [词库、导入与词书管理](specs/R06-library-import.md) | R02, R03, R04 | G2 | 进行中：导入、组合筛选、分页、行选择/编辑草稿保护、删书预览、“/”搜索、流式读取及来源列去重已接入；**代码缺口：归一化批次峰值内存无固定上限**，需设上限并在 CI 测峰值 |
| R07 | [复习队列与专注学习](specs/R07-review.md) | R06, R05 | G2 | 待CI：队列/评分事务、正反面/快捷键、来源与区间展示、缺释义自动补全、失败可评分/重试、进度重置已接入；真实输入法从 AC 删除 |
| R08 | [词元分析、已知词与选词](specs/R08-knowledge-analyzer.md) | R02, R06 | G2 | 待CI：领域规则、fixtures 与库存查询已接入；工作流验收由自动化测试覆盖 |
| R09 | [文章工作区与生成流程](specs/R09-articles-generation.md) | R05, R07, R08 | G2 | 进行中：生成事务、领取/取消/重启、今天/历史/日期筛选、已读投影、低覆盖诊断已验证；**代码缺口：30 篇以上历史分页的自动化测试未写**；真实模型质量不在 AC |
| R10 | [阅读、语境释义与反馈](specs/R10-reader-lookup-feedback.md) | R09 | G2 | 待CI：阅读/释义/标记、反馈恢复、多句反馈、正文选择、位置保存/恢复、响应式词语面板已验证；IME 与触摸从 AC 删除 |
| R11 | [生词箱批量整理](specs/R11-inbox.md) | R10, R06 | G2 | 待CI：批次版本/原子回滚、零选择隐藏操作栏、释义来源标签与源定位已接入；行为验收由自动化测试覆盖 |
| R12 | [音频、朗读与费用护栏](specs/R12-audio.md) | R10, R05 | G2 | 进行中：远程/本机朗读、配额、缓存、真实播放器、完成回调、缓存状态与结束后重播已接入；**代码缺口：设置页 TTS 当日用量展示** |
| R13 | [今日与学习热力图](specs/R13-today-heatmap.md) | R07, R09, R11, R12 | G3 | 待CI：统一学习日查询、13×7 热力图、鼠标/键盘日期详情、未来格拒绝及完成刷新统计已接入；浅深/窄屏截图由 CI 或标注来源的本机 Xvfb 产出 |
| R14 | [统计与对照实验](specs/R14-stats-experiment.md) | R10, R12, R13 | G3 | 进行中：统计与实验查询/UI、覆盖率单点状态及已学词分页已接入；**代码缺口：50 条以上已学词分页的自动化测试未写** |
| R15 | [学习、外观与实验设置](specs/R15-preferences.md) | R02, R04, R05 | G3 | 进行中：设备偏好与重启恢复、独立草稿、校验/滑块、冲突重载、窗口主题隔离已通过增量验证；**代码缺口：远端 TTS 配置显式清除** |
| R16 | [旧数据迁入与备份恢复](specs/R16-migration-backup.md) | R03, R06, R08, R10, R11, R12, R15 | G3 | 进行中：WAL 一致性旧库转换、报告、音频校验迁入、隔离备份恢复已接入；**代码缺口：真实 ENOSPC 与 SIGKILL 注入测试未写**；备份/迁移人工链路改为自动化 |
| R17 | [桌面完整验收与本地打包](specs/R17-desktop-acceptance.md) | R14, R15, R16 | G3 | 进行中：Linux 现有证据保留；**代码缺口：截图自动驱动与 release 性能采样未写**；Xvfb 启动冒烟与打包已进 CI，Windows/macOS 已从 AC 删除 |
| R18 | [Web/WASM与手机浏览器适配](specs/R18-wasm-web.md) | R17 | G4 | 后续（远期，暂不实施）：仅保留共享 UI 的 wasm32 类型检查作为编译资产；浏览器仓储、离线、权限和真实设备均未实现 |
| R19 | [Android/iOS原生可行性验证](specs/R19-mobile-native-spike.md) | R00 | G5 | 后续（远期，暂不实施）：既有结论为锁定栈无 Android 后端（no-go）、iOS 需 macOS/真机（blocked），记录见 docs/spikes/R19-mobile.md |
| R20 | [移动原生产品实现](specs/R20-mobile-native.md) | R19, R17 | G5 | 后续（远期）：不建立移动原生入口，待 R19 得到 go 后再评估 |
| R21 | [跨端同步协议设计](specs/R21-sync-design.md) | R18 | G6 | 后续（远期）：状态机、冲突规则、选项代价与12条故障向量已存草案；待 R18 存储适配验证与实施批准，无服务部署 |

## 如何执行

先读 [plan](plan.md)、任务spec与涉及的UI／领域文档。根 Rust workspace 已建立；使用 [TESTING](TESTING.md) 中的实际命令验证，原型证据与产品验收分开记录。

关键路径（本轮）：R00 → R01 → R02 → R03；R04提供共用界面；R05和R06准备服务与词库，R07–R12完成学习闭环，R13–R16补完整产品和迁移，R17桌面验收收口。R18–R21 不再属于关键路径，也不阻塞桌面交付；它们的既有结论与草案只作为重新纳入范围时的起点。

一个任务可以分多次小提交，但不要用“页面画完”代替存储／失败／验收。状态只在这里维护：未开始、进行中、待CI、受阻（注明原因）、完成（附证据）、后续。待CI 表示实现已接入、剩余验收全部由流水线覆盖；CI 未全绿前不得改标完成。

## 共同完成定义

验收只覆盖 Linux 桌面，证据以 CI 为准（[D54](DECISIONS.md)）。本机与 CI 的分工见 [D56](DECISIONS.md)：本机跑秒级反馈，CI 出证据。

- 任务所有 AC 以 CI 运行结果为证据；本机命令用于复现同一结果与排错，不单独作为验收证据。
- CI 必须跑通 `cargo fmt --check`、`cargo clippy --all-targets -D warnings`、`cargo test --workspace`，另含空库迁移、锁文件解析与快速故障注入；verify 是单一 job。
- 慢测试、release 性能、打包冒烟与截图巡检放 nightly／tag。
- 写入具有事务、幂等和事件证据；迁移不改源库／旧迁移。真实 ENOSPC 与 SIGKILL 注入待测试写出后进 CI。
- **不在本轮 AC 内**：Windows／macOS、真实中文 IME、触摸、读屏、人工听感、付费真实模型质量、系统凭据真实读写。它们既不算通过，也不再阻塞任务（D54）。
- 配置、密钥、私人词书不进提交；发布与真实付费调用不包含在普通任务验收中。

依赖／迁移／提示词版本分别保持原子变更；提交使用Conventional Commits，正文标`Task: Rxx`。
