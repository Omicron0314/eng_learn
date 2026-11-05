use super::*;
use cijian_domain::{
    import::{BookKind, ImportRow, ParsedImport},
    provider::ProviderConfig,
};
use cijian_ports::provider::{Secret, SecretPatch};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

fn stub() -> (String, Arc<AtomicBool>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    std::thread::spawn(move || {
        while !flag.load(Ordering::Acquire) {
            let Ok((mut stream, _)) = listener.accept() else {
                std::thread::sleep(std::time::Duration::from_millis(20));
                continue;
            };
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 4096];
            loop {
                let read = stream.read(&mut buffer).unwrap_or(0);
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
                let text = String::from_utf8_lossy(&request);
                if let Some(head) = text.find("\r\n\r\n") {
                    let length = text[..head]
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|value| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if request.len() >= head + 4 + length {
                        break;
                    }
                }
            }
            let request = String::from_utf8_lossy(&request);
            let content = if request.contains("meaningZh") {
                r#"{"lemma":"saffron","pos":"n.","meaningZh":"藏红花","meaningEn":"a spice from a flower","ipa":"/ˈsæfrən/"}"#.to_owned()
            } else {
                let words = (0..120)
                    .map(|index| format!("baseline{index}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                serde_json::json!({"title":"Original E2E","body":format!("saffron {words}"),"names":[]})
                    .to_string()
            };
            let body = serde_json::json!({"choices":[{"message":{"content":content}}]}).to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (address, stop)
}

fn baseline_words() -> Vec<String> {
    (0..120).map(|index| format!("baseline{index}")).collect()
}

fn import(backend: &DesktopBackend, kind: BookKind, rows: Vec<ImportRow>, tag: &str) {
    let row_ids = rows
        .iter()
        .enumerate()
        .map(|(index, _)| cijian_ports::library::ImportIds {
            lexeme: format!("{tag}-lexeme-{index}"),
            card: format!("{tag}-card-{index}"),
            entry: format!("{tag}-entry-{index}"),
        })
        .collect();
    backend
        .store
        .import(cijian_ports::library::ImportWords {
            profile_id: backend.profile.clone(),
            operation_id: format!("{tag}-import"),
            event_id: format!("{tag}-event"),
            book_id: format!("{tag}-book"),
            import_id: format!("{tag}-import-id"),
            name: format!("{tag} book"),
            kind,
            source_name: format!("{tag}.csv"),
            source_hash: format!("{tag}-hash"),
            format: "csv".into(),
            now: now(),
            parsed: ParsedImport {
                parsed: rows.len() as u64,
                rows,
                ..Default::default()
            },
            row_ids,
        })
        .unwrap();
}

#[test]
fn local_stub_generation_reader_lookup_mark_and_feedback_are_auditable() {
    let (endpoint, stop) = stub();
    let directory = tempfile::tempdir().unwrap();
    let backend = DesktopBackend::open_for_tests(directory.path()).unwrap();
    import(
        &backend,
        BookKind::Baseline,
        baseline_words()
            .into_iter()
            .enumerate()
            .map(|(index, word)| ImportRow {
                word: word.clone(),
                raw: word.clone(),
                front: word,
                meaning: "基线词".into(),
                example: String::new(),
                line: index as u64 + 1,
                phrase: false,
            })
            .collect(),
        "baseline",
    );
    import(
        &backend,
        BookKind::Study,
        vec![ImportRow {
            word: "saffron".into(),
            raw: "saffron".into(),
            front: "saffron".into(),
            meaning: String::new(),
            example: "A saffron field.".into(),
            line: 1,
            phrase: false,
        }],
        "study",
    );
    let Response::Settings {
        mut value,
        revision,
        ..
    } = backend.execute(Command::LoadSettings).unwrap()
    else {
        panic!("settings");
    };
    value.article_min_words = 100;
    value.article_max_words = 200;
    value.target_count = 1;
    value.control_ratio = 0.;
    backend
        .execute(Command::SaveSettings {
            value,
            revision,
            operation: "e2e-settings".into(),
        })
        .unwrap();
    backend
        .store
        .rate(cijian_ports::library::RateCard {
            profile_id: backend.profile.clone(),
            operation_id: "e2e-seed-review".into(),
            event_id: "e2e-seed-review-event".into(),
            review_id: "e2e-seed-review-log".into(),
            card_id: "study-card-0".into(),
            expected_revision: 1,
            rating: cijian_domain::srs::Rating::Again,
            duration_ms: 1,
            revealed: true,
            now: now(),
        })
        .unwrap();
    backend
        .execute(Command::SaveProvider {
            config: ProviderConfig {
                kind: ProviderKind::Llm,
                endpoint: endpoint.clone(),
                model: "stub".into(),
                voice: None,
                revision: 1,
            },
            revision: 0,
            secret: SecretPatch::Replace(Secret::new("original-test-key".into()).unwrap()),
            operation: "e2e-provider".into(),
        })
        .unwrap();
    let Response::Job(job) = backend
        .execute(Command::GenerateArticle {
            tier: cijian_domain::settings::Tier::Easy,
            genre: cijian_domain::settings::Genre::Story,
            operation: "e2e-generation".into(),
        })
        .unwrap()
    else {
        panic!("job");
    };
    let mut status = job.status;
    let mut article_id = None;
    for _ in 0..100 {
        let Response::Job(job) = backend
            .execute(Command::ObserveJob {
                job: job.id.clone(),
            })
            .unwrap()
        else {
            panic!("observe");
        };
        status = job.status;
        article_id = job.result_ref;
        if !matches!(
            status,
            cijian_ports::articles::JobStatus::Queued | cijian_ports::articles::JobStatus::Running
        ) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(status, cijian_ports::articles::JobStatus::Succeeded);
    let article_id = article_id.unwrap();
    let Response::Article {
        article,
        session,
        audio: _,
        feedback: _,
    } = backend
        .execute(Command::OpenArticle {
            id: article_id.clone(),
        })
        .unwrap()
    else {
        panic!("article");
    };
    assert!(article.body.split_whitespace().count() >= 100);
    let start = article.body.find("saffron").unwrap();
    let Response::Lookup {
        context, meaning, ..
    } = backend
        .execute(Command::Lookup {
            article: article_id.clone(),
            body_hash: article.body_hash.clone(),
            range: cijian_domain::reading::TextRange {
                start,
                end: start + "saffron".len(),
            },
        })
        .unwrap()
    else {
        panic!("lookup");
    };
    assert_eq!(context.selection.lemma, "saffron");
    backend
        .execute(Command::CompleteReading {
            article: article_id.clone(),
            body_hash: article.body_hash.clone(),
            session: session.id.clone(),
        })
        .unwrap();
    backend
        .execute(Command::Feedback {
            article: article_id.clone(),
            body_hash: article.body_hash.clone(),
            value: cijian_domain::reading::Difficulty::JustRight,
            mode: cijian_domain::reading::FeedbackMode::Reading,
            operation: "e2e-feedback".into(),
        })
        .unwrap();
    backend
        .execute(Command::Mark {
            article: article_id,
            body_hash: article.body_hash,
            range: context.selection.range,
            action: cijian_ports::inbox::MarkAction::AddToReview,
            revision: 0,
            meaning: meaning.clone(),
            operation: "e2e-mark".into(),
        })
        .unwrap();
    let card = backend
        .store
        .cards(
            &backend.profile,
            cijian_ports::library::CardFilter {
                search: "saffron".into(),
                page_size: 30,
                ..Default::default()
            },
        )
        .unwrap()
        .cards
        .into_iter()
        .find(|card| card.front == "saffron")
        .expect("looked-up word becomes a card");
    assert_eq!(card.meaning, meaning, "the fetched gloss is kept");
    assert_eq!(card.text_source, "lookup");
    let audit = backend
        .store
        .export_profile(&backend.profile, false)
        .unwrap();
    let tables = audit.payload["tables"].as_object().unwrap();
    let calls = tables["llm_calls"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|call| call["status"] == "success")
        .count();
    assert!(calls >= 2, "generation and gloss must both be recorded");
    let events = tables["events"].as_array().unwrap();
    for kind in [
        "article_generate",
        "article_complete",
        "difficulty_feedback",
        "add_to_review",
    ] {
        assert!(
            events.iter().any(|event| event["type"] == kind),
            "missing event {kind}"
        );
    }
    stop.store(true, Ordering::Release);
}
