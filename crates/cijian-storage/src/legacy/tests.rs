use super::*;
fn fixture() -> (tempfile::TempDir, Connection, PathBuf, PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("legacy.sqlite");
    let template = inspect::legacy_schema().unwrap();
    template
        .execute("VACUUM INTO ?1", [path.to_str().unwrap()])
        .unwrap();
    let db = Connection::open(&path).unwrap();
    db.execute_batch("PRAGMA journal_mode=WAL;PRAGMA wal_autocheckpoint=0;PRAGMA foreign_keys=ON;")
        .unwrap();
    db.execute("INSERT INTO users VALUES ('p',0,1)", [])
        .unwrap();
    db.execute(
        "INSERT INTO settings VALUES ('p',?1,1)",
        [
            json!({"articleMinWords":275,"articleMaxWords":360,"generationMaxRetries":0})
                .to_string(),
        ],
    )
    .unwrap();
    db.execute("INSERT INTO provider_config VALUES ('p',?1,1)",[json!({"llm":{"provider":"openai-compatible","baseURL":"https://example.invalid/v1","model":"configured-fixture","apiKey":"ORIGINAL-SECRET-NEVER-EXPORT"}}).to_string()]).unwrap();
    db.execute("INSERT INTO lexemes VALUES ('lex','harbor','',0)", [])
        .unwrap();
    db.execute("INSERT INTO cards VALUES ('card','p','lex','harbor',?1,'Original',1,'import',2000,4,5,'review',3,1,1000,3,4,0,0,1)",[json!({"meaning":"港湾","example":"A harbor shelters a boat.","meaningSource":"user"}).to_string()]).unwrap();
    db.execute_batch("INSERT INTO word_lists VALUES ('book','p','Original','study','original.csv',0,1);INSERT INTO word_list_items VALUES ('member','book','p','lex','Harbor',0);INSERT INTO lexeme_marks VALUES ('p','lex','unknown',1,1);INSERT INTO review_logs VALUES ('review','p','card','good','learning',100,1000,42);").unwrap();
    let analysis = json!({"tokenCount":5,"coverage":0.8,"typeCoverage":0.75,"unknownRatio":0.2,"unknownLemmas":[{"lemma":"boat","count":1}],"targetOccurrences":{"harbor":1},"names":[]});
    db.execute("INSERT INTO articles VALUES ('article','p','generated','Original','A harbor shelters a boat.','story','easy','original.v1','configured-fixture','generated',NULL,?1,0,1)",[analysis.to_string()]).unwrap();
    db.execute_batch("INSERT INTO article_targets VALUES ('article','lex','exposed',1);INSERT INTO inbox VALUES ('i1','p','lex','A harbor shelters a boat.','article','pending',0,1);INSERT INTO inbox VALUES ('i2','p','lex','A harbor shelters a boat.','article','pending',0,2);").unwrap();
    db.execute(
        "INSERT INTO gloss_cache VALUES ('lex','original-context',?1,0)",
        [json!({"meaning":"港湾"}).to_string()],
    )
    .unwrap();
    db.execute("INSERT INTO llm_calls VALUES ('call','p','article','original-provider','configured-fixture',10,20,NULL,4,'error','ORIGINAL-SECRET-NEVER-EXPORT',0)",[]).unwrap();
    let audio_root = directory.path().join("blobs");
    std::fs::create_dir_all(audio_root.join("audio")).unwrap();
    let mut wave = b"RIFF".to_vec();
    wave.extend_from_slice(&38_u32.to_le_bytes());
    wave.extend_from_slice(b"WAVEfmt ");
    wave.extend_from_slice(&16_u32.to_le_bytes());
    wave.extend_from_slice(&1_u16.to_le_bytes());
    wave.extend_from_slice(&1_u16.to_le_bytes());
    wave.extend_from_slice(&8000_u32.to_le_bytes());
    wave.extend_from_slice(&16000_u32.to_le_bytes());
    wave.extend_from_slice(&2_u16.to_le_bytes());
    wave.extend_from_slice(&16_u16.to_le_bytes());
    wave.extend_from_slice(b"data");
    wave.extend_from_slice(&2_u32.to_le_bytes());
    wave.extend_from_slice(&0_i16.to_le_bytes());
    std::fs::write(audio_root.join("audio/original.wav"), wave).unwrap();
    db.execute("INSERT INTO audios VALUES ('audio','article','openai-compatible','configured-voice','audio/original.wav',NULL,?1,26,5)",["a".repeat(64)]).unwrap();
    for (id, kind, payload, at) in [
        ("opened", "article_open", json!({"articleId":"article"}), 1),
        (
            "completed",
            "article_complete",
            json!({"articleId":"article","openId":"opened"}),
            2,
        ),
        (
            "reserved",
            "audio_reserve",
            json!({"articleId":"article","chars":26}),
            3,
        ),
        (
            "generated",
            "audio_generate",
            json!({"articleId":"article","audioId":"audio","chars":26,"reservationId":"reserved"}),
            5,
        ),
    ] {
        db.execute(
            "INSERT INTO events VALUES (?1,'p',?2,?3,?4)",
            params![id, kind, payload.to_string(), at],
        )
        .unwrap();
    }
    (directory, db, path, audio_root)
}
fn inspect_fixture(path: &Path, audio: &Path) -> LegacySnapshot {
    let mut n = 0;
    Store::inspect_legacy(path, None, Some(audio), || {
        n += 1;
        format!("new-original-{n}")
    })
    .unwrap()
}
#[test]
fn wal_snapshot_preserves_ids_schedule_analysis_and_never_exports_credentials() {
    let (_directory, source, path, audio) = fixture();
    let before = file_hash(&path).unwrap();
    let wal = file_hash(&path.with_extension("sqlite-wal")).unwrap();
    let snapshot = inspect_fixture(&path, &audio);
    assert_eq!(before, file_hash(&path).unwrap());
    assert_eq!(wal, file_hash(&path.with_extension("sqlite-wal")).unwrap());
    assert!(
        !serde_json::to_string(&snapshot.rows)
            .unwrap()
            .contains("ORIGINAL-SECRET-NEVER-EXPORT")
    );
    let preview = Store::dry_run_legacy(&snapshot, "preview", 10).unwrap();
    assert_eq!(preview.source_counts["cards"], 1);
    assert_eq!(preview.target_counts["inbox"], 1);
    let target = Store::memory().unwrap();
    let report = target.import_legacy(&snapshot, "import", 10).unwrap();
    assert_eq!(report.target_counts["review_logs"], 1);
    assert_eq!(target.settings("p").unwrap().0.article_min_words, 275);
    assert_eq!(target.settings("p").unwrap().0.generation_max_retries, 0);
    let db = target.connection.lock().unwrap();
    let state: (i64, i64, Option<i64>) = db
        .query_row(
            "SELECT due,suspended,archived_at FROM cards WHERE id='card'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(state, (2000, 1, None));
    assert_eq!(
        db.query_row(
            "SELECT due_after FROM review_logs WHERE id='review'",
            [],
            |r| r.get::<_, Option<i64>>(0)
        )
        .unwrap(),
        None
    );
    assert_eq!(
        db.query_row("SELECT sum(chars) FROM audio_reservations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        26
    );
    assert_eq!(
        db.query_row(
            "SELECT completed_at FROM reading_sessions WHERE id='opened'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        2
    );
    assert_eq!(
        db.query_row("SELECT body_hash FROM articles", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        hash("A harbor shelters a boat.")
    );
    let converted: Value = serde_json::from_str(
        &db.query_row("SELECT analysis_json FROM articles", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
    )
    .unwrap();
    let original: Value = serde_json::from_str(
        &source
            .query_row("SELECT analysis_json FROM articles", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(converted["legacy"], original);
    drop(db);
    target.import_legacy(&snapshot, "repeat", 20).unwrap();
    assert_eq!(
        target
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM events", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        5
    );
    let output = tempfile::tempdir().unwrap();
    snapshot.copy_audio(output.path()).unwrap();
    assert_eq!(
        file_hash(&output.path().join(&snapshot.blobs[0].key))
            .unwrap()
            .0,
        snapshot.blobs[0].checksum
    );
    let package = target.export_profile("p", false).unwrap();
    assert!(
        !serde_json::to_string(&package)
            .unwrap()
            .contains("ORIGINAL-SECRET-NEVER-EXPORT")
    );
    let restored = Store::memory().unwrap();
    restored
        .restore_profile(
            package,
            "restored-profile",
            "restore-event",
            "restore-operation",
            30,
        )
        .unwrap();
    assert!(
        restored
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM legacy_records", [], |r| r
                .get::<_, i64>(0))
            .unwrap()
            > 0
    );
}
#[test]
fn invalid_state_missing_blob_and_failed_event_never_publish_partial_profiles() {
    let (_directory, source, path, audio) = fixture();
    let snapshot = inspect_fixture(&path, &audio);
    let target = Store::memory().unwrap();
    assert!(target.import_legacy(&snapshot, "opened", 10).is_err());
    assert_eq!(
        target
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM profiles", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    source
        .execute("UPDATE cards SET state='future-state'", [])
        .unwrap();
    let invalid = inspect_fixture(&path, &audio);
    assert!(Store::dry_run_legacy(&invalid, "preview", 10).is_err());
    std::fs::remove_file(audio.join("audio/original.wav")).unwrap();
    let missing = inspect_fixture(&path, &audio);
    assert!(!missing.report.blockers.is_empty());
    assert!(target.import_legacy(&missing, "missing", 10).is_err());
}
#[test]
fn v3_review_upgrade_preserves_after_fields_and_append_only_guards() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("v3.sqlite");
    {
        let db = Connection::open(&path).unwrap();
        db.execute_batch(
            "CREATE TABLE migrations(version INTEGER PRIMARY KEY,checksum TEXT NOT NULL)",
        )
        .unwrap();
        for (index, migration) in MIGRATIONS.iter().take(3).enumerate() {
            db.execute_batch(migration).unwrap();
            db.execute(
                "INSERT INTO migrations VALUES (?1,?2)",
                params![index as i64 + 1, hash(migration)],
            )
            .unwrap();
        }
        db.execute_batch("INSERT INTO profiles VALUES ('p','原创',0,0);INSERT INTO lexemes(id,lemma,pos,kind) VALUES ('l','harbor','','word');INSERT INTO cards(id,profile_id,lexeme_id,front,text_source,origin,state,due,stability,difficulty,reps,lapses,last_review,elapsed_days,scheduled_days,learning_steps,scheduler_version,created_at,updated_at,revision) VALUES ('c','p','l','harbor','missing','import','learning',1,1,5,1,0,0,0,0,1,'original-v3',0,0,2);INSERT INTO review_logs(id,profile_id,card_id,operation_id,rating,state_before,state_after,due_before,due_after,scheduler_version,reviewed_at,duration_ms) VALUES ('r','p','c','op','again','new','learning',0,1,'original-v3',0,2);").unwrap();
    }
    let store = Store::open(path).unwrap();
    let db = store.connection.lock().unwrap();
    assert_eq!(
        db.query_row(
            "SELECT state_after,due_after FROM review_logs WHERE id='r'",
            [],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        )
        .unwrap(),
        ("learning".into(), 1)
    );
    assert_eq!(
        db.query_row("SELECT ease_factor FROM cards WHERE id='c'", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        cijian_domain::srs::INITIAL_EASE_FACTOR
    );
    assert!(db.execute("DELETE FROM review_logs", []).is_err());
    assert!(
        db.execute("UPDATE review_logs SET rating='easy'", [])
            .is_err()
    );
}

#[test]
fn unknown_schema_is_rejected_and_orphan_reservations_still_count() {
    let (_directory, source, path, audio) = fixture();
    source
        .execute(
            "INSERT INTO events VALUES ('orphan','p','audio_reserve',?1,6)",
            [json!({"articleId":"removed-article","chars":9}).to_string()],
        )
        .unwrap();
    let snapshot = inspect_fixture(&path, &audio);
    let target = Store::memory().unwrap();
    target.import_legacy(&snapshot, "import", 10).unwrap();
    assert_eq!(
        target
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT sum(chars) FROM audio_reservations", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        35
    );
    source
        .execute("INSERT INTO _migrations VALUES ('9999_future',10)", [])
        .unwrap();
    assert!(Store::inspect_legacy(&path, None, Some(&audio), || "new".into()).is_err());
}
