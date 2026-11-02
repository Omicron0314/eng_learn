use super::*;
struct Chunks<'a> {
    bytes: &'a [u8],
    size: usize,
}
impl Read for Chunks<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = buffer.len().min(self.size).min(self.bytes.len());
        buffer[..count].copy_from_slice(&self.bytes[..count]);
        self.bytes = &self.bytes[count..];
        Ok(count)
    }
}
#[test]
fn chunk_boundaries_preserve_chinese_columns_multiline_positions_and_hash() {
    let text = "\u{feff}单词,例句,中文释义\r\nApple,\"line one\nline two\",苹果\r\napple,,不覆盖\r\n,,错误\r\ntake off,,起飞\r\n";
    for size in [1, 2, 7, 8192] {
        let (parsed, hash) = csv(
            Chunks {
                bytes: text.as_bytes(),
                size,
            },
            false,
        )
        .unwrap();
        assert_eq!(parsed.rows[0].meaning, "苹果");
        assert_eq!(parsed.rows[0].example, "line one\nline two");
        assert_eq!(parsed.duplicates, 1);
        assert_eq!(parsed.errors[0].line, 5);
        assert_eq!(parsed.phrases, 1);
        assert_eq!(hash, format!("{:x}", Sha256::digest(text.as_bytes())));
        assert_eq!(
            serde_json::to_value(&parsed).unwrap(),
            serde_json::to_value(cijian_domain::import::parse_card_csv(text.as_bytes()).unwrap())
                .unwrap()
        );
    }
    for (text, expected) in [
        ("\u{feff}\r\n,错误\r\napple,苹果", 2),
        ("\u{feff}\n\nword,meaning\n,错误", 4),
    ] {
        for size in [1, 7, 8192] {
            let (parsed, _) = csv(
                Chunks {
                    bytes: text.as_bytes(),
                    size,
                },
                false,
            )
            .unwrap();
            assert_eq!(
                parsed.errors[0].line, expected,
                "BOM/blank prefix at chunk {size}"
            );
        }
    }
    let text =
        "\u{feff}Apple 苹果\r\nUnit 1\ncolour/color 颜色\n# original comment\ntake off 起飞\n";
    for size in [1, 7, 8192] {
        let (parsed, hash) = txt(Chunks {
            bytes: text.as_bytes(),
            size,
        })
        .unwrap();
        assert_eq!(
            serde_json::to_value(parsed).unwrap(),
            serde_json::to_value(cijian_domain::wordlist::parse(text.as_bytes()).unwrap()).unwrap()
        );
        assert_eq!(hash, format!("{:x}", Sha256::digest(text.as_bytes())));
    }
}
#[test]
fn resource_guard_limits_records_including_quoted_newlines_not_total_files() {
    let text = format!("apple,\"{}\"", "x\n".repeat(RECORD_LIMIT));
    assert!(
        csv(text.as_bytes(), false)
            .unwrap_err()
            .message
            .contains("1MiB")
    );
    assert!(
        txt(vec![b'x'; RECORD_LIMIT + 1].as_slice())
            .unwrap_err()
            .message
            .contains("1MiB")
    );
    let blank = std::io::repeat(b'\n').take((RECORD_LIMIT * 2) as u64);
    assert_eq!(csv(blank, false).unwrap().0.parsed, 0);
    let lines = "seed,a\"b\n".repeat(150000);
    let (parsed, _) = csv(lines.as_bytes(), false).unwrap();
    assert_eq!(parsed.rows.len(), 1);
    assert_eq!(parsed.duplicates, 149999);
    assert_eq!(parsed.rows[0].meaning, "a\"b");
    let escaped = "word,meaning\napple,\"he said \"\"hi\"\"\nand left\"\npear,fruit\n";
    let (parsed, _) = csv(
        Chunks {
            bytes: escaped.as_bytes(),
            size: 1,
        },
        false,
    )
    .unwrap();
    assert_eq!(parsed.rows[0].meaning, "he said \"hi\"\nand left");
    assert_eq!(parsed.rows[1].line, 4);
}
#[test]
fn bad_encoding_and_unsupported_file_types_fail_before_import() {
    assert!(csv(&[b'a', b',', 0xff][..], false).is_err());
    assert!(txt(&[0xff][..]).is_err());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("original.bin");
    std::fs::write(&path, "apple").unwrap();
    assert!(read(&path, true).is_err());
    let path = dir.path().join("original.csv");
    std::fs::write(&path, "word,meaning\napple,苹果\n").unwrap();
    let (parsed, hash) = read(&path, false).unwrap();
    assert_eq!(parsed.rows.len(), 1);
    assert_eq!(
        hash,
        format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()))
    );
}
