//! Streaming file reads, source-byte hashing and bounded individual records.
use super::{invalid, read_apkg_source};
use cijian_domain::{
    import::{CardCsvImporter, ParsedImport},
    wordlist::WordListImporter,
};
use cijian_ports::Result;
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    fs::File,
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom},
    path::Path,
};
const RECORD_LIMIT: usize = 1024 * 1024;
const RECORD_ERROR: &str = "单条导入记录超过1MiB资源保护，请拆分长字段或移除无关列";
#[derive(Clone, Copy)]
enum Field {
    Start,
    Plain,
    Quoted,
    AfterQuote,
}
struct RecordGuard {
    csv: bool,
    field: Field,
    bytes: usize,
    prefix: Vec<u8>,
}
impl RecordGuard {
    fn new(csv: bool) -> Self {
        Self {
            csv,
            field: Field::Start,
            bytes: 0,
            prefix: Vec::new(),
        }
    }
    fn push(&mut self, byte: u8) -> io::Result<()> {
        if self.prefix.len() < 3 {
            self.prefix.push(byte);
            if self.prefix.len() == 3 && self.prefix != [0xef, 0xbb, 0xbf] {
                for byte in self.prefix.clone() {
                    self.scan(byte)?;
                }
            }
            return Ok(());
        }
        self.scan(byte)
    }
    fn scan(&mut self, byte: u8) -> io::Result<()> {
        self.bytes += 1;
        if !self.csv {
            if byte == b'\n' {
                self.bytes = 0;
            }
        } else {
            self.field = match (self.field, byte) {
                (Field::Start, b'"') | (Field::AfterQuote, b'"') => Field::Quoted,
                (Field::Quoted, b'"') => Field::AfterQuote,
                (Field::Quoted, _) => Field::Quoted,
                (_, b',') => Field::Start,
                (_, b'\n' | b'\r') => {
                    self.bytes = 0;
                    Field::Start
                }
                _ => Field::Plain,
            };
        }
        if self.bytes > RECORD_LIMIT {
            return Err(io::Error::new(io::ErrorKind::InvalidData, RECORD_ERROR));
        }
        Ok(())
    }
}
struct NewlineRun {
    start: u64,
    end: u64,
    line_after: u64,
}
struct Lines {
    runs: VecDeque<NewlineRun>,
    offset: u64,
    line: u64,
    previous_cr: bool,
}
impl Default for Lines {
    fn default() -> Self {
        Self {
            runs: VecDeque::new(),
            offset: 0,
            line: 1,
            previous_cr: false,
        }
    }
}
impl Lines {
    fn push(&mut self, byte: u8) {
        if byte == b'\r' || byte == b'\n' {
            if byte == b'\r' || !self.previous_cr {
                self.line += 1;
            }
            if let Some(run) = self.runs.back_mut()
                && run.end == self.offset
            {
                run.end += 1;
                run.line_after = self.line;
            } else {
                self.runs.push_back(NewlineRun {
                    start: self.offset,
                    end: self.offset + 1,
                    line_after: self.line,
                });
            }
        }
        self.previous_cr = byte == b'\r';
        self.offset += 1;
    }
    fn at(&mut self, byte: u64) -> u64 {
        while self.runs.len() > 1 && self.runs[1].start <= byte {
            self.runs.pop_front();
        }
        self.runs
            .front()
            .filter(|run| run.start <= byte)
            .map_or(1, |run| run.line_after)
    }
}
struct Source<R> {
    inner: R,
    hash: Sha256,
    guard: RecordGuard,
    lines: Option<Lines>,
}
impl<R> Source<R> {
    fn new(inner: R, csv: bool) -> Self {
        Self {
            inner,
            hash: Sha256::new(),
            guard: RecordGuard::new(csv),
            lines: csv.then(Lines::default),
        }
    }
}
impl<R: Read> Read for Source<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(buffer)?;
        for byte in &buffer[..count] {
            self.guard.push(*byte)?;
            if let Some(lines) = &mut self.lines {
                lines.push(*byte);
            }
        }
        self.hash.update(&buffer[..count]);
        Ok(count)
    }
}
fn read_error(error: &io::Error) -> cijian_ports::AppError {
    invalid(
        if error.kind() == io::ErrorKind::InvalidData && error.to_string() == RECORD_ERROR {
            RECORD_ERROR
        } else {
            "无法读取导入文件"
        },
    )
}
fn csv<R: Read>(source: R, word_list: bool) -> Result<(ParsedImport, String)> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(Source::new(source, true));
    let mut parser = CardCsvImporter::default();
    let mut record = csv::StringRecord::new();
    loop {
        let exists = reader
            .read_record(&mut record)
            .map_err(|error| match error.kind() {
                csv::ErrorKind::Io(error) => read_error(error),
                csv::ErrorKind::Utf8 { .. } => invalid("文件必须为UTF-8"),
                _ => invalid("CSV格式无效"),
            })?;
        if !exists {
            break;
        }
        let mut position = record.position().map_or(0, |position| position.byte());
        // The CSV reader reports byte zero for a first record after a BOM and blank lines.
        if position == 0 && reader.get_ref().guard.prefix == [0xef, 0xbb, 0xbf] {
            position = 3;
        }
        let line = reader
            .get_mut()
            .lines
            .as_mut()
            .expect("CSV line tracker")
            .at(position);
        parser.push_record(&record.iter().collect::<Vec<_>>(), line);
    }
    let hash = format!("{:x}", reader.into_inner().hash.finalize());
    let parsed = parser.finish();
    Ok((
        if word_list {
            cijian_domain::wordlist::from_csv(parsed)
        } else {
            parsed
        },
        hash,
    ))
}
fn txt<R: Read>(source: R) -> Result<(ParsedImport, String)> {
    let mut reader = BufReader::new(Source::new(source, false));
    let mut parser = WordListImporter::default();
    let mut line = Vec::new();
    let mut number = 0;
    loop {
        line.clear();
        if reader
            .read_until(b'\n', &mut line)
            .map_err(|error| read_error(&error))?
            == 0
        {
            break;
        }
        number += 1;
        let text = std::str::from_utf8(&line).map_err(|_| invalid("词表必须为UTF-8"))?;
        parser.push_line(text.trim_end_matches(['\r', '\n']), number);
    }
    Ok((
        parser.finish(),
        format!("{:x}", reader.into_inner().hash.finalize()),
    ))
}
pub(super) fn read(path: &Path, word_list: bool) -> Result<(ParsedImport, String)> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(
        (extension.as_str(), word_list),
        ("csv", _) | ("txt", true) | ("apkg", false)
    ) {
        return Err(invalid("词表支持CSV/TXT；词卡支持CSV/兼容APKG"));
    }
    let metadata = std::fs::metadata(path).map_err(|_| invalid("无法读取所选文件"))?;
    if !metadata.is_file() {
        return Err(invalid("请选择普通文件"));
    }
    let mut file = File::open(path).map_err(|_| invalid("无法打开所选文件"))?;
    let before = file.metadata().map_err(|_| invalid("无法读取文件信息"))?;
    if !before.is_file() {
        return Err(invalid("请选择普通文件"));
    }
    let result = match extension.as_str() {
        "csv" => csv(&mut file, word_list)?,
        "txt" => txt(&mut file)?,
        _ => {
            let mut hash = Sha256::new();
            let mut buffer = [0_u8; 65536];
            loop {
                let count = file.read(&mut buffer).map_err(|error| read_error(&error))?;
                if count == 0 {
                    break;
                }
                hash.update(&buffer[..count]);
            }
            file.seek(SeekFrom::Start(0))
                .map_err(|_| invalid("无法定位APKG"))?;
            (
                read_apkg_source(&mut file)?,
                format!("{:x}", hash.finalize()),
            )
        }
    };
    let after = file.metadata().map_err(|_| invalid("无法复核文件信息"))?;
    if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
        return Err(invalid("导入文件在读取期间发生变化，请重新选择"));
    }
    Ok(result)
}
#[cfg(test)]
mod tests;
