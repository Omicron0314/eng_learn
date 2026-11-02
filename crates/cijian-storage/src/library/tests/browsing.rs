use super::*;
#[test]
fn baseline_browsing_searches_shared_meanings_without_creating_cards() {
    let store = Store::memory().unwrap();
    store.create_profile("p", "测试", 0).unwrap();
    store.create_profile("other", "隔离", 0).unwrap();
    store
        .import(import("base", BookKind::Baseline, "Apple\npear"))
        .unwrap();
    let mut filter = CardFilter {
        book_id: Some("base-book".into()),
        page_size: 1,
        ..Default::default()
    };
    let first = store.book_entries("p", &filter).unwrap();
    assert_eq!(first.total, 2);
    assert_eq!(first.entries[0].raw, "Apple");
    assert!(first.entries[0].card.is_none());
    assert_eq!(store.cards("p", page()).unwrap().total, 0);
    filter.page = 1;
    assert_eq!(
        store.book_entries("p", &filter).unwrap().entries[0].lemma,
        "pear"
    );
    assert_eq!(store.book_entries("other", &filter).unwrap().total, 0);
    store
        .import(import("study", BookKind::Study, "apple,苹果"))
        .unwrap();
    filter.page = 0;
    filter.search = "苹果".into();
    let result = store.book_entries("p", &filter).unwrap();
    assert_eq!(result.total, 1);
    assert_eq!(result.entries[0].card.as_ref().unwrap().id, "study-c-0");
    filter.suspended = Some(true);
    assert_eq!(store.book_entries("p", &filter).unwrap().total, 0);
    store.connection.lock().unwrap().execute("INSERT INTO imports(id,profile_id,source_name,format,kind,report_json,source_hash,created_at) VALUES ('legacy','p','历史.csv','csv','study','{}','legacy',2000000000000)",[]).unwrap();
    let last = store.latest_import_report("p").unwrap().unwrap();
    assert_eq!(last.0, "原创.csv");
    assert_eq!(last.1.cards_added, 1);
}
#[test]
fn stale_explicit_target_does_not_merge_into_replacement_book() {
    let store = Store::memory().unwrap();
    store.create_profile("p", "测试", 0).unwrap();
    store
        .import(import("old", BookKind::Study, "apple"))
        .unwrap();
    let preview = store.preview_delete_book("p", "old-book").unwrap();
    store
        .delete_book(DeleteBook {
            profile_id: "p".into(),
            operation_id: "delete-old".into(),
            event_id: "delete-old-event".into(),
            book_id: "old-book".into(),
            fingerprint: preview.fingerprint,
            now: 2,
        })
        .unwrap();
    store
        .import(import("replacement", BookKind::Study, "pear"))
        .unwrap();
    let before: i64 = store
        .connection
        .lock()
        .unwrap()
        .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
        .unwrap();
    let mut stale = import("stale", BookKind::Study, "orange");
    stale.book_id = "old-book".into();
    assert_eq!(store.import(stale).unwrap_err().code, ErrorCode::StaleState);
    assert_eq!(store.cards("p", page()).unwrap().total, 1);
    let db = store.connection.lock().unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM events", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        before
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM imports", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
}
#[test]
fn ten_thousand_baseline_words_are_paged_and_counted() {
    let store = Store::memory().unwrap();
    store.create_profile("p", "测试", 0).unwrap();
    let text = (0_u32..10000)
        .map(|mut n| {
            let mut suffix = [b'a'; 4];
            for letter in suffix.iter_mut().rev() {
                *letter += (n % 26) as u8;
                n /= 26;
            }
            format!("seed{}", String::from_utf8(suffix.to_vec()).unwrap())
        })
        .collect::<Vec<_>>()
        .join("\n");
    let report = store
        .import(import("large", BookKind::Baseline, &text))
        .unwrap();
    assert_eq!(report.entries_added, 10000);
    assert_eq!(report.cards_added, 0);
    let result = store
        .book_entries(
            "p",
            &CardFilter {
                book_id: Some("large-book".into()),
                page: 333,
                page_size: 30,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.total, 10000);
    assert_eq!(result.entries.len(), 10);
    assert!(result.entries.iter().all(|entry| entry.card.is_none()));
    let payload: String = store
        .connection
        .lock()
        .unwrap()
        .query_row(
            "SELECT payload_json FROM events WHERE type='import_complete'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&payload).unwrap()["counts"]["entries_added"],
        10000
    );
}
