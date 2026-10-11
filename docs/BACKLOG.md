# 任务总表(BACKLOG)

来源:`docs/plan.md` §8、§9.2–9.5。agent 按依赖顺序领取任务,完成后把状态改为“完成”。

规模:S 小 / M 中。每个任务对应 `docs/specs/` 里的一份 spec;spike 的结论写入 `docs/spikes/Sx.md`。

## 任务表

| ID | 任务 | 依赖 | 规模 | 状态 |
| --- | --- | --- | --- | --- |
| T0.1 | monorepo、lint、测试、CI、AGENTS.md | — | S | 远程 CI / 镜像构建通过；compose 待验 |
| T0.2 | Drizzle schema、迁移、仓储层 | T0.1 | S | 完成（自动化验证） |
| T0.3 | providers 接口与 Fake 实现 | T0.1 | S | 完成（自动化验证） |
| S1 | 词形还原 spike | — | S | 有限实测；待完整验证 |
| S2 | .apkg 格式 spike | — | S | 合成 + 单一私有文件计数实测；双格式待验 |
| S4 | 词表导入 spike | — | S | 规则已测；待用户词表 |
| T1.1 | CSV 与基础 .apkg 导入 | T0.2, S2 | M | 功能已验；真实双格式兼容待验 |
| T1.2 | FSRS 复习队列与评分 | T0.2 | M | 完成（自动化验证） |
| T1.3 | `/review`、`/library` 页面 | T1.1, T1.2, T0.3 | M | 完成（自动化验证） |
| T1.4 | review\_logs 与 events 写入 | T1.2 | S | 完成（自动化验证） |
| T1.5 | `/settings` 页面 | T0.2, T0.3, T1.2 | S | 功能已验；缺配时启动退出 |
| T2.1 | 分词、词形还原、analyzeText | T0.1, S1 | M | 功能已测；完整语料待验 |
| T2.2 | 基线词表导入 | T2.1, S4, T1.1 | S | 功能已测；规模 / 用户词表待验 |
| T2.3 | KnownModel v0 | T2.2, T1.2 | S | 完成（自动化验证） |
| S3 | 提示词评测 spike | T0.3, T2.1 | S | 工具完成；真实评测未运行 |
| T3.1 | selectTargets | T1.2, T2.3 | S | 完成（自动化验证） |
| T3.2 | generateArticle(含重试) | T3.1, T0.3, S3 | M | 完成（自动化验证） |
| T3.3 | `/article/[id]` 页面 | T3.2 | M | 完成（自动化验证） |
| T3.4 | `/today` 入口 | T3.3 | S | 完成（自动化验证） |
| T4.1 | 点词弹层与语境释义(缓存) | T3.3, T0.3 | M | 完成（自动化验证） |
| T4.2 | 三按钮与 lookupMode | T4.1, T1.2 | S | 完成（自动化验证） |
| T4.3 | `/inbox` | T4.2 | S | 完成（自动化验证） |
| T5.1 | 音频生成与缓存 | T3.2, T0.3 | M | 完成（自动化验证） |
| T5.2 | 播放器(变速、先听后看) | T5.1, T6.1 | S | 完成（自动化验证） |
| T5.3 | 成本护栏 | T5.1 | S | 完成（自动化验证） |
| T6.1 | 难度反馈与“不自然”标记 | T3.3 | S | 完成（自动化验证） |
| T6.2 | experiment 报告脚本 | T3.2, T1.4 | S | 完成（含 CLI 种子库验证） |
| T6.3 | `/stats` | T1.4, T6.2, T6.1 | S | 完成（自动化验证） |

## Services 阶段进度（历史，仅后端）

M0–M6 的仓储/provider/core 与服务编排已补齐，自动验证与 API 导出见 `docs/SERVICES-HANDOFF.md`。页面、routes、CI、部署、工具脚本及 e2e 未完成，因此本阶段不将混合任务整项标为“完成”。S1/S2/S4 有实测的有限结论，但缺完整语料/真实导出/用户词表证据；S3 的真实付费评测和 M6 的长期自用验证未执行。下一阶段应按现有依赖关系接入这些 services，并以全栈验收结果更新上表。

## Web 阶段进度

七页面、API、安全门禁、PWA、部署文件、工具与端到端闭环已实现。实测为 237 单元 / 集成测试、4 Chromium E2E、生产构建、standalone 临时文件库重启持久化与配置 fail-fast；详情见 `docs/WEB-VALIDATION.md`。Docker 本机不可用；修复首次冷启动 E2E 失败后，远程 CI / Docker 镜像构建已通过（run 38135610501，239 单测 / 4 E2E）。compose / 容器重启、真实 provider / 完整 spike / 长期自用证据不标完成。表中的“完成（自动化验证）”只表示对应可运行功能与离线测试，不代表全部里程碑的真实质量或人工验收。

## 规格索引

每个任务的 spec 在 `docs/specs/` 下,文件名以任务 ID 开头,例如 `docs/specs/T0.1-monorepo-scaffold.md`。所有 spec 共用的约定(时间与日界、事件类型、服务层位置等)见 `docs/specs/README.md`。

## 里程碑与验收

### M0 脚手架(S)

- T0.1 monorepo、lint、Vitest、Playwright、CI、AGENTS.md
- T0.2 Drizzle schema、迁移、仓储层
- T0.3 providers 接口与假实现(FakeLLM / FakeTTS / MemoryBlob)
- 验收:`pnpm test`、`pnpm lint`、`pnpm typecheck` 全绿;空库迁移成功;端到端冒烟(首页可打开)通过。

### M1 词库与复习闭环(M)

- S2 .apkg 格式 spike(新旧格式、字段结构)
- T1.1 CSV 与基础 .apkg 导入;T1.2 FSRS 复习队列与评分;T1.3 `/review`、`/library`(含缺释义时的 AI 释义);T1.4 review\_logs 与 events 写入;T1.5 `/settings`
- 验收:导入 100 词 CSV 后可完成一轮复习;4 档评分对应的 due 变化有单测;每次评分产生 1 条 review\_log 和 1 条 event;重启后数据仍在。

### M2 分析器与已知词(M,可与 M1 并行)

- S1 词形还原 spike:准备约 200 句标注样本,比较候选方案,词元级准确率建议 ≥95%
- S4 词表导入 spike:用户词表(TXT/CSV)里常见的变形、短语和重复项如何归一到词元,产出导入规则
- T2.1 分词、词形还原、`analyzeText`;T2.2 基线词表导入(标为“基线”或“学习词书”);T2.3 KnownModel v0
- 验收:golden 测试通过(缩写、专有名词、复数与时态);对 20 篇样例文本输出稳定;导入高中 3500 词表后,可计算四级难度样例文章的覆盖率;短语与变形词的导入规则有测试覆盖。

### M3 文章生成(M)

- S3 提示词评测 spike:30 组目标词样例,记录 JSON 合法率、coverage、目标词出现次数
- T3.1 `selectTargets`;T3.2 `generateArticle`(含重试);T3.3 `/article/[id]`;T3.4 `/today` 入口
- 验收:FakeLLM 集成测试覆盖 JSON 非法、coverage 不足触发重试、目标词缺失三种情况;真实 LLM 评测(手动触发)JSON 合法率 ≥95%,“适中”档未知词占比中位数 ≤5%;`llm_calls` 有记录。

### M4 点词交互(M)

- T4.1 点词弹层与语境释义(缓存);T4.2 三按钮与 lookupMode;T4.3 `/inbox`
- 验收:`markOnly` 下标记只产生 event 和 inbox 行,不产生卡片;“加入复习”产生卡片并计入每日新词上限;同一(词,句)的释义第二次命中缓存,不再调用 LLM。

### M5 TTS 听力(S–M)

- TTS 服务商不做盲听选型:到 M5 时由作者直接指定一家(待定),agent 按 `TTSProvider` 接口接入
- T5.1 音频生成与缓存;T5.2 播放器(变速、先听后看);T5.3 成本护栏
- 验收:同一文章重复请求不重复调用 TTS;超过每日上限返回明确错误;播放器端到端测试通过(FakeTTS);切换 `TTSProvider` 实现只需改配置。

### M6 度量与自用验证(S)

- T6.1 难度反馈与“不自然”标记;T6.2 experiment 报告脚本;T6.3 `/stats`
- 验收:报告脚本在种子数据上输出正确的差值与样本量;stats 数字与 review\_logs 对账一致。
- 之后自用 2–4 周,产出一页验证结论(H1–H3)。

## 任务 spec 模板

每个任务一份 `docs/specs/Txx-名称.md`,包含:

1. 目标
2. 范围与非范围
3. 接口(类型签名)
4. 验收标准(可自动验证)
5. 测试计划
6. 依赖与风险

spike(S1–S4)的产出写入 `docs/spikes/Sx.md`:数据、结论、选型理由。spike 代码不直接进入产品。

## 工作方式

- 先写 spec 和失败的测试,再实现;一个 PR 只做一个任务,变更尽量小。
- 原子化提交:每个提交只做一件逻辑上完整的事,且单独通过 lint / typecheck / test;依赖变更、schema 迁移、提示词变更各自单独提交;提交信息用 Conventional Commits(英文),正文注明 `Task: Txx`。
- 验收标准写成可执行的测试,agent 自行跑通后再提交。人只看测试覆盖不到的东西:体验、提示词质量、取舍是否合理。
- 人工必审:schema 迁移、提示词变更、新增依赖、许可与密钥相关改动。

### 并行

M1 与 M2 的包不重叠,可并行。同一时间只允许一个任务改 schema。提示词与分析器的改动先过评测再合并。

### 评测与可复现

LLM 与 TTS 调用全部经 providers,测试默认使用 Fake。`pnpm eval:generation` 手动触发真实调用,带费用上限(建议 $1),输出 JSON 合法率、coverage、目标词出现次数等指标表,并写入当前配置的模型名。提示词变更必须附评测前后对比。

## 审查修复证据

已修复两轮具体领域/部署/UI 问题：TTS 失败绕配额、评分资格/日上限、control 修复泄漏、变体+音标、导入回滚/分析与复习重开覆盖、standalone Origin、64 MiB apkg、阅读选择保持、基线新增统计、autoAdd 模式警告/回显。完整命令和离线 standalone（0.0.0.0 监听与显式 HTTPS 公共来源）实测见 WEB-VALIDATION。父 agent 额外验证 CLI 种子库出现组 10 样本 / Again 0.2、对照组 5 样本 / Again 0.4、差值 −0.2，第二次复习不影响结果。S2 新计数为父审查提供，不复制/分发牌组；S1/S2/S3/S4 真实完整验收与长期 H1–H3 仍不标完成。
