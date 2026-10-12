use std::collections::HashMap;
use std::sync::OnceLock;

const BINARY_MAGIC: &[u8; 8] = b"SMYDIC01";

/// 辞書のエントリ
#[derive(Debug, Clone)]
pub struct DictEntry {
    pub surface: String, // 表記
    pub left_id: u16,    // 左文脈ID
    pub right_id: u16,   // 右文脈ID
    pub cost: i32,       // 生起コスト
}

/// 読みから候補を検索する辞書。
#[derive(Debug, Default)]
pub struct Dictionary {
    entries: HashMap<String, Vec<DictEntry>>, // キーは読み
}

impl Dictionary {
    /// コンストラクタ。
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// ビルド時に生成したMozc辞書を初回アクセス時に読み込む。
    /// 
    /// 戻り値:
    /// * `&'static Self`: 辞書の参照
    pub fn embedded() -> &'static Self {
        static DICTIONARY: OnceLock<Dictionary> = OnceLock::new();
        DICTIONARY.get_or_init(|| {
            Self::from_bytes(include_bytes!(concat!(env!("OUT_DIR"), "/dictionary.bin")))
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

        let key_count = reader.read_u32()? as usize;
        let mut entries = HashMap::with_capacity(key_count);
        for _ in 0..key_count {
            let reading = reader.read_string()?;
            let candidate_count = reader.read_u32()? as usize;
            let mut candidates = Vec::with_capacity(candidate_count);
            for _ in 0..candidate_count {
                candidates.push(DictEntry {
                    left_id: reader.read_u16()?,
                    right_id: reader.read_u16()?,
                    cost: reader.read_i32()?,
                    surface: reader.read_string()?,
                });
            }
            entries.insert(reading, candidates);
        }

        if !reader.is_empty() {
            return Err("trailing bytes in dictionary");
        }
        Ok(Self { entries })
    }

    /// Mozc形式（読み\t左ID\t右ID\tコスト\t表記）のテキストを読み込む。
    /// 不正な行は無視し、読み込めたエントリを辞書へ追加する。
    ///
    /// 引数:
    /// * `text`: Mozc形式のテキスト
    pub fn load_tsv(&mut self, text: &str) {
        for line in text.lines() {
            let mut f = line.trim_end_matches('\r').split('\t');
            let (Some(reading), Some(l), Some(r), Some(c), Some(surface)) =
                (f.next(), f.next(), f.next(), f.next(), f.next())
            else {
                continue;
            };
            let (Ok(l), Ok(r), Ok(c)) = (l.parse::<u16>(), r.parse::<u16>(), c.parse::<i32>())
            else {
                continue;
            };
            self.entries
                .entry(reading.to_string())
                .or_default()
                .push(DictEntry {
                    surface: surface.to_string(),
                    left_id: l,
                    right_id: r,
                    cost: c,
                });
        }
    }

    /// 読み込み後に呼ぶ。候補をコストの小さい順に並べる。
    pub fn finish(&mut self) {
        for v in self.entries.values_mut() {
            v.sort_by_key(|e| e.cost);
        }
    }

    /// 読みの完全一致で候補を引く（コスト昇順）。
    ///
    /// 引数:
    /// * `reading`: 読み
    ///
    /// 戻り値:
    /// * `&[DictEntry]`: 候補のリスト
    pub fn lookup(&self, reading: &str) -> &[DictEntry] {
        self.entries.get(reading).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// バイナリ読み込み用構造体
struct BinaryReader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> BinaryReader<'a> {
    /// コンストラクタ。
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    /// 指定バイト数を読み込む。
    ///
    /// 引数:
    /// * `length`: 読み込むバイト数
    ///
    /// 戻り値:
    /// * `Result<&'a [u8], &'static str>`: 読み込んだバイト列、またはエラー
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
    /// * `Result<u16, &'static str>`: 読み込んだu16、またはエラー
    fn read_u16(&mut self) -> Result<u16, &'static str> {
        Ok(u16::from_le_bytes(self.read_exact(2)?.try_into().unwrap()))
    }

    /// u32を読み込む。
    ///
    /// 戻り値:
    /// * `Result<u32, &'static str>`: 読み込んだu32、またはエラー
    fn read_u32(&mut self) -> Result<u32, &'static str> {
        Ok(u32::from_le_bytes(self.read_exact(4)?.try_into().unwrap()))
    }

    /// i32を読み込む。
    /// 
    /// 戻り値:
    /// * `Result<i32, &'static str>`: 読み込んだi32、またはエラー
    fn read_i32(&mut self) -> Result<i32, &'static str> {
        Ok(i32::from_le_bytes(self.read_exact(4)?.try_into().unwrap()))
    }

    /// 文字列を読み込む。
    ///
    /// 戻り値:
    /// * `Result<String, &'static str>`: 読み込んだ文字列、またはエラー
    fn read_string(&mut self) -> Result<String, &'static str> {
        let length = self.read_u32()? as usize;
        String::from_utf8(self.read_exact(length)?.to_vec()).map_err(|_| "invalid UTF-8")
    }

    /// 読み込み位置が末尾かどうかを返す。
    ///
    /// 戻り値:
    /// * `bool`: 末尾ならtrue、そうでなければfalse
    fn is_empty(&self) -> bool {
        self.position == self.bytes.len()
    }
}


/// テスト
#[test]
fn lookup_returns_lowest_cost_first() {
    let mut d = Dictionary::new();
    d.load_tsv("きょう\t1\t1\t3000\t京\nきょう\t2\t2\t2000\t今日\n");
    d.finish();
    assert_eq!(d.lookup("きょう")[0].surface, "今日");
    assert!(d.lookup("ないよみ").is_empty());
}

#[test]
fn loads_mozc_dictionary_entry() {
    let mut dictionary = Dictionary::default();
    dictionary.load_tsv(include_str!("dictionary/dictionary06.txt"));
    dictionary.finish();

    assert!(dictionary
        .lookup("にほんご")
        .iter()
        .any(|entry| entry.surface == "日本語"));
}

#[test]
fn loads_embedded_dictionary_entry() {
    assert!(Dictionary::embedded()
        .lookup("にほんご")
        .iter()
        .any(|entry| entry.surface == "日本語"));
}

#[test]
fn rejects_invalid_dictionary_bytes() {
    assert!(matches!(
        Dictionary::from_bytes(b"XXXXXXXX"),
        Err("invalid dictionary magic")
    ));
}
