# R05 · Provider接口与AI设置

任务状态以 [BACKLOG](../BACKLOG.md) 为准。阶段：G1。依赖：R03、R04。

## 目标与范围

LLMProvider、TTSProvider、HttpTransport、SecretStore；ProviderConfigView只给脱敏状态，密钥patch为Keep/Replace/Clear。

设计追踪：screen-settings。通用约定见 [specs/README](README.md)；未列出的后续功能不在本任务扩展。

## 验收标准

- **AC1**：无配置可启动并使用本地功能；相关在线动作返回CONFIG_MISSING，不自动切fake。
- **AC2**：LLM/TTS输入显式校验，模型／端点／voice无代码默认；保存后新请求生效，在途请求保留快照。
- **AC3**：保存／撤销不发测试调用；密钥留空保持、显式清除、替换可区分；key不可出现在日志／事件／备份。
- **AC4**：每个底层LLM调用含失败有日志，SDK自动重试关闭，结构非法最多修复一次。
- **AC5**：系统凭据不可用时会话内模式明确，UI展示真实配置来源；test doubles只在测试依赖。

## 验证与交付

本地协议stub、配置／secret adapter测试、provider状态GPUI测试；不调用付费端点。

每条AC在交付记录中对应 CI 运行结果；本机用同一条命令复现用于排错，不单独作为验收证据（[D56](../DECISIONS.md)）。变更业务写入时同时断言events；失败路径检查不产生部分成功。

## 边界与风险

浏览器密钥与CORS单独由R18适配；此任务不保证远端服务质量。

执行中发现设计或契约变化时更新 [DECISIONS](../DECISIONS.md) 和相应规格；没有实证的能力标未验证。
