use super::*;
use cijian_domain::{import::BookKind, srs::State};
use cijian_ports::library::CardFilter;
fn filter() -> CardFilter {
    CardFilter {
        page_size: 30,
        ..Default::default()
    }
}
#[test]
fn library_commands_preserve_scheduling_and_reports_across_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let backend = DesktopBackend::open(directory.path()).unwrap();
    let baseline = directory.path().join("baseline.txt");
    std::fs::write(&baseline, "apple\npear\n").unwrap();
    backend
        .files
        .lock()
        .unwrap()
        .insert("baseline".into(), baseline);
    let result = backend
        .execute(Command::ImportSelected {
            target_book: None,
            handle: "baseline".into(),
            name: "基线".into(),
            kind: BookKind::Baseline,
            word_list: true,
        })
        .unwrap();
    assert!(
        matches!(result,Response::Imported(report) if report.cards_added==0&&report.entries_added==2)
    );
    let Response::Library { page, books, .. } = backend
        .execute(Command::LoadLibrary { filter: filter() })
        .unwrap()
    else {
        panic!("library")
    };
    assert_eq!(page.total, 0);
    let baseline_id = books[0].id.clone();
    let study = directory.path().join("study.csv");
    std::fs::write(&study, "word,meaning\napple,苹果\npear,梨\n").unwrap();
    backend.files.lock().unwrap().insert("study".into(), study);
    backend
        .execute(Command::ImportSelected {
            target_book: None,
            handle: "study".into(),
            name: "学习".into(),
            kind: BookKind::Study,
            word_list: false,
        })
        .unwrap();
    let Response::Library { books, page, .. } = backend
        .execute(Command::LoadLibrary { filter: filter() })
        .unwrap()
    else {
        panic!("library")
    };
    let study_id = books
        .iter()
        .find(|book| book.kind == BookKind::Study)
        .unwrap()
        .id
        .clone();
    let card = page
        .cards
        .iter()
        .find(|card| card.front == "apple")
        .unwrap()
        .clone();
    let merge = directory.path().join("merge.csv");
    std::fs::write(&merge, "apple,不覆盖\norange,橙\n").unwrap();
    backend.files.lock().unwrap().insert("merge".into(), merge);
    let result = backend
        .execute(Command::ImportSelected {
            target_book: Some(study_id.clone()),
            handle: "merge".into(),
            name: "应忽略的名称".into(),
            kind: BookKind::Study,
            word_list: false,
        })
        .unwrap();
    assert!(
        matches!(result,Response::Imported(report) if report.cards_added==1&&report.duplicates==1)
    );
    let Response::DeleteBookPreview(preview) = backend
        .execute(Command::PreviewDeleteBook {
            book: study_id.clone(),
        })
        .unwrap()
    else {
        panic!("preview")
    };
    assert_eq!(preview.deleted_cards, 3);
    let Response::LibraryCardEdited(edited) = backend
        .execute(Command::EditLibraryCard {
            card: card.id.clone(),
            revision: card.revision,
            meaning: card.meaning.clone(),
            example: card.example.clone(),
            suspended: true,
            operation: "pause".into(),
        })
        .unwrap()
    else {
        panic!("edited")
    };
    assert_eq!(edited.srs, card.srs);
    assert_eq!(edited.text_source, card.text_source);
    let Response::Library { page, .. } = backend
        .execute(Command::LoadLibrary {
            filter: CardFilter {
                book_id: Some(study_id.clone()),
                search: "苹果".into(),
                state: Some(State::New),
                suspended: Some(true),
                ..filter()
            },
        })
        .unwrap()
    else {
        panic!("filtered")
    };
    assert_eq!(page.total, 1);
    assert_eq!(page.cards[0].id, card.id);
    assert_eq!(
        backend
            .execute(Command::DeleteBook {
                book: study_id.clone(),
                fingerprint: preview.fingerprint,
                operation: "delete".into()
            })
            .unwrap_err()
            .code,
        ErrorCode::StaleState
    );
    let Response::DeleteBookPreview(preview) = backend
        .execute(Command::PreviewDeleteBook {
            book: study_id.clone(),
        })
        .unwrap()
    else {
        panic!("preview")
    };
    backend
        .execute(Command::DeleteBook {
            book: study_id,
            fingerprint: preview.fingerprint,
            operation: "delete".into(),
        })
        .unwrap();
    drop(backend);
    let backend = DesktopBackend::open(directory.path()).unwrap();
    let Response::Library {
        page,
        entries,
        latest_import,
        ..
    } = backend
        .execute(Command::LoadLibrary {
            filter: CardFilter {
                book_id: Some(baseline_id),
                ..filter()
            },
        })
        .unwrap()
    else {
        panic!("reopened")
    };
    assert_eq!(page.total, 0);
    let entries = entries.unwrap();
    assert_eq!(entries.total, 2);
    assert!(entries.entries.iter().all(|entry| entry.card.is_none()));
    let (name, report) = latest_import.unwrap();
    assert_eq!(name, "merge.csv");
    assert_eq!(report.cards_added, 1);
}
