# Services 阶段交接（M0–M6 后端）

本阶段只实施 packages、`apps/web/src/server`、测试与测量文档；没有 pages、routes、CI、部署、付费调用或提交。不能据此宣布完整 MVP 验收完成。

## 装配与约束

`createAppDeps(env, overrides?)` 在 `apps/web/src/server/deps.ts`，返回 `AppDeps`：repos、userId、同步事务 `tx(fn)`、llm、tts（可为 null）、blobs、clock、rng、动态 `settings()`、安全 provider 状态、price、close。

- 真实配置缺少 LLM 变量时 fail-fast，绝不自动替换成假模型。演示须显式 `LLM_PROVIDER=fake`。
- TTS 默认 `none`，浏览器 SpeechSynthesis 由下一阶段 UI 实现；真实配置使用 `TTS_PROVIDER=openai-compatible` 与全部 TTS_* 参数。
- 每个运行进程复用一个 AppDeps，不要每个 HTTP 请求重新创建数据库连接。in-flight 释义/音频去重和音频配额串行锁以该实例为作用域。
- `tx` 回调只能同步读写仓储，禁止 async；provider IO 在事务之外。测试覆盖事件失败时业务回滚。
- API 入参仍须 route 层 zod 验证；服务层也校验业务参数。`AppError` 携带 code/status/message，路由可统一映射；不要把任意异常对象返回客户端。
- `llmConfigStatus(env)` 可独立展示缺失变量且不返回密钥；工厂本身不会绕过配置失败。

## 导出接口

统一从 `apps/web/src/server/services/index.ts` 导入；所有函数第一个参数为 deps。

| 域 | 导出与输入 |
| --- | --- |
| 导入 | `importCards({fileName,bytes,deck})` → added/skipped/duplicates/errors；`importWordList({fileName,bytes,name,kind,mergeIfExists?})` → listId/parsed/skippedLines/duplicates/phrases/cardsAdded |
| 复习 | `getQueue()` → learning/review/new **卡片 ID 数组**；`getReviewCard(cardId)` → card + 四档 preview；`rate({cardId,rating,durationMs})` → SrsCard |
| 卡片 | `ensureMeaning(cardId)` → meaning/source；`updateCardText({cardId,meaning?,example?})`；`setSuspended({cardId,suspended})`；`listCards({q?,deck?,state?,suspended?,offset?,limit?})` → rows/total |
| 设置 | `getSettingsView()` → settings/providers/ttsUsage；`updateSettings(patch)`；别名 `settingsService.update` |
| 知识 | `getKnownSet({targets})`；`selectTargetsForUser()` |
| 生成 | `generateArticle({tier?,genre?})` → articleId/coverage/belowTier/missingTargets；`ArticleAnalysisSchema` |
| 文章 | `getArticleView(articleId)` → null 或标题/正文/元信息/分析/段落 tokens；`getTodayView()` → 队列数量/今日文章/defaults |
| 阅读事件 | `recordArticleEvent({articleId,type,via?,openId?})` → event id；打开时返回 openId，完成时传回该 ID，服务端按打开会话去重 |
| 点词 | 首选 `lookupArticleWord({articleId,start})` 从实际正文推导词/句；`getGloss({lemma,surface,sentence,articleId?})` → lexemeId/gloss/cacheHit；`contextHash(sentence)` |
| 标记 | `markKnown({lemma,sentence,articleId?})`；`markUnknown(...)` → inboxId?/cardId?；`addToReview(...)` → cardId/created |
| Inbox | `listInbox()` → 待处理行数组（含可选 gloss）；`batchInbox({action,ids})` → processed/cardsCreated；非法/重复/非 pending ID 整批拒绝 |
| 音频 | `ensureAudio(articleId)` → browser/text 或 server/audioId/url/durationS/cacheHit；`readAudio(key,range?)` → status/headers/data；`recordAudioPlay({articleId,rate,via})`；`getTtsUsage()`、`checkTtsBudget(chars)` |
| 反馈 | `submitDifficulty({articleId,value,after})`；`flagUnnatural({articleId,sentenceIndexes,note?})` → events；`latestDifficulty(articleId)` |
| 统计 | `getStats({offset?,limit?})` → 规格 StatsView + learnedWordsTotal；`getExperiment(windowDays=7)` → exposed/control/diff，供未来 report 脚本复用 |

除导入与独立 helper 外，各模块另有规格命名的 service 对象，暴露相同函数。

## 下一阶段需遵守的细节

- 段落 `text` **含原始段间分隔符**；tokens 的 start/end 和 sentenceIndex 为正文全局值。拼接段落 text 必须仍等于 body，渲染 token 间空白不能丢。
- completion 事件增加向后兼容的可选 `openId`，不改迁移。前端还需按本次打开去重滚动/按钮请求。音频播放首次事件由页面自身去重。
- 控制词不进入任何生成提示，包括 coverage 重试避免列表；coverage 只把 exposed 自动视为已知，另行统计两组出现次数。
- lookup 事件与 lexeme 创建同事务，AI 失败也记录点击；gloss_cache、llm_calls 为日志/缓存例外，不另写事件。
- 原有 SDK 自动重试已禁用，JSON 模式降级失败也记录 llm_calls，避免未计量的隐式请求。
- 音频缺 blob 时重新收费；为不因覆盖 audios 行而抹去用量，今日用量取 audios 总量与 audio_generate 事件总字符量的较大值。不同 hash 的合成串行检查预算。
- 音频 URL 为 `/api/audio/audio/<hash>.<ext>`；下一阶段 route 可使用 catch-all `[...key]` 或映射等价路径。readAudio 已支持普通/后缀 Range、416 与安全 key 校验。
- 所有 `getStats` 文章分析从仓储读取并 zod 验证，不再只做 TS cast。时间日界修复了默认 04:00 在 DST 附近按 UTC 减小时的错误。

## 未证明/未交付

- 没有 UI、HTTP 路由、密码保护、PWA、部署、报告/评测脚本或 e2e；这些归下一阶段。
- 真实 LLM 文章/释义质量、真实 TTS 声音与长 MP3 连播、部分合成失败所消耗字符的成本记录尚未实测。MP3 分段拼接可能有间隙，durationS 为 null。
- 配额锁仅限同一进程/AppDeps；多进程部署会有竞争，不在单用户 MVP 本阶段范围。
- 基础 apkg 已通过合成 SQLite/ZIP 测试；anki21b 明确拒绝，真实 Anki 版本兼容性仍待验证。
- S1/S2/S4 的数据量、实物与人工验证缺口见 spike 文档，不能把小样本结果冒充完整验收。2–4 周自用留存未执行。

## Web 接入后续

本交接是 services 阶段历史记录。后续 Web / HTTP / 工具实现与实际验证见 `docs/WEB-VALIDATION.md`；本页“未交付”中的页面、routes、门禁、报告与 e2e 已由后续阶段补齐，真实 provider / spike / 留存限制仍保留。
