//! Native credential storage. Tests exercise only the in-memory session implementation.
#[cfg(not(target_family = "wasm"))]
pub mod audio;
pub mod files;
pub mod kokoro;
#[cfg(not(target_family = "wasm"))]
pub mod speech;
pub mod timestretch;
use cijian_ports::{
    AppError, ErrorCode, Result,
    provider::{Secret, SecretPatch, SecretSource, SecretStore},
};
#[cfg(test)]
use std::sync::Arc;
use std::{
    collections::HashMap,
    sync::{
        Mutex,
        atomic::{AtomicU8, Ordering},
    },
};

/// Credentials use the operating system's store when it works, otherwise only this process.
/// The source is observable so the product never claims persistence that did not happen.
pub struct RuntimeSecrets {
    system: Option<SystemBackend>,
    session: SessionSecrets,
    source: AtomicU8,
}
enum SystemBackend {
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    Keyring(SystemSecrets),
    #[cfg(test)]
    Memory(Arc<SessionSecrets>),
    #[cfg(test)]
    Unavailable,
}
const UNKNOWN: u8 = 0;
const SYSTEM: u8 = 1;
const SESSION: u8 = 2;
impl SystemBackend {
    async fn read(&self, reference: &str) -> Result<Option<Secret>> {
        match self {
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            Self::Keyring(store) => store.read(reference).await,
            #[cfg(test)]
            Self::Memory(store) => store.read(reference).await,
            #[cfg(test)]
            Self::Unavailable => Err(AppError::new(
                ErrorCode::UnsupportedCapability,
                "原创测试系统凭据不可用",
            )),
        }
    }
    async fn write(&self, reference: &str, patch: SecretPatch) -> Result<()> {
        match self {
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            Self::Keyring(store) => store.write(reference, patch).await,
            #[cfg(test)]
            Self::Memory(store) => store.write(reference, patch).await,
            #[cfg(test)]
            Self::Unavailable => Err(AppError::new(
                ErrorCode::UnsupportedCapability,
                "原创测试系统凭据不可用",
            )),
        }
    }
}
impl RuntimeSecrets {
    /// Prefer the system store; operations that fail fall back to session memory.
    pub fn system_or_session() -> Self {
        Self {
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            system: Some(SystemBackend::Keyring(SystemSecrets)),
            #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
            system: None,
            session: SessionSecrets::default(),
            source: AtomicU8::new(UNKNOWN),
        }
    }
    /// Never touches the operating system store. Tests and explicit session-only use.
    pub fn session_only() -> Self {
        Self {
            system: None,
            session: SessionSecrets::default(),
            source: AtomicU8::new(SESSION),
        }
    }
    #[cfg(test)]
    fn with_system(system: SystemBackend) -> Self {
        Self {
            system: Some(system),
            session: SessionSecrets::default(),
            source: AtomicU8::new(UNKNOWN),
        }
    }
    pub fn source(&self) -> SecretSource {
        if self.source.load(Ordering::Acquire) == SYSTEM {
            SecretSource::System
        } else {
            SecretSource::Session
        }
    }
}
impl SecretStore for RuntimeSecrets {
    async fn read(&self, reference: &str) -> Result<Option<Secret>> {
        if let Some(system) = &self.system {
            match system.read(reference).await {
                Ok(value) => {
                    self.source.store(SYSTEM, Ordering::Release);
                    return Ok(value);
                }
                Err(_) => {
                    self.source.store(SESSION, Ordering::Release);
                }
            }
        }
        self.session.read(reference).await
    }
    async fn write(&self, reference: &str, patch: SecretPatch) -> Result<()> {
        if let Some(system) = &self.system {
            match system.write(reference, patch.clone()).await {
                Ok(()) => {
                    self.source.store(SYSTEM, Ordering::Release);
                    return Ok(());
                }
                Err(_) => {
                    self.source.store(SESSION, Ordering::Release);
                }
            }
        }
        self.session.write(reference, patch).await
    }
}

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
    async fn runtime_prefers_a_working_system_store_and_reports_its_source() {
        let system = Arc::new(SessionSecrets::default());
        let store = RuntimeSecrets::with_system(SystemBackend::Memory(system.clone()));
        store
            .write(
                "p/llm",
                SecretPatch::Replace(Secret::new("original-system".into()).unwrap()),
            )
            .await
            .unwrap();
        assert_eq!(store.source(), SecretSource::System);
        assert!(store.read("p/llm").await.unwrap().is_some());
        store.write("p/llm", SecretPatch::Clear).await.unwrap();
        assert!(store.read("p/llm").await.unwrap().is_none());
        assert_eq!(store.source(), SecretSource::System);
        assert!(system.read("p/llm").await.unwrap().is_none());
    }
    #[tokio::test]
    async fn unavailable_system_store_falls_back_to_session_memory() {
        let store = RuntimeSecrets::with_system(SystemBackend::Unavailable);
        store
            .write(
                "p/llm",
                SecretPatch::Replace(Secret::new("original-session".into()).unwrap()),
            )
            .await
            .unwrap();
        assert_eq!(store.source(), SecretSource::Session);
        assert_eq!(
            store.read("p/llm").await.unwrap().unwrap().expose(),
            "original-session"
        );
        assert_eq!(store.source(), SecretSource::Session);
        assert_eq!(
            RuntimeSecrets::session_only().source(),
            SecretSource::Session
        );
    }
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
