use cijian_ports::audio::SystemVoice;
use cijian_ports::{AppError, ErrorCode, Result};
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
fn unavailable() -> AppError {
    AppError::new(
        ErrorCode::UnsupportedCapability,
        "没有可用的本机英语朗读引擎",
    )
}
pub fn voices() -> Result<Vec<SystemVoice>> {
    if !cfg!(target_os = "linux") {
        return Err(unavailable());
    }
    let output = Command::new("espeak-ng")
        .arg("--voices")
        .output()
        .map_err(|_| unavailable())?;
    if !output.status.success() {
        return Err(unavailable());
    }
    let voices = String::from_utf8_lossy(&output.stdout)
        .lines()
        .skip(1)
        .filter_map(|line| {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            if fields.len() < 5 || !(fields[1] == "en" || fields[1].starts_with("en-")) {
                return None;
            }
            Some(SystemVoice {
                id: fields[4].into(),
                label: fields[3].replace('_', " "),
                language: fields[1].into(),
            })
        })
        .collect::<Vec<_>>();
    if voices.is_empty() {
        Err(unavailable())
    } else {
        Ok(voices)
    }
}
pub fn synthesize(text: &str, voice: &str, path: &Path) -> Result<()> {
    if text.trim().is_empty() || text.len() > 1024 * 1024 {
        return Err(AppError::new(
            ErrorCode::InvalidInput,
            "本机朗读正文为空或过长",
        ));
    }
    if !voices()?.iter().any(|available| available.id == voice) {
        return Err(unavailable());
    }
    let mut child = Command::new("espeak-ng")
        .args(["-v", voice, "--stdin", "-w"])
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| unavailable())?;
    let mut stdin = child.stdin.take().ok_or_else(unavailable)?;
    let text = text.to_owned();
    let writer = std::thread::spawn(move || stdin.write_all(text.as_bytes()));
    let deadline = Instant::now() + Duration::from_secs(30);
    let result = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    Ok(())
                } else {
                    Err(unavailable())
                };
            }
            Ok(None) => {}
            Err(_) => break Err(unavailable()),
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            break Err(AppError::new(
                ErrorCode::UnsupportedCapability,
                "本机英语合成超时",
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
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
    #[ignore = "requires an installed native English voice"]
    fn native_english_voice_generates_decodable_original_speech() {
        let voice = voices().unwrap().remove(0);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("original.wav");
        synthesize(
            "This original sentence tests offline English speech.",
            &voice.id,
            &path,
        )
        .unwrap();
        assert!(
            crate::audio::validate_audio(std::fs::read(path).unwrap())
                .unwrap()
                .unwrap()
                > 0.
        );
    }
}
