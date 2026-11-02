use super::*;
use rusqlite::{OpenFlags, types::ValueRef};
const SOURCES: &[&str] = &[
    "users",
    "settings",
    "lexemes",
    "cards",
    "review_logs",
    "events",
    "articles",
    "article_targets",
    "audios",
    "inbox",
    "gloss_cache",
    "llm_calls",
    "word_lists",
    "word_list_items",
    "lexeme_marks",
];
pub(super) fn legacy_schema() -> Result<Connection> {
    let db = Connection::open_in_memory().map_err(unavailable)?;
    db.execute_batch("CREATE TABLE _migrations(id TEXT PRIMARY KEY,applied_at INTEGER NOT NULL)")
        .map_err(unavailable)?;
    for (index, part) in include_str!("../../../../packages/db/src/migrations.ts")
        .split("sql: `")
        .skip(1)
        .enumerate()
    {
        db.execute_batch(part.split('`').next().ok_or(invalid("旧schema资产损坏"))?)
            .map_err(unavailable)?;
        db.execute("INSERT INTO _migrations VALUES (?1,0)", [KNOWN[index]])
            .map_err(unavailable)?;
    }
    Ok(db)
}
const KNOWN: &[&str] = &[
    "0001_init",
    "0002_append_only_triggers",
    "0003_provider_config",
];
type LegacyColumn = (String, String, i64, Option<String>, i64);
fn shape(db: &Connection, table: &str) -> Result<Vec<LegacyColumn>> {
    db.prepare(&format!("PRAGMA table_info({table})"))
        .map_err(unavailable)?
        .query_map([], |r| {
            Ok((r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
        })
        .map_err(unavailable)?
        .collect::<std::result::Result<_, _>>()
        .map_err(unavailable)
}
fn read_rows(db: &Connection, sql: &str, profile: &str) -> Result<Vec<Value>> {
    let mut query = db.prepare(sql).map_err(unavailable)?;
    let names = query
        .column_names()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut rows = query.query([profile]).map_err(unavailable)?;
    let mut result = Vec::new();
    while let Some(row) = rows.next().map_err(unavailable)? {
        let mut value = serde_json::Map::new();
        for (index, name) in names.iter().enumerate() {
            let field = match row.get_ref(index).map_err(unavailable)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(value) => json!(value),
                ValueRef::Real(value) => {
                    if !value.is_finite() {
                        return Err(invalid("旧库包含非有限数"));
                    }
                    json!(value)
                }
                ValueRef::Text(value) => {
                    json!(std::str::from_utf8(value).map_err(|_| invalid("旧库文本不是UTF-8"))?)
                }
                ValueRef::Blob(_) => return Err(invalid("旧库领域字段包含二进制值")),
            };
            value.insert(name.clone(), field);
        }
        result.push(Value::Object(value));
    }
    result.sort_by_key(canonical_json);
    Ok(result)
}
impl Store {
    pub fn legacy_profiles(path: &Path) -> Result<Vec<cijian_ports::legacy::LegacyProfile>> {
        let source = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(unavailable)?;
        source
            .execute_batch("PRAGMA query_only=ON;PRAGMA trusted_schema=OFF")
            .map_err(unavailable)?;
        for table in ["users", "cards", "articles"] {
            let kind: String = source
                .query_row(
                    "SELECT type FROM sqlite_master WHERE name=?1",
                    [table],
                    |r| r.get(0),
                )
                .map_err(unavailable)?;
            if kind != "table" {
                return Err(invalid("所选文件不是支持的旧学习库"));
            }
        }
        source.prepare("SELECT u.id,u.created_at,(SELECT count(*) FROM cards c WHERE c.user_id=u.id),(SELECT count(*) FROM articles a WHERE a.user_id=u.id) FROM users u ORDER BY u.id").map_err(unavailable)?.query_map([],|r|Ok(cijian_ports::legacy::LegacyProfile{id:r.get(0)?,created_at:r.get(1)?,cards:r.get::<_,i64>(2)? as u64,articles:r.get::<_,i64>(3)? as u64})).map_err(unavailable)?.collect::<std::result::Result<Vec<_>,_>>().map_err(unavailable)
    }
    /// A read transaction pins one WAL-aware SQLite snapshot. No plaintext snapshot file is made.
    pub fn inspect_legacy(
        path: &Path,
        profile: Option<&str>,
        audio_directory: Option<&Path>,
        mut next_id: impl FnMut() -> String,
    ) -> Result<LegacySnapshot> {
        let mut source = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(unavailable)?;
        source
            .execute_batch(
                "PRAGMA query_only=ON; PRAGMA trusted_schema=OFF; PRAGMA foreign_keys=ON;",
            )
            .map_err(unavailable)?;
        let tx = source.transaction().map_err(unavailable)?;
        if tx
            .query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .map_err(unavailable)?
            != "ok"
        {
            return Err(invalid("旧库完整性检查失败"));
        }
        if tx
            .prepare("PRAGMA foreign_key_check")
            .map_err(unavailable)?
            .exists([])
            .map_err(unavailable)?
        {
            return Err(invalid("旧库存在外键损坏"));
        }
        let versions = tx
            .prepare("SELECT id FROM _migrations ORDER BY id")
            .map_err(unavailable)?
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(unavailable)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(unavailable)?;
        if versions.is_empty()
            || versions.len() > KNOWN.len()
            || versions
                .iter()
                .zip(KNOWN)
                .any(|(actual, expected)| actual != expected)
        {
            return Err(invalid("旧库迁移版本不受支持"));
        }
        let tables=tx.prepare("SELECT name,type FROM sqlite_master WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%'").map_err(unavailable)?.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(unavailable)?.collect::<std::result::Result<Vec<_>,_>>().map_err(unavailable)?;
        let expected = legacy_schema()?;
        for (table, kind) in &tables {
            if kind != "table"
                || !(SOURCES.contains(&table.as_str())
                    || table == "_migrations"
                    || (table == "provider_config" && versions.len() == 3))
            {
                return Err(invalid("旧库包含未知表或视图"));
            }
            if shape(&tx, table)? != shape(&expected, table)? {
                return Err(invalid("旧库字段结构与已知版本不一致"));
            }
        }
        if SOURCES
            .iter()
            .any(|name| !tables.iter().any(|(table, _)| table == name))
        {
            return Err(invalid("旧库缺少必要表"));
        }
        let profiles = tx
            .prepare("SELECT id FROM users ORDER BY id")
            .map_err(unavailable)?
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(unavailable)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(unavailable)?;
        let profile = match profile {
            Some(profile) if profiles.iter().any(|id| id == profile) => profile.to_owned(),
            None if profiles.len() == 1 => profiles[0].clone(),
            _ => return Err(invalid("请选择一个明确的旧档案；不能自动混合多个档案")),
        };
        let lexemes = "SELECT lexeme_id FROM cards WHERE user_id=?1 UNION SELECT lexeme_id FROM word_list_items WHERE user_id=?1 UNION SELECT lexeme_id FROM lexeme_marks WHERE user_id=?1 UNION SELECT lexeme_id FROM inbox WHERE user_id=?1 UNION SELECT t.lexeme_id FROM article_targets t JOIN articles a ON a.id=t.article_id WHERE a.user_id=?1";
        let mut rows = BTreeMap::new();
        for table in SOURCES {
            let sql=match *table{
                "users"=>"SELECT * FROM users WHERE id=?1".into(),
                "lexemes"=>format!("SELECT * FROM lexemes WHERE id IN ({lexemes})"),
                "gloss_cache"=>format!("SELECT * FROM gloss_cache WHERE lexeme_id IN ({lexemes})"),
                "article_targets"|"audios"=>format!("SELECT source.* FROM {table} source JOIN articles a ON a.id=source.article_id WHERE a.user_id=?1"),
                "llm_calls"=>"SELECT id,user_id,purpose,provider,model,tokens_in,tokens_out,cost_usd,latency_ms,status,NULL AS error,ts FROM llm_calls WHERE user_id=?1".into(),
                _=>format!("SELECT * FROM {table} WHERE user_id=?1")
            };
            let values = read_rows(&tx, &sql, &profile)?;
            for value in &values {
                for key in [
                    "json",
                    "payload_json",
                    "back_json",
                    "gloss_json",
                    "analysis_json",
                ] {
                    if let Some(encoded) = value.get(key) {
                        let decoded: Value = serde_json::from_str(
                            encoded.as_str().ok_or(invalid("旧JSON字段无效"))?,
                        )
                        .map_err(|_| invalid("旧JSON字段损坏"))?;
                        if has_secret(&decoded) {
                            return Err(invalid("旧领域数据含凭据字段，迁入已阻止"));
                        }
                    }
                }
            }
            rows.insert((*table).to_owned(), values);
        }
        let configs = if versions.len() == 3 {
            read_rows(
                &tx,
                "SELECT user_id,json_object('llm',CASE WHEN json_type(json,'$.llm')='object' THEN json_object('provider',json_extract(json,'$.llm.provider'),'baseURL',json_extract(json,'$.llm.baseURL'),'model',json_extract(json,'$.llm.model')) END,'tts',CASE WHEN json_type(json,'$.tts')='object' THEN json_object('provider',json_extract(json,'$.tts.provider'),'baseURL',json_extract(json,'$.tts.baseURL'),'model',json_extract(json,'$.tts.model'),'voice',json_extract(json,'$.tts.voice')) END) AS json,updated_at FROM provider_config WHERE user_id=?1",
                &profile,
            )?
        } else {
            vec![]
        };
        rows.insert("provider_config".into(), configs);
        tx.commit().map_err(unavailable)?;
        let mut report = LegacyReport {
            profile,
            source_fingerprint: String::new(),
            mapping_version: MAPPING_VERSION.into(),
            source_counts: rows
                .iter()
                .map(|(name, rows)| (name.clone(), rows.len() as u64))
                .collect(),
            target_counts: BTreeMap::new(),
            warnings: vec![
                "旧格式未记录迁移checksum；已校验已知版本、字段、完整性和外键".into(),
                "provider密钥不迁入；旧调用错误文本已去除，防止历史错误泄漏凭据".into(),
            ],
            blockers: vec![],
            audio_bytes: 0,
        };
        let mut blobs = Vec::new();
        for audio in &rows["audios"] {
            let audio_id = text(audio, "id")?;
            let Some(root) = audio_directory else {
                report
                    .blockers
                    .push(format!("音频 {audio_id} 尚未指定原文件目录"));
                continue;
            };
            let key = Path::new(text(audio, "blob_key")?);
            if key
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
            {
                return Err(invalid("旧音频路径越界"));
            }
            let source = match root.join(key).canonicalize() {
                Ok(path) => path,
                Err(_) => {
                    report.blockers.push(format!("音频 {audio_id} 文件缺失"));
                    continue;
                }
            };
            if !source.starts_with(root.canonicalize().map_err(unavailable)?) {
                return Err(invalid("旧音频符号链接越界"));
            }
            let extension = key
                .extension()
                .and_then(|value| value.to_str())
                .filter(|value| ["wav", "mp3"].contains(value))
                .ok_or(invalid("旧音频格式不受支持"))?;
            let (checksum, bytes) = file_hash(&source)?;
            let mut header = [0_u8; 12];
            let header_ok = std::fs::File::open(&source)
                .and_then(|mut file| file.read_exact(&mut header))
                .is_ok();
            let valid_header = header_ok
                && if extension == "wav" {
                    bytes >= 44 && &header[..4] == b"RIFF" && &header[8..] == b"WAVE"
                } else {
                    &header[..3] == b"ID3" || (header[0] == 0xff && header[1] & 0xe0 == 0xe0)
                };
            if !valid_header {
                report.blockers.push(format!("音频 {audio_id} 格式头损坏"));
            }
            if bytes == 0 {
                report.blockers.push(format!("音频 {audio_id} 文件为空"));
            }
            report.audio_bytes = report
                .audio_bytes
                .checked_add(bytes)
                .ok_or(invalid("旧音频总大小超出范围"))?;
            blobs.push(LegacyBlob {
                id: audio_id.into(),
                source,
                key: format!("{checksum}.{extension}"),
                checksum,
            });
        }
        if !blobs.is_empty() {
            report.warnings.push("旧音频缺少完整模型/端点来源，仅作为已链接历史音频；本次计算的blob校验和用于复制对账".into());
        }
        report.source_fingerprint = hash(&canonical_json(
            &json!({"rows":rows,"blobs":blobs.iter().map(|blob|(&blob.id,&blob.checksum)).collect::<Vec<_>>(),"versions":versions}),
        ));
        let mut new_ids = BTreeMap::new();
        let mut keys = vec!["provider:llm".to_owned(), "provider:tts".to_owned()];
        for card in &rows["cards"] {
            keys.push(format!("book:{}", text(card, "deck")?));
            keys.push(format!("entry:{}", text(card, "id")?));
        }
        for event in &rows["events"] {
            keys.push(format!("charge:event:{}", text(event, "id")?));
        }
        for audio in &rows["audios"] {
            keys.push(format!("charge:audio:{}", text(audio, "id")?));
        }
        keys.sort();
        keys.dedup();
        for key in keys {
            let id = next_id();
            cijian_domain::Id::try_from(id.clone()).map_err(|_| invalid("迁移标识来源无效"))?;
            if new_ids.values().any(|existing| existing == &id) {
                return Err(invalid("迁移标识来源重复"));
            }
            new_ids.insert(key, id);
        }
        Ok(LegacySnapshot {
            report,
            rows,
            new_ids,
            blobs,
        })
    }
}
