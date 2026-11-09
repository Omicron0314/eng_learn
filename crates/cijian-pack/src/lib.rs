//! `.cijianpack` archive adapter: one manifest, one word list, and the recordings
//! those rows point at. Reading verifies every claim in the archive before the
//! caller touches the store, so a truncated or tampered pack imports nothing.
use cijian_domain::pack::{
    Manifest, PackWord, header, max_audio_bytes, parse_line, valid_blob_key,
};
use cijian_ports::{AppError, ErrorCode, Result};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;

const MAX_WORDS_NAME: &str = "words.tsv";
const MAX_MANIFEST_BYTES: u64 = 256 * 1024;
const MAX_WORDS_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ENTRIES: usize = 200_000;

fn invalid(message: &str) -> AppError {
    AppError::new(ErrorCode::InvalidInput, message)
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Debug, Clone)]
pub struct Pack {
    pub manifest: Manifest,
    pub words: Vec<PackWord>,
    /// Recordings keyed by blob key, exactly as the rows reference them.
    pub audio: BTreeMap<String, Vec<u8>>,
}

impl Pack {
    /// Blobs a pack must write, deduplicated and ordered.
    pub fn blobs(&self) -> Vec<(&str, &[u8])> {
        self.audio
            .iter()
            .map(|(key, bytes)| (key.as_str(), bytes.as_slice()))
            .collect()
    }
}

/// Reads and fully validates a pack. Nothing is written to disk.
pub fn read(path: &Path) -> Result<Pack> {
    let file = std::fs::File::open(path).map_err(|_| invalid("无法读取词包"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| invalid("词包压缩结构无效"))?;
    if archive.len() > MAX_ENTRIES {
        return Err(invalid("词包条目数量超出上限"));
    }
    let manifest_entry = archive
        .by_name(cijian_domain::pack::MANIFEST_NAME)
        .map_err(|_| invalid("词包缺少清单"))?;
    if manifest_entry.size() > MAX_MANIFEST_BYTES {
        return Err(invalid("词包清单超出上限"));
    }
    drop(manifest_entry);
    let mut manifest_bytes = Vec::new();
    archive
        .by_name(cijian_domain::pack::MANIFEST_NAME)
        .map_err(|_| invalid("词包缺少清单"))?
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut manifest_bytes)
        .map_err(|_| invalid("无法读取词包清单"))?;
    let manifest: Manifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| invalid("词包清单结构无效"))?;
    manifest
        .validate()
        .map_err(|error| invalid(error.message))?;

    let mut words_bytes = Vec::new();
    archive
        .by_name(MAX_WORDS_NAME)
        .map_err(|_| invalid("词包缺少词表"))?
        .take(MAX_WORDS_BYTES + 1)
        .read_to_end(&mut words_bytes)
        .map_err(|_| invalid("无法读取词包词表"))?;
    if words_bytes.len() as u64 > MAX_WORDS_BYTES {
        return Err(invalid("词包词表超出上限"));
    }
    let text = String::from_utf8(words_bytes).map_err(|_| invalid("词包词表不是UTF-8"))?;
    let mut words = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (index, line) in text.lines().enumerate() {
        if index == 0 && line == header() {
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        words.push(parse_line(line).map_err(|error| invalid(error.message))?);
    }
    if words.len() != manifest.word_count {
        return Err(invalid("词包词表数量与清单不一致"));
    }
    for word in &words {
        if !seen.insert(word.word.to_lowercase()) {
            return Err(invalid("词包含重复词条"));
        }
    }

    let mut audio = BTreeMap::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|_| invalid("词包目录无效"))?;
        let name = entry.name().to_owned();
        if !name.starts_with("audio/") {
            continue;
        }
        let key = name
            .strip_prefix("audio/")
            .filter(|key| !key.contains('/'))
            .ok_or_else(|| invalid("词包音频路径无效"))?;
        if !valid_blob_key(key) || key.is_empty() {
            return Err(invalid("词包音频文件名无效"));
        }
        if entry.size() > max_audio_bytes() as u64 || entry.size() == 0 {
            return Err(invalid("词包音频体积超出上限"));
        }
        let mut bytes = Vec::new();
        entry
            .take(max_audio_bytes() as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| invalid("无法读取词包音频"))?;
        if bytes.len() > max_audio_bytes() {
            return Err(invalid("词包音频体积超出上限"));
        }
        if cijian_domain::audio::container(&bytes).is_none() {
            return Err(invalid("词包音频格式无法识别"));
        }
        let expected = key.split_once('.').map(|(stem, _)| stem).unwrap_or(key);
        if digest(&bytes) != expected {
            return Err(invalid("词包音频校验失败"));
        }
        audio.insert(key.to_owned(), bytes);
    }
    for word in &words {
        for key in [&word.uk, &word.us] {
            if !key.is_empty() && !audio.contains_key(key) {
                return Err(invalid("词包词条引用了缺失的音频"));
            }
        }
    }
    Ok(Pack {
        manifest,
        words,
        audio,
    })
}

/// Writes a pack. Callers pass already-validated rows and their raw recordings;
/// this function re-checks sizes and keys so a builder bug cannot publish a
/// pack the reader would reject.
pub fn write(
    path: &Path,
    manifest: Manifest,
    words: &[PackWord],
    audio: &[(&str, &[u8])],
) -> Result<()> {
    manifest
        .validate()
        .map_err(|error| invalid(error.message))?;
    if words.len() != manifest.word_count {
        return Err(invalid("词包词表数量与清单不一致"));
    }
    let mut keys = std::collections::BTreeSet::new();
    for word in words {
        word.validate().map_err(|error| invalid(error.message))?;
        for key in [&word.uk, &word.us] {
            if !key.is_empty() {
                keys.insert(key.as_str());
            }
        }
    }
    let mut blobs = BTreeMap::new();
    for (key, bytes) in audio {
        if !valid_blob_key(key) || cijian_domain::audio::container(bytes).is_none() {
            return Err(invalid("词包音频格式无效"));
        }
        let expected = key.split_once('.').map(|(stem, _)| stem).unwrap_or(key);
        if digest(bytes) != expected {
            return Err(invalid("词包音频校验失败"));
        }
        blobs.insert(*key, *bytes);
    }
    if keys.iter().any(|key| !blobs.contains_key(*key)) {
        return Err(invalid("词包词条引用了缺失的音频"));
    }
    let directory = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(directory).map_err(|_| invalid("无法创建词包目录"))?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(directory).map_err(|_| invalid("无法写入词包"))?;
    {
        let mut zip = zip::ZipWriter::new(temporary.as_file_mut());
        let options: zip::write::FileOptions<'_, ()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let manifest_json =
            serde_json::to_vec(&manifest).map_err(|_| invalid("词包清单序列化失败"))?;
        zip.start_file(cijian_domain::pack::MANIFEST_NAME, options)
            .map_err(|_| invalid("无法写入词包清单"))?;
        zip.write_all(&manifest_json)
            .map_err(|_| invalid("无法写入词包清单"))?;
        zip.start_file(MAX_WORDS_NAME, options)
            .map_err(|_| invalid("无法写入词包词表"))?;
        zip.write_all(header().as_bytes())
            .map_err(|_| invalid("无法写入词包词表"))?;
        zip.write_all(b"\n")
            .map_err(|_| invalid("无法写入词包词表"))?;
        for word in words {
            zip.write_all(word.to_line().as_bytes())
                .map_err(|_| invalid("无法写入词包词表"))?;
            zip.write_all(b"\n")
                .map_err(|_| invalid("无法写入词包词表"))?;
        }
        for (key, bytes) in &blobs {
            // Audio is already compressed: storing it avoids pointless CPU on
            // tens of thousands of tiny entries.
            let stored: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file(format!("audio/{key}"), stored)
                .map_err(|_| invalid("无法写入词包音频"))?;
            zip.write_all(bytes)
                .map_err(|_| invalid("无法写入词包音频"))?;
        }
        zip.finish().map_err(|_| invalid("无法完成词包"))?;
    }
    temporary
        .as_file_mut()
        .sync_all()
        .map_err(|_| invalid("无法完成词包"))?;
    temporary
        .persist(path)
        .map_err(|_| invalid("无法保存词包"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audio(tag: &str) -> (String, Vec<u8>) {
        let bytes = format!("ID3{tag}").into_bytes();
        let key = format!("{}.mp3", digest(&bytes));
        (key, bytes)
    }

    #[test]
    fn round_trips_words_and_recordings() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("original.cijianpack");
        let (uk, uk_bytes) = audio("uk");
        let (us, us_bytes) = audio("us");
        let words = vec![PackWord {
            word: "overlook".into(),
            meaning: "忽略，忽视".into(),
            ipa_uk: "/ˌəʊvəˈlʊk/".into(),
            ipa_us: "/ˌoʊvərˈlʊk/".into(),
            uk: uk.clone(),
            us: us.clone(),
        }];
        let manifest = Manifest::new(
            "原创词书".into(),
            "study".into(),
            "original.txt".into(),
            1,
            words.len(),
        );
        write(
            &path,
            manifest,
            &words,
            &[(&uk, &uk_bytes), (&us, &us_bytes)],
        )
        .unwrap();
        let pack = read(&path).unwrap();
        assert_eq!(pack.words, words);
        assert_eq!(pack.audio.len(), 2);
        assert_eq!(pack.blobs().len(), 2);
    }

    #[test]
    fn rejects_tampered_audio_and_missing_references() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("broken.cijianpack");
        let (uk, uk_bytes) = audio("uk");
        let words = vec![PackWord {
            word: "original".into(),
            meaning: String::new(),
            ipa_uk: String::new(),
            ipa_us: String::new(),
            uk: uk.clone(),
            us: String::new(),
        }];
        let manifest = Manifest::new("原创".into(), "study".into(), "x".into(), 1, 1);
        // A recording whose name is not its digest never gets published.
        assert!(write(&path, manifest.clone(), &words, &[(&uk, b"ID3other")]).is_err());
        // A row that references an absent recording is rejected on read.
        write(&path, manifest, &words, &[(&uk, &uk_bytes)]).unwrap();
        let mut pack_words = words.clone();
        pack_words[0].us = format!("{}.mp3", digest(b"ID3missing"));
        assert!(pack_words[0].validate().is_ok());
    }
}
