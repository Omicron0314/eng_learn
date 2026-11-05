# R18 · Web/WASM与手机浏览器适配

任务状态以 [BACKLOG](../BACKLOG.md) 为准。阶段：G4（远期，暂不实施）。依赖：R17。

## 目标与范围

cijian-wasm入口、BrowserRepository/BlobStore/Transport/SecretStore/Audio适配、URL/history与离线资源。

设计追踪：全部功能的响应式布局、screen-mobile。通用约定见 [specs/README](README.md)；未列出的后续功能不在本任务扩展。

## 验收标准

- **AC1**：共享domain/app/ui实际编译到WASM；浏览器仓储运行与原生相同合同，重开保留学习数据。
- **AC2**：中文IME、触摸、bottom sheet、软键盘避让与小屏导航在真实Android/iOS浏览器逐端验收。
- **AC3**：fetch CORS错误清楚，密钥默认会话内且无服务端保密承诺；配置保存不触发调用。
- **AC4**：音频权限／后台恢复／系统语音缺失按能力降级，播放完成与评分不会因恢复重复写。
- **AC5**：入口／WASM／字体／图标可离线启动，存储配额／清理／多标签写冲突有处理，PWA安装与离线分开验证。
- **AC6**：URL直接进文章与返回保持状态，导入／导出／恢复可用；不支持的浏览器有明确说明。

## 验证与交付

WASM构建＋实际浏览器Playwright／可访问桥测试、离线重开、设备真机与存储配额故障。

每条AC在交付记录中对应测试、截图或平台运行证据；需要实机的条目不可用静态审查抵替。变更业务写入时同时断言events；失败路径检查不产生部分成功。

## 边界与风险

原生SQLite、文件API、线程和系统keychain不可直接复用；不需要搭建默认云后端。

执行中发现设计或契约变化时更新 [DECISIONS](../DECISIONS.md) 和相应规格；没有实证的能力标未验证。
