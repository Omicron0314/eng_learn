#[cfg(test)]
mod library_tests;
#[cfg(test)]
mod tests;
use cijian_app::commands::{Command, CommandBus, Response, SelectedFile};
use cijian_domain::import::book_name;
use cijian_domain::provider::ProviderKind;
use cijian_platform::SessionSecrets;
use cijian_ports::{AppError, ErrorCode, Result, library::*};
use cijian_ports::{
    SaveSettings,
    provider::{SaveProviderConfig, SecretStore},
};
use cijian_storage::Store;
use futures::future::BoxFuture;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
pub(super) fn id() -> String {
    uuid::Uuid::now_v7().to_string()
}
fn failure() -> AppError {
    AppError::new(ErrorCode::StorageUnavailable, "无法读取本地文件或数据库")
}
pub struct DesktopBackend {
    pub(super) audio_epoch: std::sync::atomic::AtomicU64,
    pub(super) audio: cijian_platform::audio::NativeAudio,
    pub(super) system_audio: Mutex<Option<tempfile::TempPath>>,
    pub(super) playback_gate: Mutex<()>,
    pub(super) audio_lock: Mutex<()>,
    pub(super) directory: PathBuf,
    pub(super) store: Arc<Store>,
    pub(super) device: String,
    pub(super) profile: String,
    pub(super) legacy_files: Mutex<HashMap<String, PathBuf>>,
    pub(super) legacy_snapshots:
        Mutex<HashMap<String, Arc<cijian_storage::legacy::LegacySnapshot>>>,
    files: Mutex<HashMap<String, PathBuf>>,
    pub(super) secrets: SessionSecrets,
    pub(super) config_lock: Mutex<()>,
    pub(super) runtime: tokio::runtime::Runtime,
    _directory_lock: std::fs::File,
}
impl DesktopBackend {
    pub fn open(directory: &Path) -> Result<Self> {
        std::fs::create_dir_all(directory).map_err(|_| failure())?;
        let directory_lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(directory.join("instance.lock"))
            .map_err(|_| failure())?;
        directory_lock.try_lock().map_err(|_| {
            AppError::new(
                ErrorCode::StorageUnavailable,
                "此学习目录正由另一个词间实例使用",
            )
        })?;
        let store = Store::open(directory.join("cijian.sqlite"))?;
        let profile = match store.first_profile()? {
            Some(p) => p,
            None => {
                let p = id();
                store.create_profile(&p, "本机学习", now())?;
                p
            }
        };
        let device_path = directory.join("device-id");
        let device = match std::fs::read_to_string(&device_path) {
            Ok(device) if uuid::Uuid::parse_str(device.trim()).is_ok() => device.trim().to_owned(),
            Ok(_) => {
                return Err(AppError::new(
                    ErrorCode::StorageUnavailable,
                    "本机身份文件无效",
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                use std::io::Write;
                let device = id();
                let mut file = tempfile::NamedTempFile::new_in(directory).map_err(|_| failure())?;
                file.write_all(device.as_bytes()).map_err(|_| failure())?;
                file.as_file().sync_all().map_err(|_| failure())?;
                file.persist_noclobber(device_path).map_err(|_| failure())?;
                device
            }
            Err(_) => return Err(failure()),
        };
        store.ensure_device(&profile, &device, std::env::consts::OS)?;
        store.interrupt_generation_jobs(&profile, now())?;
        let store = Arc::new(store);
        let completion_store = store.clone();
        let completion_profile = profile.clone();
        let audio = cijian_platform::audio::NativeAudio::with_completion(move |state, now| {
            let (Some(session), Some(article)) = (state.session, state.article) else {
                return Ok(());
            };
            completion_store.complete_audio_session(cijian_ports::audio::AudioSessionCommand {
                profile: completion_profile.clone(),
                operation: format!("audio-ended-{session}"),
                event: id(),
                session,
                article,
                audio: None,
                source: "remote".into(),
                rate: state.rate,
                now,
            })
        });
        Ok(Self {
            audio_epoch: std::sync::atomic::AtomicU64::new(0),
            audio,
            audio_lock: Mutex::new(()),
            system_audio: Mutex::new(None),
            playback_gate: Mutex::new(()),
            directory: directory.into(),
            _directory_lock: directory_lock,
            store,
            device,
            profile,
            legacy_files: Mutex::new(HashMap::new()),
            legacy_snapshots: Mutex::new(HashMap::new()),
            files: Mutex::new(HashMap::new()),
            secrets: SessionSecrets::default(),
            config_lock: Mutex::new(()),
            runtime: tokio::runtime::Runtime::new().map_err(|_| failure())?,
        })
    }
    fn save_preferences(
        &self,
        update: impl FnOnce(&mut cijian_domain::settings::DevicePreferences) -> Result<()>,
    ) -> Result<Response> {
        let _guard = self.config_lock.lock().map_err(|_| failure())?;
        let (mut value, revision) = self.store.device_preferences(&self.profile, &self.device)?;
        update(&mut value)?;
        self.store
            .save_device_preferences(cijian_ports::SaveDevicePreferences {
                profile: self.profile.clone(),
                device: self.device.clone(),
                operation: id(),
                event: id(),
                expected_revision: revision,
                value: value.clone(),
                now: now(),
            })?;
        Ok(Response::Preferences(value))
    }
    fn settings_response(&self) -> Result<Response> {
        let (mut value, revision) = self.store.settings(&self.profile)?;
        let (preferences, _) = self.store.device_preferences(&self.profile, &self.device)?;
        value.theme = preferences.theme;
        value.playback_rate = preferences.playback_rate;
        let provider = self
            .store
            .provider_config(&self.profile, ProviderKind::Llm)?;
        let has_session_key =
            futures::executor::block_on(self.secrets.read(&format!("{}/llm", self.profile)))?
                .is_some();
        Ok(Response::Settings {
            value,
            revision,
            provider,
            has_session_key,
            tts: self
                .store
                .provider_config(&self.profile, ProviderKind::Tts)?,
            system_voice: preferences.system_voice,
            voices: cijian_platform::speech::voices().unwrap_or_default(),
            has_tts_key: futures::executor::block_on(
                self.secrets.read(&format!("{}/tts", self.profile)),
            )?
            .is_some(),
        })
    }
    fn execute(&self, command: Command) -> Result<Response> {
        match command {
            Command::SetTheme { value } => self.save_preferences(|preferences| {
                preferences.theme = value;
                Ok(())
            }),
            Command::SetPlaybackRate { rate } => {
                let result = self.save_preferences(|preferences| {
                    preferences.playback_rate = rate;
                    Ok(())
                })?;
                self.audio
                    .command(cijian_platform::audio::AudioCommand::Rate(rate))?;
                Ok(result)
            }
            Command::SetSystemVoice { voice } => self.save_preferences(|preferences| {
                if !cijian_platform::speech::voices()?
                    .iter()
                    .any(|available| available.id == voice)
                {
                    return Err(AppError::new(
                        ErrorCode::UnsupportedCapability,
                        "本机语音不可用",
                    ));
                }
                preferences.system_voice = Some(voice);
                Ok(())
            }),
            Command::PlayStoredAudio { article, audio } => self.play_stored_audio(&article, &audio),
            Command::EnsureSystemAudio { article, voice } => {
                self.ensure_system_audio(&article, &voice)
            }
            Command::EnsureAudio { article, operation } => self.ensure_audio(&article, &operation),
            Command::AudioPause => self.audio_command(cijian_platform::audio::AudioCommand::Pause),
            Command::AudioResume => {
                self.audio_command(cijian_platform::audio::AudioCommand::Resume)
            }
            Command::AudioStop => self.audio_command(cijian_platform::audio::AudioCommand::Stop),
            Command::AudioSeek { position } => {
                self.audio_command(cijian_platform::audio::AudioCommand::Seek(position))
            }
            Command::AudioRate { rate } => {
                self.audio_command(cijian_platform::audio::AudioCommand::Rate(rate))
            }
            Command::AudioStatus => {
                self.audio_command(cijian_platform::audio::AudioCommand::Status)
            }
            Command::InspectLegacy => self.inspect_legacy(),
            Command::SelectLegacyProfile { selection, profile } => {
                self.select_legacy_profile(&selection, &profile)
            }
            Command::ImportLegacy { selection } => self.import_legacy(&selection),
            Command::ExportBackup => self.export_backup(),
            Command::RestoreBackup => self.restore_backup(),
            Command::LoadStats { page } => Ok(Response::Stats(Box::new(self.store.stats(
                &self.profile,
                now(),
                page,
            )?))),
            Command::LoadArticles { page } => Ok(Response::Articles(self.store.articles(
                &self.profile,
                None,
                None,
                page,
            )?)),
            Command::GenerateArticle {
                tier,
                genre,
                operation,
            } => self.generate_article(tier, genre, operation),
            Command::ObserveJob { job } => Ok(Response::Job(
                self.store.generation_job(&self.profile, &job)?,
            )),
            Command::CancelGeneration { job } => Ok(Response::Job(self.store.stop_generation(
                &self.profile,
                &job,
                cijian_ports::articles::JobStatus::Cancelled,
                now(),
            )?)),
            Command::OpenArticle { id } => self.open_article(&id),
            Command::Lookup {
                article,
                body_hash,
                range,
            } => self.lookup(&article, &body_hash, range),
            Command::SaveReadingPosition {
                article,
                body_hash,
                session,
                position,
                revision,
            } => {
                self.store.save_reading_position(
                    &self.reader_command(article, body_hash, id()),
                    &session,
                    position,
                    revision,
                )?;
                Ok(Response::ReadingPositionSaved {
                    session,
                    position,
                    revision,
                })
            }
            Command::CompleteReading {
                article,
                body_hash,
                session,
            } => {
                let session = self.store.complete_reading(
                    self.reader_command(article, body_hash, format!("complete-{session}")),
                    &session,
                )?;
                Ok(Response::Completed(session))
            }
            Command::Feedback {
                article,
                body_hash,
                value,
                mode,
                operation,
            } => {
                let feedback = self.store.submit_feedback(
                    self.reader_command(article, body_hash, operation),
                    cijian_ports::reading::Feedback { mode, value },
                )?;
                Ok(Response::FeedbackSaved(feedback))
            }
            Command::FlagSentences {
                article,
                body_hash,
                flag,
                operation,
            } => {
                self.store.flag_sentences(
                    self.reader_command(article.clone(), body_hash.clone(), operation),
                    flag,
                )?;
                Ok(Response::SentencesFlagged { article, body_hash })
            }
            Command::Mark {
                article,
                body_hash,
                range,
                action,
                revision,
                operation,
            } => Ok(Response::Marked(self.store.mark_word(
                cijian_ports::inbox::MarkWord {
                    context: self.reader_command(article, body_hash, operation),
                    range,
                    action,
                    expected_revision: revision,
                    lexeme_id: id(),
                    card_id: id(),
                    inbox_id: id(),
                    meaning: String::new(),
                },
            )?)),
            Command::LoadInbox { page } => Ok(Response::Inbox {
                items: self.store.inbox(&self.profile, page)?,
                total: self.store.pending_inbox_count(&self.profile)?,
            }),
            Command::BatchInbox {
                action,
                selections,
                operation,
            } => Ok(Response::InboxProcessed(
                self.store.batch_inbox(cijian_ports::inbox::BatchInbox {
                    profile: self.profile.clone(),
                    operation,
                    action,
                    selections: selections
                        .into_iter()
                        .map(|(item, revision)| cijian_ports::inbox::InboxSelection {
                            id: item,
                            revision,
                            card_id: id(),
                            event_id: id(),
                        })
                        .collect(),
                    now: now(),
                })?,
            )),
            Command::LoadSettings => self.settings_response(),
            Command::SaveSettings {
                value,
                revision,
                operation,
            } => {
                let value: cijian_domain::settings::ValidatedSettings =
                    value
                        .try_into()
                        .map_err(|e: cijian_domain::ValidationError| {
                            AppError::new(ErrorCode::InvalidInput, e.to_string())
                        })?;
                let saved = value.get().clone();
                let revision = self.store.update_settings(SaveSettings {
                    profile_id: self.profile.clone(),
                    operation_id: operation,
                    event_id: id(),
                    expected_revision: revision,
                    now: now(),
                    value,
                })?;
                Ok(Response::LearningSettingsSaved {
                    value: saved,
                    revision,
                })
            }
            Command::SaveProvider {
                config,
                revision,
                secret,
                operation,
            } => {
                let _guard = self.config_lock.lock().map_err(|_| failure())?;
                let kind = config.kind;
                self.store.save_provider_config(SaveProviderConfig {
                    profile: self.profile.clone(),
                    operation,
                    event: id(),
                    config,
                    expected_revision: revision,
                    now: now(),
                })?;
                let current = self
                    .store
                    .provider_config(&self.profile, kind)?
                    .ok_or_else(failure)?;
                if current.revision != revision + 1 {
                    return Err(AppError::new(
                        ErrorCode::StaleState,
                        "配置已更新，旧密钥操作未重放",
                    ));
                }
                let reference = format!(
                    "{}/{}",
                    self.profile,
                    if kind == ProviderKind::Llm {
                        "llm"
                    } else {
                        "tts"
                    }
                );
                futures::executor::block_on(self.secrets.write(&reference, secret))?;
                let has_key = futures::executor::block_on(self.secrets.read(&reference))?.is_some();
                Ok(Response::ProviderSaved {
                    config: current,
                    has_key,
                })
            }
            Command::EditLibraryCard {
                card,
                revision,
                meaning,
                example,
                suspended,
                operation,
            } => Ok(Response::LibraryCardEdited(self.store.edit_card(
                cijian_ports::library::EditCard {
                    profile_id: self.profile.clone(),
                    operation_id: operation,
                    event_id: id(),
                    card_id: card,
                    expected_revision: revision,
                    meaning,
                    example,
                    suspended,
                    now: now(),
                },
            )?)),
            Command::PreviewDeleteBook { book } => Ok(Response::DeleteBookPreview(
                self.store.preview_delete_book(&self.profile, &book)?,
            )),
            Command::DeleteBook {
                book,
                fingerprint,
                operation,
            } => Ok(Response::BookDeleted(self.store.delete_book(
                cijian_ports::library::DeleteBook {
                    profile_id: self.profile.clone(),
                    operation_id: operation,
                    event_id: id(),
                    book_id: book,
                    fingerprint,
                    now: now(),
                },
            )?)),
            Command::LoadLibrary { filter } => {
                let books = self.store.books(&self.profile)?;
                let selected = if let Some(book) = &filter.book_id {
                    Some(
                        books
                            .iter()
                            .find(|value| &value.id == book)
                            .ok_or(AppError::new(ErrorCode::NotFound, "词书不存在或已归档"))?,
                    )
                } else {
                    None
                };
                let entries = if selected
                    .is_some_and(|book| book.kind == cijian_domain::import::BookKind::Baseline)
                {
                    Some(self.store.book_entries(&self.profile, &filter)?)
                } else {
                    None
                };
                Ok(Response::Library {
                    latest_import: self.store.latest_import_report(&self.profile)?,
                    page: self.store.cards(&self.profile, filter)?,
                    books,
                    entries,
                })
            }
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
                target_book,
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
                let (name, book_id) = if let Some(target) = &target_book {
                    let books = self.store.books(&self.profile)?;
                    let book = books
                        .into_iter()
                        .find(|book| &book.id == target && book.kind == kind)
                        .ok_or(AppError::new(
                            ErrorCode::InvalidInput,
                            "目标词书不存在或用途不匹配",
                        ))?;
                    (book.name, book.id)
                } else {
                    (name, id())
                };
                let (parsed, source_hash) = crate::import_file::read(&path, word_list)?;
                let row_ids = (0..parsed.rows.len())
                    .map(|_| ImportIds {
                        lexeme: id(),
                        card: id(),
                        entry: id(),
                    })
                    .collect();
                let report = self.store.import(ImportWords {
                    profile_id: self.profile.clone(),
                    operation_id: format!(
                        "import-{handle}-{}",
                        target_book.as_deref().unwrap_or("new")
                    ),
                    event_id: id(),
                    book_id,
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
                    stats: Box::new(self.store.stats(&self.profile, now, 0)?),
                    books: self.store.books(&self.profile)?,
                    inbox_pending: self.store.pending_inbox_count(&self.profile)?,
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
