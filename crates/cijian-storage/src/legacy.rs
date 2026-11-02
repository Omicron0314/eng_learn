mod convert;
mod inspect;
#[cfg(test)]
mod tests;
use super::*;
use cijian_domain::json::canonical_json;
use cijian_ports::legacy::LegacyReport;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
};
const MAPPING_VERSION: &str = "legacy-web.v1";
pub struct LegacySnapshot {
    report: LegacyReport,
    rows: BTreeMap<String, Vec<Value>>,
    new_ids: BTreeMap<String, String>,
    blobs: Vec<LegacyBlob>,
}
struct LegacyBlob {
    id: String,
    source: PathBuf,
    key: String,
    checksum: String,
}
fn text<'a>(row: &'a Value, key: &str) -> Result<&'a str> {
    row[key].as_str().ok_or(invalid("旧库文本字段无效"))
}
fn number(row: &Value, key: &str) -> Result<i64> {
    row[key]
        .as_i64()
        .or_else(|| {
            row[key]
                .as_f64()
                .filter(|n| {
                    n.is_finite()
                        && n.fract() == 0.
                        && *n >= i64::MIN as f64
                        && *n < i64::MAX as f64
                })
                .map(|n| n as i64)
        })
        .ok_or(invalid("旧库整数字段无效"))
}
fn parsed(row: &Value, key: &str) -> Result<Value> {
    serde_json::from_str(text(row, key)?).map_err(|_| invalid("旧库JSON损坏"))
}
fn file_hash(path: &Path) -> Result<(String, u64)> {
    let mut file = std::fs::File::open(path).map_err(unavailable)?;
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(unavailable)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        bytes = bytes
            .checked_add(count as u64)
            .ok_or(invalid("音频大小超出范围"))?;
    }
    Ok((format!("{:x}", hasher.finalize()), bytes))
}
pub(super) fn has_secret(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            ([
                "apikey",
                "api_key",
                "authorization",
                "password",
                "secret",
                "secret_ref",
                "access_token",
                "refresh_token",
            ]
            .contains(&key.to_ascii_lowercase().as_str())
                && value.is_string())
                || has_secret(value)
        }),
        Value::Array(values) => values.iter().any(has_secret),
        _ => false,
    }
}
pub(super) fn validate_archive(row: &Value) -> Result<()> {
    let value: Value = serde_json::from_str(
        row["payload_json"]
            .as_str()
            .ok_or(invalid("历史档案结构无效"))?,
    )
    .map_err(|_| invalid("历史档案JSON无效"))?;
    if has_secret(&value) {
        return Err(invalid("历史档案含凭据字段"));
    }
    for key in [
        "json",
        "payload_json",
        "back_json",
        "gloss_json",
        "analysis_json",
    ] {
        if let Some(encoded) = value.get(key) {
            let nested: Value =
                serde_json::from_str(encoded.as_str().ok_or(invalid("历史JSON字段无效"))?)
                    .map_err(|_| invalid("历史JSON损坏"))?;
            if has_secret(&nested) {
                return Err(invalid("历史档案含嵌套凭据字段"));
            }
        }
    }
    Ok(())
}
impl LegacySnapshot {
    pub fn report(&self) -> &LegacyReport {
        &self.report
    }
    pub fn estimated_bytes(&self) -> Result<u64> {
        let mut bytes = self.report.audio_bytes;
        for records in self.rows.values() {
            for record in records {
                bytes = bytes
                    .checked_add(
                        (record.to_string().len() as u64)
                            .checked_mul(3)
                            .ok_or(invalid("迁移大小超出范围"))?,
                    )
                    .ok_or(invalid("迁移大小超出范围"))?;
            }
        }
        bytes
            .checked_add(16 * 1024 * 1024)
            .ok_or(invalid("迁移大小超出范围"))
    }
    pub fn copy_audio(&self, directory: &Path) -> Result<()> {
        std::fs::create_dir_all(directory).map_err(unavailable)?;
        for blob in &self.blobs {
            let destination = directory.join(&blob.key);
            if destination.exists() {
                if file_hash(&destination)?.0 == blob.checksum {
                    continue;
                }
                return Err(invalid("迁入音频目标冲突"));
            }
            let mut source = std::fs::File::open(&blob.source).map_err(unavailable)?;
            let mut output = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&destination)
                .map_err(unavailable)?;
            std::io::copy(&mut source, &mut output).map_err(unavailable)?;
            output.sync_all().map_err(unavailable)?;
            if file_hash(&destination)?.0 != blob.checksum {
                return Err(AppError::new(
                    ErrorCode::StaleState,
                    "原音频在迁入期间发生变化",
                ));
            }
        }
        Ok(())
    }
}
