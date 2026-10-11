# 英语学习软件:MVP 方案与远期规划

面向 Agent 开发 · v0.2 草案 · 2026-10-10

## 0. 阅读说明与已定决策

本文档按“可以直接交给编码 agent 执行”的粒度编写:每个里程碑都有交付物和可自动验证的验收标准,关键接口、数据表、提示词骨架已给出。

**假设**:“Agent 开发”指用编码 agent(如 Claude Code)实现本项目,人负责拍板和审查。产品内的 AI 流程在 MVP 阶段是确定性的工作流,而不是自主 agent(原因见 §9.6)。

**已定决策**

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
| TTS 服务商 | M5 前通过盲听选型(S5);服务商、模型、音色全部走配置 | 待定 |
| 开源许可证 | 暂缓:MVP 期间仓库保持私有,不添加 LICENSE 文件 | 暂缓 |

待确认事项见 §12。

### 0.1 本文档的定位与展开方式

本文档是人与 agent 共用的单一事实来源,不是 AGENTS.md,也不是任务文档。agent 应通读后,按下表把它拆成仓库文件;不要把全文复制进 AGENTS.md,因为 AGENTS.md 每次会话都会加载,必须短小。

| 仓库文件 | 来源章节 | 说明 |
| --- | --- | --- |
| `AGENTS.md`(英文,≤100 行) | §9.1 | 命令、架构边界、约定、完成定义 |
| `docs/PRD.md` | §0、§1、§2 | 目标、范围、假设 |
| `docs/ARCHITECTURE.md` | §3、§4 | 选型、目录、边界 |
| `docs/DATA_MODEL.md` | §5 | 表结构与约定 |
| `docs/SPEC-CORE.md` | §6 | 各功能规格 |
| `docs/BACKLOG.md` | §8.0 | 任务总表(ID、依赖、规模、状态) |
| `docs/specs/Txx-*.md` | §8、§9.2 | 每个任务一份,按模板扩写 |
| `docs/ROADMAP.md` | §11 | 远期规划与进入条件 |
| `docs/DECISIONS.md` | §0.2、§12 | 决策记录(已定、待定、变更) |

交给本地 agent 的第一条指令:

```
Read docs/PLAN.md (the full plan) first.
1. Split it into the repo files listed in section 0.1. Never drop items marked 已定; never decide items marked 待定 yourself.
2. Produce AGENTS.md (English, at most 100 lines) and docs/BACKLOG.md first, then stop for my review.
3. Then generate docs/specs/Txx-*.md in BACKLOG order using the section 9.2 template; stop after every 3 specs.
4. Stop and ask me in every situation listed in section 0.3.
5. Do not write product code before the M0 specs are approved.
```

### 0.2 状态标记

**已定**:作者拍板,agent 不得改动。**默认**:建议值,可调整,但改动要记入 `docs/DECISIONS.md`。**待定**:尚未决定,agent 遇到必须停下来问人。正文里未标记的规格视为“默认”。

**暂缓**:有意推迟的事项(目前只有开源许可证)。agent 不处理,也不必为此提问。

### 0.3 必须停下来问人的情形

- 遇到“待定”项。
- 要新增依赖。
- 要修改已合并的数据库迁移。
- 要改变提示词的语义。
- 涉及密钥、付费 API 调用、许可证的操作。
- 规格互相矛盾,或缺失到写不出可测试的验收标准。

### 0.4 术语表

| 术语 | 含义 |
| --- | --- |
| 词元(lemma) | 词形还原后的原形,如 ran → run |
| token | 文本中的一个词次,出现几次算几个 |
| 词种(type) | 去重后的不同词 |
| 覆盖率(coverage) | 已知词 token 数 ÷ 总 token 数,含目标词,跳过数字与专有名词 |
| 已知词集合(KnownSet) | §6.4 定义的确定性集合 |
| 目标词 | 本篇文章要重点复现的词 |
| exposed / control | 出现在文章中的目标词 / 刻意留出不出现的对照词 |
| 档位(tier) | 轻松、适中、挑战三档覆盖率目标 |
| 事件(event) | `events` 表中的一条只追加记录 |

### 0.5 语言约定

AGENTS.md、代码标识符、提交信息用英文;产品文档保持中文即可;面向用户的界面文案默认中文。

## 1. 目标与验证假设

MVP 只回答一个问题:**用我已背的词生成的文章和听力,值不值得每天用?** 拆成三个假设(阈值是初始建议,可调整):

| 假设 | 衡量 | 初始目标 |
| --- | --- | --- |
| H1 愿意每天用 | 自用 4 周内每周打开天数、文章阅读完成率 | 每周 ≥5 天;完成率 ≥60% |
| H2 文章质量可接受 | 难度反馈中“刚好”占比;文章未知词占比中位数 | ≥60%;≤5% |
| H3 比纯词卡更好记 | 文章中出现的词 vs 对照词,之后复习的 Again 率 | 出现组更低(样本小,仅作参考) |

“愿意为省事付费”(H4)属于 SaaS 阶段,不在 MVP 验证。

## 2. MVP 范围

**做**:导入词表(CSV、基础 .apkg)→ FSRS 复习 → 用当天词生成文章 → 点词标记与可选加入复习 → 文章 TTS 音频 → 事件日志与统计 → 难度反馈与对照实验。

**不做**:词汇量初测(先让用户选等级)、外刊匹配、真实内容导入、逐句听力与听写、生成-校验多轮重写(MVP 最多重试 1 次)、账号体系、同步、手机端和桌面端。

## 3. 技术选型

选型原则:主流、类型安全、文档多、易测试,方便 agent 实现与自检。

| 层 | 选择 | 备注 |
| --- | --- | --- |
| 语言与仓库 | TypeScript,pnpm monorepo | 类型即契约 |
| Web | Next.js(App Router)+ Tailwind + shadcn/ui,带 PWA manifest | 全栈一体,SaaS 阶段同栈 |
| 数据库 | SQLite(better-sqlite3)+ Drizzle ORM | 自托管零运维;Drizzle 也支持 Postgres,便于以后迁移 |
| 校验 | zod | LLM 输出、API 入参统一校验 |
| 复习调度 | ts-fsrs | 开源 FSRS 实现 |
| 词形还原 | 接口 `Lemmatizer`;候选 wink-lemmatizer 或离线词形表 | 由 spike S1 决定 |
| LLM | `LLMProvider` 接口;适配器基于 openai SDK;`LLM_BASE_URL`、`LLM_API_KEY`、`LLM_MODEL` 全部来自环境变量 | 当前填 DeepSeek 的取值;代码和文档里不写死任何模型名;见 §6.5.1 |
| TTS | `TTSProvider` 接口;服务商、模型、音色全部来自配置;浏览器 SpeechSynthesis 作零成本兜底 | 待定:服务商由 spike S5 决定,见 §12 |
| 文件存储 | `BlobStore` 接口,MVP 用本地文件 | 以后换对象存储 |
| 测试 | Vitest(单元)、Playwright(端到端)、固定样本评测脚本 | 见 §9 |
| 质量与 CI | Biome 或 ESLint+Prettier、GitHub Actions |  |
| 部署 | `pnpm dev`;`docker compose up`(单容器 + 数据卷) | 设置 `APP_PASSWORD`,避免暴露公网时被他人消耗你的 API 额度 |

## 4. 架构与目录

```
/
├─ AGENTS.md                 # agent 工作规则(见 §9.1)
├─ docs/specs/               # 每个任务一份 spec: Txx-*.md
├─ prompts/                  # 版本化提示词: article.v1.md ...
├─ fixtures/                 # 测试样本、LLM 录制回放
├─ packages/
│  ├─ core/                  # 纯逻辑,禁止 IO
│  │  ├─ lexicon/            # 分词、词形还原、词表
│  │  ├─ analyzer/           # analyzeText
│  │  ├─ knowledge/          # KnownModel v0
│  │  ├─ srs/                # ts-fsrs 封装与队列规则
│  │  └─ generation/         # 选词与文章生成工作流
│  ├─ providers/             # LLM / TTS / BlobStore 接口与实现
│  └─ db/                    # Drizzle schema、迁移、仓储层
└─ apps/web/                 # Next.js: 页面 + API 路由
```

**边界规则**:`core` 通过接口注入依赖,不直接依赖 db 与 providers 的实现;`web` 只通过仓储层访问数据库;所有写操作同时写 `events`。

## 5. 数据模型

```
users(id, created_at)                        -- MVP 只有一行,但所有表都带 user_id
lexemes(id, lemma, pos, UNIQUE(lemma,pos))   -- 词的统一标识,与词库解耦
cards(id, user_id, lexeme_id, front, back_json, deck, suspended,
      due, stability, difficulty, state, reps, lapses, last_review, updated_at)
review_logs(id, user_id, card_id, rating, state_before, due_before,
            reviewed_at, duration_ms)        -- 只追加,永不修改
events(id, user_id, type, payload_json, ts) -- 只追加
   -- type: lookup | mark_unknown | mark_known | add_to_review | article_open
   --       | audio_play | difficulty_feedback | unnatural_flag ...
articles(id, user_id, kind, title, body, genre, level_tier, prompt_version,
         model, source, license, analysis_json, created_at)
article_targets(article_id, lexeme_id, role, occurrences)  -- role: exposed | control
audios(id, article_id, provider, voice, blob_key, duration_s, text_hash)
inbox(id, user_id, lexeme_id, context_sentence, article_id, status, created_at)
gloss_cache(lexeme_id, context_hash, gloss_json)           -- 语境释义缓存
llm_calls(id, user_id, purpose, provider, model, tokens_in, tokens_out,
          cost_usd, latency_ms, ts)          -- 成本与调试;SaaS 阶段直接复用
```

**为 SaaS 预留的低成本约定**:id 用 UUIDv7;所有表有 `user_id` 与 `updated_at`;`events` 和 `review_logs` 只追加;文章记录来源与许可字段。

## 6. 关键规格

### 6.1 复习

默认每日新词 20、复习上限 200、目标保持率 0.9(设置可调),使用 ts-fsrs 默认参数,4 档评分(Again / Hard / Good / Easy)。每次评分写 1 条 `review_logs` 和 1 条 `events`。

### 6.2 选词 `selectTargets(userId, n=8)`

优先级:① 今天评了 Again/Hard 的词;② 今天新学的词;③ 已到期且可提取率最低的词。排除近 3 天已在文章中出现 ≥3 次的词。候选中随机 20% 标为 `control`:不进入文章,但记录到 `article_targets`,用于验证 H3。

### 6.3 分析器 `analyzeText(text, known)`

```ts
type AnalysisResult = {
  tokenCount: number;
  coverage: number;                       // 已知词 token 占比(含目标词)
  unknownLemmas: { lemma: string; count: number }[];
  targetOccurrences: Record<string, number>;  // lemma -> 出现次数
  names: string[];                        // 被判为专有名词而跳过的词
};
```

规则:小写化;分词时保留缩写(don't);词形还原;跳过数字与专有名词(句中大写词,或 LLM 返回的 `names`);功能词默认已知。覆盖率按 token 计,同时报告按词种(type)的值。

### 6.4 已知词模型 v0(确定性集合)

已知词 = 用户导入并标为“基线”的词表 ∪ 状态为 Review 且可提取率 ≥0.8 的卡片词 ∪ 用户标记“认识”的词 ∪ 本次目标词。排除:近期被标记“不认识”且之后未复习过的词。概率化模型留到后期。

### 6.4.1 词表来源

**默认**:MVP 不内置任何词表,导入时由用户把词表标为“基线(已知)”或“学习词书”。作者本人的设置:基线 = 高中 3500 词,学习词书 = 四级词书。词表文件格式沿用 §6.9。公开发布前才需要内置词表,届时必须逐个核实许可(NGSL、CEFR-J 等各有条款)并记录到 `docs/licenses.md`,这不属于 MVP。

### 6.5 文章生成工作流 `generateArticle`

步骤:`selectTargets` → 构建提示词 → LLM(JSON 输出,zod 校验)→ `analyzeText` → 若 `coverage` 低于所选档位下限,带“需避免的词”重试 1 次(重试次数为配置项)→ 落库(articles、article\_targets、analysis\_json、llm\_calls)→ 返回。

- 难度档位:轻松 ≥98% / 适中 95–98% / 挑战 90–95%,默认适中。
- 长度默认 300–400 词(约 2–3 分钟音频);体裁:短故事 / 对话 / 独白。

提示词骨架(`prompts/article.v1.md`,版本号写入 `articles.prompt_version`):

```
System: You write short reading/listening texts for English learners.
Task: Write a {genre} of {minWords}-{maxWords} words for a learner at CEFR {level}.
Target words: {targets}
Rules:
- Use each target word at least twice, in contexts that make its meaning inferable.
- Apart from the target words, use only common, high-frequency vocabulary.
- {avoidWords ? "Do NOT use these words: " + avoidWords : ""}
- Natural spoken-style sentences; no lists of words; no explanations.
Output JSON only: {"title": string, "body": string, "names": string[]}
```

### 6.5.1 LLM 调用约定

- 走 OpenAI 兼容接口。`LLM_BASE_URL`、`LLM_API_KEY`、`LLM_MODEL` 全部来自环境变量;代码、提示词、测试和文档里不出现具体模型名,`.env.example` 只放占位符。当前由你在 `.env` 里填 DeepSeek 的取值。
- 启动时检查这三个变量,缺失则给出明确报错,不使用任何默认模型。
- 更换模型只改 `.env`,不改代码。每次调用的模型名记入 `llm_calls.model` 与 `articles.model`;评测脚本(§9.5)按当前配置运行,并把模型名写进结果。
- TTS 同理:服务商、模型、音色全部来自配置,不写进代码。
- 结构化输出:先尝试 JSON 模式,不支持或失败时退回提示词约束。无论哪种,输出都必须过 zod 校验;校验失败用“修复提示”重试 1 次,仍失败则返回明确错误并记入 `llm_calls`。各家对 JSON 严格度不同,不能假设输出总是合规。
- 文章生成优先选便宜快速的非推理型号,推理型号只留给评测或复杂校验;具体型号由你在 `.env` 里选。
- 提示词骨架里的 `{level}` 取自设置项,作者的默认值为“B1(约中国大学英语四级水平)”。

### 6.6 点词交互

点击正文任意词,弹层显示:词形还原后的词、发音、“在本句中的释义”(LLM 生成,缓存到 `gloss_cache`),以及三个按钮:**认识 / 标记不认识 / 加入复习**。

设置项 `lookupMode`:`markOnly`(默认:只记录并进入 inbox,不建卡)、`autoAdd`(直接建卡)。加入复习的卡片计入每日新词上限;`/inbox` 页可批量加入或忽略。

### 6.7 听力

文章页一键生成音频(整篇一条,按文本哈希缓存,相同文本不重复合成)。播放器:变速(0.75–1.25)、先听后看(正文默认折叠)、听完提示难度反馈。逐句重复与听写留到后续版本。成本护栏:每日 TTS 字符数上限(设置项)。

### 6.8 度量

- 难度反馈(太难 / 刚好 / 太简单)和“不自然”标记写入 `events`。
- `pnpm report:experiment` 输出 exposed 与 control 两组的 Again 率和样本量。
- `/stats` 展示:今日与本周新学、复习数量,已学词列表,文章覆盖率趋势。

### 6.9 导入

CSV(列:word,meaning,example,后两列可选)。.apkg:解压并读取笔记字段,前两个字段映射为正面/背面,去 HTML,忽略媒体与调度数据;若检测到新版压缩格式,提示用户导出时选择兼容旧版(是否可行由 spike S2 确认)。缺释义的词在首次展示时由 LLM 生成简明释义并缓存,用户可编辑,来源标记为 AI 生成。

## 7. 页面清单

- `/today`:今日复习与新词、今日文章入口
- `/review`:复习界面
- `/article/[id]`:阅读 + 听力 + 点词
- `/library`:词卡列表、导入
- `/inbox`:待处理生词
- `/stats`:统计与对照实验结果
- `/settings`:等级、每日上限、lookupMode、provider 状态

## 8. 里程碑与任务

规模:S 小 / M 中。每个任务对应 `docs/specs/` 里的一份 spec。

### 8.0 任务总表

agent 按依赖顺序领取任务,完成后把状态改为“完成”。

| ID | 任务 | 依赖 | 规模 | 状态 |
| --- | --- | --- | --- | --- |
| T0.1 | monorepo、lint、测试、CI、AGENTS.md | — | S | 待办 |
| T0.2 | Drizzle schema、迁移、仓储层 | T0.1 | S | 待办 |
| T0.3 | providers 接口与 Fake 实现 | T0.1 | S | 待办 |
| S1 | 词形还原 spike | — | S | 待办 |
| S2 | .apkg 格式 spike | — | S | 待办 |
| S4 | 词表导入 spike | — | S | 待办 |
| S5 | TTS 盲听 spike | — | S | 待办 |
| T1.1 | CSV 与基础 .apkg 导入 | T0.2, S2 | M | 待办 |
| T1.2 | FSRS 复习队列与评分 | T0.2 | M | 待办 |
| T1.3 | `/review`、`/library` 页面 | T1.2 | M | 待办 |
| T1.4 | review\_logs 与 events 写入 | T1.2 | S | 待办 |
| T2.1 | 分词、词形还原、analyzeText | T0.1, S1 | M | 待办 |
| T2.2 | 基线词表导入 | T2.1, S4 | S | 待办 |
| T2.3 | KnownModel v0 | T2.2, T1.2 | S | 待办 |
| S3 | 提示词评测 spike | T0.3, T2.1 | S | 待办 |
| T3.1 | selectTargets | T1.2, T2.3 | S | 待办 |
| T3.2 | generateArticle(含重试) | T3.1, T0.3, S3 | M | 待办 |
| T3.3 | `/article/[id]` 页面 | T3.2 | M | 待办 |
| T3.4 | `/today` 入口 | T3.3 | S | 待办 |
| T4.1 | 点词弹层与语境释义(缓存) | T3.3, T0.3 | M | 待办 |
| T4.2 | 三按钮与 lookupMode | T4.1, T1.2 | S | 待办 |
| T4.3 | `/inbox` | T4.2 | S | 待办 |
| T5.1 | 音频生成与缓存 | T3.2, T0.3, S5 | M | 待办 |
| T5.2 | 播放器(变速、先听后看) | T5.1 | S | 待办 |
| T5.3 | 成本护栏 | T5.1 | S | 待办 |
| T6.1 | 难度反馈与“不自然”标记 | T3.3 | S | 待办 |
| T6.2 | experiment 报告脚本 | T3.2, T1.4 | S | 待办 |
| T6.3 | `/stats` | T1.4, T6.2 | S | 待办 |

**M0 脚手架(S)**

- T0.1 monorepo、lint、Vitest、Playwright、CI、AGENTS.md
- T0.2 Drizzle schema、迁移、仓储层
- T0.3 providers 接口与假实现(FakeLLM / FakeTTS / MemoryBlob)
- 验收:`pnpm test`、`pnpm lint`、`pnpm typecheck` 全绿;空库迁移成功;端到端冒烟(首页可打开)通过。

**M1 词库与复习闭环(M)**

- S2 .apkg 格式 spike(新旧格式、字段结构)
- T1.1 CSV 与基础 .apkg 导入;T1.2 FSRS 复习队列与评分;T1.3 `/review`、`/library`;T1.4 review\_logs 与 events 写入
- 验收:导入 100 词 CSV 后可完成一轮复习;4 档评分对应的 due 变化有单测;每次评分产生 1 条 review\_log 和 1 条 event;重启后数据仍在。

**M2 分析器与已知词(M,可与 M1 并行)**

- S1 词形还原 spike:准备约 200 句标注样本,比较候选方案,词元级准确率建议 ≥95%
- S4 词表导入 spike:用户词表(TXT/CSV)里常见的变形、短语和重复项如何归一到词元,产出导入规则
- T2.1 分词、词形还原、`analyzeText`;T2.2 基线词表导入(标为“基线”或“学习词书”);T2.3 KnownModel v0
- 验收:golden 测试通过(缩写、专有名词、复数与时态);对 20 篇样例文本输出稳定;导入高中 3500 词表后,可计算四级难度样例文章的覆盖率;短语与变形词的导入规则有测试覆盖。

**M3 文章生成(M)**

- S3 提示词评测 spike:30 组目标词样例,记录 JSON 合法率、coverage、目标词出现次数
- T3.1 `selectTargets`;T3.2 `generateArticle`(含重试);T3.3 `/article/[id]`;T3.4 `/today` 入口
- 验收:FakeLLM 集成测试覆盖 JSON 非法、coverage 不足触发重试、目标词缺失三种情况;真实 LLM 评测(手动触发)JSON 合法率 ≥95%,“适中”档未知词占比中位数 ≤5%;`llm_calls` 有记录。

**M4 点词交互(M)**

- T4.1 点词弹层与语境释义(缓存);T4.2 三按钮与 lookupMode;T4.3 `/inbox`
- 验收:`markOnly` 下标记只产生 event 和 inbox 行,不产生卡片;“加入复习”产生卡片并计入每日新词上限;同一(词,句)的释义第二次命中缓存,不再调用 LLM。

**M5 TTS 听力(S–M)**

- S5 TTS 盲听 spike:同一段约 300 词的文本分别用候选服务合成,盲听打分(自然度、清晰度、语速可控),并记录单篇成本;结论写入 `docs/spikes/S5.md`,由你拍板选定默认服务
- T5.1 音频生成与缓存;T5.2 播放器(变速、先听后看);T5.3 成本护栏
- 验收:同一文章重复请求不重复调用 TTS;超过每日上限返回明确错误;播放器端到端测试通过(FakeTTS);切换 `TTSProvider` 实现只需改配置。

**M6 度量与自用验证(S)**

- T6.1 难度反馈与“不自然”标记;T6.2 experiment 报告脚本;T6.3 `/stats`
- 验收:报告脚本在种子数据上输出正确的差值与样本量;stats 数字与 review\_logs 对账一致。
- 之后自用 2–4 周,产出一页验证结论(H1–H3)。

## 9. Agent 开发工作流

### 9.1 AGENTS.md 骨架

```
# AGENTS.md
## 命令
pnpm install | pnpm dev | pnpm test | pnpm lint | pnpm typecheck | pnpm e2e
pnpm eval:generation   # 手动触发,真实调用,带费用上限
## 架构边界
- packages/core 禁止 IO(无 fetch、fs、db);依赖通过接口注入
- 所有 LLM/TTS 调用必须经 packages/providers;测试默认使用 Fake 实现
- 模型名、音色名、服务商端点只出现在配置(.env / 设置)里,不得写进代码、提示词或测试
- 所有写操作必须同时写 events(复习另写 review_logs);这两张表只追加
- 业务代码不得直接访问 db,统一走仓储层
## 约定
- 提示词放 prompts/ 并带版本号;改提示词必须附评测前后对比
- 新增依赖、schema 迁移、提示词变更:提交前停下,等待人工确认
- 不提交密钥;不修改已合并的迁移文件
- 不添加 LICENSE 文件,也不在 package.json 里声明开源许可证(暂缓)
## 完成定义(DoD)
- 对应 spec 的验收标准全部有测试覆盖且通过
- test / lint / typecheck 全绿
- 涉及数据写入时,events 已写入并有断言
- docs/specs/ 已同步更新;涉及词表时 docs/licenses.md 也同步更新
```

### 9.2 任务 spec 模板

每个任务一份 `docs/specs/Txx-名称.md`,包含:目标、范围与非范围、接口(类型签名)、验收标准(可自动验证)、测试计划、依赖与风险。

spike(S1–S4)的产出写入 `docs/spikes/Sx.md`:数据、结论、选型理由。spike 代码不直接进入产品。

### 9.3 工作方式

- 先写 spec 和失败的测试,再实现;一个 PR 只做一个任务,变更尽量小。
- 验收标准写成可执行的测试,agent 自行跑通后再提交。人只看测试覆盖不到的东西:体验、提示词质量、取舍是否合理。
- 人工必审:schema 迁移、提示词变更、新增依赖、许可与密钥相关改动。

### 9.4 并行

M1 与 M2 的包不重叠,可并行。同一时间只允许一个任务改 schema。提示词与分析器的改动先过评测再合并。

### 9.5 评测与可复现

LLM 与 TTS 调用全部经 providers,测试默认使用 Fake。`pnpm eval:generation` 手动触发真实调用,带费用上限(建议 $1),输出 JSON 合法率、coverage、目标词出现次数等指标表。提示词变更必须附评测前后对比。

### 9.6 产品内 AI 是工作流,不是自主 agent

选词、生成、校验、重试都是确定性代码,LLM 只负责写文本和给释义。这样可测试、可控、可计量成本。自主 agent(如 AI 导师)留到后期,届时把 `core` 里的函数作为工具暴露给它。

## 10. 风险与对策

| 风险 | 对策 |
| --- | --- |
| 文章套路化、目标词被机械堆叠 | 目标词 ≤8 个;体裁轮换;用户“不自然”标记写入 events,供后续调提示词 |
| 词形还原误差导致覆盖率失真 | S1 选型 + golden 测试;覆盖率只作档位参考,不作硬性判定 |
| 释义错误或不贴合语境 | 用户可编辑;来源标记为 AI 生成 |
| LLM / TTS 成本失控 | `llm_calls` 全量记录;每日 TTS 字符上限;重试次数上限 |
| .apkg 新版格式解析困难 | S2 提前验证;CSV 作为兜底路径 |
| 词表或词库许可问题 | S4 提前核实;自用阶段与公开发布前分别处理 |
| 范围膨胀 | MVP 清单之外的需求一律进 backlog,过 §11 的进入条件才排期 |

## 11. 远期规划

**Phase 1:MVP 与自用验证(M0–M6)**

交付 §8 的全部里程碑,自用 2–4 周,产出 H1–H3 的验证结论。

**Phase 2:开源发布与体验打磨(v1.x)**

- 进入条件:H1、H2 达标。
- 听力:逐句重复、遮词听写;听力专用脚本(对话、多音色)。
- 四级备考:四级听力题型模拟(短对话、长对话、短文听力加选择题),脚本与题目原创,不使用真题原文。
- 内容质量:生成-校验-重写闭环(最多 3 轮);导入文本并评估覆盖率。
- 用户模型:词汇量初测(自建分层 yes/no 测试;现成测试需核实许可);难度档位随反馈自适应。
- 复习:完整 .apkg 导入(复习日志用于初始化 FSRS);FSRS 参数优化。
- 输出练习:读后复述、造句,并给 AI 反馈。
- 工程:PWA 离线;开源许可证暂缓,公开仓库前再决定(见 §12)。

**Phase 3:SaaS 化**

- 进入条件:H1–H3 有正面证据,并有外部试用者的留存数据。
- 账号与多租户(Postgres)。
- 同步:基于只追加事件 + UUID + 时间戳。
- 托管模型网关、配额与计费(复用 `llm_calls`)。
- 内置词库(词典 + 考纲),逐个核实许可。
- 内容库:只收开放许可或公共领域来源;外刊匹配走检索,不让 AI 改写禁止改编(ND)的内容。
- 合规与支付:隐私政策;面向中国大陆的备案要求另行核实;支付与应用商店规则。

**Phase 4:多端**

- 手机端:同为 TypeScript monorepo,优先评估 React Native/Expo 复用 `core`,或 PWA 加 Capacitor 外壳。
- 桌面端:Tauri 或 Electron 外壳。
- 离线优先与冲突合并。

**Phase 5:智能化**

- 概率化词汇量模型(FSRS 可提取率 + 点词标记 + 难度反馈)。
- 个性化主题与长期内容规划。
- AI 导师 agent,通过工具调用 `core` API。
- 真实内容导入(播客转写)与难度评估。
- 口语与发音(语音识别)。

## 12. 待确认

1. **TTS 服务商(待定,M5 前确定,不阻塞 M0–M4)**:做 spike S5 盲听后由你选定。候选只作参考(OpenAI、ElevenLabs、Azure、Google 等),服务商、模型、音色一律走配置,不写进代码。面向中国大陆的 SaaS 阶段需另行核实各家的可用性与合规。
2. **开源许可证(暂缓)**:MVP 期间仓库保持私有,agent 不添加 LICENSE 文件,也不在 package.json 里声明开源许可证;公开仓库前再决定。
3. **agent 工具与仓库托管(不阻塞)**:由执行的 agent 在写 AGENTS.md 与 CI 时按所用工具自行适配,并记入 `docs/DECISIONS.md`。
4. **基线词表做法(默认,按此执行)**:MVP 不内置词表,由作者导入高中 3500 词与四级词书。
5. **模型配置**:LLM 与 TTS 的模型名都不写死,见 §6.5.1。
