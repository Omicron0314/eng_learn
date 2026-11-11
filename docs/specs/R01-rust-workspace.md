# R01 · Rust工作区、工具链与CI

任务状态以 [BACKLOG](../BACKLOG.md) 为准。阶段：G1。依赖：R00。

## 目标与范围

建立ARCHITECTURE中的workspace与cijian-desktop空壳；入口装配ports，domain无平台依赖。输出可构建的模块骨架、Cargo.lock、CI与准确开发命令。

设计追踪：通用工程基础。通用约定见 [specs/README](README.md)。

## 验收标准

- **AC1**：新工程能打开空壳且无LLM配置也可启动；旧pnpm工程仍能独立使用。
- **AC2**：所有Git依赖／patch使用R00验证revision，仓库以外相邻checkout不会被隐式读取。
- **AC3**：fmt、clippy、test 在 Linux CI 通过；CI 作业覆盖 Xvfb 下的 UI 测试、release 性能记录、低空间／中断故障注入与包构建启动冒烟，不混入未支持 features。
- **AC4**：边界检查防止domain引入IO／GPUI，production依赖不导出test doubles；新增模块标明职责。

## 验证与交付

干净检出构建、cargo metadata依赖检查、最小边界测试；CI 未实际运行时标待验。作业清单见 [TESTING](../TESTING.md)。

## 边界与风险

不要以脚手架通过代替业务或平台验收；不在本任务迁移旧数据。
