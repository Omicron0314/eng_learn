# 路线图 (Roadmap)

详细任务依赖与当前状态见 [BACKLOG](BACKLOG.md)，平台与验收边界见 [plan](plan.md)。

## 当前交付阶段（Linux 桌面端）

| 阶段 | 任务 | 核心交付内容 |
| --- | --- | --- |
| G0 技术验证 | R00 | GPUI 最小消费者、锁定依赖与补丁、FSRS 状态转换与 SQLite/音频探针 |
| G1 基础架构 | R01–R05 | Rust workspace、纯领域与事务存储、M3 主题与应用壳、可配置 AI 服务 |
| G2 学习闭环 | R06–R12, R22 | 词库/词包导入 → Anki 间隔复习（含单词发音）→ 文章生成 → 阅读/点词 → 生词箱 → 整篇朗读 |
| G3 完整桌面产品 | R13–R17 | 今日与热力图、统计与对照实验、设置偏好、旧库迁移/备份恢复、Linux 打包与 CI 验收 |

## 远期规划（不排期）

相关规格与调研记录已归档至 `docs/archive/`，仅在重新纳入范围时启动：

| 阶段 | 归档任务 | 启动前置条件 |
| --- | --- | --- |
| G4 Web 与手机浏览器 | [R18](archive/R18-wasm-web.md) | 浏览器持久化仓储（IndexedDB/OPFS）、离线资源、CORS 与移动端输入实测 |
| G5 移动原生 | [R19](archive/R19-mobile-native-spike.md) → [R20](archive/R20-mobile-native.md) | 上游具备可用 Android/iOS 渲染与生命周期后端，并完成真机验证 |
| G6 跨端同步 | [R21](archive/R21-sync-design.md) | 基于 [同步协议草案](archive/SYNC-PROTOCOL.md) 完成多端存储与冲突故障向量验证 |

## 后续候选方向

逐句听写、原创测试题型、外部文章导入、词汇量初测、FSRS 参数自适应优化、口语练习等能力需单独新增规格，不隐含在当前桌面交付范围内。
