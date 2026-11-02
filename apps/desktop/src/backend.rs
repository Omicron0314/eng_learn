use cijian_app::commands::{Command, CommandBus, Response, SelectedFile};
use cijian_domain::import::book_name;
use cijian_ports::{AppError, ErrorCode, Result, library::*};
use cijian_storage::Store;
use futures::future::BoxFuture;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
fn id() -> String {
    uuid::Uuid::now_v7().to_string()
}
fn failure() -> AppError {
    AppError::new(ErrorCode::StorageUnavailable, "无法读取本地文件或数据库")
}
pub struct DesktopBackend {
    store: Store,
    profile: String,
    files: Mutex<HashMap<String, PathBuf>>,
}
impl DesktopBackend {
    pub fn open(directory: &Path) -> Result<Self> {
        std::fs::create_dir_all(directory).map_err(|_| failure())?;
        let store = Store::open(directory.join("cijian.sqlite"))?;
        let profile = match store.first_profile()? {
            Some(p) => p,
            None => {
                let p = id();
                store.create_profile(&p, "本机学习", now())?;
                p
            }
        };
        Ok(Self {
            store,
            profile,
            files: Mutex::new(HashMap::new()),
        })
    }
    fn execute(&self, command: Command) -> Result<Response> {
        match command {
            Command::LoadLibrary => Ok(Response::Library(self.store.cards(
                &self.profile,
                CardFilter {
                    page_size: 100,
                    ..Default::default()
                },
            )?)),
            Command::ChooseImport { word_list } => {
                let selected = rfd::FileDialog::new()
                    .set_title("选择导入文件")
                    .add_filter(
                        if word_list {
                            "词表 CSV / TXT"
                        } else {
                            "词卡 CSV / APKG"
                        },
                        if word_list {
                            &["csv", "txt"]
                        } else {
                            &["csv", "apkg"]
                        },
                    )
                    .pick_file();
                if let Some(path) = selected {
                    let name = book_name(
                        path.file_name()
                            .and_then(|s| s.to_str())
                            .ok_or_else(failure)?,
                    )
                    .map_err(|e| AppError::new(ErrorCode::InvalidInput, e.to_string()))?;
                    let handle = id();
                    self.files
                        .lock()
                        .map_err(|_| failure())?
                        .insert(handle.clone(), path);
                    Ok(Response::Selected(Some(SelectedFile { handle, name })))
                } else {
                    Ok(Response::Selected(None))
                }
            }
            Command::ImportSelected {
                handle,
                name,
                kind,
                word_list,
            } => {
                if !word_list && kind != cijian_domain::import::BookKind::Study {
                    return Err(AppError::new(
                        ErrorCode::InvalidInput,
                        "词卡不能作为基线导入",
                    ));
                }
                let path = self
                    .files
                    .lock()
                    .map_err(|_| failure())?
                    .get(&handle)
                    .cloned()
                    .ok_or(AppError::new(ErrorCode::InvalidInput, "请先选择文件"))?;
                let parsed = crate::import_file::read(&path, word_list)?;
                let mut source = std::fs::File::open(&path).map_err(|_| failure())?;
                let mut hasher = Sha256::new();
                let mut buffer = [0_u8; 65536];
                loop {
                    let read = source.read(&mut buffer).map_err(|_| failure())?;
                    if read == 0 {
                        break;
                    }
                    hasher.update(&buffer[..read]);
                }
                let source_hash = format!("{:x}", hasher.finalize());
                let row_ids = (0..parsed.rows.len())
                    .map(|_| ImportIds {
                        lexeme: id(),
                        card: id(),
                        entry: id(),
                    })
                    .collect();
                let report = self.store.import(ImportWords {
                    profile_id: self.profile.clone(),
                    operation_id: format!("import-{handle}"),
                    event_id: id(),
                    book_id: id(),
                    import_id: id(),
                    name,
                    kind,
                    source_name: path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("词卡.csv")
                        .into(),
                    source_hash,
                    format: path
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("csv")
                        .to_ascii_lowercase(),
                    now: now(),
                    parsed,
                    row_ids,
                })?;
                Ok(Response::Imported(report))
            }
            Command::LoadReview => {
                let now = now();
                let (cards, waiting) = self.store.review_queue(&self.profile, now)?;
                let (settings, _) = self.store.settings(&self.profile)?;
                let intervals = cards
                    .first()
                    .map(|card| {
                        cijian_domain::srs::preview(&card.srs, now, settings.request_retention)
                            .map(|states| states.map(|s| s.due - now))
                    })
                    .transpose()
                    .map_err(|e| AppError::new(ErrorCode::InvalidInput, e.to_string()))?;
                Ok(Response::Review {
                    cards,
                    waiting,
                    intervals,
                })
            }
            Command::Rate {
                card,
                revision,
                rating,
                operation,
                duration_ms,
            } => Ok(Response::Rated(self.store.rate(RateCard {
                profile_id: self.profile.clone(),
                operation_id: operation,
                event_id: id(),
                review_id: id(),
                card_id: card,
                expected_revision: revision,
                rating,
                duration_ms,
                revealed: true,
                now: now(),
            })?)),
        }
    }
}
impl CommandBus for DesktopBackend {
    fn dispatch(&self, command: Command) -> BoxFuture<'_, Result<Response>> {
        Box::pin(async move { self.execute(command) })
    }
}
