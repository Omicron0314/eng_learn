use crate::backend::{DesktopBackend, id, now};
use cijian_app::{
    commands::Response,
    generation::{GenerationInput, generate},
};
use cijian_domain::{
    RandomSource,
    knowledge::select_targets,
    settings::{Genre, Tier},
};
use cijian_ports::{
    AppError, ErrorCode, Result,
    articles::*,
    provider::*,
    reading::{LookupContext, ReaderCommand},
};
use cijian_providers::{NativeHttp, OpenAiCompatible};
use cijian_storage::Store;
use sha2::{Digest, Sha256};
use std::sync::Arc;
struct Random(u64);
impl RandomSource for Random {
    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
}
struct Recorder(Arc<Store>);
impl CallRecorder for Recorder {
    async fn start(&self, call: CallStart) -> Result<()> {
        self.0.start(call).await
    }
    async fn finish(&self, call: CallFinish) -> Result<()> {
        self.0.finish(call).await
    }
}
struct Cancellable {
    model: OpenAiCompatible<NativeHttp, Recorder>,
    store: Arc<Store>,
    profile: String,
    job: String,
}
impl LanguageModel for Cancellable {
    async fn complete(
        &self,
        snapshot: &ProviderSnapshot,
        input: CompletionInput,
        call: CallStart,
    ) -> Result<String> {
        if self.store.generation_job(&self.profile, &self.job)?.status != JobStatus::Running {
            return Err(AppError::new(ErrorCode::Cancelled, "生成已取消"));
        }
        self.model.complete(Some(snapshot), input, call).await
    }
}
impl DesktopBackend {
    pub(super) fn reader_command(
        &self,
        article: String,
        body_hash: String,
        operation: String,
    ) -> ReaderCommand {
        ReaderCommand {
            profile: self.profile.clone(),
            operation,
            event: id(),
            article,
            body_hash,
            now: now(),
        }
    }
    pub(super) fn open_article(&self, article: &str) -> Result<Response> {
        let article = self.store.article(&self.profile, article)?;
        let session = self.store.open_reading(
            self.reader_command(article.id.clone(), article.body_hash.clone(), id()),
            &id(),
        )?;
        Ok(Response::Article {
            article: Box::new(article),
            session,
        })
    }
    pub(super) fn lookup(
        &self,
        article: &str,
        body_hash: &str,
        range: cijian_domain::reading::TextRange,
    ) -> Result<Response> {
        let context: LookupContext = self.store.lookup_context(
            &self.reader_command(article.into(), body_hash.into(), id()),
            range,
        )?;
        let (revision, mut meaning) = self
            .store
            .lookup_mark(&self.profile, &context.selection.lemma)?;
        let snapshot = {
            let _guard = self
                .config_lock
                .lock()
                .map_err(|_| AppError::new(ErrorCode::StorageUnavailable, "配置暂不可用"))?;
            self.store
                .provider_config(&self.profile, cijian_domain::provider::ProviderKind::Llm)?
                .map(|config| {
                    futures::executor::block_on(self.secrets.read(&format!("{}/llm", self.profile)))
                        .map(|secret| ProviderSnapshot { config, secret })
                })
                .transpose()?
        };
        if let Some(snapshot) = snapshot {
            let fingerprint = format!(
                "{:x}",
                Sha256::digest(format!(
                    "{}\n{}",
                    snapshot.config.endpoint, snapshot.config.model
                ))
            );
            let context_hash = format!(
                "{:x}",
                Sha256::digest(context.selection.sentence.as_bytes())
            );
            let gloss = match self.store.cached_gloss(
                &self.profile,
                &context.selection.lemma,
                &context_hash,
                &fingerprint,
            )? {
                Some(gloss) => gloss,
                None => {
                    let model = OpenAiCompatible {
                        transport: NativeHttp::new()?,
                        recorder: Recorder(self.store.clone()),
                    };
                    let gloss = self.runtime.block_on(cijian_app::gloss::explain(
                        &model,
                        &snapshot,
                        &context.selection,
                        CallStart {
                            profile_id: self.profile.clone(),
                            call_id: id(),
                            purpose: "gloss".into(),
                            model: snapshot.config.model.clone(),
                            prompt_version: "gloss.v1".into(),
                            provider_fingerprint: fingerprint.clone(),
                            now: now(),
                        },
                    ));
                    let gloss = match gloss {
                        Ok(gloss) => gloss,
                        Err(_) => {
                            return Ok(Response::Lookup {
                                context,
                                revision,
                                meaning: if meaning.is_empty() {
                                    "语境释义暂不可用，仍可标记".into()
                                } else {
                                    format!("{meaning}\n语境释义暂不可用")
                                },
                            });
                        }
                    };
                    self.store.cache_gloss(
                        &self.reader_command(article.into(), body_hash.into(), id()),
                        range,
                        &fingerprint,
                        &gloss,
                        &id(),
                    )?;
                    gloss
                }
            };
            meaning = format!(
                "{} {}\n{}\n{}",
                gloss.pos, gloss.ipa, gloss.meaning_zh, gloss.meaning_en
            );
        }
        Ok(Response::Lookup {
            context,
            revision,
            meaning,
        })
    }
    pub(super) fn generate_article(
        &self,
        tier: Tier,
        genre: Genre,
        operation: String,
    ) -> Result<Response> {
        let _guard = self
            .config_lock
            .lock()
            .map_err(|_| AppError::new(ErrorCode::StorageUnavailable, "配置暂不可用"))?;
        if let Some(job) = self
            .store
            .generation_job_for_operation(&self.profile, &operation)?
        {
            return Ok(Response::Job(job));
        }
        let config = self
            .store
            .provider_config(&self.profile, cijian_domain::provider::ProviderKind::Llm)?
            .ok_or(AppError::new(
                ErrorCode::ConfigMissing,
                "请先在设置中配置 AI 服务",
            ))?;
        let secret =
            futures::executor::block_on(self.secrets.read(&format!("{}/llm", self.profile)))?;
        let (settings, _) = self.store.settings(&self.profile)?;
        let at = now();
        let material = self.store.generation_material(&self.profile, at)?;
        let bytes = uuid::Uuid::now_v7().into_bytes();
        let mut random = Random(u64::from_le_bytes(
            bytes[8..16].try_into().expect("UUID tail"),
        ));
        let targets = select_targets(
            &material.candidates,
            settings.target_count,
            settings.control_ratio,
            &mut random,
        )
        .map_err(|e| AppError::new(ErrorCode::NoTargets, e.to_string()))?;
        let snapshot = GenerationSnapshot {
            tier,
            genre,
            exposed: targets.exposed.clone(),
            control: targets.control.clone(),
            known: material.known.iter().cloned().collect(),
            settings: settings.clone(),
            provider: config.clone(),
        };
        let job = self.store.start_generation(StartGeneration {
            profile: self.profile.clone(),
            operation: operation.clone(),
            event: id(),
            job: id(),
            now: at,
            snapshot,
        })?;
        if job.status != JobStatus::Queued
            || !self.store.claim_generation(&self.profile, &job.id, at)?
        {
            return Ok(Response::Job(job));
        }
        let input = GenerationInput {
            profile: self.profile.clone(),
            operation: operation.clone(),
            now: at,
            settings: settings
                .try_into()
                .map_err(|e: cijian_domain::ValidationError| {
                    AppError::new(ErrorCode::InvalidInput, e.to_string())
                })?,
            tier,
            genre,
            last_auto_genre: material.last_auto_genre,
            targets: targets.clone(),
            known: material.known,
            provider: ProviderSnapshot {
                config: config.clone(),
                secret,
            },
        };
        let store = self.store.clone();
        let profile = self.profile.clone();
        let job_id = job.id.clone();
        self.runtime.spawn(async move {
            let result = async {
                let model = Cancellable {
                    model: OpenAiCompatible {
                        transport: NativeHttp::new()?,
                        recorder: Recorder(store.clone()),
                    },
                    store: store.clone(),
                    profile: profile.clone(),
                    job: job_id.clone(),
                };
                let generated = generate(&model, &input).await?;
                let mut saved_targets = Vec::new();
                for (words, role) in [
                    (&targets.exposed, TargetRole::Exposed),
                    (&targets.control, TargetRole::Control),
                ] {
                    for word in words {
                        let lexeme_id = material
                            .lexeme_ids
                            .get(word)
                            .ok_or(AppError::new(ErrorCode::StaleState, "目标词已失效"))?
                            .clone();
                        saved_targets.push(ArticleTarget {
                            lexeme_id,
                            role,
                            occurrences: generated
                                .analysis
                                .occurrences
                                .get(word)
                                .copied()
                                .unwrap_or(0)
                                .try_into()
                                .unwrap_or(u32::MAX),
                            selection_reason: if role == TargetRole::Control {
                                "control"
                            } else {
                                "study_candidate"
                            }
                            .into(),
                        });
                    }
                }
                let fingerprint = format!(
                    "{:x}",
                    Sha256::digest(format!("{}\n{}", config.endpoint, config.model))
                );
                store.save_article(SaveArticle {
                    profile: profile.clone(),
                    operation: format!("save-{operation}"),
                    event: id(),
                    job: job_id.clone(),
                    id: id(),
                    title: generated.draft.title,
                    body: generated.draft.body,
                    genre: generated.genre,
                    tier,
                    analysis: generated.analysis,
                    diagnostics: generated.diagnostics,
                    targets: saved_targets,
                    model: config.model,
                    provider_fingerprint: fingerprint,
                    now: now(),
                })?;
                Ok::<_, AppError>(())
            }
            .await;
            if let Err(error) = result {
                let outcome = if error.code == ErrorCode::Cancelled {
                    JobStatus::Cancelled
                } else {
                    JobStatus::Failed
                };
                let _ = store.stop_generation(&profile, &job_id, outcome, now());
            }
        });
        Ok(Response::Job(
            self.store.generation_job(&self.profile, &job.id)?,
        ))
    }
}
