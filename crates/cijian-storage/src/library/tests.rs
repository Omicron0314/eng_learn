mod browsing;
use super::*;
use cijian_domain::{import::parse_card_csv, srs::Rating};
fn import(prefix: &str, kind: BookKind, text: &str) -> ImportWords {
    let parsed = parse_card_csv(text.as_bytes()).unwrap();
    let row_ids = (0..parsed.rows.len())
        .map(|i| ImportIds {
            lexeme: format!("{prefix}-l-{i}"),
            card: format!("{prefix}-c-{i}"),
            entry: format!("{prefix}-e-{i}"),
        })
        .collect();
    ImportWords {
        profile_id: "p".into(),
        operation_id: prefix.into(),
        event_id: format!("{prefix}-event"),
        book_id: format!("{prefix}-book"),
        import_id: format!("{prefix}-import"),
        name: "词书".into(),
        kind,
        source_name: "原创.csv".into(),
        source_hash: format!("hash-{prefix}"),
        format: "csv".into(),
        now: 1_793_592_000_000,
        parsed,
        row_ids,
    }
}
fn rate(id: &str, op: &str, revision: u64, now: i64) -> RateCard {
    RateCard {
        profile_id: "p".into(),
        operation_id: op.into(),
        event_id: format!("{op}-event"),
        review_id: format!("{op}-review"),
        card_id: id.into(),
        expected_revision: revision,
        rating: Rating::Good,
        duration_ms: 500,
        revealed: true,
        now,
    }
}
#[test]
fn delete_preview_detects_review_and_preserves_history() {
    let store = Store::memory().unwrap();
    store.create_profile("p", "测试", 0).unwrap();
    let command = import("book", BookKind::Study, "apple,苹果\npear,梨");
    let now = command.now;
    store.import(command).unwrap();
    let before = store.preview_delete_book("p", "book-book").unwrap();
    assert_eq!(before.deleted_cards, 2);
    store
        .rate(rate("book-c-0", "review-before-delete", 1, now))
        .unwrap();
    assert_eq!(
        store
            .delete_book(DeleteBook {
                profile_id: "p".into(),
                operation_id: "delete".into(),
                event_id: "delete-event".into(),
                book_id: "book-book".into(),
                fingerprint: before.fingerprint,
                now: now + 1
            })
            .unwrap_err()
            .code,
        ErrorCode::StaleState
    );
    let current = store.preview_delete_book("p", "book-book").unwrap();
    assert_eq!(current.archived_cards, 1);
    assert_eq!(current.deleted_cards, 1);
    store
        .delete_book(DeleteBook {
            profile_id: "p".into(),
            operation_id: "delete".into(),
            event_id: "delete-event".into(),
            book_id: "book-book".into(),
            fingerprint: current.fingerprint,
            now: now + 1,
        })
        .unwrap();
    let card = store.cards("p", page()).unwrap().cards.remove(0);
    assert!(card.archived && card.suspended);
    assert!(
        store
            .edit_card(EditCard {
                profile_id: "p".into(),
                operation_id: "resume".into(),
                event_id: "resume-event".into(),
                card_id: card.id,
                expected_revision: card.revision,
                meaning: card.meaning,
                example: card.example,
                suspended: false,
                now: now + 2
            })
            .is_err()
    );
    assert_eq!(
        store
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM review_logs", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn deleting_one_book_preserves_shared_card() {
    let store = Store::memory().unwrap();
    store.create_profile("p", "测试", 0).unwrap();
    store
        .import(import("a", BookKind::Study, "apple,苹果"))
        .unwrap();
    let mut second = import("b", BookKind::Study, "apple,其他释义");
    second.name = "第二本".into();
    store.import(second).unwrap();
    let preview = store.preview_delete_book("p", "a-book").unwrap();
    assert_eq!(preview.shared_cards, 1);
    store
        .delete_book(DeleteBook {
            profile_id: "p".into(),
            operation_id: "delete-a".into(),
            event_id: "delete-a-event".into(),
            book_id: "a-book".into(),
            fingerprint: preview.fingerprint,
            now: 2,
        })
        .unwrap();
    assert_eq!(store.cards("p", page()).unwrap().total, 1);
    assert_eq!(store.books("p").unwrap().len(), 1);
}

fn page() -> CardFilter {
    CardFilter {
        page_size: 500,
        ..Default::default()
    }
}
#[test]
fn baseline_and_study_merge_without_overwriting_meaning() {
    let store = Store::memory().unwrap();
    store.create_profile("p", "测试", 0).unwrap();
    let baseline = store
        .import(import(
            "base",
            BookKind::Baseline,
            "word,meaning\napple,苹果\n",
        ))
        .unwrap();
    assert_eq!(baseline.words_added, 1);
    assert_eq!(baseline.cards_added, 0);
    let study = store
        .import(import(
            "study",
            BookKind::Study,
            "apple,用户释义\npear,梨\n",
        ))
        .unwrap();
    assert_eq!(study.words_added, 1);
    assert_eq!(study.cards_added, 2);
    let again = import("again", BookKind::Study, "apple,\npear,其他释义\n");
    assert_eq!(store.import(again.clone()).unwrap().duplicates, 2);
    assert_eq!(store.import(again).unwrap().duplicates, 2);
    let result = store.cards("p", page()).unwrap();
    assert_eq!(result.total, 2);
    assert_eq!(
        result
            .cards
            .iter()
            .find(|c| c.front == "apple")
            .unwrap()
            .meaning,
        "用户释义"
    );
    assert_eq!(
        store
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM word_books", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
}
#[test]
fn hundred_word_import_respects_twenty_new_limit_and_learning_exception() {
    let store = Store::memory().unwrap();
    store.create_profile("p", "测试", 0).unwrap();
    let text = (0..100)
        .map(|i| format!("original{i},原创\n"))
        .collect::<String>();
    let command = import("words", BookKind::Study, &text);
    let now = command.now;
    store.import(command).unwrap();
    let cards = store.cards("p", page()).unwrap().cards;
    for (i, card) in cards.iter().take(20).enumerate() {
        store
            .rate(rate(&card.id, &format!("r{i}"), 1, now))
            .unwrap();
    }
    assert_eq!(
        store
            .rate(rate(&cards[20].id, "over-limit", 1, now))
            .unwrap_err()
            .code,
        ErrorCode::QuotaExceeded
    );
    let first = &cards[0];
    let command = rate(&first.id, "learning", 2, now + 600_000);
    let updated = store.rate(command.clone()).unwrap();
    assert_eq!(updated.srs.state, State::Review);
    assert_eq!(store.rate(command).unwrap().revision, 3);
    assert_eq!(
        store
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM review_logs", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        21
    );
    assert_eq!(
        store
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM events WHERE type='review'", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        21
    );
}
#[test]
fn waiting_learning_step_reports_next_due_and_reappears() {
    let store = Store::memory().unwrap();
    store.create_profile("p", "测试", 0).unwrap();
    let command = import("waiting", BookKind::Study, "apple,苹果");
    let now = command.now;
    store.import(command).unwrap();
    let card = store.cards("p", page()).unwrap().cards.remove(0);
    let mut command = rate(&card.id, "waiting-again", 1, now);
    command.rating = Rating::Again;
    store.rate(command).unwrap();

    let (cards, next_due) = store.review_queue("p", now).unwrap();
    assert!(cards.is_empty());
    assert_eq!(next_due, Some(now + 60_000));

    let (cards, next_due) = store.review_queue("p", now + 60_000).unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].id, card.id);
    assert_eq!(next_due, None);
}
#[test]
fn duplicate_event_rolls_back_import_and_rating() {
    let store = Store::memory().unwrap();
    store.create_profile("p", "测试", 0).unwrap();
    let mut command = import("bad", BookKind::Study, "apple,苹果");
    command.event_id = "bootstrap-p".into();
    assert!(store.import(command).is_err());
    assert_eq!(store.cards("p", page()).unwrap().total, 0);
    let command = import("ok", BookKind::Study, "apple,苹果");
    let now = command.now;
    store.import(command).unwrap();
    let card = store.cards("p", page()).unwrap().cards.remove(0);
    let mut command = rate(&card.id, "bad-rating", 1, now);
    command.event_id = "bootstrap-p".into();
    assert!(store.rate(command).is_err());
    assert_eq!(store.cards("p", page()).unwrap().cards[0].revision, 1);
    assert_eq!(
        store
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM review_logs", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
