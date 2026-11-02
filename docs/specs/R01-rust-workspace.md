# R01 · Rust工作区、工具链与CI

任务状态以 [BACKLOG](../BACKLOG.md) 为准。阶段：G1。依赖：R00。

## 目标与范围

建立ARCHITECTURE中的workspace与cijian-desktop空壳；入口装配ports，domain无平台依赖。输出可构建的模块骨架、Cargo.lock、CI与准确开发命令。

设计追踪：通用工程基础。通用约定见 [specs/README](README.md)；未列出的后续功能不在本任务扩展。

## 验收标准

- **AC1**：新工程能打开空壳且无LLM配置也可启动；旧pnpm工程仍能独立使用。
- **AC2**：所有Git依赖／patch使用R00验证revision，仓库以外相邻checkout不会被隐式读取。
- **AC3**：fmt、clippy、test在已配置平台通过，CI区分native和WASM目标，不混入未支持features。
- **AC4**：边界检查防止domain引入IO／GPUI，production依赖不导出test doubles；新增模块标明职责。

## 验证与交付

干净检出构建、cargo metadata依赖检查、最小边界测试；CI未实际运行时标待验。

每条AC在交付记录中对应测试、截图或平台运行证据；需要实机的条目不可用静态审查抵替。变更业务写入时同时断言events；失败路径检查不产生部分成功。

## 边界与风险

不要以脚手架通过代替业务或平台验收；不在本任务迁移旧数据。

执行中发现设计或契约变化时更新 [DECISIONS](../DECISIONS.md) 和相应规格；没有实证的能力标未验证。
