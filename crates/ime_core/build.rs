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

const INVALID_INDEX: u32 = u32::MAX;  // 無効なインデックス


/// 辞書のエントリ
#[derive(Debug)]
struct Entry {
    surface: String,  // 表層
    left_id: u16, // 左文脈ID
    right_id: u16, // 右文脈ID
    cost: i32, // コスト
}

/// ビルド時にTrieを構築するためのノード
struct BuildNode {
    ch: char,  // このノードが担当する文字
    children: BTreeMap<char, u32>,  // 子ノードのマップ。キーは文字、値はインデックス
    candidates: Vec<Entry>,  // このノードが表す読みに対する候補のリスト
}

impl BuildNode {
    /// コンストラクタ。
    fn new(ch: char) -> Self {
        Self {
            ch,
            children: BTreeMap::new(),
            candidates: Vec::new(),
        }
    }
}

/// バイナリへ出力するTrieノード
struct SerializedTrieNode {
    ch: u32,  // このノードが担当する文字（UTF-32コードユニット）
    first_child: u32,  // 先頭の子ノードのインデックス。なければINVALID_INDEX
    next_sibling: u32,  // 次の兄弟ノードのインデックス。なければINVALID_INDEX
    entry_start: u32,  // このノードが表す読みに対する候補の開始インデックス
    entry_count: u32,  // このノードが表す読みに対する候補の数
}

/// usizeをu32へ安全に変換する。
/// 
/// 引数
/// * `value`: 変換対象のusize値
/// * `description`: エラー発生時に表示される説明文字列
/// 
/// 戻り値
/// * `Ok(u32)`: 変換成功
/// * `Err(io::Error)`: 変換失敗（値がu32の範囲を超えている場合）
fn to_u32(value: usize, description: &str) -> io::Result<u32> {
    u32::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{description} exceeds u32 range"),
        )
    })
}

/// メイン関数
///
/// 戻り値:
/// * `Ok(())`: 正常終了
/// * `Err(io::Error)`: エラー発生時
fn main() -> io::Result<()> {
    let dictionary_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("src")
        .join("dictionary");

    // 既存と同じ方法で読みごとに候補を集約
    let mut entries = BTreeMap::<String, Vec<Entry>>::new();

    for file_name in DICTIONARY_FILES {
        let path = dictionary_dir.join(file_name);
        println!("cargo:rerun-if-changed={}", path.display());

        let text = fs::read_to_string(path)?;

        for line in text.lines() {
            let mut fields = line.trim_end_matches('\r').split('\t');

            let (
                Some(reading),
                Some(left_id),
                Some(right_id),
                Some(cost),
                Some(surface),
            ) = (
                fields.next(),
                fields.next(),
                fields.next(),
                fields.next(),
                fields.next(),
            )
            else {
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

    // Trieのルートノードを作成
    let mut build_nodes = vec![BuildNode::new('\0')];

    // 読みの共通接頭辞をTrie内で共有
    for (reading, candidates) in entries {
        let mut node_index = 0usize;

        for ch in reading.chars() {
            let next_index = if let Some(&existing) =
                build_nodes[node_index].children.get(&ch)
            {
                existing
            } else {
                let new_index = to_u32(build_nodes.len(), "Trie node count")?;

                build_nodes.push(BuildNode::new(ch));
                build_nodes[node_index]
                    .children
                    .insert(ch, new_index);

                new_index
            };

            node_index = next_index as usize;
        }

        build_nodes[node_index].candidates = candidates;
    }

    let mut serialized_nodes: Vec<SerializedTrieNode> = build_nodes
        .iter()
        .map(|node| SerializedTrieNode {
            ch: node.ch as u32,
            first_child: INVALID_INDEX,
            next_sibling: INVALID_INDEX,
            entry_start: 0,
            entry_count: 0,
        })
        .collect();

    // 各ノードの子と兄弟をインデックスで接続
    for parent_index in 0..build_nodes.len() {
        let children: Vec<u32> = build_nodes[parent_index]
            .children
            .values()
            .copied()
            .collect();

        if let Some(&first_child) = children.first() {
            serialized_nodes[parent_index].first_child = first_child;
        }

        for pair in children.windows(2) {
            serialized_nodes[pair[0] as usize].next_sibling = pair[1];
        }
    }

    // 各ノードの候補を連続した配列へ格納
    let mut flat_entries = Vec::<Entry>::new();

    for index in 0..build_nodes.len() {
        let entry_start = to_u32(flat_entries.len(), "Dictionary entry offset")?;
        let candidates = std::mem::take(&mut build_nodes[index].candidates);
        let entry_count = to_u32(candidates.len(), "Candidate count")?;

        serialized_nodes[index].entry_start = entry_start;
        serialized_nodes[index].entry_count = entry_count;

        flat_entries.extend(candidates);
    }

    let node_count = to_u32(serialized_nodes.len(), "Trie node count")?;
    let entry_count = to_u32(flat_entries.len(), "Dictionary entry count")?;

    // バイナリ形式:
    // magic
    // node_count
    // entry_count
    // Trie nodes
    // Dictionary entries
    let output_path = PathBuf::from(env::var_os("OUT_DIR").unwrap())
        .join("dictionary.bin");

    let mut output = fs::File::create(output_path)?;

    output.write_all(b"SMYTRI01")?;
    write_u32(&mut output, node_count)?;
    write_u32(&mut output, entry_count)?;

    // ノード1個あたり5個のu32を書き込む
    for node in &serialized_nodes {
        write_u32(&mut output, node.ch)?;
        write_u32(&mut output, node.first_child)?;
        write_u32(&mut output, node.next_sibling)?;
        write_u32(&mut output, node.entry_start)?;
        write_u32(&mut output, node.entry_count)?;
    }

    // 読みはTrieの経路で表現されるため、ここでは表記と属性を保存
    for entry in flat_entries {
        write_u16(&mut output, entry.left_id)?;
        write_u16(&mut output, entry.right_id)?;
        output.write_all(&entry.cost.to_le_bytes())?;
        write_string(&mut output, &entry.surface)?;
    }

    Ok(())
}

/// 文字列を書き込む。
/// 
/// 引数:
/// * `output`: 出力先ファイル
/// * `value`: 書き込む文字列
/// 
/// 戻り値:
/// * `Ok(())`: 正常終了
/// * `Err(io::Error)`: エラー発生時
fn write_string(output: &mut fs::File, value: &str) -> io::Result<()> {
    write_u32(
        output,
        to_u32(value.len(), "String length")?,
    )?;
    output.write_all(value.as_bytes())
}

/// u16を書き込む。
/// 
/// 引数:
/// * `output`: 出力先ファイル
/// * `value`: 書き込むu16値
/// 
/// 戻り値:
/// * `Ok(())`: 正常終了
/// * `Err(io::Error)`: エラー発生時
fn write_u16(output: &mut fs::File, value: u16) -> io::Result<()> {
    output.write_all(&value.to_le_bytes())
}

/// u32を書き込む。
/// 
/// 引数:
/// * `output`: 出力先ファイル
/// * `value`: 書き込むu32値
/// 
/// 戻り値:
/// * `Ok(())`: 正常終了
/// * `Err(io::Error)`: エラー発生時
fn write_u32(output: &mut fs::File, value: u32) -> io::Result<()> {
    output.write_all(&value.to_le_bytes())
}
