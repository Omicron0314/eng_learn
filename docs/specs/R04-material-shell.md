# R04 · Material设计系统与应用壳

任务状态以 [BACKLOG](../BACKLOG.md) 为准。阶段：G1。依赖：R01。

## 目标与范围

实现CijianTheme、AppRoute、NavigationShell、sheet/dialog/snackbar基础组合与测试场景；颜色和尺寸来自DESIGN-SYSTEM。

设计追踪：overview、screen-states、screen-mobile、screen-dark及导航。通用约定见 [specs/README](README.md)；未列出的后续功能不在本任务扩展。

## 验收标准

- **AC1**：主入口六项顺序固定，设置独立；Article详情归文章，返回栈保留上下文。
- **AC2**：浅深色语义token正确，100–200%字体和中文fallback可读；render不修改主题或发IO。
- **AC3**：compact底栏／expanded rail、三栏收敛与sheet切换符合UI-SPEC，360dp命中区不重叠。
- **AC4**：modal焦点限制、Escape、焦点恢复、输入中快捷键隔离与可访问名称可测。
- **AC5**：空／加载／错误／禁用／完成通用状态可演示；动画遵守reduced motion且空闲无持续重绘。

## 验证与交付

GPUI实际组件测试＋固定数据浅深／四种尺寸截图，核对设计token与导航行为。

每条AC在交付记录中对应 CI 运行结果；本机用同一条命令复现用于排错，不单独作为验收证据（[D56](../DECISIONS.md)）。变更业务写入时同时断言events；失败路径检查不产生部分成功。

## 边界与风险

控件上游默认外观不一定等于词间稿；先复用行为再做最小外观映射。

执行中发现设计或契约变化时更新 [DECISIONS](../DECISIONS.md) 和相应规格；没有实证的能力标未验证。
