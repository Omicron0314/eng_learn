# R11 · 生词箱批量整理

任务状态以 [BACKLOG](../BACKLOG.md) 为准。阶段：G2。依赖：R10、R06。

## 目标与范围

query_inbox、batch_inbox(action,ids,expectedRevisions,operationId)→processed/cardsCreated；显示来源与上下文。

设计追踪：screen-inbox、空箱状态。通用约定见 [specs/README](README.md)。

## 验收标准

- **AC1**：pending数量与导航badge／首页一致，缺释义和来源缺失状态清楚。
- **AC2**：多选才出现操作栏，全选范围明确，清选／筛选变化不带入隐形旧选择。
- **AC3**：加入不重置已有卡FSRS、不覆盖用户释义；忽略只处理选中pending。
- **AC4**：过期ID或写入失败整批回滚并保留可恢复选择，重复操作不重复建卡／事件。
- **AC5**：空状态可去文章，来源链接定位正确；处理完成及时更新各页投影。

## 验证与交付

多选／全选／过期批处理／回滚合同、来源丢失、空态和事件对账。

## 边界与风险

同词多次标记需符合pending唯一约束，不能简单按文章重复建生词卡。
