use std::collections::HashMap;
use std::sync::OnceLock;


const BINARY_MAGIC: &[u8; 8] = b"SMYTRI01"; // バイト列「SMYTRI01」
const INVALID_INDEX: u32 = u32::MAX; // 無効なインデックス


/// 辞書のエントリ
#[derive(Debug, Clone)]
pub struct DictEntry {
    pub surface: String, // 表記
    pub left_id: u16,    // 左文脈ID
    pub right_id: u16,   // 右文脈ID
    pub cost: i32,       // 生起コスト
}

/// 接頭辞検索の結果
#[derive(Debug)]
pub struct PrefixMatch<'input, 'dictionary> {
    pub reading: &'input str, // 入力された読みのうち、辞書に一致した部分
    pub candidates: &'dictionary [DictEntry], // その読みの変換候補
}

/// Trieのノード
#[derive(Debug, Clone)]
struct TrieNode {
    ch: char, // ノードが表す文字
    first_child: u32, // 最初の子ノードのインデックス
    next_sibling: u32, // 次の兄弟ノードのインデックス
    entry_start: u32, // 候補エントリの開始位置
    entry_count: u32, // 候補エントリの数
}

impl TrieNode {
    /// ノードを生成する。
    /// 
    /// 引数:
    /// * `ch`: ノードが表す文字
    ///
    /// 戻り値:
    /// * `Self`: 生成されたノード
    fn new(ch: char) -> Self {
        Self {
            ch,
            first_child: INVALID_INDEX,
            next_sibling: INVALID_INDEX,
            entry_start: 0,
            entry_count: 0,
        }
    }
}

/// 読みから候補を検索する辞書
#[derive(Debug)]
pub struct Dictionary {
    nodes: Vec<TrieNode>, // Trieのノード
    entries: Vec<DictEntry>, // 候補エントリ
    pending_entries: Option<HashMap<String, Vec<DictEntry>>>, // 変換中のエントリ
}

impl Default for Dictionary {
    /// コンストラクタ。
    fn default() -> Self {
        Self::new()
    }
}

impl Dictionary {
    /// コンストラクタ。
    pub fn new() -> Self {
        Self {
            nodes: vec![TrieNode::new('\0')],
            entries: Vec::new(),
            pending_entries: Some(HashMap::new()),
        }
    }

    /// ビルド時に生成したMozc辞書を初回アクセス時に読み込む。
    ///
    /// 戻り値:
    /// * `&'static Self`: 辞書の参照
    pub fn embedded() -> &'static Self {
        static DICTIONARY: OnceLock<Dictionary> = OnceLock::new();

        DICTIONARY.get_or_init(|| {
            Self::from_bytes(include_bytes!(concat!(
                env!("OUT_DIR"),
                "/dictionary.bin"
            )))
            .expect("embedded dictionary is invalid")
        })
    }

    /// ビルド時生成バイナリから辞書を復元する。
    /// 
    /// 引数:
    /// * `bytes`: バイナリデータ
    ///
    /// 戻り値:
    /// * `Result<Self, &'static str>`: 辞書のインスタンス
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        let mut reader = BinaryReader::new(bytes);

        if reader.read_exact(BINARY_MAGIC.len())? != BINARY_MAGIC {
            return Err("invalid dictionary magic");
        }

        let node_count = reader.read_u32()? as usize;
        let entry_count = reader.read_u32()? as usize;

        if node_count == 0 {
            return Err("dictionary has no Trie root");
        }

        // ノードは5個のu32、合計20バイトで格納
        let node_bytes = node_count
            .checked_mul(20)
            .ok_or("invalid Trie node count")?;

        if node_bytes > bytes.len().saturating_sub(reader.position) {
            return Err("truncated dictionary");
        }

        let mut nodes = Vec::with_capacity(node_count);

        for _ in 0..node_count {
            let codepoint = reader.read_u32()?;
            let ch = char::from_u32(codepoint)
                .ok_or("invalid Trie node character")?;

            nodes.push(TrieNode {
                ch,
                first_child: reader.read_u32()?,
                next_sibling: reader.read_u32()?,
                entry_start: reader.read_u32()?,
                entry_count: reader.read_u32()?,
            });
        }

        // ノードの参照先と候補領域が範囲内にあることを検証
        for node in &nodes {
            for index in [node.first_child, node.next_sibling] {
                if index != INVALID_INDEX && index as usize >= node_count {
                    return Err("Trie node index out of bounds");
                }
            }

            let end = node
                .entry_start
                .checked_add(node.entry_count)
                .ok_or("invalid dictionary entry range")?;

            if end as usize > entry_count {
                return Err("dictionary entry range out of bounds");
            }
        }

        if nodes[0].ch != '\0' {
            return Err("invalid Trie root");
        }

        // 各エントリには最低12バイト必要
        // left_id(2) + right_id(2) + cost(4) + 文字列長(4)
        if entry_count > bytes.len().saturating_sub(reader.position) / 12 {
            return Err("truncated dictionary");
        }

        let mut entries = Vec::with_capacity(entry_count);

        for _ in 0..entry_count {
            entries.push(DictEntry {
                left_id: reader.read_u16()?,
                right_id: reader.read_u16()?,
                cost: reader.read_i32()?,
                surface: reader.read_string()?,
            });
        }

        if !reader.is_empty() {
            return Err("trailing bytes in dictionary");
        }

        Ok(Self {
            nodes,
            entries,
            pending_entries: None,
        })
    }

    /// Mozc形式（読み\t左ID\t右ID\tコスト\t表記）のテキストを読み込む。
    ///
    /// 引数:
    /// * `text`: Mozc形式のテキスト
    pub fn load_tsv(&mut self, text: &str) {
        // finish()後に追加読み込みする場合は、既存のTrieを一時辞書へ戻す。
        if self.pending_entries.is_none() {
            self.pending_entries = Some(self.collect_entries_by_reading());
        }

        let pending_entries = self.pending_entries.as_mut().unwrap();

        for line in text.lines() {
            let mut fields = line.trim_end_matches('\r').split('\t');

            let (Some(reading), Some(left), Some(right), Some(cost), Some(surface)) = (
                fields.next(),
                fields.next(),
                fields.next(),
                fields.next(),
                fields.next(),
            ) else {
                continue;
            };

            let (Ok(left_id), Ok(right_id), Ok(cost)) = (
                left.parse::<u16>(),
                right.parse::<u16>(),
                cost.parse::<i32>(),
            ) else {
                continue;
            };

            pending_entries
                .entry(reading.to_string())
                .or_default()
                .push(DictEntry {
                    surface: surface.to_string(),
                    left_id,
                    right_id,
                    cost,
                });
        }
    }

    /// 読み込み後に呼ぶ。
    pub fn finish(&mut self) {
        let Some(pending_entries) = self.pending_entries.take() else {
            return;
        };

        self.nodes.clear();
        self.nodes.push(TrieNode::new('\0'));
        self.entries.clear();

        // HashMapの列挙順に依存しないTrieを生成する。
        let mut readings: Vec<_> = pending_entries.into_iter().collect();
        readings.sort_unstable_by(|a, b| a.0.cmp(&b.0));

        for (reading, mut candidates) in readings {
            candidates.sort_by_key(|entry| entry.cost);
            self.insert_finalized(&reading, candidates);
        }
    }

    /// 読みの完全一致で候補を検索する（生起コスト昇順）。
    ///
    /// 引数:
    /// * `reading`: 読み
    ///
    /// 戻り値:
    /// * `&[DictEntry]`: 候補のリスト
    pub fn lookup(&self, reading: &str) -> &[DictEntry] {
        // finish()前でも、構築中の候補を検索できるようにする。
        if let Some(pending_entries) = &self.pending_entries {
            if let Some(candidates) = pending_entries.get(reading) {
                return candidates;
            }
        }

        let Some(node_index) = self.find_node(reading) else {
            return &[];
        };

        self.candidates_at(node_index)
    }

    /// 入力の先頭から一致するすべての読みを検索する。
    ///
    /// 引数:
    /// * `input`: 検索対象の読み
    ///
    /// 戻り値:
    /// * `Vec<PrefixMatch>`: 一致した読みと候補のリスト
    pub fn common_prefix_search<'input, 'dictionary>(
        &'dictionary self,
        input: &'input str,
    ) -> Vec<PrefixMatch<'input, 'dictionary>> {
        let mut matches = Vec::new();

        // 構築中にも接頭辞検索ができるようにする。
        if let Some(pending_entries) = &self.pending_entries {
            for (byte_start, ch) in input.char_indices() {
                let byte_end = byte_start + ch.len_utf8();
                let reading = &input[..byte_end];

                if let Some(candidates) = pending_entries.get(reading) {
                    matches.push(PrefixMatch {
                        reading,
                        candidates,
                    });
                }
            }

            return matches;
        }

        let mut node_index = 0;

        for (byte_start, ch) in input.char_indices() {
            let Some(next_index) = self.find_child(node_index, ch) else {
                break;
            };

            node_index = next_index;

            let node = &self.nodes[node_index as usize];

            if node.entry_count > 0 {
                let byte_end = byte_start + ch.len_utf8();

                matches.push(PrefixMatch {
                    reading: &input[..byte_end],
                    candidates: self.candidates_at(node_index),
                });
            }
        }

        matches
    }

    /// 指定ノードから子ノードを探す。
    /// 
    /// 引数:
    /// * `parent_index`: 親ノードのインデックス
    /// * `ch`: 子ノードが表す文字
    ///
    /// 戻り値:
    /// * `Option<u32>`: 子ノードのインデックス
    fn find_child(&self, parent_index: u32, ch: char) -> Option<u32> {
        let mut child_index = self.nodes[parent_index as usize].first_child;

        while child_index != INVALID_INDEX {
            let child = &self.nodes[child_index as usize];

            if child.ch == ch {
                return Some(child_index);
            }

            child_index = child.next_sibling;
        }

        None
    }

    /// 読み全体に対応するノードを探す。
    /// 
    /// 引数:
    /// * `reading`: 検索対象の読み
    /// 
    /// 戻り値:
    /// * `Option<u32>`: ノードのインデックス
    fn find_node(&self, reading: &str) -> Option<u32> {
        let mut node_index = 0;

        for ch in reading.chars() {
            node_index = self.find_child(node_index, ch)?;
        }

        Some(node_index)
    }

    /// 子ノードを追加する。
    /// 
    /// 引数:
    /// * `parent_index`: 親ノードのインデックス
    /// * `ch`: 子ノードが表す文字
    /// 
    /// 戻り値:
    /// * `u32`: 新しく追加した子ノードのインデックス
    fn add_child(&mut self, parent_index: u32, ch: char) -> u32 {
        let new_index = u32::try_from(self.nodes.len())
            .expect("dictionary has too many Trie nodes");

        let first_child = self.nodes[parent_index as usize].first_child;

        self.nodes.push(TrieNode {
            ch,
            first_child: INVALID_INDEX,
            next_sibling: first_child,
            entry_start: 0,
            entry_count: 0,
        });

        self.nodes[parent_index as usize].first_child = new_index;

        new_index
    }

    /// 完成済みの候補群をTrieへ登録する。
    /// 
    /// 引数:
    /// * `reading`: 読み
    /// * `candidates`: 候補のリスト
    fn insert_finalized(&mut self, reading: &str, candidates: Vec<DictEntry>) {
        let mut node_index = 0;

        for ch in reading.chars() {
            node_index = match self.find_child(node_index, ch) {
                Some(index) => index,
                None => self.add_child(node_index, ch),
            };
        }

        debug_assert_eq!(
            self.nodes[node_index as usize].entry_count,
            0,
            "duplicate reading should be checked before insertion"
        );

        let entry_start = u32::try_from(self.entries.len())
            .expect("dictionary has too many entries");
        let entry_count = u32::try_from(candidates.len())
            .expect("too many candidates for one reading");

        self.entries.extend(candidates);

        let node = &mut self.nodes[node_index as usize];
        node.entry_start = entry_start;
        node.entry_count = entry_count;
    }

    /// 指定ノードに格納されている候補を取得する。
    /// 
    /// 引数:
    /// * `node_index`: ノードのインデックス
    /// 
    /// 戻り値:
    /// * `&[DictEntry]`: 候補のリスト
    fn candidates_at(&self, node_index: u32) -> &[DictEntry] {
        let node = &self.nodes[node_index as usize];

        let start = node.entry_start as usize;
        let end = start + node.entry_count as usize;

        self.entries.get(start..end).unwrap_or(&[])
    }

    /// Trieの内容を、読みごとの候補マップへ複製する。
    /// 
    /// 引数:
    /// * `result`: 候補のマップ
    /// 
    /// 戻り値:
    /// * `HashMap<String, Vec<DictEntry>>`: 候補のマップ
    fn collect_entries_by_reading(&self) -> HashMap<String, Vec<DictEntry>> {
        let mut result = HashMap::new();
        let mut path = String::new();

        self.collect_node_entries(0, &mut path, &mut result);

        result
    }

    /// ノード以下の登録済み候補を再帰的に収集する。
    /// 
    /// 引数:
    /// * `node_index`: ノードのインデックス
    /// * `path`: 現在のパス
    /// * `result`: 候補のマップ
    /// 
    /// 戻り値:
    /// * `HashMap<String, Vec<DictEntry>>`: 候補のマップ
    fn collect_node_entries(
        &self,
        node_index: u32,
        path: &mut String,
        result: &mut HashMap<String, Vec<DictEntry>>,
    ) {
        let node = &self.nodes[node_index as usize];

        if node.entry_count > 0 {
            result.insert(
                path.clone(),
                self.candidates_at(node_index).to_vec(),
            );
        }

        let mut child_index = node.first_child;

        while child_index != INVALID_INDEX {
            let (ch, next_sibling) = {
                let child = &self.nodes[child_index as usize];
                (child.ch, child.next_sibling)
            };

            path.push(ch);
            self.collect_node_entries(child_index, path, result);
            path.pop();

            child_index = next_sibling;
        }
    }
}

/// バイナリ読み込み用構造体
struct BinaryReader<'a> {
    bytes: &'a [u8], // 読み込むバイナリデータ
    position: usize, // 現在の読み位置
}

impl<'a> BinaryReader<'a> {
    /// コンストラクタ。
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            position: 0,
        }
    }

    /// 指定バイト数を読み込む。
    /// 
    /// 引数:
    /// * `length`: 読み込むバイト数
    /// 
    /// 戻り値:
    /// * `Result<&'a [u8], &'static str>`: 読み込んだバイト列
    fn read_exact(&mut self, length: usize) -> Result<&'a [u8], &'static str> {
        let end = self
            .position
            .checked_add(length)
            .ok_or("dictionary is too large")?;

        let value = self
            .bytes
            .get(self.position..end)
            .ok_or("truncated dictionary")?;

        self.position = end;

        Ok(value)
    }

    /// u16を読み込む。
    /// 
    /// 戻り値:
    /// * `Result<u16, &'static str>`: 読み込んだu16
    fn read_u16(&mut self) -> Result<u16, &'static str> {
        Ok(u16::from_le_bytes(
            self.read_exact(2)?.try_into().unwrap(),
        ))
    }

    /// u32を読み込む。
    /// 
    /// 戻り値:
    /// * `Result<u32, &'static str>`: 読み込んだu32
    fn read_u32(&mut self) -> Result<u32, &'static str> {
        Ok(u32::from_le_bytes(
            self.read_exact(4)?.try_into().unwrap(),
        ))
    }

    /// i32を読み込む。
    /// 
    /// 戻り値:
    /// * `Result<i32, &'static str>`: 読み込んだi32
    fn read_i32(&mut self) -> Result<i32, &'static str> {
        Ok(i32::from_le_bytes(
            self.read_exact(4)?.try_into().unwrap(),
        ))
    }

    /// 文字列を読み込む。
    /// 
    /// 戻り値:
    /// * `Result<String, &'static str>`: 読み込んだ文字列
    fn read_string(&mut self) -> Result<String, &'static str> {
        let length = self.read_u32()? as usize;

        String::from_utf8(self.read_exact(length)?.to_vec())
            .map_err(|_| "invalid UTF-8")
    }

    /// 読み込み位置が末尾かどうかを返す。
    fn is_empty(&self) -> bool {
        self.position == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_returns_lowest_cost_first() {
        let mut dictionary = Dictionary::new();

        dictionary.load_tsv(
            "きょう\t1\t1\t3000\t京\n\
             きょう\t2\t2\t2000\t今日\n",
        );
        dictionary.finish();

        assert_eq!(dictionary.lookup("きょう")[0].surface, "今日");
        assert!(dictionary.lookup("ないよみ").is_empty());
    }

    #[test]
    fn loads_mozc_dictionary_entry() {
        let mut dictionary = Dictionary::default();

        dictionary.load_tsv(include_str!("dictionary/dictionary06.txt"));
        dictionary.finish();

        assert!(
            dictionary
                .lookup("にほんご")
                .iter()
                .any(|entry| entry.surface == "日本語")
        );
    }

    #[test]
    fn loads_embedded_dictionary_entry() {
        assert!(
            Dictionary::embedded()
                .lookup("にほんご")
                .iter()
                .any(|entry| entry.surface == "日本語")
        );
    }

    #[test]
    fn loads_embedded_ut_dictionary_entry() {
        assert!(
            Dictionary::embedded()
                .lookup("あぁあのなつかしのしゅうぉっち")
                .iter()
                .any(|entry| entry.surface == "あぁあの懐かしのシュウォッチ")
        );
    }

    #[test]
    fn rejects_invalid_dictionary_bytes() {
        assert!(matches!(
            Dictionary::from_bytes(b"XXXXXXXX"),
            Err("invalid dictionary magic")
        ));
    }

    #[test]
    fn common_prefix_search_returns_all_matching_readings() {
        let mut dictionary = Dictionary::new();

        dictionary.load_tsv(
            "きょ\t1\t1\t3000\tきょ\n\
             きょう\t1\t1\t3000\t京\n\
             きょう\t2\t2\t2000\t今日\n\
             きょうは\t1\t1\t3000\t今日は\n",
        );
        dictionary.finish();

        let matches = dictionary.common_prefix_search("きょうはいい");
        let readings: Vec<&str> =
            matches.iter().map(|item| item.reading).collect();

        assert_eq!(readings, vec!["きょ", "きょう", "きょうは"]);
        assert_eq!(matches[1].candidates[0].surface, "今日");
        assert!(dictionary.common_prefix_search("みつからない").is_empty());
    }

    #[test]
    fn common_prefix_search_respects_utf8_boundaries() {
        let mut dictionary = Dictionary::new();

        dictionary.load_tsv(
            "に\t1\t1\t1000\t二\n\
             にほん\t1\t1\t1000\t日本\n",
        );
        dictionary.finish();

        let matches = dictionary.common_prefix_search("にほんご");

        assert_eq!(
            matches.iter().map(|item| item.reading).collect::<Vec<_>>(),
            vec!["に", "にほん"]
        );
        assert_eq!(matches[0].candidates[0].surface, "二");
        assert_eq!(matches[1].candidates[0].surface, "日本");
    }

    #[test]
    fn lookup_still_works_after_loading_more_tsv() {
        let mut dictionary = Dictionary::new();

        dictionary.load_tsv("にほん\t1\t1\t2000\t日本\n");
        dictionary.finish();

        dictionary.load_tsv("ご\t1\t1\t1000\t語\n");
        dictionary.finish();

        assert_eq!(dictionary.lookup("にほん")[0].surface, "日本");
        assert_eq!(dictionary.lookup("ご")[0].surface, "語");
    }
}
