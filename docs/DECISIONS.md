# 决策记录(DECISIONS)

来源:`docs/plan.md` §0、§0.2、§12。

## 状态标记

- **已定**:作者拍板,agent 不得改动。
- **默认**:建议值,可调整,但改动要记入本文件。
- **待定**:尚未决定,agent 遇到必须停下来问人。
- **暂缓**:有意推迟的事项。agent 不处理,也不必为此提问。

正文里未标记的规格视为“默认”。

## 决策表

| 项 | 决策 | 状态 |
| --- | --- | --- |
| 形态 | 先做 Web(PWA),手机端和桌面端留到 SaaS 阶段 | 已定 |
| 路线 | 先自用并开源验证,SaaS 在验证之后 | 已定 |
| LLM | DeepSeek(OpenAI 兼容接口);baseURL、密钥、模型名全部来自环境变量,不写死 | 已定 |
| 目标词书 | 大学英语四级(CET-4);作者当前水平约在高中 3500 词到四级之间 | 已定 |
| AI 接入 | MVP 自带 API Key(BYOK),密钥只存服务端环境变量 | 已定 |
| 复习 | 自研复习流程,调度直接用开源 FSRS 库;Anki 只作导入来源 | 已定 |
| 基线已知词 | 用户自行导入词表(作者:高中 3500 词作基线,四级词书作学习词书),MVP 不内置词表 | 默认 |
| MVP 不做 | 外刊匹配、词汇量初测、多端同步、原生 App、账号体系、SaaS | 已定 |
| TTS 服务商 | 配置驱动的 OpenAI 兼容语音适配器；默认 none 使用浏览器朗读，见 MVP-EXECUTION | 已授权实施 |
| 开源许可证 | MVP 期间仓库保持私有,不添加 LICENSE 文件,也不在 package.json 里声明许可证 | 暂缓 |
| 模型配置 | LLM 与 TTS 的模型名都不写死(见 `docs/SPEC-CORE.md` §5.1) | 已定 |
| agent 工具与仓库托管 | 由执行的 agent 按所用工具自行适配,记入本文件 | 默认(不阻塞) |

## 待确认

1. **开源许可证(暂缓)**:公开仓库前再决定。
2. 当前 MVP 的待实施做法已由作者一次性授权 agent 决定；不再要求点击确认。真实付费调用、密钥、发布与修改已合并迁移仍不在授权内，见 `docs/MVP-EXECUTION.md`。

## 变更日志

### 2026-10-11 文档拆分

- 执行 agent 为 Cursor agent。Cursor 原生读取根目录 `AGENTS.md`,因此不另建 `.cursor/rules`。
- CI 按 `docs/ARCHITECTURE.md` 使用 GitHub Actions;仓库托管尚未确定(当前目录还不是 git 仓库),T0.1 时确认。
- 完整方案的文件名为 `docs/plan.md`(plan §0.1 里写的是 `docs/PLAN.md`),各文档统一引用 `docs/plan.md`。
- lint 工具(Biome 或 ESLint+Prettier)尚未选定,留到 T0.1 spec 中决定并记入本文件。

### 2026-10-11 git 与提交规范(作者决定)

- 仓库已 `git init`,默认分支 `main`;远程托管仍未确定。
- 要求原子化提交(作者决定):每个提交一个逻辑变更,且单独通过 lint / typecheck / test。
- 提交信息格式采用 Conventional Commits(默认,agent 提议)。

### 2026-10-11 补齐编写 spec 时发现的缺口(agent 提议,待确认)

- 新增任务 T1.5 `/settings` 页面(plan §7 有该页面,但没有对应任务)。设置的存储与默认值放在 T0.2。
- plan §6.9 的“缺释义时由 LLM 生成释义”没有对应任务,归入 T1.3;因此 T1.3 增加对 T0.3 的依赖。
- plan §1 的 H1 要统计文章阅读完成率,但事件类型里没有“读完”;新增事件 `article_complete`,在 T3.3 实现。
- 部署相关(`.env.example`、`APP_PASSWORD` 访问保护、PWA manifest、Dockerfile 与 docker compose)没有对应任务,归入 T0.1。
- schema 补充(词表、标记状态、设置等)见 `docs/DATA_MODEL.md` §3,需要人工确认后才能写迁移。
- 编写 spec 时补充的依赖:T1.3 依赖 T1.1(导入表单)、T1.5 依赖 T1.2(上限生效测试)、T2.2 依赖 T1.1(学习词书建卡)、T5.2 依赖 T6.1(听完后的难度反馈)、T6.3 依赖 T6.1(H2 数据)。
- 目标词缺失时不额外重试(要求补词需要修改提示词);只记录到 `missingTargets`。
- 新提示词 `json-repair.v1`(T0.3)、`definition.v1`(T1.3)、`gloss.v1`(T4.1)的草稿写在对应 spec 中,实现前需人审。

### 2026-10-11 T0.1 / T0.2 前置确认(作者决定)

- lint 与格式化工具:Biome。
- pnpm 安装方式:`npm i -g pnpm`。
- `docs/DATA_MODEL.md` §3 的 schema 补充:同意,T0.2 按此写首个迁移。
- T0.1 依赖清单:作者尚未批准,等待答复。

### 2026-10-11 取消 TTS 盲听 spike(作者决定)

- 删除任务 S5;T5.1 不再依赖 S5。plan.md 与各拆分文档已同步。
- TTS 服务商仍为待定:到 M5 时由作者直接指定一家,agent 按 `TTSProvider` 接口接入。

### 2026-10-11 MVP 一次实施（作者授权）

- 现有全部 spec 与待实现选择批准，详见 `docs/MVP-EXECUTION.md`，覆盖上方历史日志中的依赖与提示词待确认；不允许付费实调用、使用密钥、发布、提交或修改合并迁移。
- 真实 TTS 为配置驱动的兼容端点适配器；默认 none，提供显式 fake demo。不声称假服务代表真实质量。
- Next 16 使用 Node runtime 的 `proxy.ts`；API 路由仍自行验权。密码门禁不扩展为账号体系。
- UI 使用 Tailwind + 原生最小可访问组件，保留 components.json 与 cn，不增加 Radix 依赖。PWA 仅 manifest / 图标，离线 service worker 仍属远期。
- （历史方案，已被下方审查修复替代）Next proxy request buffer 设为 22 MB，导入流 / 文件仍限 20 MB（另留少量 multipart 元数据）；避免框架默认 10 MB 截断有效文件。JSON 请求限 64 KiB。
- 设置缺配显示与启动 fail-fast 不可同时观察时，按已定 fail-fast：启动退出 1，报缺少变量名，不泄漏密钥。设置页展示有效配置的安全状态。
- 根工具复用已有 workspace core / providers 与 zod；严格 TS 含 exactOptionalPropertyTypes。评测每次 IO 前预留保守费用上界，输出上限 2048 token，真实脚本必须 --allow-paid 且预算 ≤$1；所有调用写 llm_calls，fake 用内存库。
- GitHub Actions、Node 24、pnpm 12.11.2 与 standalone Docker 文件已实现；本机 Node 26 测试通过，Docker 和远程 CI 未执行。真实评测、完整 spike 与留存仍手动待验，见 `docs/WEB-VALIDATION.md`。

### MVP 审查修复（同一授权，离线验证）

- 保留所有既有迁移与 MVP 实现，不新增依赖、迁移、提示词版本或付费调用。TTS 以追加事件预留完整文本字符；独立提交早于 provider、blob 和缓存事务，任何不确定失败不退款。linked audio_generate 不重复计费；历史成功事件与 audios 兼容保留。此为保守护栏，不是 provider 账单。
- 评分在事务内重新构建当前队列资格，拒绝每日新词/review 配额耗尽、未到期/旧标签页重提；learning/relearning 按既定规则不占 review 配额。
- JSON 修复允许确定性过滤原回复与诊断的 control 词；保持 prompt 文件语义与版本不变，调用日志仍保留原诊断。变体斜线先保护，再剥音标，保留无空格附着 IPA 的既有行为。
- .apkg 允许 64 MiB，CSV/TXT 20 MiB；multipart 总流限 64 MiB + 64 KiB，proxy buffer 65 MiB，集合解压仍 100 MiB。原 20 MiB 单一限制会拒绝私有牌组，因此按审查明确需求调整。峰值内存与并发大文件性能未测。
- standalone 同源校验使用直接 Host 与请求协议而不是监听地址；不以 Forwarded/X-Forwarded-* 推断可信公共地址。反向代理必须显式 APP_ORIGIN，仅允许 http/https origin，启动校验；HTTPS override 启用 Secure cookie，代理须正确限制域名/阻止后端绕过。
- 阅读反馈保存到父级最新数据，折叠后仍显示选择；基线导入 wordsAdded/cardsAdded 分开显示；抽屉每次打开读取 lookupMode，自动建卡警告与实际 cardId/inboxId 结果明确显示。
- 新增导入→KnownSet→原创文章分析、五表导入回滚和评分重开回归。真实四级语料与 Anki 兼容/默认双导出证据仍未补齐，不把合成测试当作真实验收。
