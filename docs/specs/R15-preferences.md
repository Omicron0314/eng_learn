# R15 · 学习、外观与实验设置

任务状态以 [BACKLOG](../BACKLOG.md) 为准。阶段：G3。依赖：R02、R04、R05。

## 目标与范围

edit_settings草稿→validate→save_settings；device_preferences与profile学习设置分开。

设计追踪：screen-learning、settings-experiment及深色偏好。通用约定见 [specs/README](README.md)；未列出的后续功能不在本任务扩展。

## 验收标准

- **AC1**：等级、难度、点词模式、上限／保持率／长度／字符额度／日界／实验字段全可编辑并使用统一范围。
- **AC2**：滑块配键盘数值输入，文章min≤max，autoAdd明确警告；改变日界解释统计变化。
- **AC3**：保存原子生效并有settings_update；撤销／关闭脏草稿规则一致，无隐式网络请求。
- **AC4**：主题跟随系统／浅／深和播放速度按设备持久化，启动恢复；不会覆盖旧用户合法值。
- **AC5**：实验导航定位高级区域，默认值与SPEC-CORE一致，非法配置逐字段提示。

## 验证与交付

表单校验、草稿保护、日界重算、主题切换与重启持久化UI测试；远端 TTS 配置显式清除由自动化测试覆盖。

每条AC在交付记录中对应 CI 运行结果；本机用同一条命令复现用于排错，不单独作为验收证据（[D56](../DECISIONS.md)）。变更业务写入时同时断言events；失败路径检查不产生部分成功。

## 边界与风险

provider密钥由R05管理，普通settings patch不能覆盖secret。

执行中发现设计或契约变化时更新 [DECISIONS](../DECISIONS.md) 和相应规格；没有实证的能力标未验证。
