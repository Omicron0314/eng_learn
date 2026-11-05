# R00 · GPUI消费者与平台能力验证

任务状态以 [BACKLOG](../BACKLOG.md) 为准。阶段：G0。依赖：无。

## 目标与范围

输入：上游完整revision与本地平台信息。输出：可重复最小消费者、依赖图、能力矩阵、FSRS转换向量与选型记录，结论写docs/spikes/R00-platform.md。原型放独立spike目录，不当正式产品。

设计追踪：全部画板所需基础能力。通用约定见 [specs/README](README.md)；未列出的后续功能不在本任务扩展。

## 验收标准

- **AC1**：从Git依赖建立最小Kit＋Material程序，列出所有root patches，只有一个匹配的GPUI类型入口；锁定Rust工具链。
- **AC2**：在 CI 的 Xvfb 会话中打开中文界面，验证 sheet、焦点、浅深主题和字体缩放；IME 与触摸不在 AC（D54）。
- **AC3**：构建最小消费者并记录依赖图；WASM 探针归远期 R18，本任务不再作为门槛。
- **AC4**：验证候选Rust FSRS可读旧状态，四档固定输入向量有对比；库选择失败时给出具体差异，不重置旧状态。
- **AC5**：做 SQLite 重开与音频解码／seek／rate 探针，能力缺失记为 unknown；文件选择、系统朗读与凭据读写的实机项不在 AC。
- **AC6**：选定依赖与consumer revisions，记录渲染后端与能力缺口；不把资料阅读或cargo check当运行通过。

## 验证与交付

最小消费者构建／运行日志、固定调度向量、CI 截图、能力表；不用真实AI与用户库，不读取操作者凭据。

每条AC在交付记录中对应 CI 运行结果；本机用同一条命令复现用于排错，不单独作为验收证据（[D56](../DECISIONS.md)）。变更业务写入时同时断言events；失败路径检查不产生部分成功。

## 边界与风险

上游预发布及consumer patches是主要风险；Android/iOS原生不在此任务宣布支持，由R19验证。

执行中发现设计或契约变化时更新 [DECISIONS](../DECISIONS.md) 和相应规格；没有实证的能力标未验证。
