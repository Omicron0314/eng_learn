use crate::backend::{DesktopBackend, id, now};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use cijian_app::commands::Response;
use cijian_ports::{AppError, ErrorCode, Result, backup::BackupPackage};
use cijian_storage::Store;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{BufReader, Read, Write},
    path::Path,
};
fn file_error() -> AppError {
    AppError::new(
        ErrorCode::StorageUnavailable,
        "备份文件操作失败，请检查位置和剩余空间",
    )
}
fn audio_error() -> AppError {
    AppError::new(
        ErrorCode::StorageUnavailable,
        "音频缓存缺失或损坏，无法包含音频；可重试或改为不含音频的备份",
    )
}
fn valid_blob_key(key: &str) -> bool {
    key.rsplit_once('.').is_some_and(|(stem, extension)| {
        stem.len() == 64
            && stem.bytes().all(|byte| byte.is_ascii_hexdigit())
            && ["wav", "mp3"].contains(&extension)
    })
}
fn seal(package: &mut BackupPackage) {
    package.checksum = format!(
        "{:x}",
        Sha256::digest(cijian_domain::json::canonical_json(&package.payload).as_bytes())
    );
}
fn attach_audio(directory: &Path, keys: &[String], package: &mut BackupPackage) -> Result<()> {
    let mut blobs = Vec::with_capacity(keys.len());
    for key in keys {
        if !valid_blob_key(key) {
            return Err(audio_error());
        }
        let stem = key
            .rsplit_once('.')
            .map(|(stem, _)| stem)
            .unwrap_or_default();
        let mut file =
            std::fs::File::open(directory.join("audio").join(key)).map_err(|_| audio_error())?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(|_| audio_error())?;
        if format!("{:x}", Sha256::digest(&bytes)) != stem {
            return Err(audio_error());
        }
        blobs.push(json!({"blobKey":key,"sha256":stem,"bytes":STANDARD.encode(bytes)}));
    }
    package.payload["audioBlobs"] = Value::Array(blobs);
    package.payload["audioIncluded"] = json!(true);
    seal(package);
    Ok(())
}
fn decode_audio_blobs(package: &BackupPackage) -> Result<Vec<(String, Vec<u8>)>> {
    let Some(blobs) = package.payload.get("audioBlobs").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut result = Vec::with_capacity(blobs.len());
    for blob in blobs {
        let key = blob["blobKey"]
            .as_str()
            .filter(|key| valid_blob_key(key))
            .ok_or_else(|| AppError::new(ErrorCode::InvalidInput, "备份音频缓存键无效"))?;
        let stem = key
            .rsplit_once('.')
            .map(|(stem, _)| stem)
            .unwrap_or_default();
        if blob["sha256"].as_str() != Some(stem) {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                "备份音频校验和不一致",
            ));
        }
        let bytes = STANDARD
            .decode(blob["bytes"].as_str().unwrap_or_default())
            .map_err(|_| AppError::new(ErrorCode::InvalidInput, "备份音频编码无效"))?;
        if format!("{:x}", Sha256::digest(&bytes)) != stem {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                "备份音频内容与校验和不一致",
            ));
        }
        result.push((key.to_owned(), bytes));
    }
    Ok(result)
}
fn write_audio_blobs(directory: &Path, blobs: &[(String, Vec<u8>)]) -> Result<()> {
    if blobs.is_empty() {
        return Ok(());
    }
    let audio = directory.join("audio");
    std::fs::create_dir_all(&audio).map_err(|_| file_error())?;
    for (key, bytes) in blobs {
        let mut temporary = tempfile::NamedTempFile::new_in(&audio).map_err(|_| file_error())?;
        temporary.write_all(bytes).map_err(|_| file_error())?;
        temporary.as_file().sync_all().map_err(|_| file_error())?;
        temporary
            .persist_noclobber(audio.join(key))
            .map_err(|_| file_error())?;
    }
    std::fs::File::open(&audio)
        .and_then(|file| file.sync_all())
        .map_err(|_| file_error())
}
impl DesktopBackend {
    pub(super) fn export_backup(&self, include_audio: bool) -> Result<Response> {
        let Some(path) = rfd::FileDialog::new()
            .set_title("导出学习备份")
            .set_file_name(if include_audio {
                "词间学习备份（含音频）.json"
            } else {
                "词间学习备份.json"
            })
            .add_filter("词间学习备份", &["json"])
            .save_file()
        else {
            return Ok(Response::FileCancelled);
        };
        let mut package = self.store.export_profile(&self.profile, include_audio)?;
        if include_audio {
            let keys = self.store.audio_blob_keys(&self.profile)?;
            attach_audio(&self.directory, &keys, &mut package)?;
        }
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
        let blobs = decode_audio_blobs(&package)?;
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
        write_audio_blobs(directory.path(), &blobs)?;
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
#[cfg(test)]
mod tests;
