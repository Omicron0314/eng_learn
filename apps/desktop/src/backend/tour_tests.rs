use super::*;
use cijian_domain::{
    import::BookKind,
    provider::ProviderConfig,
    reading::{Difficulty, FeedbackMode, TextRange},
    settings::{Theme, Tier},
};
use cijian_ports::{
    library::{CardFilter, ImportReport},
    provider::{Secret, SecretPatch},
};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

struct Stub {
    endpoint: String,
    stop: Arc<AtomicBool>,
    llm: Arc<AtomicUsize>,
    tts: Arc<AtomicUsize>,
}

fn wave() -> Vec<u8> {
    let rate = 8000_u32;
    let frames = rate / 5;
    let mut data = Vec::with_capacity(frames as usize * 2);
    for index in 0..frames {
        let value = if (index / 20) % 2 == 0 { 800_i16 } else { -800 };
        data.extend_from_slice(&value.to_le_bytes());
    }
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&data);
    bytes
}

fn stub() -> Stub {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let llm = Arc::new(AtomicUsize::new(0));
    let tts = Arc::new(AtomicUsize::new(0));
    let flag = stop.clone();
    let llm_count = llm.clone();
    let tts_count = tts.clone();
    std::thread::spawn(move || {
        while !flag.load(Ordering::Acquire) {
            let Ok((mut stream, _)) = listener.accept() else {
                std::thread::sleep(std::time::Duration::from_millis(20));
                continue;
            };
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
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
            let text = String::from_utf8_lossy(&request).to_string();
            if text.starts_with("POST /audio/speech") {
                tts_count.fetch_add(1, Ordering::Relaxed);
                let audio = wave();
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: audio/wav\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    audio.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&audio);
                continue;
            }
            llm_count.fetch_add(1, Ordering::Relaxed);
            let body_at = text.find("\r\n\r\n").map(|at| at + 4).unwrap_or(0);
            let payload: serde_json::Value =
                serde_json::from_str(&text[body_at..]).unwrap_or(serde_json::Value::Null);
            let user = payload["messages"][1]["content"].as_str().unwrap_or("");
            let content = if user.contains("meaningZh") {
                let lemma = user
                    .lines()
                    .find_map(|line| line.strip_prefix("Word: "))
                    .and_then(|line| line.split_once(" (base form: ").map(|(_, rest)| rest))
                    .and_then(|rest| rest.strip_suffix(')'))
                    .unwrap_or("word");
                serde_json::json!({
                    "lemma": lemma,
                    "pos": "n.",
                    "meaningZh": "原创示例释义",
                    "meaningEn": "an original sample meaning",
                    "ipa": "/ˈsæmpl/"
                })
                .to_string()
            } else {
                let targets = user
                    .lines()
                    .find_map(|line| line.strip_prefix("Target words: "))
                    .unwrap_or("");
                let words = targets
                    .split(',')
                    .map(str::trim)
                    .filter(|word| !word.is_empty())
                    .collect::<Vec<_>>();
                let low = user
                    .split("of ")
                    .nth(1)
                    .and_then(|rest| rest.split('-').next())
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(100);
                let mut body = String::new();
                for word in &words {
                    body.push_str(&format!("The {word} is here and the {word} is there. "));
                }
                let filler = [
                    "It is the one and it is so. ",
                    "It is not the one but it is there. ",
                    "This is the one and that is all. ",
                ];
                let mut index = 0;
                while body.split_whitespace().count() < low {
                    body.push_str(filler[index % filler.len()]);
                    index += 1;
                }
                serde_json::json!({
                    "title": "A Quiet Morning",
                    "body": body,
                    "names": ["Mira"]
                })
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
    Stub {
        endpoint,
        stop,
        llm,
        tts,
    }
}

fn import_file(
    backend: &DesktopBackend,
    directory: &std::path::Path,
    name: &str,
    contents: &str,
    kind: BookKind,
    word_list: bool,
    book_name: &str,
) -> ImportReport {
    let path = directory.join(name);
    std::fs::write(&path, contents).unwrap();
    backend.files.lock().unwrap().insert(name.into(), path);
    let Response::Imported(report) = backend
        .execute(Command::ImportSelected {
            target_book: None,
            handle: name.into(),
            name: book_name.into(),
            kind,
            word_list,
        })
        .unwrap()
    else {
        panic!("import response")
    };
    report
}

fn settings(backend: &DesktopBackend) -> (cijian_domain::settings::Settings, u64) {
    let Response::Settings {
        value, revision, ..
    } = backend.execute(Command::LoadSettings).unwrap()
    else {
        panic!("settings")
    };
    (value, revision)
}

fn cards(backend: &DesktopBackend, filter: CardFilter) -> cijian_ports::library::CardPage {
    let Response::Library { page, .. } = backend.execute(Command::LoadLibrary { filter }).unwrap()
    else {
        panic!("library")
    };
    page
}

#[test]
fn desktop_feature_tour_reconciles_writes_and_survives_reopen() {
    let stub = stub();
    let directory = tempfile::tempdir().unwrap();
    let backend = DesktopBackend::open_for_tests(directory.path()).unwrap();

    // Learning settings and both providers are configured before any network call.
    let (mut value, revision) = settings(&backend);
    value.article_min_words = 100;
    value.article_max_words = 200;
    value.target_count = 2;
    value.control_ratio = 0.;
    value.lookup_mode = cijian_domain::settings::LookupMode::MarkOnly;
    let Response::LearningSettingsSaved { revision, .. } = backend
        .execute(Command::SaveSettings {
            value: value.clone(),
            revision,
            operation: "tour-settings".into(),
        })
        .unwrap()
    else {
        panic!("settings")
    };
    assert!(matches!(
        backend
            .execute(Command::SaveSettings {
                value: cijian_domain::settings::Settings {
                    article_min_words: 500,
                    article_max_words: 400,
                    ..value
                },
                revision,
                operation: "tour-invalid-settings".into(),
            })
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    ));
    for (kind, voice) in [
        (ProviderKind::Llm, None),
        (ProviderKind::Tts, Some("stub-voice".to_owned())),
    ] {
        let config = ProviderConfig {
            kind,
            endpoint: stub.endpoint.clone(),
            model: "original-stub".into(),
            voice,
            revision: 1,
        };
        let Response::ProviderSaved {
            config: saved,
            has_key,
            source,
        } = backend
            .execute(Command::SaveProvider {
                config,
                revision: 0,
                secret: SecretPatch::Replace(Secret::new("original-key".into()).unwrap()),
                operation: if kind == ProviderKind::Llm {
                    "tour-llm"
                } else {
                    "tour-tts"
                }
                .into(),
            })
            .unwrap()
        else {
            panic!("provider")
        };
        assert_eq!(saved.kind, kind);
        assert!(has_key);
        assert_eq!(source, cijian_ports::provider::SecretSource::Session);
    }

    // Baseline word list and a study CSV with a duplicate, a phrase and an
    // invalid empty row.
    let baseline = (0..120_u8)
        .map(|index| {
            let word = format!(
                "word{}{}",
                char::from(b'a' + index / 26),
                char::from(b'a' + index % 26)
            );
            format!("{word},基线{index},The {word} is here.\n")
        })
        .collect::<String>();
    let report = import_file(
        &backend,
        directory.path(),
        "baseline.csv",
        &format!("word,meaning,example\n{baseline}"),
        BookKind::Baseline,
        true,
        "基线词书",
    );
    assert_eq!(report.entries_added, 120);
    assert_eq!(report.cards_added, 0);
    assert_eq!(report.parsed, 120);
    let study = "word,meaning,example\n\
lantern,灯笼,He lit the lantern.\n\
meadow,草地,They crossed the meadow.\n\
harbor,港口,The harbor was quiet.\n\
willow,柳树,The willow bent in the wind.\n\
nectar,花蜜,Bees gather nectar.\n\
glade,林间空地,Sunlight entered the glade.\n\
ripple,涟漪,A ripple crossed the pond.\n\
pebble,卵石,A pebble rolled into the stream.\n\
saffron,,A saffron field glowed.\n\
thistle,,The thistle pricked her hand.\n\
saffron,重复行,Duplicate row.\n\
in the morning,在早上,Phrase row.\n\
,空行,\n";
    let report = import_file(
        &backend,
        directory.path(),
        "study.csv",
        study,
        BookKind::Study,
        false,
        "学习词书",
    );
    assert_eq!(report.duplicates, 1);
    assert_eq!(report.phrases, 1);
    assert_eq!(report.skipped, 1);
    assert_eq!(report.cards_added, 11);
    let Response::Library {
        books,
        page,
        entries,
        latest_import,
    } = backend
        .execute(Command::LoadLibrary {
            filter: CardFilter {
                page_size: 30,
                ..Default::default()
            },
        })
        .unwrap()
    else {
        panic!("library")
    };
    assert_eq!(books.len(), 2);
    assert_eq!(page.total, 11);
    assert!(entries.is_none());
    assert!(latest_import.is_some_and(|(name, report)| name == "study.csv" && report.parsed > 0));
    let Response::Library { page, entries, .. } = backend
        .execute(Command::LoadLibrary {
            filter: CardFilter {
                book_id: books
                    .iter()
                    .find(|book| book.kind == BookKind::Baseline)
                    .map(|book| book.id.clone()),
                search: "wordab".into(),
                page_size: 30,
                ..Default::default()
            },
        })
        .unwrap()
    else {
        panic!("baseline")
    };
    assert_eq!(page.total, 0);
    assert!(entries.is_some_and(|entries| entries.total >= 1));

    // Search, edit and suspend a card.
    let page = cards(
        &backend,
        CardFilter {
            search: "harbor".into(),
            page_size: 30,
            ..Default::default()
        },
    );
    let harbor = page.cards.first().unwrap().clone();
    let Response::LibraryCardEdited(edited) = backend
        .execute(Command::EditLibraryCard {
            card: harbor.id.clone(),
            revision: harbor.revision,
            meaning: "用户改写的港口".into(),
            example: harbor.example.clone(),
            suspended: true,
            operation: "tour-edit".into(),
        })
        .unwrap()
    else {
        panic!("edit")
    };
    assert_eq!(edited.text_source, "user");
    assert_eq!(edited.revision, harbor.revision + 1);
    assert_eq!(edited.srs, harbor.srs);
    let Response::LibraryCardEdited(edited) = backend
        .execute(Command::EditLibraryCard {
            card: harbor.id.clone(),
            revision: edited.revision,
            meaning: "用户改写的港口".into(),
            example: harbor.example.clone(),
            suspended: false,
            operation: "tour-unsuspend".into(),
        })
        .unwrap()
    else {
        panic!("edit")
    };
    assert!(!edited.suspended);

    // Review: one missing-meaning card is filled once through the AI gloss path.
    let Response::Review {
        cards: queue,
        waiting,
        intervals,
        ..
    } = backend.execute(Command::LoadReview).unwrap()
    else {
        panic!("review")
    };
    assert!(!waiting);
    assert!(intervals.is_some());
    let missing = queue
        .iter()
        .find(|card| card.front == "saffron")
        .unwrap()
        .clone();
    assert!(missing.meaning.is_empty());
    let Response::CardExplained(filled) = backend
        .execute(Command::ExplainCard {
            card: missing.id.clone(),
            revision: missing.revision,
            operation: "tour-gloss".into(),
        })
        .unwrap()
    else {
        panic!("gloss")
    };
    assert!(filled.meaning.contains("原创示例释义"));
    assert_eq!(filled.text_source, "generated");
    assert_eq!(filled.revision, missing.revision + 1);
    assert_eq!(stub.llm.load(Ordering::Relaxed), 1);
    let Response::CardExplained(cached) = backend
        .execute(Command::ExplainCard {
            card: filled.id.clone(),
            revision: filled.revision,
            operation: "tour-gloss-replay".into(),
        })
        .unwrap()
    else {
        panic!("gloss")
    };
    assert_eq!(cached.meaning, filled.meaning);
    assert_eq!(
        stub.llm.load(Ordering::Relaxed),
        1,
        "an existing meaning is not requested again"
    );
    assert!(matches!(
        backend
            .execute(Command::ExplainCard {
                card: filled.id.clone(),
                revision: missing.revision,
                operation: "tour-gloss-stale".into(),
            })
            .unwrap_err()
            .code,
        ErrorCode::StaleState
    ));
    for (index, (word, rating)) in [
        ("lantern", cijian_domain::srs::Rating::Again),
        ("thistle", cijian_domain::srs::Rating::Good),
    ]
    .into_iter()
    .enumerate()
    {
        let Response::Review { cards, .. } = backend.execute(Command::LoadReview).unwrap() else {
            panic!("review")
        };
        let card = cards
            .iter()
            .find(|card| card.front == word)
            .unwrap()
            .clone();
        let Response::Rated(rated) = backend
            .execute(Command::Rate {
                card: card.id.clone(),
                revision: card.revision,
                rating,
                operation: format!("tour-rate-{index}"),
                duration_ms: 1200,
            })
            .unwrap()
        else {
            panic!("rate")
        };
        assert!(rated.srs.reps >= 1);
    }

    // Generation uses the reviewed cards as targets and saves atomically.
    let Response::Job(mut job) = backend
        .execute(Command::GenerateArticle {
            tier: Tier::Easy,
            genre: cijian_domain::settings::Genre::Story,
            operation: "tour-generation".into(),
        })
        .unwrap()
    else {
        panic!("job")
    };
    for _ in 0..120 {
        if !matches!(
            job.status,
            cijian_ports::articles::JobStatus::Queued | cijian_ports::articles::JobStatus::Running
        ) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        let Response::Job(next) = backend
            .execute(Command::ObserveJob {
                job: job.id.clone(),
            })
            .unwrap()
        else {
            panic!("observe")
        };
        job = next;
    }
    assert_eq!(job.status, cijian_ports::articles::JobStatus::Succeeded);
    let article_id = job.result_ref.expect("article id");
    let Response::Articles(list) = backend
        .execute(Command::LoadArticles {
            from: None,
            until: None,
            page: 0,
        })
        .unwrap()
    else {
        panic!("articles")
    };
    assert!(list.iter().any(|article| article.id == article_id));
    let Response::Article {
        article,
        session,
        audio: _,
        feedback,
    } = backend
        .execute(Command::OpenArticle {
            id: article_id.clone(),
        })
        .unwrap()
    else {
        panic!("article")
    };
    assert_eq!(session.position, 0);
    assert!(feedback.is_empty());

    // Reader lookup keeps the fetched gloss on the new card.
    let page = cards(
        &backend,
        CardFilter {
            search: "harbor".into(),
            page_size: 30,
            ..Default::default()
        },
    );
    assert_eq!(page.cards[0].meaning, "用户改写的港口");
    let page = cards(
        &backend,
        CardFilter {
            search: "thistle".into(),
            page_size: 30,
            ..Default::default()
        },
    );
    assert!(page.cards[0].meaning.is_empty());
    assert_eq!(page.cards[0].text_source, "missing");
    let range = {
        let start = article
            .body
            .find("thistle")
            .expect("target word in article");
        TextRange {
            start,
            end: start + "thistle".len(),
        }
    };
    let Response::Lookup {
        context,
        revision,
        meaning,
    } = backend
        .execute(Command::Lookup {
            article: article_id.clone(),
            body_hash: article.body_hash.clone(),
            range,
        })
        .unwrap()
    else {
        panic!("lookup")
    };
    assert_eq!(context.selection.lemma, "thistle");
    assert!(meaning.contains("原创示例释义"));
    let Response::Marked(marked) = backend
        .execute(Command::Mark {
            article: article_id.clone(),
            body_hash: article.body_hash.clone(),
            range,
            action: cijian_ports::inbox::MarkAction::AddToReview,
            revision,
            meaning: meaning.clone(),
            operation: "tour-mark".into(),
        })
        .unwrap()
    else {
        panic!("mark")
    };
    assert!(marked.card_id.is_some());
    let page = cards(
        &backend,
        CardFilter {
            search: "thistle".into(),
            page_size: 30,
            ..Default::default()
        },
    );
    assert_eq!(page.cards[0].meaning, meaning);
    assert_eq!(page.cards[0].text_source, "lookup");

    // Reading progress, completion, both feedback modes and sentence flags.
    backend
        .execute(Command::SaveReadingPosition {
            article: article_id.clone(),
            body_hash: article.body_hash.clone(),
            session: session.id.clone(),
            position: 12,
            revision: 1,
        })
        .unwrap();
    let Response::Completed(completed) = backend
        .execute(Command::CompleteReading {
            article: article_id.clone(),
            body_hash: article.body_hash.clone(),
            session: session.id.clone(),
        })
        .unwrap()
    else {
        panic!("complete")
    };
    assert!(completed.completed_at.is_some());
    let Response::Articles(list) = backend
        .execute(Command::LoadArticles {
            from: None,
            until: None,
            page: 0,
        })
        .unwrap()
    else {
        panic!("articles")
    };
    assert!(
        list.iter()
            .any(|article| article.id == article_id && article.read_at.is_some()),
        "the article list projects completion"
    );
    for mode in [FeedbackMode::Reading, FeedbackMode::Listening] {
        assert!(matches!(
            backend
                .execute(Command::Feedback {
                    article: article_id.clone(),
                    body_hash: article.body_hash.clone(),
                    value: Difficulty::JustRight,
                    mode,
                    operation: format!("tour-feedback-{mode:?}"),
                })
                .unwrap(),
            Response::FeedbackSaved(_)
        ));
    }
    assert!(matches!(
        backend
            .execute(Command::FlagSentences {
                article: article_id.clone(),
                body_hash: article.body_hash.clone(),
                flag: cijian_ports::reading::SentenceFlag {
                    ranges: vec![cijian_domain::reading::sentences(&article.body)[0]],
                    note: "原创不适配句子".into(),
                },
                operation: "tour-flag".into(),
            })
            .unwrap(),
        Response::SentencesFlagged { .. }
    ));

    // Inbox: unknown mark, stale batch rollback, then a successful batch.
    let other = article
        .body
        .find("lantern")
        .map(|start| TextRange {
            start,
            end: start + "lantern".len(),
        })
        .expect("second target in article");
    let Response::Lookup {
        context, revision, ..
    } = backend
        .execute(Command::Lookup {
            article: article_id.clone(),
            body_hash: article.body_hash.clone(),
            range: other,
        })
        .unwrap()
    else {
        panic!("lookup")
    };
    assert_eq!(context.selection.lemma, "lantern");
    backend
        .execute(Command::Mark {
            article: article_id.clone(),
            body_hash: article.body_hash.clone(),
            range: other,
            action: cijian_ports::inbox::MarkAction::Unknown,
            revision,
            meaning: String::new(),
            operation: "tour-inbox-mark".into(),
        })
        .unwrap();
    let Response::Inbox { items, total } = backend.execute(Command::LoadInbox { page: 0 }).unwrap()
    else {
        panic!("inbox")
    };
    assert_eq!(total, 1);
    let item = items.first().unwrap().clone();
    assert_eq!(item.lemma, "lantern");
    assert!(item.article_id.is_some());
    assert!(matches!(
        backend
            .execute(Command::BatchInbox {
                action: cijian_ports::inbox::InboxAction::Add,
                selections: vec![(item.id.clone(), item.revision + 10)],
                operation: "tour-inbox-stale".into(),
            })
            .unwrap_err()
            .code,
        ErrorCode::StaleState
    ));
    let Response::Inbox { total, .. } = backend.execute(Command::LoadInbox { page: 0 }).unwrap()
    else {
        panic!("inbox")
    };
    assert_eq!(total, 1);
    let Response::InboxProcessed(result) = backend
        .execute(Command::BatchInbox {
            action: cijian_ports::inbox::InboxAction::Add,
            selections: vec![(item.id.clone(), item.revision)],
            operation: "tour-inbox-add".into(),
        })
        .unwrap()
    else {
        panic!("batch")
    };
    assert_eq!(result.processed, 1);
    assert_eq!(result.cards_created, 0, "the imported card is reused");
    let Response::Inbox { total, .. } = backend.execute(Command::LoadInbox { page: 0 }).unwrap()
    else {
        panic!("inbox")
    };
    assert_eq!(total, 0);

    // Speech is local. Without the installed worker the command must fail
    // gracefully, create no asset and never reach the network stub.
    let installed = cijian_platform::kokoro::installation()
        .ok()
        .is_some_and(|root| root.join("venv/bin/python").exists());
    let audio = backend.execute(Command::EnsureAudio {
        article: article_id.clone(),
        operation: "tour-audio".into(),
    });
    let assets = backend
        .store
        .article_audio_assets(&backend.profile, &article_id)
        .unwrap();
    if installed {
        assert!(
            audio.is_ok(),
            "installed worker should synthesize: {audio:?}"
        );
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].voice, "af_heart", "default accent is American");
        assert!(assets[0].model.starts_with("kokoro-v1.0-q8f16-"));
        assert!(
            directory
                .path()
                .join("audio")
                .join(&assets[0].blob_key)
                .exists()
        );
        let _ = backend.execute(Command::EnsureAudio {
            article: article_id.clone(),
            operation: "tour-audio-cached".into(),
        });
        assert_eq!(
            backend
                .store
                .article_audio_assets(&backend.profile, &article_id)
                .unwrap()
                .len(),
            1,
            "a cached article is not synthesized twice"
        );
    } else {
        assert_eq!(
            audio.err().map(|error| error.code),
            Some(ErrorCode::UnsupportedCapability),
            "a missing local model is reported, never a fake success"
        );
        assert!(assets.is_empty());
    }
    assert_eq!(
        stub.tts.load(Ordering::Relaxed),
        0,
        "local speech never calls a remote TTS endpoint"
    );
    let _ = backend.execute(Command::AudioStop);

    // Settings projections, stats and the experiment report read real writes.
    let Response::Settings {
        value,
        has_session_key,
        ..
    } = backend.execute(Command::LoadSettings).unwrap()
    else {
        panic!("settings")
    };
    assert_eq!(value.daily_new_limit, 20);
    assert!(has_session_key);
    backend
        .execute(Command::SetTheme { value: Theme::Dark })
        .unwrap();
    backend
        .execute(Command::SetPlaybackRate { rate: 1.25 })
        .unwrap();
    let Response::Stats(stats) = backend.execute(Command::LoadStats { page: 0 }).unwrap() else {
        panic!("stats")
    };
    assert_eq!(stats.today.reviews, 0);
    assert_eq!(stats.today.new_cards, 2);
    assert_eq!(stats.reading_completion, Some(1.));
    assert_eq!(stats.feedback_just_right, Some(1.));
    assert_eq!(stats.experiment.exposed.unreviewed, 2);
    assert_eq!(stats.experiment.exposed.sample, 0);
    assert_eq!(
        stub.llm.load(Ordering::Relaxed),
        4,
        "generation, two lookups, and one review gloss were recorded"
    );

    // The same directory reopens with all learned state intact.
    let profile = backend.profile.clone();
    let audit = backend.store.export_profile(&profile, true).unwrap();
    drop(backend);
    let reopened = DesktopBackend::open_for_tests(directory.path()).unwrap();
    let Response::Library { page, .. } = reopened
        .execute(Command::LoadLibrary {
            filter: CardFilter {
                page_size: 30,
                ..Default::default()
            },
        })
        .unwrap()
    else {
        panic!("library")
    };
    assert_eq!(page.total, 11);
    let Response::Review { cards, .. } = reopened.execute(Command::LoadReview).unwrap() else {
        panic!("review")
    };
    // The two cards rated into 1m/10m learning steps are still inside Anki's
    // 20-minute learn-ahead window, so they remain in the queue alongside the
    // nine untouched new cards.
    assert_eq!(cards.len(), 11);
    let Response::Settings {
        value,
        provider,
        has_session_key,
        ..
    } = reopened.execute(Command::LoadSettings).unwrap()
    else {
        panic!("settings")
    };
    assert_eq!(value.theme, Theme::Dark);
    assert_eq!(value.playback_rate, 1.25);
    assert!(provider.is_some());
    assert!(!has_session_key, "session secrets are not persisted");
    let tables = audit.payload["tables"].as_object().unwrap();
    assert_eq!(tables["cards"].as_array().unwrap().len(), 11);
    assert_eq!(tables["articles"].as_array().unwrap().len(), 1);
    assert_eq!(
        tables["review_logs"].as_array().unwrap().len(),
        2,
        "both ratings are retained"
    );
    for kind in [
        "import_complete",
        "card_gloss",
        "review",
        "article_generate",
        "lookup",
        "add_to_review",
        "mark_unknown",
        "audio_play",
        "difficulty_feedback",
        "unnatural_flag",
    ] {
        assert!(
            tables["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|event| event["type"] == kind),
            "missing event {kind}"
        );
    }
    stub.stop.store(true, Ordering::Release);
}

/// Build a deterministic, original UI fixture for manual acceptance runs:
/// CIJIAN_ACCEPTANCE_DIR=/tmp/cijian-ui CARGO_TARGET_DIR=... cargo test -p cijian-desktop seed_acceptance_fixture -- --ignored --nocapture
#[test]
#[ignore]
fn seed_acceptance_fixture() {
    let directory = std::env::var("CIJIAN_ACCEPTANCE_DIR").expect("acceptance directory");
    let endpoint = std::env::var("CIJIAN_STUB_ENDPOINT")
        .unwrap_or_else(|_| "http://127.0.0.1:18080/v1".into());
    std::fs::create_dir_all(&directory).unwrap();
    let backend = DesktopBackend::open_for_tests(std::path::Path::new(&directory)).unwrap();
    let (mut value, revision) = settings(&backend);
    value.article_min_words = 100;
    value.article_max_words = 200;
    value.target_count = 2;
    value.control_ratio = 0.;
    let Response::LearningSettingsSaved { .. } = backend
        .execute(Command::SaveSettings {
            value,
            revision,
            operation: "seed-settings".into(),
        })
        .unwrap()
    else {
        panic!("settings")
    };
    for (kind, voice) in [
        (ProviderKind::Llm, None),
        (ProviderKind::Tts, Some("stub-voice".to_owned())),
    ] {
        backend
            .execute(Command::SaveProvider {
                config: ProviderConfig {
                    kind,
                    endpoint: endpoint.clone(),
                    model: "original-stub".into(),
                    voice,
                    revision: 1,
                },
                revision: 0,
                secret: SecretPatch::Replace(Secret::new("original-key".into()).unwrap()),
                operation: format!("seed-provider-{kind:?}"),
            })
            .unwrap();
    }
    let baseline = (0..120_u8)
        .map(|index| {
            let word = format!(
                "word{}{}",
                char::from(b'a' + index / 26),
                char::from(b'a' + index % 26)
            );
            format!("{word},基线{index},The {word} is here.\n")
        })
        .collect::<String>();
    import_file(
        &backend,
        std::path::Path::new(&directory),
        "baseline.csv",
        &format!("word,meaning,example\n{baseline}"),
        BookKind::Baseline,
        true,
        "基线词书",
    );
    import_file(
        &backend,
        std::path::Path::new(&directory),
        "study.csv",
        "word,meaning,example\n\
lantern,灯笼,He lit the lantern.\n\
meadow,草地,They crossed the meadow.\n\
harbor,港口,The harbor was quiet.\n\
willow,柳树,The willow bent in the wind.\n\
nectar,花蜜,Bees gather nectar.\n\
glade,林间空地,Sunlight entered the glade.\n\
ripple,涟漪,A ripple crossed the pond.\n\
pebble,卵石,A pebble rolled into the stream.\n\
saffron,,A saffron field glowed.\n\
thistle,,The thistle pricked her hand.\n\
in the morning,在早上,Phrase row.\n",
        BookKind::Study,
        false,
        "学习词书",
    );
    for (index, word) in ["lantern", "thistle"].into_iter().enumerate() {
        let Response::Review { cards, .. } = backend.execute(Command::LoadReview).unwrap() else {
            panic!("review")
        };
        let card = cards
            .iter()
            .find(|card| card.front == word)
            .expect("seed card")
            .clone();
        backend
            .execute(Command::Rate {
                card: card.id,
                revision: card.revision,
                rating: if index == 0 {
                    cijian_domain::srs::Rating::Again
                } else {
                    cijian_domain::srs::Rating::Good
                },
                operation: format!("seed-rate-{index}"),
                duration_ms: 1500,
            })
            .unwrap();
    }
    let Response::Job(mut job) = backend
        .execute(Command::GenerateArticle {
            tier: Tier::Easy,
            genre: cijian_domain::settings::Genre::Story,
            operation: "seed-generation".into(),
        })
        .unwrap()
    else {
        panic!("job")
    };
    for _ in 0..120 {
        if !matches!(
            job.status,
            cijian_ports::articles::JobStatus::Queued | cijian_ports::articles::JobStatus::Running
        ) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        let Response::Job(next) = backend
            .execute(Command::ObserveJob {
                job: job.id.clone(),
            })
            .unwrap()
        else {
            panic!("job")
        };
        job = next;
    }
    assert_eq!(job.status, cijian_ports::articles::JobStatus::Succeeded);
    let article_id = job.result_ref.expect("article");
    let Response::Article { article, .. } = backend
        .execute(Command::OpenArticle {
            id: article_id.clone(),
        })
        .unwrap()
    else {
        panic!("article")
    };
    let start = article.body.find("lantern").expect("target word");
    let range = TextRange {
        start,
        end: start + "lantern".len(),
    };
    let Response::Lookup { revision, .. } = backend
        .execute(Command::Lookup {
            article: article_id.clone(),
            body_hash: article.body_hash.clone(),
            range,
        })
        .unwrap()
    else {
        panic!("lookup")
    };
    backend
        .execute(Command::Mark {
            article: article_id.clone(),
            body_hash: article.body_hash.clone(),
            range,
            action: cijian_ports::inbox::MarkAction::Unknown,
            revision,
            meaning: String::new(),
            operation: "seed-inbox".into(),
        })
        .unwrap();
    let _ = backend.execute(Command::EnsureAudio {
        article: article_id,
        operation: "seed-audio".into(),
    });
    println!("acceptance data ready in {directory}");
}
