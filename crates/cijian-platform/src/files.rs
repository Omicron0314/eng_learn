use cijian_ports::{AppError, ErrorCode, Result};
use std::path::Path;
/// Publish a fully validated sibling staging directory without replacing an existing destination.
pub fn publish_directory(source: &Path, destination: &Path) -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    {
        rustix::fs::renameat_with(
            rustix::fs::CWD,
            source,
            rustix::fs::CWD,
            destination,
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(|_| {
            AppError::new(
                ErrorCode::StorageUnavailable,
                "新学习库发布失败：目标已存在或文件系统不可写",
            )
        })
    }
    #[cfg(target_os = "windows")]
    {
        std::fs::rename(source, destination).map_err(|_| {
            AppError::new(
                ErrorCode::StorageUnavailable,
                "新学习库发布失败：目标已存在或文件系统不可写",
            )
        })
    }
    #[cfg(not(any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "windows"
    )))]
    {
        let _ = (source, destination);
        Err(AppError::new(
            ErrorCode::UnsupportedCapability,
            "当前平台尚未提供无覆盖目录发布",
        ))
    }
}
pub fn available_bytes(path: &Path) -> Result<Option<u64>> {
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    {
        let value = rustix::fs::statvfs(path)
            .map_err(|_| AppError::new(ErrorCode::StorageUnavailable, "无法检查目标空间"))?;
        Ok(value.f_bavail.checked_mul(value.f_frsize))
    }
    #[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
    {
        let _ = path;
        Ok(None)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn publishing_never_replaces_even_an_empty_existing_directory() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("staging");
        let target = root.path().join("target");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&target).unwrap();
        std::fs::write(source.join("original"), "original fixture").unwrap();
        assert!(publish_directory(&source, &target).is_err());
        assert!(source.join("original").exists());
        std::fs::remove_dir(&target).unwrap();
        publish_directory(&source, &target).unwrap();
        assert!(target.join("original").exists());
        assert!(!source.exists());
    }
}
