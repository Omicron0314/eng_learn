use crate::backend::{DesktopBackend, id, now};
use cijian_app::commands::Response;
use cijian_ports::{AppError, ErrorCode, Result, backup::BackupPackage};
use cijian_storage::Store;
use std::io::{BufReader, Write};
fn file_error() -> AppError {
    AppError::new(
        ErrorCode::StorageUnavailable,
        "备份文件操作失败，请检查位置和剩余空间",
    )
}
impl DesktopBackend {
    pub(super) fn export_backup(&self) -> Result<Response> {
        let Some(path) = rfd::FileDialog::new()
            .set_title("导出学习备份")
            .set_file_name("词间学习备份.json")
            .add_filter("词间学习备份", &["json"])
            .save_file()
        else {
            return Ok(Response::FileCancelled);
        };
        let package = self.store.export_profile(&self.profile)?;
        let parent = path.parent().ok_or_else(file_error)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|_| file_error())?;
        serde_json::to_writer(&mut temporary, &package).map_err(|_| file_error())?;
        temporary.flush().map_err(|_| file_error())?;
        temporary.as_file().sync_all().map_err(|_| file_error())?;
        temporary.persist_noclobber(&path).map_err(|_| {
            AppError::new(
                ErrorCode::StorageUnavailable,
                "未覆盖已有文件，请选择一个新的备份文件名",
            )
        })?;
        Ok(Response::BackupSaved)
    }
    pub(super) fn restore_backup(&self) -> Result<Response> {
        let Some(path) = rfd::FileDialog::new()
            .set_title("选择词间学习备份")
            .add_filter("词间学习备份", &["json"])
            .pick_file()
        else {
            return Ok(Response::FileCancelled);
        };
        let file = std::fs::File::open(path).map_err(|_| file_error())?;
        let package: BackupPackage = serde_json::from_reader(BufReader::new(file))
            .map_err(|_| AppError::new(ErrorCode::InvalidInput, "备份文件不是有效的词间备份"))?;
        let Some(parent) = rfd::FileDialog::new()
            .set_title("选择新学习库的保存位置")
            .pick_folder()
        else {
            return Ok(Response::FileCancelled);
        };
        let directory = tempfile::Builder::new()
            .prefix("cijian-restored-")
            .tempdir_in(parent)
            .map_err(|_| file_error())?;
        {
            let restored = Store::open(directory.path().join("cijian.sqlite"))?;
            restored.restore_profile(package, &id(), &id(), &id(), now())?;
        }
        let path = directory.keep();
        std::process::Command::new(std::env::current_exe().map_err(|_| file_error())?)
            .arg("--data-dir")
            .arg(&path)
            .spawn()
            .map_err(|_| {
                AppError::new(
                    ErrorCode::StorageUnavailable,
                    format!(
                        "恢复已完成，但无法打开新窗口。学习库已保存在 {}",
                        path.display()
                    ),
                )
            })?;
        Ok(Response::RestoreOpened)
    }
}
