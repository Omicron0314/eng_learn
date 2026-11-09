//! Fills missing word recordings into a local learning directory.
//!
//! This is the maintainer-side half of word audio: packs ship with recordings, and
//! this tool produces them for a directory that was imported from a plain word list.
//! It is not part of the running app and never runs while the app holds the directory.
use cijian_domain::audio::Accent;
use cijian_ports::library::SaveWordAudio;
use cijian_storage::Store;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().try_into().unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn usage() -> ! {
    eprintln!(
        "用法：cijian-fill --data-dir 目录 [--limit N] [--interval 毫秒] [--dry-run]\n\
         关闭词间后再运行；它会为缺少发音的词卡补齐英式与美式录音。"
    );
    std::process::exit(2)
}

struct Options {
    directory: PathBuf,
    limit: usize,
    interval: Duration,
    dry_run: bool,
}

fn parse() -> Options {
    let mut directory = None;
    let mut limit = usize::MAX;
    let mut interval = Duration::from_millis(250);
    let mut dry_run = false;
    let mut arguments = std::env::args().skip(1);
    while let Some(flag) = arguments.next() {
        match flag.as_str() {
            "--data-dir" => directory = arguments.next().map(PathBuf::from),
            "--limit" => {
                limit = arguments
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or_else(|| usage())
            }
            "--interval" => {
                interval = Duration::from_millis(
                    arguments
                        .next()
                        .and_then(|value| value.parse().ok())
                        .unwrap_or_else(|| usage()),
                )
            }
            "--dry-run" => dry_run = true,
            _ => usage(),
        }
    }
    Options {
        directory: directory.unwrap_or_else(|| usage()),
        limit,
        interval,
        dry_run,
    }
}

/// Content-addressed write: the file name is the digest of its own bytes, so a
/// repeated fetch of the same word costs no extra disk and needs no cleanup.
fn write_blob(directory: &Path, bytes: &[u8], format: &str) -> Result<String, String> {
    let key = format!("{:x}.{format}", Sha256::digest(bytes));
    let audio = directory.join("audio");
    std::fs::create_dir_all(&audio).map_err(|_| "无法创建音频目录".to_owned())?;
    let path = audio.join(&key);
    if path.exists() {
        return Ok(key);
    }
    let mut temporary =
        tempfile::NamedTempFile::new_in(&audio).map_err(|_| "无法写入音频".to_owned())?;
    std::io::Write::write_all(&mut temporary, bytes).map_err(|_| "无法写入音频".to_owned())?;
    match temporary.persist_noclobber(&path) {
        Ok(_) => Ok(key),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(key),
        Err(_) => Err("无法保存音频".to_owned()),
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let options = parse();
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(options.directory.join("instance.lock"))
        .unwrap_or_else(|_| {
            eprintln!("无法打开学习目录：{}", options.directory.display());
            std::process::exit(1)
        });
    if lock.try_lock().is_err() {
        eprintln!("该学习目录正被词间使用，请先关闭应用");
        std::process::exit(1);
    }
    let store = match Store::open(options.directory.join("cijian.sqlite")) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("无法打开学习库：{}", error.message);
            std::process::exit(1);
        }
    };
    let profile = match store.first_profile() {
        Ok(Some(profile)) => profile,
        Ok(None) => {
            eprintln!("学习库还没有档案，请先在应用内导入词书");
            std::process::exit(1);
        }
        Err(error) => {
            eprintln!("无法读取学习档案：{}", error.message);
            std::process::exit(1);
        }
    };
    let words = match store.words_missing_audio(&profile) {
        Ok(words) => words,
        Err(error) => {
            eprintln!("无法读取待补词表：{}", error.message);
            std::process::exit(1);
        }
    };
    println!("待补发音的词：{} 个", words.len());
    if options.dry_run {
        for (_, lemma) in words.iter().take(20) {
            println!("  {lemma}");
        }
        return;
    }
    let http = match cijian_providers::NativeHttp::new() {
        Ok(http) => http,
        Err(error) => {
            eprintln!("无法初始化网络连接：{}", error.message);
            std::process::exit(1);
        }
    };
    let mut filled = 0;
    let mut failed = Vec::new();
    for (lexeme, lemma) in words.iter().take(options.limit) {
        let started = Instant::now();
        match http.word_entry(lemma).await {
            Ok(entry) => {
                let mut written = Vec::new();
                for (accent, bytes) in [(Accent::Uk, &entry.uk), (Accent::Us, &entry.us)] {
                    let format = cijian_providers::youdao::audio_format(bytes)
                        .expect("validated by word_entry");
                    match write_blob(&options.directory, bytes, format) {
                        Ok(blob_key) => written.push((accent, blob_key, bytes.len(), format)),
                        Err(message) => {
                            failed.push(format!("{lemma}：{message}"));
                            continue;
                        }
                    }
                }
                for (accent, blob_key, bytes, format) in written {
                    let saved = store.save_word_audio(SaveWordAudio {
                        profile_id: profile.clone(),
                        lexeme_id: lexeme.clone(),
                        accent,
                        blob_key,
                        format: format.to_owned(),
                        bytes: bytes as u64,
                        duration: None,
                        source: "youdao".into(),
                        ipa_uk: Some(entry.ipa_uk.clone()),
                        ipa_us: Some(entry.ipa_us.clone()),
                        now: now(),
                    });
                    match saved {
                        Ok(()) => filled += 1,
                        Err(error) => failed.push(format!("{lemma}：{}", error.message)),
                    }
                }
                println!("  ✓ {lemma}  {} {}", entry.ipa_uk, entry.ipa_us);
            }
            Err(error) => failed.push(format!("{lemma}：{}", error.message)),
        }
        if let Some(rest) = options.interval.checked_sub(started.elapsed()) {
            tokio::time::sleep(rest).await;
        }
    }
    println!("已保存 {filled} 条发音");
    if !failed.is_empty() {
        println!("失败 {} 条：", failed.len());
        for line in failed.iter().take(50) {
            println!("  {line}");
        }
    }
    if filled == 0 && !failed.is_empty() {
        std::process::exit(1);
    }
}
