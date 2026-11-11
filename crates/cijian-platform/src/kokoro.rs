//! Local Kokoro worker. The installed helper runs offline in its own process;
//! full WAV publication and cache ownership belong to the application layer.
use cijian_domain::audio::Accent;
use cijian_ports::{AppError, ErrorCode, Result};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub const MODEL: &str = "kokoro-v1.0-q8f16-04c658ae-helper-v1";
pub fn voice(accent: Accent) -> &'static str {
    match accent {
        Accent::Us => "af_heart",
        Accent::Uk => "bf_emma",
    }
}
pub fn installation() -> Result<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .ok_or_else(unavailable)?;
    Ok(installation_at(&base))
}
pub fn installation_at(data_home: &Path) -> PathBuf {
    data_home.join("cijian/kokoro")
}
fn unavailable() -> AppError {
    AppError::new(
        ErrorCode::UnsupportedCapability,
        "本地朗读模型未安装或运行失败，请运行 tools/install-kokoro.py",
    )
}
pub fn synthesize(
    text: &str,
    accent: Accent,
    path: &Path,
    cancelled: impl Fn() -> bool,
) -> Result<()> {
    if text.trim().is_empty() || text.len() > 1024 * 1024 {
        return Err(AppError::new(ErrorCode::InvalidInput, "朗读正文为空或过长"));
    }
    synthesize_at(&installation()?, text, accent, path, cancelled)
}
pub fn synthesize_at(
    root: &Path,
    text: &str,
    accent: Accent,
    path: &Path,
    cancelled: impl Fn() -> bool,
) -> Result<()> {
    let mut child = Command::new(root.join("venv/bin/python"))
        .arg("-c")
        .arg(include_str!("kokoro.py"))
        .arg(root.join("assets"))
        .arg(voice(accent))
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| unavailable())?;
    let mut stdin = child.stdin.take().ok_or_else(unavailable)?;
    let text = text.to_owned();
    let writer = std::thread::spawn(move || stdin.write_all(text.as_bytes()));
    let deadline = Instant::now() + Duration::from_secs(1800);
    let result = loop {
        if cancelled() {
            break Err(AppError::new(ErrorCode::Cancelled, "朗读生成已取消"));
        }
        if Instant::now() >= deadline {
            break Err(AppError::new(
                ErrorCode::ProviderError,
                "本地朗读生成超时，可重试",
            ));
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    Ok(())
                } else {
                    Err(unavailable())
                };
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => break Err(unavailable()),
        }
    };
    if result.is_err() {
        let _ = child.kill();
    }
    let _ = child.wait();
    let written = writer
        .join()
        .map_err(|_| unavailable())?
        .map_err(|_| unavailable());
    result?;
    written
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accents_map_to_their_pinned_voices_and_share_one_install_path() {
        assert_eq!(voice(Accent::Us), "af_heart");
        assert_eq!(voice(Accent::Uk), "bf_emma");
        assert!(MODEL.starts_with("kokoro-v1.0-q8f16-"));
        assert_eq!(
            installation_at(Path::new("/data")),
            PathBuf::from("/data/cijian/kokoro")
        );
    }
    #[test]
    fn a_missing_worker_reports_an_unsupported_capability_without_writing_a_file() {
        let folder = tempfile::tempdir().unwrap();
        let output = folder.path().join("out.wav");
        let error = synthesize_at(
            folder.path(),
            "An original sentence.",
            Accent::Us,
            &output,
            || false,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::UnsupportedCapability);
        assert!(!output.exists());
    }
    #[test]
    #[ignore = "requires `python3 tools/install-kokoro.py` on this machine"]
    fn installed_model_synthesizes_both_accents_as_decodable_wav() {
        let folder = tempfile::tempdir().unwrap();
        for accent in [Accent::Us, Accent::Uk] {
            let path = folder.path().join(format!("{}.wav", accent.key()));
            synthesize(
                "This original sentence tests offline English speech.",
                accent,
                &path,
                || false,
            )
            .unwrap();
            let duration = crate::audio::validate_audio(std::fs::read(&path).unwrap())
                .unwrap()
                .expect("duration");
            assert!(duration > 0.5, "{accent:?} produced {duration}s");
        }
    }
    #[test]
    fn an_already_cancelled_request_never_spawns_the_worker() {
        let folder = tempfile::tempdir().unwrap();
        assert_eq!(
            synthesize(
                "An original sentence.",
                Accent::Uk,
                &folder.path().join("out.wav"),
                || true
            )
            .unwrap_err()
            .code,
            ErrorCode::Cancelled
        );
    }
}
