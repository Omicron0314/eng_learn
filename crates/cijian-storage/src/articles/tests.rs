use super::*;
use cijian_domain::{
    analysis::analyze,
    provider::{ProviderConfig, ProviderKind},
    settings::{Genre, Settings, Tier},
};
pub(crate) fn request() -> StartGeneration {
    StartGeneration {
        profile: "p".into(),
        operation: "start".into(),
        event: "requested".into(),
        job: "job".into(),
        now: 0,
        snapshot: GenerationSnapshot {
            tier: Tier::Easy,
            genre: Genre::Story,
            exposed: vec!["cat".into()],
            control: vec!["dog".into()],
            known: vec![],
            settings: Settings::default(),
            provider: ProviderConfig {
                kind: ProviderKind::Llm,
                endpoint: "https://example.invalid/v1".into(),
                model: "configured".into(),
                voice: None,
                revision: 1,
            },
        },
    }
}
pub(crate) fn article() -> SaveArticle {
    SaveArticle {
        profile: "p".into(),
        operation: "save".into(),
        event: "saved".into(),
        job: "job".into(),
        id: "article".into(),
        title: "A cat".into(),
        body: "A cat sees a cat.".into(),
        genre: Genre::Story,
        tier: Tier::Easy,
        analysis: analyze(
            "A cat sees a cat.",
            &BTreeSet::new(),
            &BTreeSet::from(["cat".into()]),
            &BTreeSet::from(["dog".into()]),
            &BTreeSet::new(),
        ),
        diagnostics: vec!["文章长度不在设置范围内".into()],
        targets: vec![
            ArticleTarget {
                lexeme_id: "cat".into(),
                role: TargetRole::Exposed,
                occurrences: 2,
                selection_reason: "reviewed".into(),
            },
            ArticleTarget {
                lexeme_id: "dog".into(),
                role: TargetRole::Control,
                occurrences: 0,
                selection_reason: "control".into(),
            },
        ],
        model: "configured".into(),
        provider_fingerprint: "fingerprint".into(),
        now: 1,
    }
}
pub(crate) fn store() -> Store {
    let store = Store::memory().unwrap();
    store.create_profile("p", "test", 0).unwrap();
    store
        .connection
        .lock()
        .unwrap()
        .execute_batch(
            "INSERT INTO lexemes(id,lemma,pos,kind) VALUES ('cat','cat','','word'),('dog','dog','','word');",
        )
        .unwrap();
    store
}
#[test]
fn duplicate_claim_atomic_save_and_replay() {
    let store = store();
    assert_eq!(
        store.start_generation(request()).unwrap().status,
        JobStatus::Queued
    );
    let mut duplicate = request();
    duplicate.job = "other".into();
    duplicate.event = "other-event".into();
    assert_eq!(store.start_generation(duplicate).unwrap().id, "job");
    assert!(store.claim_generation("p", "job", 0).unwrap());
    assert!(!store.claim_generation("p", "job", 0).unwrap());
    let mut bad = article();
    bad.event = "bootstrap-p".into();
    assert!(store.save_article(bad).is_err());
    assert_eq!(
        store.generation_job("p", "job").unwrap().status,
        JobStatus::Running
    );
    assert!(store.articles("p", None, None, 0).unwrap().is_empty());
    let saved = store.save_article(article()).unwrap();
    assert_eq!(saved.diagnostics.len(), 1);
    assert_eq!(store.save_article(article()).unwrap().id, "article");
    assert_eq!(
        store
            .generation_job("p", "job")
            .unwrap()
            .result_ref
            .as_deref(),
        Some("article")
    );
    let db = store.connection.lock().unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM article_targets", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM events WHERE type='article_generate'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}
#[test]
fn cancellation_rejects_late_completion() {
    let store = store();
    store.start_generation(request()).unwrap();
    store.claim_generation("p", "job", 0).unwrap();
    store
        .stop_generation("p", "job", JobStatus::Cancelled, 1)
        .unwrap();
    assert_eq!(
        store.save_article(article()).unwrap_err().code,
        ErrorCode::Cancelled
    );
    assert!(store.articles("p", None, None, 0).unwrap().is_empty());
    assert!(!store.claim_generation("p", "job", 2).unwrap());
}
#[test]
fn interrupted_jobs_keep_snapshot_without_retry() {
    let store = store();
    store.start_generation(request()).unwrap();
    store.claim_generation("p", "job", 0).unwrap();
    assert_eq!(store.interrupt_generation_jobs("p", 1).unwrap(), 1);
    assert_eq!(store.interrupt_generation_jobs("p", 2).unwrap(), 0);
    assert_eq!(
        store.generation_job("p", "job").unwrap().status,
        JobStatus::Interrupted
    );
    assert_eq!(
        store
            .generation_snapshot("p", "job")
            .unwrap()
            .provider
            .model,
        "configured"
    );
    assert!(!store.claim_generation("p", "job", 3).unwrap());
}
