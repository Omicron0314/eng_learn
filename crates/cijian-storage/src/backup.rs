#[cfg(test)]
mod tests;
use super::*;
use cijian_ports::backup::*;
use rusqlite::types::{Value as SqlValue, ValueRef};
use serde_json::{Value, json};
use std::collections::BTreeMap;
const AUDIO_TABLES: &[&str] = &["audio_assets", "article_audio_links", "word_audio"];
/// Word recordings joined the audio payload in version 4; older audio packages lack them.
const WORD_AUDIO_VERSION: u32 = 4;
const TABLES: &[&str] = &[
    "profiles",
    "devices",
    "settings",
    "lexemes",
    "word_books",
    "imports",
    "book_entries",
    "cards",
    "articles",
    "article_targets",
    "reading_sessions",
    "lexeme_marks",
    "inbox",
    "audio_reservations",
    "audio_sessions",
    "review_logs",
    "events",
    "llm_calls",
    "legacy_records",
    "legacy_imports",
];
/// Insert order must satisfy foreign keys; audio assets precede sessions and links.
const RESTORE_ORDER: &[&str] = &[
    "profiles",
    "devices",
    "settings",
    "lexemes",
    "word_books",
    "imports",
    "book_entries",
    "cards",
    "articles",
    "article_targets",
    "audio_assets",
    "article_audio_links",
    "word_audio",
    "reading_sessions",
    "lexeme_marks",
    "inbox",
    "audio_reservations",
    "audio_sessions",
    "review_logs",
    "events",
    "llm_calls",
    "legacy_records",
    "legacy_imports",
];
fn secret_fields(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            [
                "apikey",
                "api_key",
                "authorization",
                "password",
                "secret",
                "secret_ref",
                "access_token",
                "refresh_token",
            ]
            .contains(&key.to_lowercase().as_str())
                || secret_fields(value)
        }),
        Value::Array(values) => values.iter().any(secret_fields),
        _ => false,
    }
}
pub(super) fn sql_value(value: &Value) -> Result<SqlValue> {
    match value {
        Value::Null => Ok(SqlValue::Null),
        Value::String(value) => Ok(SqlValue::Text(value.clone())),
        Value::Number(value) => {
            if let Some(integer) = value.as_i64() {
                Ok(SqlValue::Integer(integer))
            } else if let Some(number) = value.as_f64().filter(|n| n.is_finite()) {
                Ok(SqlValue::Real(number))
            } else {
                Err(invalid("备份数值超出范围"))
            }
        }
        _ => Err(invalid("备份列类型无效")),
    }
}
fn valid_blob_key(key: &str) -> bool {
    key.rsplit_once('.').is_some_and(|(stem, extension)| {
        stem.len() == 64
            && stem.bytes().all(|byte| byte.is_ascii_hexdigit())
            && ["wav", "mp3"].contains(&extension)
    })
}
fn columns(db: &Connection, table: &str) -> Result<Vec<String>> {
    let mut query = db
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(unavailable)?;
    query
        .query_map([], |r| r.get(1))
        .map_err(unavailable)?
        .collect::<std::result::Result<_, _>>()
        .map_err(unavailable)
}
impl Store {
    /// Consistent logical snapshot; credentials and unfinished jobs are excluded.
    /// Audio metadata and links are included only when the caller will attach verified blobs.
    pub fn export_profile(&self, profile: &str, include_audio: bool) -> Result<BackupPackage> {
        let mut db = self.connection.lock().map_err(unavailable)?;
        let tx = db.transaction().map_err(unavailable)?;
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
                [profile],
                |r| r.get(0),
            )
            .map_err(unavailable)?;
        if !exists {
            return Err(AppError::new(ErrorCode::NotFound, "用户档案不存在"));
        }
        let table_names = TABLES
            .iter()
            .copied()
            .chain(include_audio.then_some("audio_assets"))
            .chain(include_audio.then_some("article_audio_links"))
            .chain(include_audio.then_some("word_audio"))
            .collect::<Vec<_>>();
        let mut tables = serde_json::Map::new();
        for table in &table_names {
            let query = if *table == "profiles" {
                format!("SELECT * FROM {table} WHERE id=?1")
            } else if *table == "lexemes" {
                "SELECT * FROM lexemes WHERE id IN (SELECT lexeme_id FROM cards WHERE profile_id=?1 UNION SELECT lexeme_id FROM book_entries WHERE profile_id=?1 UNION SELECT lexeme_id FROM article_targets WHERE profile_id=?1 UNION SELECT lexeme_id FROM lexeme_marks WHERE profile_id=?1 UNION SELECT lexeme_id FROM inbox WHERE profile_id=?1) ORDER BY id".into()
            } else {
                format!("SELECT * FROM {table} WHERE profile_id=?1 ORDER BY rowid")
            };
            let mut statement = tx.prepare(&query).map_err(unavailable)?;
            let names = statement
                .column_names()
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>();
            let mut cursor = statement.query([profile]).map_err(unavailable)?;
            let mut rows = Vec::new();
            while let Some(row) = cursor.next().map_err(unavailable)? {
                let mut object = serde_json::Map::new();
                for (index, name) in names.iter().enumerate() {
                    let value = match row.get_ref(index).map_err(unavailable)? {
                        ValueRef::Null => Value::Null,
                        ValueRef::Integer(v) => json!(v),
                        ValueRef::Real(v) => {
                            if !v.is_finite() {
                                return Err(invalid("数据库含非法浮点数"));
                            }
                            json!(v)
                        }
                        ValueRef::Text(v) => {
                            Value::String(std::str::from_utf8(v).map_err(unavailable)?.into())
                        }
                        ValueRef::Blob(_) => return Err(invalid("学习备份不包含二进制列")),
                    };
                    object.insert(name.clone(), value);
                }
                if *table == "audio_sessions" && !include_audio {
                    object.insert("audio_id".into(), Value::Null);
                }
                if *table == "llm_calls" {
                    object.insert("job_id".into(), Value::Null);
                }
                if *table == "events" {
                    let payload: Value = serde_json::from_str(
                        object["payload_json"]
                            .as_str()
                            .ok_or(invalid("事件结构无效"))?,
                    )
                    .map_err(unavailable)?;
                    if secret_fields(&payload) {
                        return Err(invalid("事件含凭据字段，拒绝导出"));
                    }
                }
                if *table == "legacy_records" {
                    crate::legacy::validate_archive(&Value::Object(object.clone()))?;
                }
                rows.push(Value::Object(object));
            }
            tables.insert((*table).into(), Value::Array(rows));
        }
        let payload = json!({"profile":profile,"tables":tables,"audioIncluded":include_audio,"audioBlobs":[],"credentialsIncluded":false});
        let checksum = hash(&cijian_domain::json::canonical_json(&payload));
        tx.commit().map_err(unavailable)?;
        Ok(BackupPackage {
            format: "cijian-profile".into(),
            version: WORD_AUDIO_VERSION,
            checksum,
            payload,
        })
    }
    /// Cache files that a v3 audio backup must attach and verify.
    pub fn audio_blob_keys(&self, profile: &str) -> Result<Vec<String>> {
        let db = self.connection.lock().map_err(unavailable)?;
        let mut statement = db
            .prepare("SELECT blob_key FROM audio_assets WHERE profile_id=?1 UNION SELECT blob_key FROM word_audio WHERE profile_id=?1 ORDER BY 1")
            .map_err(unavailable)?;
        statement
            .query_map([profile], |row| row.get::<_, String>(0))
            .map_err(unavailable)?
            .collect::<std::result::Result<_, _>>()
            .map_err(unavailable)
    }
    /// Restore only into an empty new store; the caller publishes its directory after success.
    pub fn restore_profile(
        &self,
        package: BackupPackage,
        new_profile: &str,
        event: &str,
        operation: &str,
        now: i64,
    ) -> Result<RestoreReport> {
        if package.format != "cijian-profile"
            || ![1, 2, 3, 4].contains(&package.version)
            || hash(&if package.version == 1 {
                package.payload.to_string()
            } else {
                cijian_domain::json::canonical_json(&package.payload)
            }) != package.checksum
        {
            return Err(invalid("备份格式、版本或校验和无效"));
        }
        if new_profile.trim().is_empty() || event.is_empty() || operation.is_empty() {
            return Err(invalid("恢复标识无效"));
        }
        let source = package.payload["profile"]
            .as_str()
            .ok_or(invalid("备份档案标识缺失"))?;
        if new_profile == source {
            return Err(invalid("恢复必须使用新的档案标识"));
        }
        let mut tables = package.payload["tables"]
            .as_object()
            .cloned()
            .ok_or(invalid("备份表结构无效"))?;
        if package.version == 1 {
            if tables.contains_key("legacy_records") || tables.contains_key("legacy_imports") {
                return Err(invalid("v1备份包含未声明的迁移档案"));
            }
            tables.insert("legacy_records".into(), json!([]));
            tables.insert("legacy_imports".into(), json!([]));
        }
        if package.version < WORD_AUDIO_VERSION {
            tables.remove("word_audio");
        }
        let audio_included = package.payload["audioIncluded"]
            .as_bool()
            .ok_or(invalid("备份音频标记无效"))?;
        if package.payload["credentialsIncluded"] != false {
            return Err(invalid("备份包含凭据"));
        }
        if audio_included && package.version < 3 {
            return Err(invalid("旧版本备份不能声明音频内容"));
        }
        if audio_included {
            if AUDIO_TABLES
                .iter()
                .any(|table| !tables.contains_key(*table))
            {
                return Err(invalid("音频备份缺少音频表"));
            }
        } else if AUDIO_TABLES.iter().any(|table| tables.contains_key(*table)) {
            return Err(invalid("非音频备份包含音频表"));
        }
        let expected = TABLES.len()
            + if audio_included {
                AUDIO_TABLES.len()
            } else {
                0
            }
            - if audio_included && package.version < WORD_AUDIO_VERSION {
                1
            } else {
                0
            };
        if tables.len() != expected || TABLES.iter().any(|table| !tables.contains_key(*table)) {
            return Err(invalid("备份包含未知或缺失的内容"));
        }
        let blobs = package.payload.get("audioBlobs");
        let blobs = match (audio_included, blobs) {
            (false, None) => Vec::new(),
            (false, Some(Value::Array(blobs))) if blobs.is_empty() => Vec::new(),
            (true, Some(Value::Array(blobs))) => blobs.clone(),
            _ => return Err(invalid("备份音频清单无效")),
        };
        let assets = tables
            .get("audio_assets")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut asset_keys = std::collections::BTreeSet::new();
        for asset in &assets {
            let key = asset["blob_key"]
                .as_str()
                .filter(|key| valid_blob_key(key))
                .ok_or(invalid("音频缓存键无效"))?;
            if !asset_keys.insert(key.to_owned()) {
                return Err(invalid("音频缓存键重复"));
            }
        }
        let mut blob_keys = std::collections::BTreeSet::new();
        for blob in &blobs {
            let key = blob["blobKey"]
                .as_str()
                .filter(|key| valid_blob_key(key))
                .ok_or(invalid("音频缓存键无效"))?;
            let stem = key
                .rsplit_once('.')
                .map(|(stem, _)| stem)
                .unwrap_or_default();
            if blob["sha256"].as_str() != Some(stem)
                || blob["bytes"].as_str().is_none_or(str::is_empty)
                || !blob_keys.insert(key.to_owned())
            {
                return Err(invalid("音频缓存清单与校验和不一致"));
            }
        }
        if asset_keys != blob_keys {
            return Err(invalid("音频缓存与音频表不一致"));
        }
        let mut db = self.connection.lock().map_err(unavailable)?;
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unavailable)?;
        if tx
            .query_row("SELECT count(*) FROM profiles", [], |r| r.get::<_, i64>(0))
            .map_err(unavailable)?
            != 0
        {
            return Err(invalid("恢复目标必须为新的空学习库"));
        }
        let mut counts = BTreeMap::new();
        for table in RESTORE_ORDER {
            if !tables.contains_key(*table) {
                continue;
            }
            let names = columns(&tx, table)?;
            let rows = tables[*table].as_array().ok_or(invalid("备份行结构无效"))?;
            if ["profiles", "settings"].contains(table) && rows.len() != 1 {
                return Err(invalid("备份必须包含单个档案及其设置"));
            }
            for row in rows {
                let object = row.as_object().ok_or(invalid("备份行不是对象"))?;
                let mut object = object.clone();
                if *table == "cards" && !object.contains_key("ease_factor") {
                    object.insert(
                        "ease_factor".into(),
                        json!(cijian_domain::srs::INITIAL_EASE_FACTOR),
                    );
                }
                if object.len() != names.len() || names.iter().any(|n| !object.contains_key(n)) {
                    return Err(invalid("备份含未知或缺失的列"));
                }
                if *table == "legacy_records" {
                    crate::legacy::validate_archive(row)?;
                }
                if *table == "profiles" {
                    if object["id"] != source {
                        return Err(invalid("档案身份不一致"));
                    }
                    object.insert("id".into(), json!(new_profile));
                } else if *table != "lexemes" {
                    if object["profile_id"] != source {
                        return Err(invalid("备份包含其他档案的数据"));
                    }
                    object.insert("profile_id".into(), json!(new_profile));
                }
                if *table == "settings" {
                    let settings: Settings =
                        serde_json::from_str(object["json"].as_str().ok_or(invalid("设置无效"))?)
                            .map_err(|_| invalid("设置格式无效"))?;
                    ValidatedSettings::try_from(settings).map_err(|e| invalid(&e.to_string()))?;
                }
                if *table == "articles" {
                    let body = object["body"].as_str().ok_or(invalid("文章正文无效"))?;
                    if object["body_hash"] != hash(body) {
                        return Err(invalid("文章正文校验失败"));
                    }
                }
                if *table == "events" {
                    let payload: Value = serde_json::from_str(
                        object["payload_json"]
                            .as_str()
                            .ok_or(invalid("事件结构无效"))?,
                    )
                    .map_err(|_| invalid("事件内容无效"))?;
                    if secret_fields(&payload) {
                        return Err(invalid("备份事件包含凭据字段"));
                    }
                }
                if *table == "audio_sessions" && !object["audio_id"].is_null() {
                    return Err(invalid("无音频备份不能引用音频缓存"));
                }
                let values = names
                    .iter()
                    .map(|name| sql_value(&object[name]))
                    .collect::<Result<Vec<_>>>()?;
                let placeholders = (0..names.len()).map(|_| "?").collect::<Vec<_>>().join(",");
                tx.execute(
                    &format!(
                        "INSERT INTO {table} ({}) VALUES ({placeholders})",
                        names.join(",")
                    ),
                    rusqlite::params_from_iter(values),
                )
                .map_err(unavailable)?;
            }
            counts.insert((*table).into(), rows.len() as u64);
        }
        {
            let mut query = tx.prepare("SELECT * FROM cards").map_err(unavailable)?;
            let cards = query
                .query_map([], super::library::read_card)
                .map_err(unavailable)?;
            for card in cards {
                card.map_err(unavailable)?
                    .srs
                    .validate()
                    .map_err(|e| invalid(&e.to_string()))?;
            }
        }
        if tx
            .query_row("PRAGMA foreign_key_check", [], |_| Ok(()))
            .optional()
            .map_err(unavailable)?
            .is_some()
        {
            return Err(invalid("恢复数据的引用不完整"));
        }
        tx.execute("INSERT INTO events VALUES (?1,?2,NULL,?3,0,'backup_restore',1,?4,?5)",params![event,new_profile,operation,json!({"sourceFingerprint":package.checksum,"mappingVersion":format!("backup.v{}",package.version),"summary":counts}).to_string(),now]).map_err(unavailable)?;
        tx.commit().map_err(unavailable)?;
        Ok(RestoreReport {
            profile: new_profile.into(),
            counts,
            audio_included,
            credentials_included: false,
        })
    }
}
