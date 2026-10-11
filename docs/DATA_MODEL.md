# 数据模型(DATA_MODEL)

来源:`docs/plan.md` §5。schema 迁移属于人工必审项;不得修改已合并的迁移。

## 1. 表结构

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

## 2. 为 SaaS 预留的低成本约定

- id 用 UUIDv7。
- 所有表有 `user_id` 与 `updated_at`。
- `events` 和 `review_logs` 只追加。
- 文章记录来源与许可字段(`articles.source`、`articles.license`)。

## 3. 补充(已确认,T0.2 实施)

plan §5 的草案缺了几处功能需要的存储,且“所有表有 `user_id` 与 `updated_at`”与草案本身不一致。以下补充已由作者确认(2026-10-11),由 T0.2 写入首个迁移。

**新增表**

```
word_lists(id, user_id, name, kind, source_file, created_at, updated_at)
   -- kind: baseline | study;baseline 词不建卡,study 词书走卡片导入
word_list_items(id, list_id, user_id, lexeme_id, raw, created_at,
                UNIQUE(list_id, lexeme_id))
lexeme_marks(user_id, lexeme_id, status, marked_at, updated_at,
             PRIMARY KEY(user_id, lexeme_id))
   -- status: known | unknown;最新一次“认识 / 不认识”标记的当前状态,历史在 events
settings(user_id PRIMARY KEY, json, updated_at)
   -- 一行一个用户,json 经 zod 校验并带默认值
```

**对草案的调整**

- `lexemes.pos`:`NOT NULL DEFAULT ''`,避免 SQLite 中 NULL 绕过唯一约束。`lexemes` 是全局词标识,不带 `user_id`。
- `cards` 补齐 ts-fsrs `Card` 的全部字段(以 T0.1 锁定的 ts-fsrs 版本为准,如 `scheduled_days`、`learning_steps`),并增加 `origin`(`import` | `lookup` | `inbox` | `autoAdd`)与 `created_at`;`UNIQUE(user_id, lexeme_id)`,一个词只有一张卡。
- `llm_calls` 增加 `status`(`ok` | `invalid_output` | `error`)与 `error`(TEXT,可空),失败调用也要落库。
- `audios` 增加 `chars`、`created_at`,用于每日 TTS 字符上限统计;`text_hash` 唯一。
- `gloss_cache` 为全局缓存,不带 `user_id`;增加 `created_at`;主键 `(lexeme_id, context_hash)`。
- `user_id` 规则:用户拥有的数据都带 `user_id`(cards、review_logs、events、articles、inbox、llm_calls、word_lists、word_list_items、lexeme_marks、settings);从属于文章的表(article_targets、audios)通过 `article_id` 归属,不重复存 `user_id`;全局表(lexemes、gloss_cache)不带。
- `updated_at` 规则:可变表带 `updated_at`(users、cards、articles、inbox、word_lists、lexeme_marks、settings);只追加表与缓存表不带。
- 时间统一存 Unix 毫秒整数;JSON 列存 TEXT,读写时经 zod 校验。

**事件类型**:完整清单与 payload 定义见 `docs/specs/README.md`。
