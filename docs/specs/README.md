# Spec 共用约定

所有 `docs/specs/*.md` 默认遵守本文件。单个 spec 与本文件冲突时,以单个 spec 为准,并在 spec 中写明原因。

## 1. 文件与模板

- 文件名:`<任务ID>-<英文短名>.md`,如 `T1.2-fsrs-queue.md`、`S1-lemmatizer.md`。
- 章节固定为:目标、范围与非范围、接口、验收标准、测试计划、依赖与风险。
- 验收标准编号 `AC1`、`AC2`…,每条都要能对应到至少一个自动化测试;测试名里带上编号,如 `it("AC3: ...")`。
- spike 的 spec 也放在这里;spike 结论写入 `docs/spikes/Sx.md`,一次性代码放 `spikes/Sx/`。

## 2. 代码位置

| 内容 | 位置 |
| --- | --- |
| 纯逻辑 | `packages/core/src/<module>/` |
| 接口与实现(LLM / TTS / Blob) | `packages/providers/src/` |
| schema、迁移、仓储 | `packages/db/src/` |
| 编排(事务、事件、调用 provider) | `apps/web/src/server/services/` |
| 依赖装配 | `apps/web/src/server/deps.ts`:`createAppDeps(env)` 返回 `AppDeps` |
| 页面 | `apps/web/src/app/` |
| 单元测试 | 与源码同目录 `*.test.ts` |
| 端到端测试 | `apps/web/e2e/*.spec.ts` |

服务函数统一形如 `fn(deps: AppDeps, input)`。`AppDeps` 至少包含:`repos`、`llm`、`tts`、`blobs`、`clock: () => Date`、`rng: () => number`、`settings`(读取当前设置)。

## 3. 时间、日界与随机数

- 数据库里的时间一律存 Unix 毫秒整数。
- “今天”按设置项 `timezone`(默认 `Asia/Shanghai`)和 `dayStartHour`(默认 4,与 Anki 一致)计算,由 `packages/core/src/time` 的 `dayRange(now, { timezone, dayStartHour })` 给出 `[start, end)`。
- `core` 不调用 `Date.now()` 或 `Math.random()`,一律由参数传入 `now` 与 `rng`。测试用固定时间和带种子的 rng。

## 4. 设置项与默认值

由 T0.2 的 `settings` 表存储,`SettingsSchema`(zod)提供默认值;T1.5 提供编辑页面。

| 键 | 默认 | 说明 |
| --- | --- | --- |
| `level` | `B1` | 提示词中的 CEFR 等级;界面显示“B1(约中国大学英语四级水平)” |
| `dailyNewLimit` | 20 | 每日新词上限(含点词加入的卡片) |
| `dailyReviewLimit` | 200 | 每日复习上限 |
| `requestRetention` | 0.9 | FSRS 目标保持率 |
| `tier` | `medium` | `easy`(≥98%)/ `medium`(95–98%)/ `hard`(90–95%) |
| `articleMinWords` / `articleMaxWords` | 300 / 400 | 文章长度 |
| `targetCount` | 8 | 每篇目标词总数(含 control) |
| `controlRatio` | 0.2 | control 占比 |
| `generationMaxRetries` | 1 | coverage 不足时的重试次数 |
| `lookupMode` | `markOnly` | `markOnly` / `autoAdd` |
| `ttsDailyCharLimit` | 20000 | 每日 TTS 合成字符上限 |
| `timezone` | `Asia/Shanghai` | 日界计算 |
| `dayStartHour` | 4 | 日界小时 |

## 5. 环境变量

`.env.example` 只放占位符,不出现任何具体模型名或音色名。

| 变量 | 必填 | 说明 |
| --- | --- | --- |
| `DATABASE_PATH` | 否,默认 `./data/app.sqlite` | SQLite 文件 |
| `BLOB_DIR` | 否,默认 `./data/blobs` | 本地文件存储 |
| `APP_PASSWORD` | 否 | 设置后整站需要密码 |
| `LLM_PROVIDER` | 否,默认 `openai-compatible` | `openai-compatible` 或 `fake`(测试 / e2e) |
| `LLM_BASE_URL` / `LLM_API_KEY` / `LLM_MODEL` | `openai-compatible` 时必填 | 缺失时启动报错,不用任何默认值 |
| `LLM_PRICE_INPUT_PER_MTOK` / `LLM_PRICE_OUTPUT_PER_MTOK` | 否 | 每百万 token 美元单价,用于计算 `llm_calls.cost_usd`;未配置时记为 NULL |
| `TTS_PROVIDER` | 否,默认 `none` | `none`(前端用浏览器 SpeechSynthesis)、`fake`;真实服务商待定 |
| `TTS_BASE_URL` / `TTS_API_KEY` / `TTS_MODEL` / `TTS_VOICE` | 视服务商 | 全部走配置 |
| `EVAL_MAX_COST_USD` | 否,默认 1 | `pnpm eval:generation` 的费用上限 |

## 6. 事件类型

所有写操作在同一事务里写一条 `events`。例外:`llm_calls`(本身就是日志)和 `gloss_cache`(缓存)的写入不需要事件。事件类型与 payload 用 zod 定义在 `packages/db/src/events.ts`(T1.4 完成类型化)。

| type | payload | 产生方 |
| --- | --- | --- |
| `review` | `{ cardId, rating, stateBefore, stateAfter, dueBefore, dueAfter, durationMs }` | T1.2 |
| `import` | `{ source: "csv" \| "apkg" \| "txt", fileName, kind: "cards" \| "baseline" \| "study", deck?, listId?, added, skipped, duplicates }` | T1.1、T2.2 |
| `card_edit` | `{ cardId, fields: string[] }` | T1.3 |
| `card_suspend` | `{ cardId, suspended }` | T1.3 |
| `settings_update` | `{ changed: string[] }` | T1.5 |
| `article_generate` | `{ articleId, attempts, coverage, belowTier, missingTargets: string[] }` | T3.2 |
| `article_open` | `{ articleId }` | T3.3 |
| `article_complete` | `{ articleId, via: "button" \| "scroll" }` | T3.3 |
| `lookup` | `{ lexemeId, lemma, surface, sentence, articleId?, cacheHit }` | T4.1 |
| `mark_known` | `{ lexemeId, articleId? }` | T4.2 |
| `mark_unknown` | `{ lexemeId, articleId?, inboxId?, cardId? }` | T4.2 |
| `add_to_review` | `{ lexemeId, cardId, from: "lookup" \| "inbox" \| "autoAdd", articleId? }` | T4.2、T4.3 |
| `inbox_ignore` | `{ inboxId, lexemeId }` | T4.3 |
| `audio_generate` | `{ articleId, audioId, chars }` | T5.1 |
| `audio_play` | `{ articleId, rate, via: "server" \| "browser" }` | T5.2 |
| `difficulty_feedback` | `{ articleId, value: "too_hard" \| "just_right" \| "too_easy", after: "reading" \| "listening" }` | T6.1 |
| `unnatural_flag` | `{ articleId, sentenceIndex?, note? }` | T6.1 |

## 7. API 与错误

- API 路由入参用 zod 校验,失败返回 400。
- 错误统一为 `{ "error": { "code": string, "message": string } }`。
- 已知错误码:`VALIDATION_FAILED`(400)、`NO_TARGETS`(400)、`APKG_UNSUPPORTED_FORMAT`(400)、`UNAUTHORIZED`(401)、`NOT_FOUND`(404)、`PAYLOAD_TOO_LARGE`(413)、`TTS_DAILY_LIMIT_EXCEEDED`(429)、`CONFIG_MISSING`(500)、`LLM_OUTPUT_INVALID`(502)、`PROVIDER_ERROR`(502)。

## 8. 测试约定

- 单元测试不访问网络、不读写真实文件;需要数据库的测试用 `:memory:` SQLite 并跑完全部迁移。
- 端到端测试用 `LLM_PROVIDER=fake`、`TTS_PROVIDER=fake` 和临时数据库启动应用。
- 真实 LLM / TTS 调用只出现在手动脚本里(`pnpm eval:generation`),运行前须经人确认。
