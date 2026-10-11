# MVP 一次性实施与验收

## 授权与决策

用户明确授权 agent 决定全部待确认做法，并要求不再提问、一次完成当前 MVP。此授权覆盖 M0 规格批准、合理新增依赖、首个迁移之外必要的新迁移、提示词实现和规格缺口的取舍；不覆盖修改已合并迁移、读取/提交密钥、付费实调用或公开发布。许可证继续暂缓。

- TTS：实现配置驱动的 OpenAI 兼容语音端点适配器；默认 `none` 使用浏览器 SpeechSynthesis，无费用、不需要音色或模型硬编码。
- LLM：保留配置驱动的真实适配器与 fail-fast；增加明确标识的 `pnpm dev:demo`，使用假服务，绝不将演示文本冒充真实生成效果。
- 所有已经存在的规格（包括 T1.5、事件与 schema 补充）批准进入实现。
- 保留现有合并的核心、provider 与数据库代码，先测试，再补齐缺口；不改已合并迁移。
- 不内置词书；用户文件仅本地导入；不能把 blank.apkg 的内容公开成 fixtures。
- 真实 LLM 质量评测与 2–4 周留存验证无法由本次离线开发证明；提供带费用护栏的手动脚本，报告中明确未执行。

## 单写者实施板

全部 writer 串行运行于 `/home/Omicron0314/Documents/Programme/eng_learn`。父 agent 在子 writer 运行时只读、规划验证与审查；不同时写源码。

1. `services`：补齐服务层、导入解析、依赖装配、TTS provider、业务集成测试；可修复 core/db/provider 的真实缺陷，不改既有迁移。
2. `web`：基于 services 输出补齐 API、七个页面、密码保护、PWA、部署、CI、工具脚本与端到端测试。
3. `review-domain` / `review-security-ui`：并行只读审查，分别核对业务契约与安全/用户闭环。
4. `fix`：唯一 writer，修复审查确认的缺陷、完善测试与文档，运行完整验收。
5. `verify`：独立只读复核关键修复与验收证据。
6. 父 agent：检查最终 diff，亲自运行必要的完整检查与浏览器验证，更新完成状态。

## 验收合同

- `pnpm lint`、`pnpm typecheck`、`pnpm test`、`pnpm build`、`pnpm e2e` 成功。
- 端到端：导入 CSV → 复习评分 → 今日生成 → 阅读/点词 → inbox 加入/忽略 → 音频与反馈 → stats → 设置；密码页与 API 保护、manifest。
- 内存 SQLite 集成测试：每种业务写入有事务中的对应事件；JSON 修复/coverage 重试、已知词排除、卡片/释义/音频幂等缓存与日配额、报告样本数据。
- 配置错误不泄漏密钥；API 入参 zod 校验；所有网络 AI 调用只在 providers；core 无 IO。
- 重启持久化验证；Docker 可用时验证 compose 与构建，不可用时明确记录。
- BACKLOG 只按真实完成证据更新，未做的实调用与长期验证不得标作完成。
