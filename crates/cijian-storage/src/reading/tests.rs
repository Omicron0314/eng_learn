use super::*;
use crate::articles::tests::{article, request, store};
use cijian_domain::reading::Difficulty;
fn setup() -> Store {
    let s = store();
    s.start_generation(request()).unwrap();
    s.claim_generation("p", "job", 0).unwrap();
    s.save_article(article()).unwrap();
    s
}
fn command(store: &Store, operation: &str) -> ReaderCommand {
    ReaderCommand {
        profile: "p".into(),
        operation: operation.into(),
        event: format!("event-{operation}"),
        article: "article".into(),
        body_hash: store.article("p", "article").unwrap().body_hash,
        now: 10,
    }
}
#[test]
fn positions_completion_and_separate_last_feedback() {
    let store = setup();
    let open = command(&store, "open");
    store.open_reading(open.clone(), "session").unwrap();
    let events_before: i64 = store
        .connection
        .lock()
        .unwrap()
        .query_row("SELECT count(*) FROM events", [], |row| row.get(0))
        .unwrap();
    store.save_reading_position(&open, "session", 6, 1).unwrap();
    store.save_reading_position(&open, "session", 6, 1).unwrap();
    assert!(store.save_reading_position(&open, "session", 0, 1).is_err());
    assert!(store.save_reading_position(&open, "session", 0, 0).is_err());
    assert!(
        store
            .save_reading_position(&open, "session", 0, u64::MAX)
            .is_err()
    );
    assert!(
        store
            .save_reading_position(&open, "session", usize::MAX, 2)
            .is_err()
    );
    store.save_reading_position(&open, "session", 6, 2).unwrap();
    assert!(store.save_reading_position(&open, "session", 0, 1).is_err());
    store.connection.lock().unwrap().execute_batch("CREATE TRIGGER fail_position_revision BEFORE INSERT ON reading_position_revisions BEGIN SELECT RAISE(ABORT,'original fault'); END;").unwrap();
    assert!(store.save_reading_position(&open, "session", 0, 3).is_err());
    {
        let db = store.connection.lock().unwrap();
        assert_eq!(session(&db, "p", "session").unwrap().position, 6);
        assert_eq!(
            db.query_row("SELECT count(*) FROM events", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            events_before
        );
        db.execute_batch("DROP TRIGGER fail_position_revision")
            .unwrap();
    }
    store.save_reading_position(&open, "session", 6, 3).unwrap();
    assert_eq!(
        store
            .open_reading(command(&store, "reopen"), "session2")
            .unwrap()
            .position,
        6
    );
    store
        .complete_reading(command(&store, "complete"), "session")
        .unwrap();
    store
        .complete_reading(command(&store, "complete"), "session")
        .unwrap();
    store
        .complete_reading(command(&store, "complete-again"), "session")
        .unwrap();
    let db = store.connection.lock().unwrap();
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM events WHERE type='article_complete'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    drop(db);
    for (op, mode, value) in [
        ("reading1", FeedbackMode::Reading, Difficulty::TooEasy),
        ("listening", FeedbackMode::Listening, Difficulty::TooHard),
        ("reading2", FeedbackMode::Reading, Difficulty::JustRight),
    ] {
        store
            .submit_feedback(command(&store, op), Feedback { mode, value })
            .unwrap();
    }
    assert_eq!(
        store
            .feedback("p", "article", FeedbackMode::Reading)
            .unwrap()
            .unwrap()
            .value,
        Difficulty::JustRight
    );
    assert_eq!(
        store
            .feedback("p", "article", FeedbackMode::Listening)
            .unwrap()
            .unwrap()
            .value,
        Difficulty::TooHard
    );
}
#[test]
fn stale_body_partial_words_and_event_failure_are_rejected() {
    let store = setup();
    let mut stale = command(&store, "lookup");
    stale.body_hash = "stale".into();
    assert_eq!(
        store
            .lookup_context(&stale, TextRange { start: 2, end: 5 })
            .unwrap_err()
            .code,
        ErrorCode::StaleState
    );
    let valid = command(&store, "lookup");
    assert_eq!(
        store
            .lookup_context(&valid, TextRange { start: 2, end: 5 })
            .unwrap()
            .selection
            .lemma,
        "cat"
    );
    assert!(
        store
            .lookup_context(&valid, TextRange { start: 3, end: 5 })
            .is_err()
    );
    let mut failed = command(&store, "open-failed");
    failed.event = "bootstrap-p".into();
    assert!(store.open_reading(failed, "failed-session").is_err());
    assert!(session(&store.connection.lock().unwrap(), "p", "failed-session").is_err());
    let ranges = reading::sentences(&store.article("p", "article").unwrap().body);
    for _ in 0..2 {
        store
            .flag_sentences(
                command(&store, "flag"),
                SentenceFlag {
                    ranges: ranges.clone(),
                    note: "这句不自然".into(),
                },
            )
            .unwrap();
    }
    let flag = SentenceFlag {
        ranges,
        note: "另一次原创反馈".into(),
    };
    let mut failed = command(&store, "failed-flag");
    failed.event = "bootstrap-p".into();
    assert!(store.flag_sentences(failed, flag.clone()).is_err());
    assert_eq!(
        store
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT count(*) FROM events WHERE type='unnatural_flag'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    store
        .flag_sentences(command(&store, "failed-flag"), flag)
        .unwrap();
    assert_eq!(
        store
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT count(*) FROM events WHERE type='unnatural_flag'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        2
    );
    assert!(
        store
            .flag_sentences(
                command(&store, "bad-flag"),
                SentenceFlag {
                    ranges: vec![TextRange { start: 0, end: 1 }],
                    note: String::new()
                }
            )
            .is_err()
    );
}
