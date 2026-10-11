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

注:上面的表结构草案里有几张表(如 `lexemes`、`article_targets`、`audios`、`gloss_cache`、`llm_calls`)没有写出 `user_id` / `updated_at`。T0.2 的 spec 需要明确每张表是否补齐,并记入 `docs/DECISIONS.md`。
