use super::*;
fn populate(store: &Store) {
    store.create_profile("p", "test", 0).unwrap();
    let text = "猫 and a cat.";
    store.connection.lock().unwrap().execute("INSERT INTO articles VALUES ('a','p','Original',?1,?2,'\"story\"','\"easy\"','article.v1','fixture','fixture','generated',NULL,'{}','v1','v1',0)",params![text,hash(text)]).unwrap();
}
fn local(op: &str, blob: &str, voice: &str) -> SaveLocalAudio {
    SaveLocalAudio {
        profile: "p".into(),
        operation: op.into(),
        event: format!("event-{op}"),
        article: "a".into(),
        asset: AudioAsset {
            id: format!("audio-{op}"),
            cache_key: format!("key-{voice}"),
            text_hash: hash("猫 and a cat."),
            provider_fingerprint: "kokoro-fingerprint".into(),
            model: "kokoro-v1.0-q8f16-test".into(),
            voice: voice.into(),
            format: "wav".into(),
            blob_key: format!("{blob}.wav"),
            duration: Some(1.),
            chars: "猫 and a cat.".chars().count() as u64,
        },
        now: 2,
    }
}
const BLOB: &str = "8f4d6b1c8f4d6b1c8f4d6b1c8f4d6b1c8f4d6b1c8f4d6b1c8f4d6b1c8f4d6b1c";

#[test]
fn local_audio_publishes_asset_and_link_without_a_reservation() {
    let store = Store::memory().unwrap();
    populate(&store);
    let saved = store
        .save_local_audio(local("local-us", BLOB, "af_heart"))
        .unwrap();
    assert_eq!(saved.voice, "af_heart");
    assert_eq!(saved.chars, "猫 and a cat.".chars().count() as u64);
    assert_eq!(
        store.article_audio_assets("p", "a").unwrap().len(),
        1,
        "the article link is written in the same transaction as the asset"
    );
    assert!(
        store
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM audio_reservations", [], |r| r
                .get::<_, i64>(0))
            .unwrap()
            == 0,
        "local synthesis never charges the remote character quota"
    );
    let cached = store.audio_asset("p", "key-af_heart").unwrap().unwrap();
    assert_eq!(cached.id, saved.id);
    assert_eq!(cached.blob_key, saved.blob_key);
}

#[test]
fn each_accent_caches_separately_and_reuses_its_own_asset() {
    let store = Store::memory().unwrap();
    populate(&store);
    let us = store
        .save_local_audio(local("local-us", BLOB, "af_heart"))
        .unwrap();
    let uk = store
        .save_local_audio(local("local-uk", BLOB, "bf_emma"))
        .unwrap();
    assert_ne!(us.cache_key, uk.cache_key);
    assert_eq!(store.article_audio_assets("p", "a").unwrap().len(), 2);
    let again = store
        .save_local_audio(local("local-us-again", BLOB, "af_heart"))
        .unwrap();
    assert_eq!(
        again.id, us.id,
        "an existing accent reuses its published asset"
    );
    assert_eq!(store.article_audio_assets("p", "a").unwrap().len(), 2);
}

#[test]
fn stale_text_bad_metadata_and_event_failure_publish_nothing() {
    let store = Store::memory().unwrap();
    populate(&store);
    let mut stale = local("local-stale", BLOB, "af_heart");
    stale.asset.text_hash = hash("different body");
    assert_eq!(
        store.save_local_audio(stale).unwrap_err().code,
        ErrorCode::StaleState
    );
    for blob in ["digest.wav", "short.wav", &BLOB.replace(".", "")] {
        let mut broken = local("local-broken", BLOB, "af_heart");
        broken.asset.blob_key = blob.into();
        assert!(store.save_local_audio(broken).is_err());
    }
    let mut no_duration = local("local-no-duration", BLOB, "af_heart");
    no_duration.asset.duration = None;
    assert!(store.save_local_audio(no_duration).is_err());
    let mut failing = local("local-event-failure", BLOB, "af_heart");
    failing.event = "bootstrap-p".into();
    assert!(store.save_local_audio(failing).is_err());
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
}

#[test]
fn every_supported_playback_rate_opens_a_session() {
    let store = Store::memory().unwrap();
    populate(&store);
    for (index, rate) in cijian_domain::settings::PLAYBACK_RATES
        .into_iter()
        .enumerate()
    {
        store
            .open_audio_session(AudioSessionCommand {
                profile: "p".into(),
                operation: format!("system-{index}"),
                event: format!("system-event-{index}"),
                session: format!("system-session-{index}"),
                article: "a".into(),
                audio: None,
                source: "system".into(),
                rate,
                now: 1,
            })
            .unwrap_or_else(|error| panic!("rate {rate} rejected: {error:?}"));
    }
    assert_eq!(
        store
            .open_audio_session(AudioSessionCommand {
                profile: "p".into(),
                operation: "unsupported".into(),
                event: "unsupported-event".into(),
                session: "unsupported-session".into(),
                article: "a".into(),
                audio: None,
                source: "system".into(),
                rate: 9.,
                now: 1,
            })
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
}
