use super::*;
fn wave(samples: u32) -> Vec<u8> {
    let data_size = samples * 2;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&16000_u32.to_le_bytes());
    bytes.extend_from_slice(&32000_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_size.to_le_bytes());
    for sample in 0..samples {
        let value = ((sample as f64 * 440. * std::f64::consts::TAU / 16000.).sin() * 500.) as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}
#[test]
fn validates_original_wave_without_opening_a_device() {
    assert_eq!(validate_audio(wave(16000)).unwrap(), Some(1.));
    assert!(validate_audio(b"not audio".to_vec()).is_err());
}
#[test]
#[ignore = "requires an available native audio device; plays a quiet original tone"]
fn native_audio_controls_and_natural_completion() {
    let temp = tempfile::tempdir().unwrap();
    let long = temp.path().join("tone.wav");
    std::fs::write(&long, wave(32000)).unwrap();
    let completed = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let recorded = completed.clone();
    let mut observed_at = None;
    let player = NativeAudio::with_completion(move |state, at| {
        assert_eq!(state.session.as_deref(), Some("second"));
        assert!(at > 0);
        if let Some(first) = observed_at {
            assert_eq!(first, at);
        } else {
            observed_at = Some(at);
        }
        if recorded.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            return Err(AppError::new(
                ErrorCode::StorageUnavailable,
                "original transient test failure",
            ));
        }
        Ok(())
    });
    let state = player
        .command(AudioCommand::Play {
            path: long,
            session: "first".into(),
            article: "original".into(),
            rate: 1.,
        })
        .unwrap();
    assert_eq!(state.state, PlaybackState::Playing);
    player.command(AudioCommand::Seek(0.5)).unwrap();
    assert_eq!(
        player.command(AudioCommand::Pause).unwrap().state,
        PlaybackState::Paused
    );
    assert_eq!(player.command(AudioCommand::Rate(1.25)).unwrap().rate, 1.25);
    player.command(AudioCommand::Resume).unwrap();
    assert_eq!(
        player.command(AudioCommand::Stop).unwrap().state,
        PlaybackState::Idle
    );
    let short = temp.path().join("short.wav");
    std::fs::write(&short, wave(1600)).unwrap();
    player
        .command(AudioCommand::Play {
            path: short,
            session: "second".into(),
            article: "original".into(),
            rate: 1.,
        })
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        if completed.load(std::sync::atomic::Ordering::SeqCst) < 2 {
            assert!(
                std::time::Instant::now() < deadline,
                "completion callback did not retry without polling"
            );
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        let state = player.command(AudioCommand::Status).unwrap();
        if state.state == PlaybackState::Ended {
            assert_eq!(state.session.as_deref(), Some("second"));
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "natural completion timed out"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
#[ignore = "requires an available native audio device; plays quiet original tones"]
fn changing_rate_keeps_the_position_and_words_play_at_one_speed() {
    let temp = tempfile::tempdir().unwrap();
    let tone = temp.path().join("tone.wav");
    std::fs::write(&tone, wave(32000)).unwrap();
    let player = NativeAudio::new();
    let playing = player
        .command(AudioCommand::Play {
            path: tone.clone(),
            session: "rate".into(),
            article: "original".into(),
            rate: 1.,
        })
        .unwrap();
    assert_eq!(playing.duration, Some(2.));
    assert_eq!(playing.rate, 1.);
    // The article really plays faster, but the reported timeline stays the
    // article's own length so the progress bar never rescales.
    let faster = player.command(AudioCommand::Rate(1.25)).unwrap();
    assert_eq!(faster.rate, 1.25);
    assert_eq!(faster.duration, Some(2.));
    let slower = player.command(AudioCommand::Rate(0.75)).unwrap();
    assert_eq!(slower.duration, Some(2.));
    // Position stays continuous across a rate change instead of restarting.
    player.command(AudioCommand::Seek(0.8)).unwrap();
    std::thread::sleep(Duration::from_millis(120));
    let before = player.command(AudioCommand::Status).unwrap().position;
    let after = player.command(AudioCommand::Rate(1.5)).unwrap();
    assert!(
        (after.position - before).abs() < 0.06,
        "position jumped from {before}s to {}s",
        after.position
    );
    assert_eq!(after.duration, Some(2.));
    // 2s of article left at 1.5x finishes in about 0.8s of wall time.
    let started = std::time::Instant::now();
    loop {
        if player.command(AudioCommand::Status).unwrap().state == PlaybackState::Ended {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "1.5x playback did not finish"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "1.5x from 0.8s should finish quickly, took {:?}",
        started.elapsed()
    );
    player.command(AudioCommand::Stop).unwrap();
    // A word recording never joins the article timeline, so article state and
    // rate stay exactly as they were; the word plays at its recorded speed.
    let stopped = player.command(AudioCommand::Status).unwrap();
    player
        .command(AudioCommand::PlayWord { path: tone })
        .unwrap();
    let after_word = player.command(AudioCommand::Status).unwrap();
    assert_eq!(after_word.state, PlaybackState::Idle);
    assert_eq!(after_word.session, stopped.session);
    assert_eq!(after_word.rate, stopped.rate);
}
