//! Native credential storage. Tests exercise only the in-memory session implementation.
#[cfg(not(target_family = "wasm"))]
pub mod audio;
pub mod files;
#[cfg(not(target_family = "wasm"))]
pub mod speech;
use cijian_ports::{
    AppError, ErrorCode, Result,
    provider::{Secret, SecretPatch, SecretStore},
};
use std::{collections::HashMap, sync::Mutex};

/// A real session-only secret store, used when persistence is unavailable or unwanted.
/// It does not simulate a remote provider or survive a process restart.
#[derive(Default)]
pub struct SessionSecrets {
    values: Mutex<HashMap<String, Secret>>,
}
fn lock_error() -> AppError {
    AppError::new(ErrorCode::StorageUnavailable, "会话凭据不可用")
}
impl SecretStore for SessionSecrets {
    async fn read(&self, reference: &str) -> Result<Option<Secret>> {
        Ok(self
            .values
            .lock()
            .map_err(|_| lock_error())?
            .get(reference)
            .cloned())
    }
    async fn write(&self, reference: &str, patch: SecretPatch) -> Result<()> {
        if reference.is_empty() {
            return Err(AppError::new(ErrorCode::InvalidInput, "凭据引用无效"));
        }
        let mut values = self.values.lock().map_err(|_| lock_error())?;
        match patch {
            SecretPatch::Keep => {}
            SecretPatch::Replace(value) => {
                values.insert(reference.into(), value);
            }
            SecretPatch::Clear => {
                values.remove(reference);
            }
        }
        Ok(())
    }
}

/// Accessed only for explicit configured references. Constructing this adapter makes no IO.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub struct SystemSecrets;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn entry(reference: &str) -> Result<keyring::Entry> {
    if reference.is_empty() || reference.chars().any(char::is_control) {
        return Err(AppError::new(ErrorCode::InvalidInput, "凭据引用无效"));
    }
    keyring::Entry::new("cijian", reference).map_err(|_| {
        AppError::new(
            ErrorCode::UnsupportedCapability,
            "系统凭据存储不可用，可使用会话模式",
        )
    })
}
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
impl SecretStore for SystemSecrets {
    async fn read(&self, reference: &str) -> Result<Option<Secret>> {
        match entry(reference)?.get_password() {
            Ok(value) => Ok(Some(Secret::new(value)?)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(AppError::new(
                ErrorCode::UnsupportedCapability,
                "无法读取系统凭据，可使用会话模式",
            )),
        }
    }
    async fn write(&self, reference: &str, patch: SecretPatch) -> Result<()> {
        match patch {
            SecretPatch::Keep => Ok(()),
            SecretPatch::Replace(secret) => entry(reference)?
                .set_password(secret.expose())
                .map_err(|_| {
                    AppError::new(
                        ErrorCode::UnsupportedCapability,
                        "无法保存系统凭据，可使用会话模式",
                    )
                }),
            SecretPatch::Clear => match entry(reference)?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(_) => Err(AppError::new(
                    ErrorCode::UnsupportedCapability,
                    "无法清除系统凭据",
                )),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn keep_replace_clear_and_new_session() {
        let store = SessionSecrets::default();
        assert!(store.read("p/llm").await.unwrap().is_none());
        store
            .write(
                "p/llm",
                SecretPatch::Replace(Secret::new("original-fixture".into()).unwrap()),
            )
            .await
            .unwrap();
        store.write("p/llm", SecretPatch::Keep).await.unwrap();
        let secret = store.read("p/llm").await.unwrap().unwrap();
        assert_eq!(secret.expose(), "original-fixture");
        assert!(!format!("{secret:?}").contains("original-fixture"));
        assert!(
            SessionSecrets::default()
                .read("p/llm")
                .await
                .unwrap()
                .is_none()
        );
        store.write("p/llm", SecretPatch::Clear).await.unwrap();
        assert!(store.read("p/llm").await.unwrap().is_none());
    }
}
