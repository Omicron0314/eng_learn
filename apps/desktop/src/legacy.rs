use crate::backend::{DesktopBackend, id, now};
use cijian_app::commands::Response;
use cijian_ports::{AppError, ErrorCode, Result};
use cijian_storage::{Store, legacy::LegacySnapshot};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
fn failure() -> AppError {
    AppError::new(ErrorCode::StorageUnavailable, "旧库迁入暂不可用")
}
fn publish(snapshot: &LegacySnapshot, parent: &Path) -> Result<(PathBuf, bool)> {
    let destination = parent.join(format!(
        "cijian-legacy-{}",
        &snapshot.report().source_fingerprint[..16]
    ));
    if destination.exists() {
        Store::existing_legacy_report(&destination.join("cijian.sqlite"), snapshot)?;
        return Ok((destination, true));
    }
    let required = snapshot.estimated_bytes()?;
    if cijian_platform::files::available_bytes(parent)?
        .is_some_and(|available| available < required)
    {
        return Err(AppError::new(
            ErrorCode::StorageUnavailable,
            "目标剩余空间不足；源库和现有学习库未改动",
        ));
    }
    let staging = tempfile::Builder::new()
        .prefix(".cijian-legacy-")
        .tempdir_in(parent)
        .map_err(|_| failure())?;
    snapshot.copy_audio(&staging.path().join("audio"))?;
    let report = {
        let store = Store::open(staging.path().join("cijian.sqlite"))?;
        store.import_legacy(snapshot, &id(), now())?
    };
    let report_file = std::fs::File::create(staging.path().join("migration-report.json"))
        .map_err(|_| failure())?;
    serde_json::to_writer_pretty(&report_file, &report).map_err(|_| failure())?;
    report_file.sync_all().map_err(|_| failure())?;
    drop(report_file);
    cijian_platform::files::publish_directory(staging.path(), &destination)?;
    let _ = staging.keep();
    Ok((destination, false))
}
impl DesktopBackend {
    pub(super) fn inspect_legacy(&self) -> Result<Response> {
        let Some(path) = rfd::FileDialog::new()
            .set_title("选择旧版 SQLite 学习库（只读预检）")
            .add_filter("SQLite 学习库", &["sqlite", "sqlite3", "db"])
            .pick_file()
        else {
            return Ok(Response::FileCancelled);
        };
        let profiles = Store::legacy_profiles(&path)?;
        if profiles.is_empty() {
            return Err(AppError::new(ErrorCode::InvalidInput, "旧库没有档案"));
        }
        let token = id();
        {
            let mut selections = self.legacy_files.lock().map_err(|_| failure())?;
            selections.clear();
            selections.insert(token.clone(), path);
        }
        self.legacy_snapshots.lock().map_err(|_| failure())?.clear();
        if profiles.len() == 1 {
            self.select_legacy_profile(&token, &profiles[0].id)
        } else {
            Ok(Response::LegacyProfiles {
                selection: token,
                profiles,
            })
        }
    }
    pub(super) fn select_legacy_profile(&self, selection: &str, profile: &str) -> Result<Response> {
        let path = self
            .legacy_files
            .lock()
            .map_err(|_| failure())?
            .get(selection)
            .cloned()
            .ok_or(AppError::new(ErrorCode::StaleState, "旧库选择已过期"))?;
        let mut snapshot = Store::inspect_legacy(&path, Some(profile), None, id)?;
        if snapshot.report().source_counts["audios"] > 0 {
            let Some(directory) = rfd::FileDialog::new()
                .set_title("选择旧音频根目录（包含 audio 子目录）")
                .pick_folder()
            else {
                return Ok(Response::FileCancelled);
            };
            snapshot = Store::inspect_legacy(&path, Some(profile), Some(&directory), id)?;
        }
        let report = if snapshot.report().blockers.is_empty() {
            Store::dry_run_legacy(&snapshot, &id(), now())?
        } else {
            snapshot.report().clone()
        };
        self.legacy_snapshots
            .lock()
            .map_err(|_| failure())?
            .insert(selection.into(), Arc::new(snapshot));
        Ok(Response::LegacyPreview {
            selection: selection.into(),
            report: Box::new(report),
        })
    }
    pub(super) fn import_legacy(&self, selection: &str) -> Result<Response> {
        let snapshot = self
            .legacy_snapshots
            .lock()
            .map_err(|_| failure())?
            .get(selection)
            .cloned()
            .ok_or(AppError::new(
                ErrorCode::StaleState,
                "迁入预览已过期，请重新预检",
            ))?;
        if !snapshot.report().blockers.is_empty() {
            return Err(AppError::new(ErrorCode::InvalidInput, "请先解决预检阻断项"));
        }
        let Some(parent) = rfd::FileDialog::new()
            .set_title("选择新学习库的保存位置")
            .pick_folder()
        else {
            return Ok(Response::FileCancelled);
        };
        let (directory, already_present) = publish(&snapshot, &parent)?;
        if !already_present {
            std::process::Command::new(std::env::current_exe().map_err(|_| failure())?)
                .arg("--data-dir")
                .arg(&directory)
                .spawn()
                .map_err(|_| {
                    AppError::new(
                        ErrorCode::StorageUnavailable,
                        format!(
                            "迁入已完成，但无法打开新窗口。学习库位于 {}",
                            directory.display()
                        ),
                    )
                })?;
        }
        Ok(Response::LegacyImported {
            already_present,
            directory: directory.display().to_string(),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn publication_does_not_replace_an_existing_unrelated_directory() {
        let parent = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let path = source.path().join("source.sqlite");
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute_batch(
            "CREATE TABLE _migrations(id TEXT PRIMARY KEY,applied_at INTEGER NOT NULL)",
        )
        .unwrap();
        let versions = [
            "0001_init",
            "0002_append_only_triggers",
            "0003_provider_config",
        ];
        for (index, sql) in include_str!("../../../packages/db/src/migrations.ts")
            .split("sql: `")
            .skip(1)
            .enumerate()
        {
            db.execute_batch(sql.split('`').next().unwrap()).unwrap();
            db.execute("INSERT INTO _migrations VALUES (?1,0)", [versions[index]])
                .unwrap();
        }
        db.execute("INSERT INTO users VALUES ('p',0,0)", [])
            .unwrap();
        let snapshot = Store::inspect_legacy(&path, None, None, id).unwrap();
        let destination = parent.path().join(format!(
            "cijian-legacy-{}",
            &snapshot.report().source_fingerprint[..16]
        ));
        std::fs::create_dir(&destination).unwrap();
        assert!(publish(&snapshot, parent.path()).is_err());
        assert!(destination.exists());
        std::fs::remove_dir(&destination).unwrap();
        let (path, reused) = publish(&snapshot, parent.path()).unwrap();
        assert!(!reused);
        assert!(path.join("migration-report.json").exists());
        assert!(publish(&snapshot, parent.path()).unwrap().1);
    }
}
