# 重构任务总表

任务以依赖顺序实施。旧M0–M6的完成状态见 git 历史（提交 46f499a），不计入这里。**本轮只完成规划，所有Rust产品任务尚未实施。** R00的资料调研已记录，原型验证仍未开始。

| ID | 任务规格 | 依赖 | 阶段 | 状态 |
| --- | --- | --- | --- | --- |
| R00 | [GPUI消费者与平台能力验证](specs/R00-platform-spike.md) | — | G0 | 未开始（原型） |
| R01 | [Rust工作区、工具链与CI](specs/R01-rust-workspace.md) | R00 | G1 | 未开始 |
| R02 | [领域类型、时间与FSRS](specs/R02-domain-srs-time.md) | R01 | G1 | 未开始 |
| R03 | [存储、事务与事件](specs/R03-storage-events.md) | R02 | G1 | 未开始 |
| R04 | [Material设计系统与应用壳](specs/R04-material-shell.md) | R01 | G1 | 未开始 |
| R05 | [Provider接口与AI设置](specs/R05-providers-ai-settings.md) | R03, R04 | G1 | 未开始 |
| R06 | [词库、导入与词书管理](specs/R06-library-import.md) | R02, R03, R04 | G2 | 未开始 |
| R07 | [复习队列与专注学习](specs/R07-review.md) | R06, R05 | G2 | 未开始 |
| R08 | [词元分析、已知词与选词](specs/R08-knowledge-analyzer.md) | R02, R06 | G2 | 未开始 |
| R09 | [文章工作区与生成流程](specs/R09-articles-generation.md) | R05, R07, R08 | G2 | 未开始 |
| R10 | [阅读、语境释义与反馈](specs/R10-reader-lookup-feedback.md) | R09 | G2 | 未开始 |
| R11 | [生词箱批量整理](specs/R11-inbox.md) | R10, R06 | G2 | 未开始 |
| R12 | [音频、朗读与费用护栏](specs/R12-audio.md) | R10, R05 | G2 | 未开始 |
| R13 | [今日与学习热力图](specs/R13-today-heatmap.md) | R07, R09, R11, R12 | G3 | 未开始 |
| R14 | [统计与对照实验](specs/R14-stats-experiment.md) | R10, R12, R13 | G3 | 未开始 |
| R15 | [学习、外观与实验设置](specs/R15-preferences.md) | R02, R04, R05 | G3 | 未开始 |
| R16 | [旧数据迁入与备份恢复](specs/R16-migration-backup.md) | R03, R06, R08, R10, R11, R12, R15 | G3 | 未开始 |
| R17 | [桌面完整验收与本地打包](specs/R17-desktop-acceptance.md) | R14, R15, R16 | G3 | 未开始 |
| R18 | [Web/WASM与手机浏览器适配](specs/R18-wasm-web.md) | R17 | G4 | 未开始 |
| R19 | [Android/iOS原生可行性验证](specs/R19-mobile-native-spike.md) | R00 | G5 | 未开始（验证） |
| R20 | [移动原生产品实现](specs/R20-mobile-native.md) | R19, R17 | G5 | 受R19结果约束 |
| R21 | [跨端同步协议设计](specs/R21-sync-design.md) | R18 | G6 | 后续 |

## 如何执行

先读 [plan](plan.md)、任务spec与涉及的UI／领域文档。第一步是R00消费者和平台验证，之后R01建立workspace；当前不能直接运行规划中的cargo命令。

关键路径：R00 → R01 → R02 → R03；R04提供共用界面；R05和R06准备服务与词库，R07–R12完成学习闭环，R13–R16补完整产品和迁移，R17桌面验收，R18浏览器，R19/R20移动原生。R19可在资源允许时提前验证，R20必须等相应平台go与桌面共享层完成。R21是后续协议设计，不是首版依赖。

一个任务可以分多次小提交，但不要用“页面画完”代替存储／失败／平台验收。状态只在这里维护：未开始、进行中、受阻（注明原因）、完成（附证据）、后续。受R19约束的R20不是默认承诺所有移动端已可行。

## 共同完成定义

- 任务所有AC有自动化或明确实机证据，代码、spec与决策一致。
- 受影响的fmt/clippy/tests通过；UI按设计表有截图和行为测试。
- 写入具有事务、幂等和事件证据；迁移不改源库／旧迁移。
- 按平台分别记录通过／未验；真实AI质量与长期效果另记。
- 配置、密钥、私人词书不进提交；发布与真实付费调用不包含在普通任务验收中。

依赖／迁移／提示词版本分别保持原子变更；提交使用Conventional Commits，正文标`Task: Rxx`。本轮没有执行提交或暂存，原有暂存状态保留。
