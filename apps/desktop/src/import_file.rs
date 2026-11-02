//! Native file-format adapter. Never opens user databases except the selected APKG's temporary copy.
mod stream;
use cijian_domain::import::{ImportRow, ParsedImport, parse_rows};
use cijian_ports::{AppError, ErrorCode, Result};
use regex::Regex;
use rusqlite::{Connection, OpenFlags};
use std::{
    io::{Read, Write},
    path::Path,
    sync::OnceLock,
};
fn invalid(message: &str) -> AppError {
    AppError::new(ErrorCode::InvalidInput, message)
}
fn text(html: &str) -> String {
    static SCRIPT: OnceLock<Regex> = OnceLock::new();
    static TAG: OnceLock<Regex> = OnceLock::new();
    let html = SCRIPT
        .get_or_init(|| {
            Regex::new(r"(?is)<(?:script|style)\b[^>]*>.*?</(?:script|style)\s*>").unwrap()
        })
        .replace_all(html, "");
    let stripped = TAG
        .get_or_init(|| Regex::new(r"(?s)<[^>]*>").unwrap())
        .replace_all(&html, " ");
    html_escape::decode_html_entities(&stripped)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
pub fn read(path: &Path, word_list: bool) -> Result<(ParsedImport, String)> {
    stream::read(path, word_list)
}
#[cfg(test)]
fn read_apkg(path: &Path) -> Result<ParsedImport> {
    let mut file = std::fs::File::open(path).map_err(|_| invalid("无法读取APKG"))?;
    read_apkg_source(&mut file)
}
fn read_apkg_source(file: &mut std::fs::File) -> Result<ParsedImport> {
    let mut archive = zip::ZipArchive::new(file).map_err(|_| invalid("APKG压缩结构无效"))?;
    const LIMIT: u64 = 100 * 1024 * 1024;
    let mut collection = None;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|_| invalid("APKG目录无效"))?;
        if entry.enclosed_name().is_none() {
            return Err(invalid("APKG包含路径穿越条目"));
        }
        if entry.name() == "collection.anki21b" {
            return Err(invalid("新版压缩APKG暂不支持，请导出兼容旧版格式"));
        }
        if ["collection.anki2", "collection.anki21"].contains(&entry.name()) {
            if entry.size() > LIMIT {
                return Err(invalid("APKG集合解压超过100MiB资源保护"));
            }
            collection = Some(index);
        }
    }
    let index = collection.ok_or(invalid("APKG中没有兼容集合"))?;
    let directory = tempfile::tempdir().map_err(|_| invalid("无法创建导入临时目录"))?;
    let target = directory.path().join("collection.sqlite");
    let mut output = std::fs::File::create(&target).map_err(|_| invalid("无法写入临时集合"))?;
    let mut entry = archive
        .by_index(index)
        .map_err(|_| invalid("无法打开集合"))?
        .take(LIMIT + 1);
    let bytes = std::io::copy(&mut entry, &mut output).map_err(|_| invalid("集合解压失败"))?;
    if bytes > LIMIT {
        return Err(invalid("APKG集合解压超过100MiB资源保护"));
    }
    output.flush().map_err(|_| invalid("集合写入失败"))?;
    drop(output);
    let db = Connection::open_with_flags(&target, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| invalid("集合不是可读取的SQLite"))?;
    db.execute_batch("PRAGMA trusted_schema=OFF;")
        .map_err(|_| invalid("无法安全读取集合"))?;
    let models: String = db
        .query_row("SELECT models FROM col LIMIT 1", [], |r| r.get(0))
        .map_err(|_| invalid("集合模型结构不受支持"))?;
    let models: serde_json::Value =
        serde_json::from_str(&models).map_err(|_| invalid("集合模型JSON无效"))?;
    let mut statement = db
        .prepare("SELECT mid,flds FROM notes ORDER BY id")
        .map_err(|_| invalid("集合笔记结构不受支持"))?;
    let mut notes = statement.query([]).map_err(|_| invalid("无法读取笔记"))?;
    let mut rows = Vec::new();
    while let Some(note) = notes.next().map_err(|_| invalid("笔记读取失败"))? {
        let model: i64 = note.get(0).map_err(|_| invalid("笔记模型无效"))?;
        let fields: String = note.get(1).map_err(|_| invalid("笔记字段无效"))?;
        let values = fields.split('\u{1f}').map(text).collect::<Vec<_>>();
        let fields = models[model.to_string()]["flds"].as_array();
        let get = |aliases: &[&str]| -> Option<String> {
            for alias in aliases {
                if let Some(fields) = fields {
                    for (index, field) in fields.iter().enumerate() {
                        if field["name"]
                            .as_str()
                            .is_some_and(|name| name.eq_ignore_ascii_case(alias))
                        {
                            let ordinal =
                                field["ord"].as_u64().map(|n| n as usize).unwrap_or(index);
                            if let Some(value) =
                                values.get(ordinal).filter(|v| !v.trim().is_empty())
                            {
                                return Some(value.clone());
                            }
                        }
                    }
                }
            }
            None
        };
        let raw = values.first().cloned().unwrap_or_default();
        let front = get(&["Front", "正面"]).unwrap_or_else(|| raw.clone());
        let meaning = get(&["Back", "释义", "中文释义", "Meaning", "Definition", "翻译"])
            .unwrap_or_else(|| values.get(1).cloned().unwrap_or_default());
        rows.push(ImportRow {
            word: raw.clone(),
            raw,
            front,
            meaning,
            example: String::new(),
            line: rows.len() as u64 + 1,
            phrase: false,
        });
    }
    Ok(parse_rows(rows))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_apkg_field_aliases_and_html() {
        let temp = tempfile::tempdir().unwrap();
        let dbpath = temp.path().join("source.sqlite");
        {
            let db = Connection::open(&dbpath).unwrap();
            db.execute_batch("CREATE TABLE col(models TEXT);CREATE TABLE notes(id INTEGER,mid INTEGER,flds TEXT);").unwrap();
            db.execute("INSERT INTO col VALUES (?1)",[r#"{"1":{"flds":[{"name":"Raw","ord":0},{"name":"Notes","ord":1},{"name":"Front","ord":2},{"name":"中文释义","ord":3}]}}"#]).unwrap();
            db.execute(
                "INSERT INTO notes VALUES (1,1,?1)",
                ["apple\u{1f}不要误取笔记\u{1f}<b>Apple</b>\u{1f}苹果 &amp; 水果"],
            )
            .unwrap();
        }
        let path = temp.path().join("original.apkg");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        zip.start_file("collection.anki2", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&std::fs::read(dbpath).unwrap()).unwrap();
        zip.finish().unwrap();
        let result = read_apkg(&path).unwrap();
        assert_eq!(result.rows[0].word, "apple");
        assert_eq!(result.rows[0].front, "Apple");
        assert_eq!(result.rows[0].meaning, "苹果 & 水果");
    }
    #[test]
    fn traversal_and_modern_compression_are_rejected() {
        for entry in ["../outside", "collection.anki21b"] {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("bad.apkg");
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            zip.start_file(entry, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"original").unwrap();
            zip.finish().unwrap();
            assert!(read_apkg(&path).is_err());
        }
    }
}
