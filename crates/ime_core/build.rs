use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

/// 辞書ファイルのリスト
const DICTIONARY_FILES: [&str; 10] = [
    "dictionary00.txt",
    "dictionary01.txt",
    "dictionary02.txt",
    "dictionary03.txt",
    "dictionary04.txt",
    "dictionary05.txt",
    "dictionary06.txt",
    "dictionary07.txt",
    "dictionary08.txt",
    "dictionary09.txt",
];

/// 辞書のエントリ
#[derive(Debug)]
struct Entry {
    surface: String,
    left_id: u16,
    right_id: u16,
    cost: i32,
}

/// メイン関数
/// 
/// 戻り値:
/// * `io::Result<()>`: 成功時はOk、失敗時はErr
fn main() -> io::Result<()> {
    let dictionary_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("src")
        .join("dictionary");
    let mut entries = BTreeMap::<String, Vec<Entry>>::new();

    for file_name in DICTIONARY_FILES {
        let path = dictionary_dir.join(file_name);
        println!("cargo:rerun-if-changed={}", path.display());
        let text = fs::read_to_string(path)?;
        for line in text.lines() {
            let mut fields = line.trim_end_matches('\r').split('\t');
            let (Some(reading), Some(left_id), Some(right_id), Some(cost), Some(surface)) = (
                fields.next(),
                fields.next(),
                fields.next(),
                fields.next(),
                fields.next(),
            ) else {
                continue;
            };
            let (Ok(left_id), Ok(right_id), Ok(cost)) = (
                left_id.parse::<u16>(),
                right_id.parse::<u16>(),
                cost.parse::<i32>(),
            ) else {
                continue;
            };
            entries.entry(reading.to_owned()).or_default().push(Entry {
                surface: surface.to_owned(),
                left_id,
                right_id,
                cost,
            });
        }
    }

    for candidates in entries.values_mut() {
        candidates.sort_by_key(|entry| entry.cost);
    }

    let output_path = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("dictionary.bin");
    let mut output = fs::File::create(output_path)?;
    output.write_all(b"SMYDIC01")?;
    write_u32(&mut output, entries.len() as u32)?;
    for (reading, candidates) in entries {
        write_string(&mut output, &reading)?;
        write_u32(&mut output, candidates.len() as u32)?;
        for entry in candidates {
            write_u16(&mut output, entry.left_id)?;
            write_u16(&mut output, entry.right_id)?;
            output.write_all(&entry.cost.to_le_bytes())?;
            write_string(&mut output, &entry.surface)?;
        }
    }

    Ok(())
}

/// 文字列を書き込む。
/// 引数:
/// * `output`: 書き込み先のファイル
/// * `value`: 書き込む文字列
/// 戻り値:
/// * `io::Result<()>`: 成功時はOk、失敗時はErr
fn write_string(output: &mut fs::File, value: &str) -> io::Result<()> {
    write_u32(output, value.len() as u32)?;
    output.write_all(value.as_bytes())
}

/// u16を書き込む。
/// 引数:
/// * `output`: 書き込み先のファイル
/// * `value`: 書き込むu16値
/// 戻り値:
/// * `io::Result<()>`: 成功時はOk、失敗時はErr
fn write_u16(output: &mut fs::File, value: u16) -> io::Result<()> {
    output.write_all(&value.to_le_bytes())
}

/// u32を書き込む。
/// 引数:
/// * `output`: 書き込み先のファイル
/// * `value`: 書き込むu32値
/// 戻り値:
/// * `io::Result<()>`: 成功時はOk、失敗時はErr
fn write_u32(output: &mut fs::File, value: u32) -> io::Result<()> {
    output.write_all(&value.to_le_bytes())
}
