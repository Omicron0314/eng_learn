# 数据模型与事件契约

这是新 Rust 存储的逻辑模型，独立迁移位于 `crates/cijian-storage/migrations`。旧实物 schema 在 `packages/db/src/schema.ts`，不可通过改写旧迁移达到新结构。转换见 [MIGRATION](MIGRATION.md)。Native SQLite 与浏览器仓储共用业务约束，不要求物理实现完全相同。

## 身份与通用规则

本地 profile 不是云账号。新对象用 UUIDv7；导入的合法旧 ID 原样保留，不为整齐重新编号。Profile 拥有的数据显式带 profile_id；引用必须同属一个 profile。UTC时间用毫秒，业务日期从设置推导。

可变对象有 created_at、updated_at、revision；append-only 日志只有记录时间与版本，不虚设 updated_at。写命令带 operation_id，唯一约束保证重放不重复。默认外键 RESTRICT，不级联删除复习历史。

## 逻辑实体

| 实体 | 关键字段与约束 |
| --- | --- |
| profiles | id、名称、created_at、updated_at；首轮本地一份，恢复可建新profile |
| devices | id、profile_id、platform；设备身份，不等于同步已实现 |
| settings | profile_id PK、schema_version、json、revision；仅学习／实验设置 |
| device_preferences | profile_id＋device_id PK、theme、playback_rate、窗口／阅读偏好 |
| provider_configs | id、profile_id、kind、endpoint、model、voice?、secret_ref?、revision；无明文key |
| lexemes | id、lemma、pos（非NULL，未知为''）、kind word/phrase；UNIQUE(lemma,pos,kind) |
| word_books | id、profile_id、name、kind baseline/study、source、archived_at?；有效同名同类唯一 |
| book_entries | id、profile_id、book_id、lexeme_id、raw、import_id?；UNIQUE(book_id,lexeme_id) |
| cards | id、profile_id、lexeme_id、front、meaning、example、text_source、origin、suspended、archived_at?、FSRS完整状态、Anki ease/间隔、scheduler_version、revision；UNIQUE(profile_id,lexeme_id) |
| review_logs | id、profile_id、card_id、operation_id、rating、state_before/after、due_before/after、scheduler_version、reviewed_at、duration_ms、旧记录metadata；append-only |
| events | id、profile_id、device_id?、operation_id、ordinal、type、schema_version、payload_json、occurred_at；UNIQUE(profile_id,operation_id,ordinal)；append-only |
| command_receipts | profile_id＋operation_id PK、command_type、result_json、completed_at；与业务写同事务 |
| imports | id、profile_id、source_name、format、kind、report_json、source_hash、created_at；不默认保存整份私有源文件 |
| articles | id、profile_id、title、body、body_hash、genre、tier、prompt_version、model、provider_fingerprint、source、license?、analysis_json、analyzer_version、experiment_version、created_at |
| article_targets | article_id＋lexeme_id PK、profile_id、role exposed/control、occurrences、selection_reason |
| reading_sessions | id、profile_id、article_id、opened_at、completed_at?、position；用于恢复和事件去重 |
| lexeme_marks | profile_id＋lexeme_id PK、status known/unknown、marked_at、revision；历史在events |
| inbox | id、profile_id、lexeme_id、article_id?、body_hash?、context_sentence、range?、status pending/added/ignored、revision；每profile×lexeme至多一个pending |
| gloss_cache | profile_id＋lexeme_id＋context_hash＋prompt_version＋provider_fingerprint联合key、gloss_json、created_at；可清理 |
| audio_assets | id、profile_id、cache_key UNIQUE、text_hash、provider_fingerprint、model、voice、format、blob_key、duration?、chars、created_at |
| article_audio_links | profile_id、article_id、audio_id；同音频可被多个相同正文文章引用 |
| audio_reservations | id、profile_id、operation_id UNIQUE、article_id、chars、reserved_at、outcome；预留金额不因失败回退 |
| audio_sessions | id、profile_id、article_id、audio_id?、source remote/system/browser、started_at、completed_at?、rate；完成去重 |
| llm_calls | id、profile_id、job_id?、purpose、provider、model、prompt_version、tokens_in/out?、cost_usd?、latency_ms、status、sanitized_error?、ts |
| jobs | id、profile_id、operation_id UNIQUE、kind、phase、config_revision、status、result_ref?、error?、timestamps；重启将未完成任务标interrupted，不自动付费重试 |
| generation_snapshots | job_id PK/FK、snapshot_json；固定目标/对照、KnownSet、学习设置及公开 provider 配置，不保存 secret；未完成 job 不进入学习备份 |
| migrations / legacy_imports | schema版本／checksum；源库fingerprint、映射版本、报告hash、目标profile、完成时间，防止重复迁入 |

调度状态含 state、due、stability、difficulty（FSRS 记忆快照）、ease_factor（Anki 千分制易度）、reps、lapses、last_review、elapsed_days、scheduled_days、learning_steps 等锁定版本所需字段。升级算法不能只保留 due；无法转换的状态报告阻塞，不全量重置词卡。

卡片不再用 deck 字符串作为唯一归属；成员从 book_entries 关联同词的卡，允许多书共享一张卡。删除一书不伤及其他书的卡；仅保留历史的卡显式归档，不混在普通暂停卡中。

## 事件目录

payload 中只放必要业务标识与摘要，不放凭据。导入与备份可单个事件记汇总；批处理逐项事件使用同一 operation_id 与不同 ordinal。

| 事件 | 关键payload |
| --- | --- |
| import_complete | importId、bookId、各计数、sourceHash |
| review | cardId、rating、before/after摘要、schedulerVersion |
| card_edit / card_suspend | cardId、changedFields或suspended |
| book_delete | bookId、removedEntries、deletedCards、archivedCards、sharedCards |
| settings_update / provider_update | changedKeys、configRevision；密钥仅“是否变更” |
| article_generate | articleId、attempts、coverage、belowTier、诊断摘要 |
| article_open / article_complete | articleId、sessionId、via |
| lookup | articleId?、lexemeId、bodyHash、range、cacheHit、status |
| mark_known / mark_unknown | lexemeId、articleId?、inboxId?、cardId? |
| add_to_review / inbox_ignore | lexemeId、inboxId?、cardId?、created、source |
| audio_reserve / audio_generate | reservationId、articleId、chars；成功另含audioId |
| audio_play / audio_complete | articleId、sessionId、source、rate |
| difficulty_feedback | articleId、mode reading/listening、value |
| unnatural_flag | articleId、bodyHash、句子区间数组、note? |
| legacy_import / backup_restore | sourceFingerprint、mappingVersion、summary；无key、无私有全文 |

历史 Web event 保留原始 schema_version 与 payload，不强行解析成最新结构；新聚合显式处理 legacy 类型。旧 `via=server` 映射为remote，`browser`保持原义；没有 audio_complete 的历史不凭 audio_play 补造完成。

## 事务边界

1. 评分：校验版本／资格 → 更新卡 → review_log → event → receipt，任何失败全部回滚。
2. 导入：先解析校验 → 建词／书／成员／卡 → import报告 → event／receipt，数据库错误整批回滚。
3. 标记／批量：marks、inbox、cards、events、receipt同事务，不能先显示成功再补事件。
4. 生成：job先记录，网络与日志独立；文章、targets、generate event、成功receipt最后原子提交。
5. 音频：blob写入在事务外并原子改名；`audio_assets`、`article_audio_links`、`audio_generate` 与 receipt 同事务提交；本地合成不写 `audio_reservations`（该表仅为旧库兼容保留）。

llm_calls、缓存、可恢复的job进度、读取位置及投影更新无需递归生成事件；业务状态变更必须有事件。事件本身不可更新。存储层按 operation_id 识别既有预留，避免重启后二次扣同一请求额度。

## 索引、兼容与恢复

原生至少索引 cards(profile,archived,suspended,due)、review_logs(profile,time)/card、events(profile,type,time)、articles(profile,created)、inbox(profile,status)、book_entries(book,lexeme)、audio_assets(profile,cache_key)。大列表用分页／游标，统计投影可重建，不能成为唯一数据。

secret_ref 指向系统凭据；无法保存时只允许会话内配置并说明重启后失效。学习备份默认排除密钥、可重建释义缓存与未完成job；音频可选且每blob有checksum。文章、评分、标记和inbox不能因清缓存丢失。

旧DB只读转换到新库，未知列／事件保留在migration metadata或原始归档，不静默丢弃。含明文provider配置的旧库不是可公开的fixture；迁移后旧备份仍敏感，由用户决定保存位置与处置，工具不自动删除源文件。
