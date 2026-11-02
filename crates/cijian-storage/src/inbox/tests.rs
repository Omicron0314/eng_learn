use super::*;
use crate::articles::tests::{article, request, store};
use cijian_domain::reading::TextRange;
use cijian_ports::reading::ReaderCommand;
fn setup() -> Store {
    let store = store();
    store.start_generation(request()).unwrap();
    store.claim_generation("p", "job", 0).unwrap();
    store.save_article(article()).unwrap();
    store
}
fn mark(store: &Store, op: &str, action: MarkAction, revision: u64) -> MarkWord {
    MarkWord {
        context: ReaderCommand {
            profile: "p".into(),
            operation: op.into(),
            event: format!("event-{op}"),
            article: "article".into(),
            body_hash: store.article("p", "article").unwrap().body_hash,
            now: 10,
        },
        range: TextRange { start: 2, end: 5 },
        action,
        expected_revision: revision,
        lexeme_id: format!("lexeme-{op}"),
        card_id: format!("card-{op}"),
        inbox_id: format!("inbox-{op}"),
        meaning: String::new(),
    }
}
#[test]
fn unknown_pending_reuse_batch_stale_rollback_and_replay() {
    let store = setup();
    let first = store
        .mark_word(mark(&store, "unknown", MarkAction::Unknown, 0))
        .unwrap();
    assert_eq!(first.revision, 1);
    let second = store
        .mark_word(mark(&store, "unknown2", MarkAction::Unknown, 1))
        .unwrap();
    assert_eq!(first.inbox_id, second.inbox_id);
    assert_eq!(store.pending_inbox_count("p").unwrap(), 1);
    let item = store.inbox("p", 0).unwrap().remove(0);
    assert_eq!(item.revision, 2);
    let selected = InboxSelection {
        id: item.id,
        revision: item.revision,
        card_id: "new-card".into(),
        event_id: "batch-event".into(),
    };
    let mut batch = BatchInbox {
        profile: "p".into(),
        operation: "batch".into(),
        action: InboxAction::Add,
        selections: vec![
            selected.clone(),
            InboxSelection {
                id: "missing".into(),
                revision: 1,
                card_id: "other".into(),
                event_id: "other".into(),
            },
        ],
        now: 20,
    };
    assert_eq!(
        store.batch_inbox(batch.clone()).unwrap_err().code,
        ErrorCode::StaleState
    );
    assert_eq!(store.pending_inbox_count("p").unwrap(), 1);
    assert_eq!(
        store
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM cards", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    batch.selections = vec![selected];
    assert_eq!(store.batch_inbox(batch.clone()).unwrap().cards_created, 1);
    assert_eq!(store.batch_inbox(batch).unwrap().cards_created, 1);
    assert_eq!(store.pending_inbox_count("p").unwrap(), 0);
    assert_eq!(
        store
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT count(*) FROM events WHERE type='add_to_review'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
}
#[test]
fn existing_paused_cards_keep_schedule_and_user_meaning() {
    let store = setup();
    store
        .mark_word(mark(&store, "add", MarkAction::AddToReview, 0))
        .unwrap();
    store.connection.lock().unwrap().execute("UPDATE cards SET meaning='自定义释义',text_source='user',suspended=1,due=1234 WHERE id='card-add'",[]).unwrap();
    let mut retry = mark(&store, "add-again", MarkAction::AddToReview, 0);
    retry.meaning = "不能覆盖".into();
    let result = store.mark_word(retry).unwrap();
    assert!(result.paused);
    assert!(!result.card_created);
    let db = store.connection.lock().unwrap();
    let pair: (String, i64) = db
        .query_row(
            "SELECT meaning,due FROM cards WHERE id='card-add'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(pair, ("自定义释义".into(), 1234));
}
#[test]
fn failed_event_rolls_back_mark_and_inbox() {
    let store = setup();
    let mut command = mark(&store, "fail", MarkAction::Unknown, 0);
    command.context.event = "bootstrap-p".into();
    assert!(store.mark_word(command).is_err());
    assert_eq!(store.pending_inbox_count("p").unwrap(), 0);
    assert_eq!(
        store
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM lexeme_marks", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
