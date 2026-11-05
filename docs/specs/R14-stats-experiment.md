# R14 · 统计与对照实验

任务状态以 [BACKLOG](../BACKLOG.md) 为准。阶段：G3。依赖：R10、R12、R13。

## 目标与范围

query_stats(range,page)、experiment_report(window=7d,version)；结果有sample/excluded/missing与nullable rate。

设计追踪：screen-stats。通用约定见 [specs/README](README.md)；未列出的后续功能不在本任务扩展。

## 验收标准

- **AC1**：今日／本周新学和复习与日志对账，完成率与最新反馈分母正确，零样本为null。
- **AC2**：趋势保留生成时快照和各篇档位；已学词分页并解释可提取率。
- **AC3**：热力图／每周活跃与首页共用口径，换时区后双方一致。
- **AC4**：出现／对照只取7天内首次有效评分，偶现control／缺失exposed列排除，后续评分不改首值。
- **AC5**：新旧experiment_version分开汇总，显示样本、Again、未复习和百分点差值，不宣称因果。

## 验证与交付

种子日志手算对账、CLI或同服务报告快照、空组／窗口边界／重复词观察、图表可访问数值；50 条以上已学词分页的自动化测试待补。

每条AC在交付记录中对应 CI 运行结果；本机用同一条命令复现用于排错，不单独作为验收证据（[D56](../DECISIONS.md)）。变更业务写入时同时断言events；失败路径检查不产生部分成功。

## 边界与风险

生成入组不等于实际阅读曝光；本轮保留观察性实验，解释混杂。

执行中发现设计或契约变化时更新 [DECISIONS](../DECISIONS.md) 和相应规格；没有实证的能力标未验证。
