# R03 · 存储、事务与事件

任务状态以 [BACKLOG](../BACKLOG.md) 为准。阶段：G1。依赖：R02。

## 目标与范围

实现Repository、UnitOfWork、CommandReceipt和新SQLite schema；每个profile一个写协调器；表、索引与事件见DATA_MODEL。

设计追踪：所有写入与持久化状态。通用约定见 [specs/README](README.md)；未列出的后续功能不在本任务扩展。

## 验收标准

- **AC1**：空库与顺序升级可用，已应用migration checksum可校验；旧Drizzle迁移未修改。
- **AC2**：评分／导入／标记等业务写与events、receipt同事务，注入事件失败时回滚。
- **AC3**：operationId重放得到原结果；stale revision不改业务和日志；双窗口不会重复评分。
- **AC4**：文件库关闭重开数据一致，append-only日志不能通过普通repo被修改／删除。
- **AC5**：缓存／日志例外明确，凭据不在普通设置和事件中；未知历史payload可保留。

## 验证与交付

内存SQLite＋临时文件库、并发冲突／故障注入、外键和索引检查；不接真实库。

每条AC在交付记录中对应 CI 运行结果；本机用同一条命令复现用于排错，不单独作为验收证据（[D56](../DECISIONS.md)）。变更业务写入时同时断言events；失败路径检查不产生部分成功。

## 边界与风险

浏览器实现后须跑相同contract tests；不能假定原生同步事务回调可直接用于IndexedDB。

执行中发现设计或契约变化时更新 [DECISIONS](../DECISIONS.md) 和相应规格；没有实证的能力标未验证。
