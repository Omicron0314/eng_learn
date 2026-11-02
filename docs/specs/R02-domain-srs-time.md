# R02 · 领域类型、时间与FSRS

任务状态以 [BACKLOG](../BACKLOG.md) 为准。阶段：G1。依赖：R01。

## 目标与范围

定义ProfileId/CardId、ValidatedSettings、StudyDay、SrsState、Rating与SchedulingPreview；now/rng/id source显式注入。

设计追踪：复习、日界与统计共用规则。通用约定见 [specs/README](README.md)；未列出的后续功能不在本任务扩展。

## 验收标准

- **AC1**：四档预览与提交使用同一FSRS版本参数，固定向量能复现。
- **AC2**：学习日／周边界、DST歧义处理符合SPEC-CORE；没有Date.now式隐藏依赖。
- **AC3**：New/Review名额和learning例外、暂停／归档排除有纯规则测试。
- **AC4**：全部设置范围、min/max交叉约束、新装默认与迁移保留规则都有校验。
- **AC5**：旧状态字段逐项转换，无法表示的状态返回错误并保留原数据。

## 验证与交付

table-driven调度／时间／校验测试；TS向量对照记录数值容差与语义差异。

每条AC在交付记录中对应测试、截图或平台运行证据；需要实机的条目不可用静态审查抵替。变更业务写入时同时断言events；失败路径检查不产生部分成功。

## 边界与风险

FSRS库版本变化可能改变预测；不把新算法差异悄悄归入浮点误差。

执行中发现设计或契约变化时更新 [DECISIONS](../DECISIONS.md) 和相应规格；没有实证的能力标未验证。
