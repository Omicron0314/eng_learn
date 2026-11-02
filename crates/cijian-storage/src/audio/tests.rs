use super::*;
fn populate(store: &Store) {
    store.create_profile("p", "test", 0).unwrap();
    let text = "猫 and a cat.";
    store.connection.lock().unwrap().execute("INSERT INTO articles VALUES ('a','p','Original',?1,?2,'\"story\"','\"easy\"','article.v1','fixture','fixture','generated',NULL,'{}','v1','v1',0)",params![text,hash(text)]).unwrap();
    let value = Settings {
        tts_daily_char_limit: text.chars().count() as u32,
        ..Settings::default()
    };
    store
        .update_settings(SaveSettings {
            profile_id: "p".into(),
            operation_id: "quota".into(),
            event_id: "quota".into(),
            expected_revision: 1,
            now: 0,
            value: value.try_into().unwrap(),
        })
        .unwrap();
}
fn reserve(op: &str) -> ReserveAudio {
    ReserveAudio {
        profile: "p".into(),
        operation: op.into(),
        event: format!("event-{op}"),
        reservation: format!("reservation-{op}"),
        article: "a".into(),
        body_hash: hash("猫 and a cat."),
        now: 1,
    }
}
#[test]
fn failures_retain_quota_and_only_one_dispatch_claims() {
    let store = Store::memory().unwrap();
    populate(&store);
    let first = store.reserve_audio(reserve("one")).unwrap();
    assert_eq!(first.chars, "猫 and a cat.".chars().count() as u64);
    assert_eq!(store.reserve_audio(reserve("one")).unwrap().id, first.id);
    assert!(store.claim_audio("p", &first.id).unwrap());
    assert!(!store.claim_audio("p", &first.id).unwrap());
    store.fail_audio("p", &first.id).unwrap();
    assert_eq!(
        store.reserve_audio(reserve("two")).unwrap_err().code,
        ErrorCode::QuotaExceeded
    );
}
#[test]
fn independent_connections_cannot_overreserve() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("quota.sqlite");
    let first = Store::open(&path).unwrap();
    populate(&first);
    let second = Store::open(&path).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let a = barrier.clone();
    let b = barrier.clone();
    let one = std::thread::spawn(move || {
        a.wait();
        first.reserve_audio(reserve("one"))
    });
    let two = std::thread::spawn(move || {
        b.wait();
        second.reserve_audio(reserve("two"))
    });
    let outcomes = [one.join().unwrap(), two.join().unwrap()];
    assert_eq!(outcomes.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|r| r
                .as_ref()
                .is_err_and(|e| e.code == ErrorCode::QuotaExceeded))
            .count(),
        1
    );
}
#[test]
fn audio_save_event_failure_leaves_reservation_charged() {
    let store = Store::memory().unwrap();
    populate(&store);
    let reservation = store.reserve_audio(reserve("one")).unwrap();
    store.claim_audio("p", &reservation.id).unwrap();
    let mut command = SaveAudio {
        profile: "p".into(),
        operation: "save".into(),
        event: "bootstrap-p".into(),
        reservation: reservation.id,
        asset: AudioAsset {
            id: "audio".into(),
            cache_key: "key".into(),
            text_hash: hash("猫 and a cat."),
            provider_fingerprint: "fingerprint".into(),
            model: "configured".into(),
            voice: "configured".into(),
            format: "wav".into(),
            blob_key: "digest.wav".into(),
            duration: Some(1.),
            chars: reservation.chars,
        },
        now: 2,
    };
    assert!(store.save_audio(command.clone()).is_err());
    assert_eq!(
        store
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM audio_assets", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        store.reserve_audio(reserve("two")).unwrap_err().code,
        ErrorCode::QuotaExceeded
    );
    command.event = "saved".into();
    store.save_audio(command).unwrap();
}
