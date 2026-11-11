# 任务规格说明 (Specs)

本目录存放桌面端重构各分项任务（`R00–R17`、`R22`）的验收规格。任务依赖与当前状态统一在 [BACKLOG](../BACKLOG.md) 维护，远期搁置任务（`R18–R21`）已归档至 `docs/archive/`。

## 通用实现契约

1. **文档基准**：界面与交互遵循 [UI-SPEC](../UI-SPEC.md) 与 [DESIGN-SYSTEM](../DESIGN-SYSTEM.md)，业务默认值与算法遵循 [SPEC-CORE](../SPEC-CORE.md)，表结构与事件遵循 [DATA_MODEL](../DATA_MODEL.md)。
2. **输入与输出校验**：所有外部文件、设置参数与模型输出均需显式业务校验，反序列化成功不等于业务合法；错误信息不得泄露密钥或内部堆栈。
3. **事务与幂等**：可变业务命令通过 `operation_id` + `revision` 保证重放幂等与并发安全；业务写入与 `events`、`command_receipts` 必须在同一 SQLite 事务内提交（复习操作同时写入 `review_logs`）。
4. **纯领域边界**：`now`、随机数与 ID 生成器显式注入领域层；Rust 内部正文区间统一使用 UTF-8 字节偏移，平台句柄与异步 IO 不进入 `cijian-domain`。
5. **测试隔离**：自动化测试一律使用原创 fixtures、固定时钟、本地协议 stub 与 `cijian-testing` 替身，不访问用户真实数据或付费 AI 接口。
