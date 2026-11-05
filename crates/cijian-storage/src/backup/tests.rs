use super::*;
use crate::articles::tests::{article, request, store};
fn populated() -> Store {
    let s = store();
    s.start_generation(request()).unwrap();
    s.claim_generation("p", "job", 0).unwrap();
    s.save_article(article()).unwrap();
    s
}
#[test]
fn roundtrip_retains_articles_events_and_excludes_credentials_jobs() {
    let source = populated();
    source.connection.lock().unwrap().execute("INSERT INTO provider_configs VALUES ('provider','p','llm','https://example.invalid/v1','model',NULL,'private-reference',1)",[]).unwrap();
    let package = source.export_profile("p", false).unwrap();
    let text = serde_json::to_string(&package).unwrap();
    assert!(!text.contains("private-reference"));
    assert!(
        !package.payload["tables"]
            .as_object()
            .unwrap()
            .contains_key("generation_snapshots")
    );
    let target = Store::memory().unwrap();
    let result = target
        .restore_profile(package, "restored", "restored-event", "restore", 20)
        .unwrap();
    assert_eq!(result.counts["articles"], 1);
    assert!(!result.credentials_included);
    assert_eq!(
        target.article("restored", "article").unwrap().body,
        source.article("p", "article").unwrap().body
    );
    assert!(
        target
            .provider_config("restored", cijian_domain::provider::ProviderKind::Llm)
            .unwrap()
            .is_none()
    );
    let db = target.connection.lock().unwrap();
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM events WHERE type='backup_restore'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}
#[test]
fn old_packages_without_ease_restore_with_the_initial_factor() {
    let source = store();
    source
        .connection
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO cards(id,profile_id,lexeme_id,front,text_source,origin,state,due,stability,difficulty,reps,lapses,elapsed_days,scheduled_days,learning_steps,scheduler_version,created_at,updated_at,revision) VALUES ('card','p','cat','cat','missing','import','new',0,0,0,0,0,0,0,0,'test',0,0,1)",
            [],
        )
        .unwrap();
    let mut package = source.export_profile("p", false).unwrap();
    package.payload["tables"]["cards"][0]
        .as_object_mut()
        .unwrap()
        .remove("ease_factor");
    package.checksum = hash(&cijian_domain::json::canonical_json(&package.payload));
    let target = Store::memory().unwrap();
    target
        .restore_profile(package, "restored", "event", "restore", 1)
        .unwrap();
    let cards = target
        .cards(
            "restored",
            cijian_ports::library::CardFilter {
                page_size: 10,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(cards.total, 1);
    assert_eq!(
        cards.cards[0].srs.ease_factor,
        cijian_domain::srs::INITIAL_EASE_FACTOR
    );
}
#[test]
fn corrupted_unknown_and_nonempty_restores_leave_destination_unchanged() {
    let source = populated();
    let package = source.export_profile("p", false).unwrap();
    let target = Store::memory().unwrap();
    let mut corrupt = package.clone();
    corrupt.payload["tables"]["articles"][0]["body"] = "tampered".into();
    assert!(
        target
            .restore_profile(corrupt.clone(), "new", "event", "op", 1)
            .is_err()
    );
    assert!(target.first_profile().unwrap().is_none());
    corrupt.checksum = hash(&cijian_domain::json::canonical_json(&corrupt.payload));
    assert!(
        target
            .restore_profile(corrupt, "new", "event", "op", 1)
            .is_err()
    );
    assert!(target.first_profile().unwrap().is_none());
    let mut future = package.clone();
    future.version = 999;
    assert!(
        target
            .restore_profile(future, "new", "event", "op", 1)
            .is_err()
    );
    target.create_profile("existing", "keep", 0).unwrap();
    assert!(
        target
            .restore_profile(package, "new", "event", "op", 1)
            .is_err()
    );
    assert_eq!(target.first_profile().unwrap().as_deref(), Some("existing"));
}
#[test]
fn restores_v1_packages_without_losing_new_archive_validation() {
    let source = Store::memory().unwrap();
    source.create_profile("p", "原创", 0).unwrap();
    let mut package = source.export_profile("p", false).unwrap();
    package.version = 1;
    let tables = package.payload["tables"].as_object_mut().unwrap();
    tables.remove("legacy_records");
    tables.remove("legacy_imports");
    package.checksum = hash(&package.payload.to_string());
    Store::memory()
        .unwrap()
        .restore_profile(package, "new", "restore-event", "restore", 1)
        .unwrap();
    let payload = json!({"json":json!({"llm":{"apiKey":"ORIGINAL-NESTED-SECRET"}}).to_string()});
    source
        .connection
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO legacy_records VALUES ('p','source','provider_config','original',?1)",
            [payload.to_string()],
        )
        .unwrap();
    assert!(source.export_profile("p", false).is_err());
}

#[test]
fn sensitive_event_payload_refuses_export() {
    let source = populated();
    source
        .connection
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO events VALUES ('bad','p',NULL,'bad',0,'legacy',1,?1,0)",
            [r#"{"apiKey":"original-test-secret"}"#],
        )
        .unwrap();
    assert!(source.export_profile("p", false).is_err());
}

#[test]
fn audio_inclusive_backup_requires_matching_verified_blob_entries() {
    let source = populated();
    let blob = format!("{}.wav", "a".repeat(64));
    {
        let db = source.connection.lock().unwrap();
        db.execute(
            "INSERT INTO audio_assets VALUES ('audio','p','cache','hash','fp','model','voice','wav',?1,1.0,2,1)",
            [&blob],
        )
        .unwrap();
        db.execute(
            "INSERT INTO article_audio_links VALUES ('p','article','audio')",
            [],
        )
        .unwrap();
    }
    let package = source.export_profile("p", true).unwrap();
    assert_eq!(package.version, 3);
    assert_eq!(package.payload["audioIncluded"], true);
    assert!(
        package.payload["tables"]
            .as_object()
            .unwrap()
            .contains_key("audio_assets")
    );
    let target = Store::memory().unwrap();
    assert!(
        target
            .restore_profile(package.clone(), "new", "event", "op", 1)
            .is_err()
    );
    assert!(target.first_profile().unwrap().is_none());
    let mut good = package;
    good.payload["audioBlobs"] = json!([{
        "blobKey": blob,
        "sha256": "a".repeat(64),
        "bytes": "AA=="
    }]);
    good.checksum = hash(&cijian_domain::json::canonical_json(&good.payload));
    let result = target
        .restore_profile(good, "new", "event", "op", 1)
        .unwrap();
    assert!(result.audio_included);
    assert_eq!(result.counts["audio_assets"], 1);
    assert_eq!(result.counts["article_audio_links"], 1);
    let db = target.connection.lock().unwrap();
    let audio: Option<String> = db
        .query_row(
            "SELECT audio_id FROM audio_sessions WHERE profile_id='new'",
            [],
            |row| row.get(0),
        )
        .ok();
    assert!(audio.is_none());
}
