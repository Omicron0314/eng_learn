# 任务总表 (Backlog)

当前交付范围为 Linux 桌面端（`R00–R17`、`R22`）。远期搁置任务（`R18–R21`：Web/WASM、移动原生、跨端同步）已移至 `docs/archive/`。当前实现详情见 [IMPLEMENTATION](IMPLEMENTATION.md)。

## 当前任务状态

状态定义：`未开始`、`进行中`（尚有代码或测试缺口）、`待CI`（代码与本地测试已就绪，待 CI 全绿收口）、`完成`。

| ID | 任务规格 | 依赖 | 阶段 | 状态与待办缺口 |
| --- | --- | --- | --- | --- |
| R00 | [GPUI消费者与平台能力验证](specs/R00-platform-spike.md) | — | G0 | 待CI（见 [R00执行记录](spikes/R00-platform.md)） |
| R01 | [Rust工作区、工具链与CI](specs/R01-rust-workspace.md) | R00 | G1 | 待CI（workspace、桌面启动与边界检查已就绪） |
| R02 | [领域类型、时间与FSRS](specs/R02-domain-srs-time.md) | R01 | G1 | 待CI（领域规则、Anki 调度与 64 组向量已就绪） |
| R03 | [存储、事务与事件](specs/R03-storage-events.md) | R02 | G1 | 待CI（事务、事件、幂等、迁移与重开校验已就绪） |
| R04 | [Material设计系统与应用壳](specs/R04-material-shell.md) | R01 | G1 | 待CI（六入口导航、浅深色主题、焦点与响应式布局已就绪） |
| R05 | [Provider接口与AI设置](specs/R05-providers-ai-settings.md) | R03, R04 | G1 | 待CI（适配器、调用日志、系统凭据优先与会话回退已就绪） |
| R06 | [词库、导入与词书管理](specs/R06-library-import.md) | R02, R03, R04 | G2 | 进行中：**缺口：归一化批次峰值内存尚无固定上限** |
| R07 | [复习队列与专注学习](specs/R07-review.md) | R06, R05 | G2 | 待CI（Anki 队列/评分、快捷键、缺释义首次翻面自动补全已就绪） |
| R08 | [词元分析、已知词与选词](specs/R08-knowledge-analyzer.md) | R02, R06 | G2 | 待CI（分词、覆盖率、KnownSet 与选词规则已就绪） |
| R09 | [文章工作区与生成流程](specs/R09-articles-generation.md) | R05, R07, R08 | G2 | 进行中：**缺口：30 篇以上历史文章分页的自动化测试未写** |
| R10 | [阅读、语境释义与反馈](specs/R10-reader-lookup-feedback.md) | R09 | G2 | 待CI（阅读会话、点词释义、标记、位置恢复与多句反馈已就绪） |
| R11 | [生词箱批量整理](specs/R11-inbox.md) | R10, R06 | G2 | 待CI（多选批处理、原子回滚、释义来源标签与源定位已就绪） |
| R12 | [音频、朗读与播放器](specs/R12-audio.md) | R10, R15 | G2 | 进行中：本地 Kokoro 全文合成、英美口音缓存与播放已验证；**缺口：Linux CI 不安装模型，仅覆盖降级路径** |
| R13 | [今日与学习热力图](specs/R13-today-heatmap.md) | R07, R09, R11, R12 | G3 | 待CI（今日统计、13×7 热力图与日期详情已就绪） |
| R14 | [统计与对照实验](specs/R14-stats-experiment.md) | R10, R12, R13 | G3 | 进行中：**缺口：50 条以上已学词分页的自动化测试未写** |
| R15 | [学习、外观与实验设置](specs/R15-preferences.md) | R02, R04, R05 | G3 | 待CI（学习/AI 双草稿独立保存、密钥按卡独立清除、设备偏好已就绪） |
| R16 | [旧数据迁入与备份恢复](specs/R16-migration-backup.md) | R03, R06, R08, R10, R11, R12, R15 | G3 | 进行中：**缺口：真实 `ENOSPC` 与 `SIGKILL` 注入测试未写** |
| R22 | [单词发音与词包音频](specs/R22-word-audio.md) | R06, R07, R12 | G2 | 进行中：`word_audio`、音标点播/英美切换、换卡自动朗读、`cijian-fill` 补齐与 `.cijianpack` 导包已接入；**缺口：音频转码压缩与导入时保留词书自带音标** |
| R17 | [桌面完整验收与本地打包](specs/R17-desktop-acceptance.md) | R14, R15, R16 | G3 | 进行中：**缺口：截图自动驱动与 release 性能采样未写** |

## 远期规划（不排期）

| ID | 归档规格 | 状态说明 |
| --- | --- | --- |
| R18 | [Web/WASM与手机浏览器适配](archive/R18-wasm-web.md) | 仅保留共享 UI 的 `wasm32` 编译检查；浏览器仓储与离线能力未实现 |
| R19 | [Android/iOS原生可行性验证](archive/R19-mobile-native-spike.md) | 锁定栈无 Android 后端（no-go）、iOS 需 macOS/真机（blocked），见 [记录](archive/R19-mobile.md) |
| R20 | [移动原生产品实现](archive/R20-mobile-native.md) | 待 R19 具备可用后端后再评估 |
| R21 | [跨端同步协议设计](archive/R21-sync-design.md) | 协议与冲突草案见 [SYNC-PROTOCOL](archive/SYNC-PROTOCOL.md) |

## 交付与提交流程

- **关键路径**：`R00 → R01 → R02 → R03`（基础层）→ `R04–R06`（UI 壳层、Provider、词库）→ `R07–R12, R22`（复习、文章、阅读、生词箱、音频闭环）→ `R13–R17`（今日、统计、设置、迁移备份与桌面打包验收）。
- **验证标准**：命令与分层见 [TESTING](TESTING.md)。业务写入必须包含事务、幂等与事件断言。
- **提交规范**：按逻辑单元、依赖、迁移和提示词版本分别保持原子提交，使用 Conventional Commits 并标注 `Task: Rxx`。
