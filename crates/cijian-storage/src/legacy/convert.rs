mod events;
use super::*;
use cijian_domain::{
    analysis::Analysis,
    settings::{Genre, Settings, Tier, ValidatedSettings},
    srs::{Rating, SrsState, State},
};
use std::collections::{BTreeSet, HashMap};
fn row(fields: &[(&str, Value)]) -> Value {
    Value::Object(
        fields
            .iter()
            .map(|(key, value)| ((*key).into(), value.clone()))
            .collect(),
    )
}
fn insert(tx: &Connection, table: &str, value: Value) -> Result<()> {
    let fields = value.as_object().ok_or(invalid("迁移行结构无效"))?;
    let values = fields
        .values()
        .map(crate::backup::sql_value)
        .collect::<Result<Vec<_>>>()?;
    tx.execute(
        &format!(
            "INSERT INTO {table}({}) VALUES ({})",
            fields.keys().cloned().collect::<Vec<_>>().join(","),
            vec!["?"; fields.len()].join(",")
        ),
        rusqlite::params_from_iter(values),
    )
    .map_err(|error| match error {
        rusqlite::Error::SqliteFailure(code, _)
            if code.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            invalid("旧数据违反目标约束，迁入已回滚")
        }
        _ => unavailable(error),
    })?;
    Ok(())
}
fn copy(value: &Value, keys: &[&str]) -> Value {
    Value::Object(
        keys.iter()
            .map(|key| ((*key).into(), value[*key].clone()))
            .collect(),
    )
}
fn legacy_settings(rows: &[Value]) -> Result<Settings> {
    let defaults = Settings {
        article_min_words: 300,
        article_max_words: 400,
        generation_max_retries: 1,
        ..Settings::default()
    };
    let mut value = serde_json::to_value(defaults).map_err(unavailable)?;
    if rows.len() > 1 {
        return Err(invalid("旧设置重复"));
    }
    if let Some(source) = rows.first() {
        let original = parsed(source, "json")?;
        for (key, field) in original.as_object().ok_or(invalid("旧设置不是对象"))? {
            value[key] = field.clone();
        }
    }
    let settings: Settings =
        serde_json::from_value(value).map_err(|_| invalid("旧设置包含未知或无效字段"))?;
    ValidatedSettings::try_from(settings.clone()).map_err(|_| invalid("旧设置范围无效"))?;
    Ok(settings)
}
fn analysis(value: &Value) -> Result<Analysis> {
    let count = value["tokenCount"]
        .as_u64()
        .ok_or(invalid("旧分析缺少词数"))?;
    let ratio = |key: &str| -> Result<Option<f64>> {
        if count == 0 || value[key].is_null() {
            return Ok(None);
        }
        let n = value[key]
            .as_f64()
            .filter(|n| (0.0..=1.0).contains(n))
            .ok_or(invalid("旧覆盖率无效"))?;
        Ok(Some(n))
    };
    let mut unknown = BTreeMap::new();
    for entry in value["unknownLemmas"]
        .as_array()
        .ok_or(invalid("旧分析未知词结构无效"))?
    {
        let lemma = text(entry, "lemma")?;
        let count = entry["count"].as_u64().ok_or(invalid("旧未知词数无效"))?;
        if unknown.insert(lemma.into(), count).is_some() {
            return Err(invalid("旧分析未知词重复"));
        }
    }
    let occurrences = serde_json::from_value(value["targetOccurrences"].clone())
        .map_err(|_| invalid("旧目标命中结构无效"))?;
    let names = serde_json::from_value::<BTreeSet<String>>(value["names"].clone())
        .map_err(|_| invalid("旧专名结构无效"))?;
    Ok(Analysis {
        version: "legacy-unversioned".into(),
        token_count: count,
        coverage: ratio("coverage")?,
        type_coverage: ratio("typeCoverage")?,
        unknown,
        occurrences,
        excluded_names: names,
    })
}
impl Store {
    pub fn existing_legacy_report(path: &Path, snapshot: &LegacySnapshot) -> Result<LegacyReport> {
        let db = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(unavailable)?;
        let report:String=db.query_row("SELECT r.payload_json FROM legacy_records r JOIN legacy_imports i ON i.profile_id=r.profile_id AND i.source_fingerprint=r.source_fingerprint WHERE r.profile_id=?1 AND r.source_fingerprint=?2 AND r.source_table='__report' AND i.mapping_version=?3",params![snapshot.report.profile,snapshot.report.source_fingerprint,MAPPING_VERSION],|r|r.get(0)).map_err(|_|invalid("目标目录已存在且不是相同的完整迁入；没有覆盖"))?;
        let report: LegacyReport = serde_json::from_str(&report).map_err(unavailable)?;
        Ok(report)
    }
    /// Executes the same conversion against a disposable in-memory target for dry-run validation.
    pub fn dry_run_legacy(
        snapshot: &LegacySnapshot,
        event: &str,
        now: i64,
    ) -> Result<LegacyReport> {
        Self::memory()?.import_legacy(snapshot, event, now)
    }
    pub fn import_legacy(
        &self,
        snapshot: &LegacySnapshot,
        event: &str,
        now: i64,
    ) -> Result<LegacyReport> {
        if !snapshot.report.blockers.is_empty() {
            return Err(invalid("旧库预检仍有阻断项"));
        }
        let profile = &snapshot.report.profile;
        let fingerprint = &snapshot.report.source_fingerprint;
        let mut db = self.connection.lock().map_err(unavailable)?;
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unavailable)?;
        if let Some(report)=tx.query_row("SELECT payload_json FROM legacy_records WHERE profile_id=?1 AND source_fingerprint=?2 AND source_table='__report'",params![profile,fingerprint],|r|r.get::<_,String>(0)).optional().map_err(unavailable)?{return serde_json::from_str(&report).map_err(unavailable);}
        if tx
            .query_row("SELECT count(*) FROM profiles", [], |r| r.get::<_, i64>(0))
            .map_err(unavailable)?
            != 0
        {
            return Err(invalid("旧库只能迁入新的空学习库"));
        }
        let mut report = snapshot.report.clone();
        let source = &snapshot.rows;
        let user = source["users"].first().ok_or(invalid("旧档案缺失"))?;
        insert(
            &tx,
            "profiles",
            json!({"id":profile,"name":"迁入的学习库","created_at":number(user,"created_at")?,"updated_at":number(user,"updated_at")?}),
        )?;
        let settings = legacy_settings(&source["settings"])?;
        // Old Web has no per-device preferences. Preserve legal source learning values verbatim.
        insert(
            &tx,
            "settings",
            json!({"profile_id":profile,"schema_version":1,"json":study_settings_json(&settings)?,"revision":1}),
        )?;
        for (table, records) in source {
            for record in records {
                let key = record
                    .get("id")
                    .cloned()
                    .unwrap_or_else(|| {
                        json!([
                            record["user_id"],
                            record["article_id"],
                            record["lexeme_id"],
                            record["context_hash"]
                        ])
                    })
                    .to_string();
                insert(
                    &tx,
                    "legacy_records",
                    json!({"profile_id":profile,"source_fingerprint":fingerprint,"source_table":table,"source_id":key,"payload_json":record.to_string()}),
                )?;
            }
        }
        for value in &source["lexemes"] {
            let lemma = text(value, "lemma")?;
            if lemma.trim().is_empty() {
                return Err(invalid("旧词元为空"));
            }
            insert(
                &tx,
                "lexemes",
                json!({"id":value["id"],"lemma":lemma,"pos":value["pos"],"kind":if lemma.split_whitespace().count()>1{"phrase"}else{"word"}}),
            )?;
        }
        let mut books: HashMap<(String, String), String> = HashMap::new();
        let mut book_ids = HashMap::new();
        let mut members = BTreeSet::new();
        let mut lists = source["word_lists"].iter().collect::<Vec<_>>();
        lists.sort_by_key(|value| (value["created_at"].as_i64(), value["id"].as_str()));
        for value in lists {
            let name = text(value, "name")?;
            let kind = text(value, "kind")?;
            if name.trim().is_empty() || !["baseline", "study"].contains(&kind) {
                return Err(invalid("旧词书名称或用途无效"));
            }
            let key = (name.into(), kind.into());
            let id = text(value, "id")?;
            if let Some(existing) = books.get(&key) {
                book_ids.insert(id.to_owned(), existing.clone());
                report.warnings.push(format!(
                    "同名同类词书合并：{id} → {existing}；原记录保留在迁移档案"
                ));
                continue;
            }
            insert(
                &tx,
                "word_books",
                json!({"id":id,"profile_id":profile,"name":name,"kind":kind,"source":value["source_file"],"archived_at":null,"created_at":value["created_at"],"updated_at":value["updated_at"],"revision":1}),
            )?;
            books.insert(key, id.into());
            book_ids.insert(id.to_owned(), id.to_owned());
        }
        for value in &source["word_list_items"] {
            let book = book_ids
                .get(text(value, "list_id")?)
                .ok_or(invalid("旧词书成员跨档案或缺少词书"))?;
            if !members.insert((book.clone(), text(value, "lexeme_id")?.to_owned())) {
                report
                    .warnings
                    .push("合并词书中的重复词条仅投影一次，全部原条目保留".into());
                continue;
            }
            insert(
                &tx,
                "book_entries",
                json!({"id":value["id"],"profile_id":profile,"book_id":book,"lexeme_id":value["lexeme_id"],"raw":value["raw"],"import_id":null}),
            )?;
        }
        for value in &source["cards"] {
            let mut state = copy(
                value,
                &[
                    "state",
                    "due",
                    "stability",
                    "difficulty",
                    "reps",
                    "lapses",
                    "last_review",
                    "elapsed_days",
                    "scheduled_days",
                    "learning_steps",
                ],
            );
            for field in [
                "elapsed_days",
                "scheduled_days",
                "learning_steps",
                "reps",
                "lapses",
            ] {
                state[field] = json!(number(value, field)?);
            }
            let srs: SrsState =
                serde_json::from_value(state.clone()).map_err(|_| invalid("旧调度状态无法转换"))?;
            srs.validate().map_err(|_| invalid("旧调度字段不兼容"))?;
            let suspended = number(value, "suspended")?;
            if ![0, 1].contains(&suspended) {
                return Err(invalid("旧暂停状态无效"));
            }
            if suspended == 1 {
                report.warnings.push(format!(
                    "暂停卡 {} 保持暂停；未根据暂停状态推断归档",
                    text(value, "id")?
                ));
            }
            let back = parsed(value, "back_json")?;
            let meaning = back["meaning"].as_str().unwrap_or("");
            let example = back["example"].as_str().unwrap_or("");
            let source_kind = match back["meaningSource"].as_str() {
                Some("import") => "imported",
                Some("ai") => "ai",
                Some("user") => "user",
                _ => return Err(invalid("旧释义来源未知")),
            };
            let mut card = copy(
                value,
                &[
                    "id",
                    "lexeme_id",
                    "front",
                    "origin",
                    "created_at",
                    "updated_at",
                ],
            );
            for (key, field) in state.as_object().expect("state object") {
                card[key] = field.clone();
            }
            for (key, field) in row(&[
                ("profile_id", json!(profile)),
                ("meaning", json!(meaning)),
                ("example", json!(example)),
                (
                    "text_source",
                    json!(if meaning.is_empty() {
                        "missing"
                    } else {
                        source_kind
                    }),
                ),
                ("suspended", json!(suspended)),
                ("archived_at", Value::Null),
                ("scheduler_version", json!("legacy-unversioned")),
                ("revision", json!(1)),
            ])
            .as_object()
            .expect("row object")
            {
                card[key] = field.clone();
            }
            insert(&tx, "cards", card)?;
            let deck = text(value, "deck")?;
            if deck.trim().is_empty() {
                return Err(invalid("旧词卡的词书名称为空"));
            }
            let book = if let Some(book) = books.get(&(deck.into(), "study".into())) {
                book.clone()
            } else {
                let book = snapshot.new_ids[&format!("book:{deck}")].clone();
                insert(
                    &tx,
                    "word_books",
                    json!({"id":book,"profile_id":profile,"name":deck,"kind":"study","source":"legacy-deck","archived_at":null,"created_at":value["created_at"],"updated_at":value["updated_at"],"revision":1}),
                )?;
                books.insert((deck.into(), "study".into()), book.clone());
                book
            };
            if members.insert((book.clone(), text(value, "lexeme_id")?.into())) {
                insert(
                    &tx,
                    "book_entries",
                    json!({"id":snapshot.new_ids[&format!("entry:{}",text(value,"id")?)],"profile_id":profile,"book_id":book,"lexeme_id":value["lexeme_id"],"raw":value["front"],"import_id":null}),
                )?;
            }
        }
        for value in &source["review_logs"] {
            serde_json::from_value::<Rating>(value["rating"].clone())
                .map_err(|_| invalid("旧评分枚举无效"))?;
            serde_json::from_value::<State>(value["state_before"].clone())
                .map_err(|_| invalid("旧评分前状态无效"))?;
            let mut review = copy(
                value,
                &[
                    "id",
                    "card_id",
                    "rating",
                    "state_before",
                    "due_before",
                    "reviewed_at",
                    "duration_ms",
                ],
            );
            review["profile_id"] = json!(profile);
            review["operation_id"] = json!(format!("legacy-review-{}", text(value, "id")?));
            review["state_after"] = Value::Null;
            review["due_after"] = Value::Null;
            review["scheduler_version"] = json!("legacy-unversioned");
            review["metadata"] =
                json!(json!({"legacy":value,"afterStateUnavailable":true}).to_string());
            insert(&tx, "review_logs", review)?;
        }
        for value in &source["articles"] {
            let genre: Genre = serde_json::from_value(value["genre"].clone())
                .map_err(|_| invalid("旧文章体裁未知"))?;
            let tier: Tier = serde_json::from_value(value["level_tier"].clone())
                .map_err(|_| invalid("旧文章难度未知"))?;
            let original = parsed(value, "analysis_json")?;
            let converted = analysis(&original)?;
            insert(
                &tx,
                "articles",
                json!({"id":value["id"],"profile_id":profile,"title":value["title"],"body":value["body"],"body_hash":hash(text(value,"body")?),"genre":serde_json::to_string(&genre).map_err(unavailable)?,"tier":serde_json::to_string(&tier).map_err(unavailable)?,"prompt_version":value["prompt_version"],"model":value["model"],"provider_fingerprint":"legacy-unversioned","source":value["source"],"license":value["license"],"analysis_json":json!({"analysis":converted,"diagnostics":["旧版分析快照"],"legacy":original}).to_string(),"analyzer_version":"legacy-unversioned","experiment_version":"legacy-unversioned","created_at":value["created_at"]}),
            )?;
        }
        for value in &source["article_targets"] {
            let mut target = value.clone();
            target["profile_id"] = json!(profile);
            target["selection_reason"] = json!("legacy-preserved");
            insert(&tx, "article_targets", target)?;
        }
        for value in &source["lexeme_marks"] {
            insert(
                &tx,
                "lexeme_marks",
                json!({"profile_id":profile,"lexeme_id":value["lexeme_id"],"status":value["status"],"marked_at":value["marked_at"],"revision":1}),
            )?;
        }
        let mut pending = BTreeSet::new();
        let mut inbox = source["inbox"].iter().collect::<Vec<_>>();
        inbox.sort_by_key(|v| std::cmp::Reverse((v["updated_at"].as_i64(), v["id"].as_str())));
        for value in inbox {
            let status = text(value, "status")?;
            if !["pending", "added", "ignored"].contains(&status) {
                return Err(invalid("旧生词箱状态未知"));
            }
            if status == "pending" && !pending.insert(text(value, "lexeme_id")?) {
                report.warnings.push(format!(
                    "重复待整理记录 {} 仅保留于迁移档案；当前投影保留最新记录",
                    text(value, "id")?
                ));
                continue;
            }
            let body_hash = if let Some(article) = value["article_id"].as_str() {
                Some(
                    tx.query_row(
                        "SELECT body_hash FROM articles WHERE profile_id=?1 AND id=?2",
                        params![profile, article],
                        |r| r.get::<_, String>(0),
                    )
                    .map_err(unavailable)?,
                )
            } else {
                None
            };
            insert(
                &tx,
                "inbox",
                json!({"id":value["id"],"profile_id":profile,"lexeme_id":value["lexeme_id"],"article_id":value["article_id"],"body_hash":body_hash,"context_sentence":value["context_sentence"],"range_json":null,"status":status,"created_at":value["created_at"],"updated_at":value["updated_at"],"revision":1}),
            )?;
        }
        for value in &source["llm_calls"] {
            let mut call = copy(
                value,
                &[
                    "id",
                    "purpose",
                    "provider",
                    "model",
                    "tokens_in",
                    "tokens_out",
                    "cost_usd",
                    "latency_ms",
                    "status",
                    "ts",
                ],
            );
            call["profile_id"] = json!(profile);
            call["job_id"] = Value::Null;
            call["prompt_version"] = json!("legacy-unversioned");
            call["sanitized_error"] = Value::Null;
            insert(&tx, "llm_calls", call)?;
        }
        for value in &source["gloss_cache"] {
            insert(
                &tx,
                "gloss_cache",
                json!({"profile_id":profile,"lexeme_id":value["lexeme_id"],"context_hash":value["context_hash"],"prompt_version":"legacy-unversioned","provider_fingerprint":"legacy-unversioned","gloss_json":value["gloss_json"],"created_at":value["created_at"]}),
            )?;
        }
        for config in &source["provider_config"] {
            let config = parsed(config, "json")?;
            for (kind, label) in [
                (cijian_domain::provider::ProviderKind::Llm, "llm"),
                (cijian_domain::provider::ProviderKind::Tts, "tts"),
            ] {
                let value = &config[label];
                if value.is_null() || value["provider"] == "none" {
                    continue;
                }
                let mapped = cijian_domain::provider::ProviderConfig {
                    kind,
                    endpoint: value["baseURL"].as_str().unwrap_or("").into(),
                    model: value["model"].as_str().unwrap_or("").into(),
                    voice: if label == "tts" {
                        value["voice"].as_str().map(str::to_owned)
                    } else {
                        None
                    },
                    revision: 1,
                };
                if value["provider"] != "openai-compatible" || mapped.validate().is_err() {
                    report.warnings.push(format!(
                        "旧 {label} 配置不满足新边界，仅保留非敏感迁移档案，请重新配置"
                    ));
                    continue;
                }
                insert(
                    &tx,
                    "provider_configs",
                    json!({"id":snapshot.new_ids[&format!("provider:{label}")],"profile_id":profile,"kind":label,"endpoint":mapped.endpoint,"model":mapped.model,"voice":mapped.voice,"secret_ref":null,"revision":1}),
                )?;
            }
        }
        for value in &source["audios"] {
            let blob = snapshot
                .blobs
                .iter()
                .find(|blob| Some(blob.id.as_str()) == value["id"].as_str())
                .ok_or(invalid("旧音频未校验"))?;
            let article = source["articles"]
                .iter()
                .find(|article| article["id"] == value["article_id"])
                .ok_or(invalid("旧音频文章缺失"))?;
            insert(
                &tx,
                "audio_assets",
                json!({"id":value["id"],"profile_id":profile,"cache_key":format!("legacy:{fingerprint}:{}",blob.id),"text_hash":hash(&format!("{}\n\n{}",text(article,"title")?,text(article,"body")?)),"provider_fingerprint":format!("legacy:{}",text(value,"provider")?),"model":"","voice":value["voice"],"format":blob.key.rsplit('.').next(),"blob_key":blob.key,"duration":value["duration_s"],"chars":value["chars"],"created_at":value["created_at"]}),
            )?;
            insert(
                &tx,
                "article_audio_links",
                json!({"profile_id":profile,"article_id":value["article_id"],"audio_id":value["id"]}),
            )?;
        }
        self.project_legacy_events(&tx, snapshot, &settings, &mut report)?;
        insert(
            &tx,
            "events",
            json!({"id":event,"profile_id":profile,"device_id":null,"operation_id":format!("legacy-import-{fingerprint}"),"ordinal":0,"type":"legacy_import","schema_version":1,"payload_json":json!({"sourceFingerprint":fingerprint,"mappingVersion":MAPPING_VERSION,"sourceCounts":report.source_counts}).to_string(),"occurred_at":now}),
        )?;
        for table in [
            "profiles",
            "settings",
            "lexemes",
            "cards",
            "word_books",
            "book_entries",
            "review_logs",
            "events",
            "articles",
            "article_targets",
            "reading_sessions",
            "inbox",
            "lexeme_marks",
            "audio_assets",
            "audio_reservations",
            "audio_sessions",
            "gloss_cache",
            "llm_calls",
            "provider_configs",
        ] {
            let count: i64 = tx
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
                .map_err(unavailable)?;
            report.target_counts.insert(table.into(), count as u64);
        }
        for (old, new) in [
            ("users", "profiles"),
            ("lexemes", "lexemes"),
            ("cards", "cards"),
            ("review_logs", "review_logs"),
            ("articles", "articles"),
            ("article_targets", "article_targets"),
            ("audios", "audio_assets"),
            ("llm_calls", "llm_calls"),
        ] {
            if report.source_counts[old] != report.target_counts[new] {
                return Err(invalid("迁移实体数量对账失败"));
            }
        }
        if tx
            .prepare("PRAGMA foreign_key_check")
            .map_err(unavailable)?
            .exists([])
            .map_err(unavailable)?
        {
            return Err(invalid("迁入外键对账失败"));
        }
        let encoded = serde_json::to_string(&report).map_err(unavailable)?;
        insert(
            &tx,
            "legacy_records",
            json!({"profile_id":profile,"source_fingerprint":fingerprint,"source_table":"__report","source_id":"mapping","payload_json":encoded}),
        )?;
        insert(
            &tx,
            "legacy_imports",
            json!({"profile_id":profile,"source_fingerprint":fingerprint,"mapping_version":MAPPING_VERSION,"report_hash":hash(&encoded),"completed_at":now}),
        )?;
        tx.commit().map_err(unavailable)?;
        Ok(report)
    }
}
