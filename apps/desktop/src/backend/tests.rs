use super::*;
use cijian_domain::{
    provider::ProviderConfig,
    settings::{Genre, Tier},
};
use cijian_ports::provider::{Secret, SecretPatch};
#[test]
fn offline_startup_and_settings_survive_reopen_without_network() {
    let directory = tempfile::tempdir().unwrap();
    {
        let backend = DesktopBackend::open_for_tests(directory.path()).unwrap();
        assert!(DesktopBackend::open_for_tests(directory.path()).is_err());
        assert!(
            matches!(backend.execute(Command::LoadReview).unwrap(),Response::Review{cards,..} if cards.is_empty())
        );
        assert_eq!(
            backend
                .execute(Command::GenerateArticle {
                    tier: Tier::Easy,
                    genre: Genre::Auto,
                    operation: "missing-config".into()
                })
                .unwrap_err()
                .code,
            ErrorCode::ConfigMissing
        );
        let Response::Settings {
            mut value,
            revision,
            ..
        } = backend.execute(Command::LoadSettings).unwrap()
        else {
            panic!("settings response")
        };
        value.daily_new_limit = 7;
        backend
            .execute(Command::SaveSettings {
                value,
                revision,
                operation: "settings".into(),
            })
            .unwrap();
        backend
            .execute(Command::SetTheme {
                value: cijian_domain::settings::Theme::Dark,
            })
            .unwrap();
        backend
            .execute(Command::SetPlaybackRate { rate: 1.25 })
            .unwrap();
        backend
            .execute(Command::SetPlaybackRate { rate: 2. })
            .unwrap();
        let Response::Settings {
            mut value,
            revision,
            ..
        } = backend.execute(Command::LoadSettings).unwrap()
        else {
            panic!("settings");
        };
        value.request_retention = 0.91;
        assert!(matches!(
            backend
                .execute(Command::SaveSettings {
                    value,
                    revision,
                    operation: "learning-at-double-speed".into()
                })
                .unwrap(),
            Response::LearningSettingsSaved { .. }
        ));
        backend
            .execute(Command::SetPlaybackRate { rate: 1.25 })
            .unwrap();
        let config = ProviderConfig {
            kind: ProviderKind::Llm,
            endpoint: "https://example.invalid/v1".into(),
            model: "configured".into(),
            voice: None,
            revision: 1,
        };
        let response = backend
            .execute(Command::SaveProvider {
                config,
                revision: 0,
                secret: SecretPatch::Replace(Secret::new("original-test-secret".into()).unwrap()),
                operation: "provider".into(),
            })
            .unwrap();
        assert!(matches!(
            response,
            Response::ProviderSaved {
                has_key: true,
                source: cijian_ports::provider::SecretSource::Session,
                ..
            }
        ));
        assert_eq!(
            backend
                .execute(Command::GenerateArticle {
                    tier: Tier::Easy,
                    genre: Genre::Auto,
                    operation: "no-targets".into()
                })
                .unwrap_err()
                .code,
            ErrorCode::NoTargets
        );
    }
    let reopened = DesktopBackend::open_for_tests(directory.path()).unwrap();
    assert!(
        matches!(reopened.execute(Command::LoadSettings).unwrap(),Response::Settings{value,has_session_key:false,source:cijian_ports::provider::SecretSource::Session,..} if value.daily_new_limit==7 && value.request_retention==0.91 && value.theme==cijian_domain::settings::Theme::Dark && value.playback_rate==1.25)
    );
}
#[test]
fn switching_accent_stops_playback_and_persists_the_choice() {
    let directory = tempfile::tempdir().unwrap();
    let backend = DesktopBackend::open_for_tests(directory.path()).unwrap();
    assert_eq!(
        backend
            .store
            .device_preferences(&backend.profile, &backend.device)
            .unwrap()
            .0
            .pronunciation_accent,
        cijian_domain::audio::Accent::Us
    );
    let _ = backend.execute(Command::AudioStatus);
    backend
        .execute(Command::SetPronunciationAccent {
            accent: cijian_domain::audio::Accent::Uk,
        })
        .unwrap();
    // British audio must never reuse the American cache key: the article asset
    // query is per accent voice, and the playback that was using the old voice
    // is stopped instead of continuing on a stale file.
    assert_eq!(
        backend
            .store
            .device_preferences(&backend.profile, &backend.device)
            .unwrap()
            .0
            .pronunciation_accent,
        cijian_domain::audio::Accent::Uk
    );
    assert!(matches!(
        backend.execute(Command::AudioStatus).unwrap(),
        Response::Audio(state) if state.state == cijian_domain::audio::PlaybackState::Idle
    ));
    drop(backend);
    let reopened = DesktopBackend::open_for_tests(directory.path()).unwrap();
    assert_eq!(
        reopened
            .store
            .device_preferences(&reopened.profile, &reopened.device)
            .unwrap()
            .0
            .pronunciation_accent,
        cijian_domain::audio::Accent::Uk,
        "the accent is a device preference that survives reopen"
    );
    // A fresh window asks for the preferences, so the accent chips show the
    // voice the backend will actually generate instead of the built-in default.
    assert!(
        matches!(
            reopened.execute(Command::LoadPreferences).unwrap(),
            Response::Preferences(value)
                if value.pronunciation_accent == cijian_domain::audio::Accent::Uk
        ),
        "loading preferences reports the stored accent"
    );
}
