//! Maintainer tool for word recordings and `.cijianpack` word packs.
//!
//! Nothing here runs inside the app: the app only reads what is already in the
//! learning directory or imports a pack the user downloaded. Every command takes
//! the exclusive directory lock, so the app must be closed while this runs.
use cijian_domain::{
    audio::Accent,
    import::{BookKind, ImportRow, ParsedImport},
    pack::Manifest,
};
use cijian_ports::library::{ImportIds, SaveWordAudio};
use cijian_storage::Store;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
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
        "用法：\n  \
         cijian-fill fill --data-dir 目录 [--limit N] [--interval 毫秒] [--dry-run]\n  \
         cijian-fill pack --data-dir 目录 --output 词包.cijianpack [--limit N] [--dry-run]\n  \
         cijian-fill import-pack --data-dir 目录 词包.cijianpack [--name 词书名称]\n\n\
         运行前请关闭词间：命令会独占学习目录。"
    );
    std::process::exit(2)
}

struct Options {
    command: String,
    directory: PathBuf,
    output: Option<PathBuf>,
    input: Option<PathBuf>,
    name: Option<String>,
    limit: usize,
    interval: Duration,
    dry_run: bool,
}

fn value(arguments: &mut impl Iterator<Item = String>) -> String {
    arguments.next().unwrap_or_else(|| usage())
}

fn parse() -> Options {
    let mut arguments = std::env::args().skip(1);
    let command = arguments.next().unwrap_or_else(|| usage());
    let mut directory = None;
    let mut output = None;
    let mut input = None;
    let mut name = None;
    let mut limit = usize::MAX;
    let mut interval = Duration::from_millis(250);
    let mut dry_run = false;
    while let Some(flag) = arguments.next() {
        match flag.as_str() {
            "--data-dir" => directory = Some(PathBuf::from(value(&mut arguments))),
            "--output" => output = Some(PathBuf::from(value(&mut arguments))),
            "--name" => name = Some(value(&mut arguments)),
            "--limit" => limit = value(&mut arguments).parse().unwrap_or_else(|_| usage()),
            "--interval" => {
                interval =
                    Duration::from_millis(value(&mut arguments).parse().unwrap_or_else(|_| usage()))
            }
            "--dry-run" => dry_run = true,
            other => {
                if input.is_none() && !other.starts_with("--") {
                    input = Some(PathBuf::from(other));
                } else {
                    usage();
                }
            }
        }
    }
    Options {
        command,
        directory: directory.unwrap_or_else(|| usage()),
        output,
        input,
        name,
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

fn read_blob(directory: &Path, key: &str) -> Result<Vec<u8>, String> {
    std::fs::read(directory.join("audio").join(key)).map_err(|_| format!("缺少音频文件 {key}"))
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
}

fn main() {
    let options = parse();
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(options.directory.join("instance.lock"))
        .unwrap_or_else(|_| {
            fail(&format!(
                "无法打开学习目录：{}",
                options.directory.display()
            ))
        });
    if lock.try_lock().is_err() {
        fail("该学习目录正被词间使用，请先关闭应用");
    }
    let store = Store::open(options.directory.join("cijian.sqlite"))
        .unwrap_or_else(|error| fail(&format!("无法打开学习库：{}", error.message)));
    if options.command == "import-pack" {
        import_pack(&store, &options);
        return;
    }
    let profile = match store.first_profile() {
        Ok(Some(profile)) => profile,
        Ok(None) => fail("学习库还没有档案，请先在应用内导入词书"),
        Err(error) => fail(&format!("无法读取学习档案：{}", error.message)),
    };
    match options.command.as_str() {
        "fill" => {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap_or_else(|_| fail("无法初始化网络运行时"));
            runtime.block_on(fill(&store, &profile, &options));
        }
        "pack" => {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap_or_else(|_| fail("无法初始化网络运行时"));
            runtime.block_on(fill(&store, &profile, &options));
            write_pack(&store, &profile, &options);
        }
        _ => usage(),
    }
}

async fn fill(store: &Store, profile: &str, options: &Options) {
    let words = match store.words_missing_audio(profile) {
        Ok(words) => words,
        Err(error) => fail(&format!("无法读取待补词表：{}", error.message)),
    };
    println!("待补发音的词：{} 个", words.len());
    if options.dry_run {
        for (_, lemma) in words.iter().take(20) {
            println!("  {lemma}");
        }
        return;
    }
    let http = cijian_providers::NativeHttp::new()
        .unwrap_or_else(|error| fail(&format!("无法初始化网络连接：{}", error.message)));
    let mut filled = 0;
    let mut failed = Vec::new();
    for (lexeme, lemma) in words.iter().take(options.limit) {
        let started = Instant::now();
        match http.word_entry(lemma).await {
            Ok(entry) => {
                let mut batch = Vec::new();
                for (accent, bytes) in [(Accent::Uk, &entry.uk), (Accent::Us, &entry.us)] {
                    let format =
                        cijian_providers::youdao::audio_format(bytes).expect("validated by entry");
                    match write_blob(&options.directory, bytes, format) {
                        Ok(blob_key) => batch.push(SaveWordAudio {
                            profile_id: profile.to_owned(),
                            lexeme_id: lexeme.clone(),
                            accent,
                            blob_key,
                            format: format.to_owned(),
                            bytes: bytes.len() as u64,
                            duration: None,
                            source: "youdao".into(),
                            ipa_uk: Some(entry.ipa_uk.clone()),
                            ipa_us: Some(entry.ipa_us.clone()),
                            now: now(),
                        }),
                        Err(message) => failed.push(format!("{lemma}：{message}")),
                    }
                }
                if let Err(error) = store.attach_word_audio(batch) {
                    failed.push(format!("{lemma}：{}", error.message));
                } else {
                    filled += 1;
                    println!("  ✓ {lemma}  {} {}", entry.ipa_uk, entry.ipa_us);
                }
            }
            Err(error) => failed.push(format!("{lemma}：{}", error.message)),
        }
        if let Some(rest) = options.interval.checked_sub(started.elapsed()) {
            tokio::time::sleep(rest).await;
        }
    }
    println!("已补齐 {filled} 个词");
    if !failed.is_empty() {
        println!("失败 {} 个：", failed.len());
        for line in failed.iter().take(50) {
            println!("  {line}");
        }
    }
}

fn write_pack(store: &Store, profile: &str, options: &Options) {
    let Some(output) = &options.output else {
        usage();
    };
    let rows = match store.pack_rows(profile) {
        Ok(rows) => rows,
        Err(error) => fail(&format!("无法读取词书内容：{}", error.message)),
    };
    if options.dry_run {
        println!("将打包 {} 个词到 {}", rows.len(), output.display());
        return;
    }
    let mut blobs = BTreeMap::new();
    for row in &rows {
        for key in [&row.uk, &row.us] {
            if key.is_empty() || blobs.contains_key(key) {
                continue;
            }
            let bytes = read_blob(&options.directory, key).unwrap_or_else(|message| fail(&message));
            blobs.insert(key.clone(), bytes);
        }
    }
    let name = options
        .name
        .clone()
        .or_else(|| {
            store.books(profile).ok().and_then(|books| {
                books
                    .into_iter()
                    .find(|book| book.kind == BookKind::Study)
                    .map(|book| book.name)
            })
        })
        .unwrap_or_else(|| "词书".into());
    let manifest = Manifest::new(
        name,
        "study".into(),
        "cijian-fill".into(),
        now(),
        rows.len(),
    );
    let audio = blobs
        .iter()
        .map(|(key, bytes)| (key.as_str(), bytes.as_slice()))
        .collect::<Vec<_>>();
    match cijian_pack::write(output, manifest, &rows, &audio) {
        Ok(()) => {
            let size = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);
            println!(
                "已生成词包：{}（{} 个词，{} 条音频，{:.1} MiB）",
                output.display(),
                rows.len(),
                blobs.len(),
                size as f64 / (1024. * 1024.)
            );
        }
        Err(error) => fail(&format!("无法生成词包：{}", error.message)),
    }
}

fn import_pack(store: &Store, options: &Options) {
    let Some(input) = &options.input else {
        usage();
    };
    let pack = match cijian_pack::read(input) {
        Ok(pack) => pack,
        Err(error) => fail(&format!("无法读取词包：{}", error.message)),
    };
    let profile = match store.first_profile() {
        Ok(Some(profile)) => profile,
        Ok(None) => {
            // A brand-new data directory gets the same bootstrap profile the app creates.
            let profile = "import-profile".to_owned();
            if let Err(error) = store.create_profile(&profile, "本机学习", now()) {
                fail(&format!("无法创建学习档案：{}", error.message));
            }
            profile
        }
        Err(error) => fail(&format!("无法读取学习档案：{}", error.message)),
    };
    if options.dry_run {
        println!(
            "词包「{}」包含 {} 个词、{} 条音频",
            pack.manifest.name,
            pack.words.len(),
            pack.audio.len()
        );
        return;
    }
    let mut rows = Vec::new();
    let mut row_ids = Vec::new();
    let mut audios = Vec::new();
    let mut written = BTreeMap::new();
    for (index, word) in pack.words.iter().enumerate() {
        let lexeme = uuid_like(index, "lexeme");
        rows.push(ImportRow {
            word: word.word.clone(),
            raw: format!("{}\t{}", word.word, word.meaning),
            front: word.word.clone(),
            meaning: word.meaning.clone(),
            example: String::new(),
            line: index as u64 + 1,
            phrase: word.word.contains(' '),
        });
        row_ids.push(ImportIds {
            lexeme: lexeme.clone(),
            card: uuid_like(index, "card"),
            entry: uuid_like(index, "entry"),
        });
        for (accent, key) in [(Accent::Uk, &word.uk), (Accent::Us, &word.us)] {
            if key.is_empty() {
                continue;
            }
            let bytes = pack
                .audio
                .get(key)
                .unwrap_or_else(|| fail("词包音频缺失"))
                .clone();
            let format = key.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp3");
            let stored = write_blob(&options.directory, &bytes, format)
                .unwrap_or_else(|message| fail(&message));
            if stored != *key {
                fail("词包音频命名与内容不一致");
            }
            written.insert(key.clone(), bytes.len());
            audios.push(SaveWordAudio {
                profile_id: profile.clone(),
                lexeme_id: lexeme.clone(),
                accent,
                blob_key: key.clone(),
                format: key
                    .rsplit_once('.')
                    .map(|(_, e)| e)
                    .unwrap_or("mp3")
                    .to_owned(),
                bytes: bytes.len() as u64,
                duration: None,
                source: "pack".into(),
                ipa_uk: Some(word.ipa_uk.clone()),
                ipa_us: Some(word.ipa_us.clone()),
                now: now(),
            });
        }
    }
    let parsed = ParsedImport {
        parsed: rows.len() as u64,
        phrases: rows.iter().filter(|row| row.phrase).count() as u64,
        duplicates: 0,
        skipped: 0,
        errors: Vec::new(),
        rows,
    };
    let kind = if pack.manifest.kind == "baseline" {
        BookKind::Baseline
    } else {
        BookKind::Study
    };
    let name = options
        .name
        .clone()
        .unwrap_or_else(|| pack.manifest.name.clone());
    let imported = store.import(cijian_ports::library::ImportWords {
        profile_id: profile.clone(),
        operation_id: format!("pack-{}", pack.manifest.created_at),
        event_id: uuid_like(0, "pack-event"),
        book_id: uuid_like(0, "pack-book"),
        import_id: uuid_like(0, "pack-import"),
        name: name.clone(),
        kind,
        source_name: input
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("词包")
            .into(),
        source_hash: format!(
            "{:x}",
            Sha256::digest(std::fs::read(input).unwrap_or_default())
        ),
        format: "cijianpack".into(),
        now: now(),
        parsed,
        row_ids,
    });
    match imported {
        Ok(report) => println!(
            "已导入「{name}」：{} 个词，{} 个新卡",
            report.words_added, report.cards_added
        ),
        Err(error) => fail(&format!("导入词书失败：{}", error.message)),
    }
    match store.attach_word_audio(audios) {
        Ok(()) => println!("已写入 {} 条发音", written.len()),
        Err(error) => fail(&format!("写入发音失败：{}", error.message)),
    }
}

/// Deterministic identifiers keep a re-import of the same pack idempotent.
fn uuid_like(index: usize, kind: &str) -> String {
    let digest = Sha256::digest(format!("{kind}-{index}"));
    format!("{kind}-{:x}", digest)[..kind.len() + 17].to_owned()
}
