//! Native SQLite adapter. Its synchronous transaction stays inside the adapter;
//! callers execute native IO on a background worker, never in GPUI render.
mod articles;
mod calls;
mod inbox;
mod library;
mod reading;
use cijian_domain::settings::{Settings, ValidatedSettings};
use cijian_ports::{AppError, ErrorCode, Event, Result, SaveSettings, SettingsRepository};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Mutex, time::Duration};

const INITIAL: &str = include_str!("../migrations/0001_initial.sql");
const MIGRATIONS: &[&str] = &[
    INITIAL,
    include_str!("../migrations/0002_generation_snapshots.sql"),
];
fn unavailable(_: impl std::fmt::Display) -> AppError {
    AppError::new(ErrorCode::StorageUnavailable, "本地数据库操作失败")
}
fn invalid(message: &str) -> AppError {
    AppError::new(ErrorCode::InvalidInput, message)
}
fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

pub struct Store {
    connection: Mutex<Connection>,
}
impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::initialize(Connection::open(path).map_err(unavailable)?)
    }
    pub fn memory() -> Result<Self> {
        Self::initialize(Connection::open_in_memory().map_err(unavailable)?)
    }
    fn initialize(mut db: Connection) -> Result<Self> {
        db.busy_timeout(Duration::from_secs(5))
            .map_err(unavailable)?;
        db.execute_batch("PRAGMA foreign_keys=ON;")
            .map_err(unavailable)?;
        let tables:i64=db.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",[],|r|r.get(0)).map_err(unavailable)?;
        let has_migrations:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='migrations')",[],|r|r.get(0)).map_err(unavailable)?;
        if tables > 0 && !has_migrations {
            return Err(AppError::new(
                ErrorCode::StorageUnavailable,
                "数据库格式未知，请使用独立迁移流程",
            ));
        }
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unavailable)?;
        tx.execute_batch("CREATE TABLE IF NOT EXISTS migrations(version INTEGER PRIMARY KEY, checksum TEXT NOT NULL);").map_err(unavailable)?;
        let versions: Vec<(i64, String)> = {
            let mut stmt = tx
                .prepare("SELECT version,checksum FROM migrations ORDER BY version")
                .map_err(unavailable)?;
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .map_err(unavailable)?
                .collect::<std::result::Result<_, _>>()
                .map_err(unavailable)?
        };
        if versions.len() > MIGRATIONS.len()
            || versions
                .iter()
                .enumerate()
                .any(|(index, (version, checksum))| {
                    *version != index as i64 + 1 || checksum != &hash(MIGRATIONS[index])
                })
        {
            return Err(AppError::new(
                ErrorCode::StorageUnavailable,
                "数据库迁移版本或校验和不匹配",
            ));
        }
        for (index, migration) in MIGRATIONS.iter().enumerate().skip(versions.len()) {
            tx.execute_batch(migration).map_err(unavailable)?;
            tx.execute(
                "INSERT INTO migrations VALUES (?1,?2)",
                params![index as i64 + 1, hash(migration)],
            )
            .map_err(unavailable)?;
        }
        tx.commit().map_err(unavailable)?;
        db.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(unavailable)?;
        Ok(Self {
            connection: Mutex::new(db),
        })
    }
    /// Bootstrap is atomic and idempotent. No provider or secret is created.
    pub fn create_profile(&self, id: &str, name: &str, now: i64) -> Result<()> {
        if id.is_empty() || name.trim().is_empty() {
            return Err(invalid("用户档案名称无效"));
        }
        let mut db = self.connection.lock().map_err(unavailable)?;
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unavailable)?;
        let inserted = tx
            .execute(
                "INSERT OR IGNORE INTO profiles VALUES (?1,?2,?3,?3)",
                params![id, name, now],
            )
            .map_err(unavailable)?;
        if inserted > 0 {
            tx.execute(
                "INSERT INTO settings VALUES (?1,1,?2,1)",
                params![
                    id,
                    serde_json::to_string(&Settings::default()).map_err(unavailable)?
                ],
            )
            .map_err(unavailable)?;
            tx.execute(
                "INSERT INTO events VALUES (?1,?2,NULL,?3,0,'profile_create',1,'{}',?4)",
                params![
                    format!("bootstrap-{id}"),
                    id,
                    format!("bootstrap-{id}"),
                    now
                ],
            )
            .map_err(unavailable)?;
        }
        tx.commit().map_err(unavailable)
    }
    /// Adapter-internal transaction: business, events and receipt commit together.
    fn command(
        &self,
        profile: &str,
        operation: &str,
        kind: &str,
        request: &str,
        now: i64,
        apply: impl FnOnce(&Transaction<'_>) -> Result<(serde_json::Value, Vec<Event>)>,
    ) -> Result<serde_json::Value> {
        if profile.is_empty() || operation.is_empty() {
            return Err(invalid("命令标识不能为空"));
        }
        let mut db = self.connection.lock().map_err(unavailable)?;
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unavailable)?;
        let previous:Option<(String,String,String)>=tx.query_row("SELECT command_type,request_hash,result_json FROM command_receipts WHERE profile_id=?1 AND operation_id=?2",params![profile,operation],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(unavailable)?;
        let request_hash = hash(request);
        if let Some((old_kind, old_hash, value)) = previous {
            if old_kind != kind || old_hash != request_hash {
                return Err(invalid("操作标识已被不同命令使用"));
            }
            return serde_json::from_str(&value).map_err(unavailable);
        }
        let (value, events) = apply(&tx)?;
        if events.is_empty() {
            return Err(invalid("业务写入缺少事件"));
        }
        for (ordinal, event) in events.into_iter().enumerate() {
            tx.execute(
                "INSERT INTO events VALUES (?1,?2,NULL,?3,?4,?5,?6,?7,?8)",
                params![
                    event.id,
                    profile,
                    operation,
                    ordinal as i64,
                    event.kind,
                    event.schema_version,
                    event.payload.to_string(),
                    now
                ],
            )
            .map_err(unavailable)?;
        }
        tx.execute(
            "INSERT INTO command_receipts VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                profile,
                operation,
                kind,
                request_hash,
                value.to_string(),
                now
            ],
        )
        .map_err(unavailable)?;
        tx.commit().map_err(unavailable)?;
        Ok(value)
    }
    pub fn first_profile(&self) -> Result<Option<String>> {
        self.connection
            .lock()
            .map_err(unavailable)?
            .query_row(
                "SELECT id FROM profiles ORDER BY created_at,id LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(unavailable)
    }

    pub fn settings(&self, profile: &str) -> Result<(Settings, u64)> {
        let db = self.connection.lock().map_err(unavailable)?;
        let result: Option<(String, i64)> = db
            .query_row(
                "SELECT json,revision FROM settings WHERE profile_id=?1",
                [profile],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(unavailable)?;
        let (json, revision) =
            result.ok_or(AppError::new(ErrorCode::NotFound, "用户档案不存在"))?;
        let settings: Settings = serde_json::from_str(&json).map_err(unavailable)?;
        ValidatedSettings::try_from(settings.clone()).map_err(unavailable)?;
        Ok((settings, u64::try_from(revision).map_err(unavailable)?))
    }
    pub fn update_settings(&self, command: SaveSettings) -> Result<u64> {
        let json = serde_json::to_string(command.value.get()).map_err(unavailable)?;
        let request=serde_json::json!({"expectedRevision":command.expected_revision,"value":command.value.get()}).to_string();
        let revision = command
            .expected_revision
            .checked_add(1)
            .ok_or(invalid("版本号超出范围"))?;
        let expected_revision =
            i64::try_from(command.expected_revision).map_err(|_| invalid("版本号超出范围"))?;
        let revision = i64::try_from(revision).map_err(|_| invalid("版本号超出范围"))?;
        let result=self.command(&command.profile_id,&command.operation_id,"save_settings",&request,command.now,|tx| {
            let changed=tx.execute("UPDATE settings SET json=?1,revision=?2 WHERE profile_id=?3 AND revision=?4",params![json,revision,command.profile_id,expected_revision]).map_err(unavailable)?;
            if changed!=1 { return Err(AppError::new(ErrorCode::StaleState,"设置已变更，请刷新后重试")); }
            Ok((serde_json::json!(revision),vec![Event{id:command.event_id,kind:"settings_update".into(),schema_version:1,payload:serde_json::json!({"configRevision":revision})}]))
        })?;
        result
            .as_u64()
            .ok_or_else(|| unavailable("invalid receipt"))
    }
}
impl SettingsRepository for Store {
    async fn load_settings(&self, profile: &str) -> Result<(Settings, u64)> {
        self.settings(profile)
    }
    async fn save_settings(&self, command: SaveSettings) -> Result<u64> {
        self.update_settings(command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn command(op: &str, event: &str, revision: u64) -> SaveSettings {
        SaveSettings {
            profile_id: "p".into(),
            operation_id: op.into(),
            event_id: event.into(),
            expected_revision: revision,
            now: 100,
            value: Settings {
                daily_new_limit: 10,
                ..Settings::default()
            }
            .try_into()
            .unwrap(),
        }
    }
    #[test]
    fn duplicate_replay_and_stale_command_do_not_repeat_events() {
        let store = Store::memory().unwrap();
        store.create_profile("p", "测试", 0).unwrap();
        assert_eq!(store.update_settings(command("op", "event", 1)).unwrap(), 2);
        assert_eq!(store.update_settings(command("op", "event", 1)).unwrap(), 2);
        assert_eq!(
            store
                .update_settings(command("stale", "event2", 1))
                .unwrap_err()
                .code,
            ErrorCode::StaleState
        );
        let db = store.connection.lock().unwrap();
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM events WHERE type='settings_update'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM command_receipts", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert!(db.execute("DELETE FROM events", []).is_err());
        assert!(
            db.execute("UPDATE events SET payload_json='{}'", [])
                .is_err()
        );
    }
    #[test]
    fn event_failure_rolls_back_settings_and_receipt() {
        let store = Store::memory().unwrap();
        store.create_profile("p", "测试", 0).unwrap();
        assert!(
            store
                .update_settings(command("op", "bootstrap-p", 1))
                .is_err()
        );
        assert_eq!(store.settings("p").unwrap().1, 1);
        assert_eq!(store.settings("p").unwrap().0.daily_new_limit, 20);
        assert_eq!(
            store
                .connection
                .lock()
                .unwrap()
                .query_row("SELECT count(*) FROM command_receipts", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn reopen_and_second_connection_reject_stale_revision() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("test.sqlite");
        {
            let a = Store::open(&path).unwrap();
            a.create_profile("p", "测试", 0).unwrap();
            let b = Store::open(&path).unwrap();
            a.update_settings(command("a", "ea", 1)).unwrap();
            assert_eq!(
                b.update_settings(command("b", "eb", 1)).unwrap_err().code,
                ErrorCode::StaleState
            );
        }
        assert_eq!(Store::open(&path).unwrap().settings("p").unwrap().1, 2);
    }
    #[test]
    fn upgrades_initial_database_without_rewriting_its_migration() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("version-one.sqlite");
        {
            let db = Connection::open(&path).unwrap();
            db.execute_batch(INITIAL).unwrap();
            db.execute_batch(
                "CREATE TABLE migrations(version INTEGER PRIMARY KEY, checksum TEXT NOT NULL);",
            )
            .unwrap();
            db.execute("INSERT INTO migrations VALUES (1,?1)", [hash(INITIAL)])
                .unwrap();
            db.execute(
                "INSERT INTO profiles VALUES ('legacy-rust','保留档案',0,0)",
                [],
            )
            .unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(
            store.first_profile().unwrap().as_deref(),
            Some("legacy-rust")
        );
        let db = store.connection.lock().unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM migrations", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            MIGRATIONS.len() as i64
        );
        assert_eq!(
            db.query_row("SELECT checksum FROM migrations WHERE version=1", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            hash(INITIAL)
        );
    }
    #[test]
    fn unknown_database_and_modified_migration_are_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("test.sqlite");
        {
            let db = Connection::open(&path).unwrap();
            db.execute("CREATE TABLE legacy(value TEXT)", []).unwrap();
        }
        assert!(Store::open(&path).is_err());
        let other = temp.path().join("new.sqlite");
        drop(Store::open(&other).unwrap());
        Connection::open(&other)
            .unwrap()
            .execute("UPDATE migrations SET checksum='tampered'", [])
            .unwrap();
        assert!(Store::open(other).is_err());
    }
}
