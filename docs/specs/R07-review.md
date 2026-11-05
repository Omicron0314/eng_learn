# R07 · 复习队列与专注学习

任务状态以 [BACKLOG](../BACKLOG.md) 为准。阶段：G2。依赖：R06、R05。

## 目标与范围

query_queue、preview_card、reveal_card、rate_card(operationId,expectedRevision,rating,duration)；UI正面／背面／提交／等待／完成状态。

设计追踪：screen-review、screen-mobile复习、完成态。通用约定见 [specs/README](README.md)；未列出的后续功能不在本任务扩展。

## 验收标准

- **AC1**：到期学习／复习／新词顺序和配额正确，首页可使用同一queue投影。
- **AC2**：Space或轻点翻面，1–4只在背面有效；输入焦点／弹窗／提交时不触发误评分。
- **AC3**：背面显示释义例句与来源，缺释义只请求一次；生成失败也能评分。
- **AC4**：一次评分更新调度状态（FSRS 记忆快照与 Anki ease）并追加一条review_log和review事件；重复或过期提交不重复计数。
- **AC5**：退出保留已提交记录，重进重新计算；等待到期与今日完成文案不同；重启一致。

## 验证与交付

队列合同、双提交、缺释义失败、100词/20新词闭环、键盘组件测试；Xvfb 巡检截图由 CI 产出。

每条AC在交付记录中对应 CI 运行结果；本机用同一条命令复现用于排错，不单独作为验收证据（[D56](../DECISIONS.md)）。变更业务写入时同时断言events；失败路径检查不产生部分成功。

## 边界与风险

画板间隔是例子；学习步骤采用 Anki 默认 1m/10m、relearn 10m、毕业 1d/4d（见 D53），复习卡间隔来自 Anki ease（见 D57），不硬编码分钟／天数。

执行中发现设计或契约变化时更新 [DECISIONS](../DECISIONS.md) 和相应规格；没有实证的能力标未验证。
