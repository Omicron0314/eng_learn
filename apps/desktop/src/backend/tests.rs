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
        let backend = DesktopBackend::open(directory.path()).unwrap();
        assert!(DesktopBackend::open(directory.path()).is_err());
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
            Response::Settings {
                has_session_key: true,
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
    let reopened = DesktopBackend::open(directory.path()).unwrap();
    assert!(
        matches!(reopened.execute(Command::LoadSettings).unwrap(),Response::Settings{value,has_session_key:false,..} if value.daily_new_limit==7)
    );
}
