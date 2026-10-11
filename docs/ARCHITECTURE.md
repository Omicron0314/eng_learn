# 架构(ARCHITECTURE)

来源:`docs/plan.md` §3、§4、§9.6。

## 1. 技术选型

选型原则:主流、类型安全、文档多、易测试,方便 agent 实现与自检。

| 层 | 选择 | 备注 |
| --- | --- | --- |
| 语言与仓库 | TypeScript,pnpm monorepo | 类型即契约 |
| Web | Next.js(App Router)+ Tailwind + shadcn/ui,带 PWA manifest | 全栈一体,SaaS 阶段同栈 |
| 数据库 | SQLite(better-sqlite3)+ Drizzle ORM | 自托管零运维;Drizzle 也支持 Postgres,便于以后迁移 |
| 校验 | zod | LLM 输出、API 入参统一校验 |
| 复习调度 | ts-fsrs | 开源 FSRS 实现 |
| 词形还原 | 接口 `Lemmatizer`;候选 wink-lemmatizer 或离线词形表 | 由 spike S1 决定 |
| LLM | `LLMProvider` 接口;适配器基于 openai SDK;`LLM_BASE_URL`、`LLM_API_KEY`、`LLM_MODEL` 全部来自环境变量 | 当前填 DeepSeek 的取值;代码和文档里不写死任何模型名;见 `docs/SPEC-CORE.md` §5.1 |
| TTS | `TTSProvider` 接口;服务商、模型、音色全部来自配置;浏览器 SpeechSynthesis 作零成本兜底 | 待定:服务商由 spike S5 决定 |
| 文件存储 | `BlobStore` 接口,MVP 用本地文件 | 以后换对象存储 |
| 测试 | Vitest(单元)、Playwright(端到端)、固定样本评测脚本 | 见 `docs/BACKLOG.md` |
| 质量与 CI | Biome 或 ESLint+Prettier、GitHub Actions | 默认;T0.1 选定后记入 `docs/DECISIONS.md` |
| 部署 | `pnpm dev`;`docker compose up`(单容器 + 数据卷) | 设置 `APP_PASSWORD`,避免暴露公网时被他人消耗你的 API 额度 |

## 2. 目录

```
/
├─ AGENTS.md                 # agent 工作规则
├─ docs/specs/               # 每个任务一份 spec: Txx-*.md
├─ docs/spikes/              # spike 结论: Sx.md
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

## 3. 边界规则

- `core` 通过接口注入依赖,不直接依赖 db 与 providers 的实现;`core` 内禁止 IO(无 fetch、fs、db)。
- `web` 只通过仓储层访问数据库。
- 所有写操作同时写 `events`(复习另写 `review_logs`)。
- 所有 LLM/TTS 调用必须经 `packages/providers`;测试默认使用 Fake 实现(FakeLLM / FakeTTS / MemoryBlob)。
- 模型名、音色名、服务商端点只出现在配置(`.env` / 设置)里。

## 4. 产品内 AI 是工作流,不是自主 agent

选词、生成、校验、重试都是确定性代码,LLM 只负责写文本和给释义。这样可测试、可控、可计量成本。自主 agent(如 AI 导师)留到后期,届时把 `core` 里的函数作为工具暴露给它。
