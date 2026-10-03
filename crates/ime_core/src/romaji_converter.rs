use crate::romaji_table::ROMAJI_TABLE;

const VOWELS: [char; 5] = ['a', 'i', 'u', 'e', 'o']; // 母音
const CONSONANTS: &str = "bcdfghjklmpqrstvwxyz"; // 「n」以外の子音

const ROMAJI_N: char = 'n'; // 「n」
const ROMAJI_Y: char = 'y'; // 「y」
const ROMAJI_X: char = 'x'; // 「x」
const ROMAJI_APOSTROPHE: char = '\''; // 「'」
const KANA_N: &str = "ん"; // 「ん」
const KANA_SMALL_TSU: &str = "っ"; // 「っ」

/// ローマ字入力の変換結果
#[derive(Debug, PartialEq, Eq)]
pub enum RomajiConversionResult {
    Pending, // 未変換
    Converted { // 変換結果
        kana: String, // ひらがな
        consumed: usize, // 消費したローマ字の文字数
    },
    Invalid, // 無効な入力
}

/// ローマ字をひらがなに変換する
/// 
/// 引数
/// * `input`: ローマ字入力
/// 
/// 戻り値
/// * `RomajiConversionResult`: 変換結果
pub fn convert(input: &str) -> RomajiConversionResult {
    // 入力が空の場合は未変換
    if input.is_empty() {
        return RomajiConversionResult::Pending;
    }

    // 「ん」の判定処理
    if let Some(consumed) = convert_n(input) {
        return RomajiConversionResult::Converted {
            kana: KANA_N.to_string(),
            consumed,
        };
    }

    // 「っ」の判定処理
    if is_sokuon(input) {
        return RomajiConversionResult::Converted {
            kana: KANA_SMALL_TSU.to_string(),
            consumed: 1,
        };
    }

    // 変換候補の検索（最長一致）
    let mut best_match: Option<(&str, &str)> = None;

    for &(romaji, kana) in ROMAJI_TABLE {
        if input.starts_with(romaji) {
            match best_match {
                Some((best_romaji, _)) if best_romaji.len() >= romaji.len() => {}
                _ => {
                    best_match = Some((romaji, kana));
                }
            }
        }
    }

    if let Some((romaji, kana)) = best_match {
        return RomajiConversionResult::Converted {
            kana: kana.to_string(),
            consumed: romaji.len(),
        };
    }

    // まだ長くなれば変換できる可能性があるか
    if ROMAJI_TABLE
        .iter()
        .any(|(romaji, _)| romaji.starts_with(input))
    {
        return RomajiConversionResult::Pending;
    }

    RomajiConversionResult::Invalid
}

/// 「ん」の変換
/// 
/// 引数
/// * `input`: ローマ字入力
/// 
/// 戻り値
/// * `Option<usize>`: 「ん」に変換した場合に消費したローマ字の文字数
fn convert_n(input: &str) -> Option<usize> {
    let chars: Vec<char> = input.chars().collect();

    // 「nn」 → 「ん」
    if chars.len() >= 2 && chars[0] == ROMAJI_N && chars[1] == ROMAJI_N {
        return Some(2);
    }

    // 「xn」 → 「ん」
    if chars.len() >= 2 && chars[0] == ROMAJI_X && chars[1] == ROMAJI_N {
        return Some(2);
    }

    // 「n'」 → 「ん」
    if chars.len() >= 2 && chars[0] == ROMAJI_N && chars[1] == ROMAJI_APOSTROPHE {
        return Some(2);
    }

    // 「n」1文字だけでは「ん」にしない
    if chars.len() == 1 {
        return None;
    }

    // 「n」 + 母音 / y はまだ「ん」にしない
    if chars[0] == ROMAJI_N
        && (VOWELS.contains(&chars[1]) || chars[1] == ROMAJI_Y)
    {
        return None;
    }

    // 「n」 + 子音 → 「ん」
    if chars[0] == ROMAJI_N && CONSONANTS.contains(chars[1]) {
        return Some(1);
    }

    // 「n」 + その他 → 「ん」
    if chars[0] == ROMAJI_N {
        return Some(1);
    }

    None
}

/// 小さい「つ」の判定処理
/// 
/// 引数
/// * `input`: ローマ字入力
/// 
/// 戻り値
/// * `bool`: 小さい「つ」に変換できるかどうか
fn is_sokuon(input: &str) -> bool {
    let chars: Vec<char> = input.chars().collect();

    // 2文字未満は小さい「つ」にしない
    if chars.len() < 2 {
        return false;
    }

    // 最初の2文字が同じ子音
    chars[0] == chars[1] && CONSONANTS.contains(chars[0]) && chars[0] != ROMAJI_N
}


/// かな変換に関するテスト
#[cfg(test)]
mod tests {
    use super::*;

    fn assert_converted(input: &str, expected_kana: &str, expected_consumed: usize) {
        assert_eq!(
            convert(input),
            RomajiConversionResult::Converted {
                kana: expected_kana.to_string(),
                consumed: expected_consumed,
            }
        );
    }

    fn assert_pending(input: &str) {
        assert_eq!(convert(input), RomajiConversionResult::Pending);
    }

    fn assert_invalid(input: &str) {
        assert_eq!(convert(input), RomajiConversionResult::Invalid);
    }

    // ============================================================
    // 基本母音
    // ============================================================

    #[test]
    fn test_vowels() {
        assert_converted("a", "あ", 1);
        assert_converted("i", "い", 1);
        assert_converted("u", "う", 1);
        assert_converted("e", "え", 1);
        assert_converted("o", "お", 1);
    }

    // ============================================================
    // 基本的な2文字ローマ字
    // ============================================================

    #[test]
    fn test_basic_kana_rows() {
        let cases = [
            ("ka", "か"),
            ("ki", "き"),
            ("ku", "く"),
            ("ke", "け"),
            ("ko", "こ"),
            ("sa", "さ"),
            ("si", "し"),
            ("su", "す"),
            ("se", "せ"),
            ("so", "そ"),
            ("ta", "た"),
            ("ti", "ち"),
            ("tu", "つ"),
            ("te", "て"),
            ("to", "と"),
            ("na", "な"),
            ("ni", "に"),
            ("nu", "ぬ"),
            ("ne", "ね"),
            ("no", "の"),
            ("ha", "は"),
            ("hi", "ひ"),
            ("hu", "ふ"),
            ("he", "へ"),
            ("ho", "ほ"),
            ("ma", "ま"),
            ("mi", "み"),
            ("mu", "む"),
            ("me", "め"),
            ("mo", "も"),
            ("ya", "や"),
            ("yu", "ゆ"),
            ("yo", "よ"),
            ("ra", "ら"),
            ("ri", "り"),
            ("ru", "る"),
            ("re", "れ"),
            ("ro", "ろ"),
            ("wa", "わ"),
            ("wi", "うぃ"),
            ("wu", "う"),
            ("we", "うぇ"),
            ("wo", "を"),
            ("ga", "が"),
            ("gi", "ぎ"),
            ("gu", "ぐ"),
            ("ge", "げ"),
            ("go", "ご"),
            ("za", "ざ"),
            ("zi", "じ"),
            ("zu", "ず"),
            ("ze", "ぜ"),
            ("zo", "ぞ"),
            ("da", "だ"),
            ("di", "ぢ"),
            ("du", "づ"),
            ("de", "で"),
            ("do", "ど"),
            ("ba", "ば"),
            ("bi", "び"),
            ("bu", "ぶ"),
            ("be", "べ"),
            ("bo", "ぼ"),
            ("pa", "ぱ"),
            ("pi", "ぴ"),
            ("pu", "ぷ"),
            ("pe", "ぺ"),
            ("po", "ぽ"),
        ];

        for (input, expected) in cases {
            assert_converted(input, expected, input.len());
        }
    }

    // ============================================================
    // ヘボン式
    // ============================================================

    #[test]
    fn test_hebon_style() {
        assert_converted("shi", "し", 3);
        assert_converted("chi", "ち", 3);
        assert_converted("tsu", "つ", 3);

        assert_converted("ja", "じゃ", 2);
        assert_converted("ju", "じゅ", 2);
        assert_converted("je", "じぇ", 2);
        assert_converted("jo", "じょ", 2);

        assert_converted("ji", "じ", 2);
    }

    // ============================================================
    // 拗音
    // ============================================================

    #[test]
    fn test_youon() {
        let cases = [
            ("kya", "きゃ"),
            ("kyi", "きぃ"),
            ("kyu", "きゅ"),
            ("kye", "きぇ"),
            ("kyo", "きょ"),
            ("sya", "しゃ"),
            ("syi", "しぃ"),
            ("syu", "しゅ"),
            ("sye", "しぇ"),
            ("syo", "しょ"),
            ("sha", "しゃ"),
            ("shu", "しゅ"),
            ("she", "しぇ"),
            ("sho", "しょ"),
            ("tya", "ちゃ"),
            ("tyi", "ちぃ"),
            ("tyu", "ちゅ"),
            ("tye", "ちぇ"),
            ("tyo", "ちょ"),
            ("cha", "ちゃ"),
            ("chu", "ちゅ"),
            ("che", "ちぇ"),
            ("cho", "ちょ"),
            ("cya", "ちゃ"),
            ("cyi", "ちぃ"),
            ("cyu", "ちゅ"),
            ("cye", "ちぇ"),
            ("cyo", "ちょ"),
            ("nya", "にゃ"),
            ("nyi", "にぃ"),
            ("nyu", "にゅ"),
            ("nye", "にぇ"),
            ("nyo", "にょ"),
            ("hya", "ひゃ"),
            ("hyi", "ひぃ"),
            ("hyu", "ひゅ"),
            ("hye", "ひぇ"),
            ("hyo", "ひょ"),
            ("mya", "みゃ"),
            ("myi", "みぃ"),
            ("myu", "みゅ"),
            ("mye", "みぇ"),
            ("myo", "みょ"),
            ("rya", "りゃ"),
            ("ryi", "りぃ"),
            ("ryu", "りゅ"),
            ("rye", "りぇ"),
            ("ryo", "りょ"),
            ("gya", "ぎゃ"),
            ("gyi", "ぎぃ"),
            ("gyu", "ぎゅ"),
            ("gye", "ぎぇ"),
            ("gyo", "ぎょ"),
            ("zya", "じゃ"),
            ("zyi", "じぃ"),
            ("zyu", "じゅ"),
            ("zye", "じぇ"),
            ("zyo", "じょ"),
            ("jya", "じゃ"),
            ("jyi", "じぃ"),
            ("jyu", "じゅ"),
            ("jye", "じぇ"),
            ("jyo", "じょ"),
            ("bya", "びゃ"),
            ("byi", "びぃ"),
            ("byu", "びゅ"),
            ("bye", "びぇ"),
            ("byo", "びょ"),
            ("pya", "ぴゃ"),
            ("pyi", "ぴぃ"),
            ("pyu", "ぴゅ"),
            ("pye", "ぴぇ"),
            ("pyo", "ぴょ"),
        ];

        for (input, expected) in cases {
            assert_converted(input, expected, input.len());
        }
    }

    // ============================================================
    // 外来音・特殊なかな
    // ============================================================

    #[test]
    fn test_foreign_sound_combinations() {
        let cases = [
            ("fya", "ふゃ"),
            ("fyu", "ふゅ"),
            ("fyo", "ふょ"),
            ("hwa", "ふぁ"),
            ("hwi", "ふぃ"),
            ("hwe", "ふぇ"),
            ("hwo", "ふぉ"),
            ("fyi", "ふぃ"),
            ("fye", "ふぇ"),
            ("kwa", "くぁ"),
            ("kwi", "くぃ"),
            ("kwu", "くぅ"),
            ("kwe", "くぇ"),
            ("kwo", "くぉ"),
            ("gwa", "ぐぁ"),
            ("gwi", "ぐぃ"),
            ("gwu", "ぐぅ"),
            ("gwe", "ぐぇ"),
            ("gwo", "ぐぉ"),
            ("wha", "うぁ"),
            ("whi", "うぃ"),
            ("whu", "う"),
            ("whe", "うぇ"),
            ("who", "うぉ"),
            ("ye", "いぇ"),
            ("va", "ゔぁ"),
            ("vi", "ゔぃ"),
            ("vu", "ゔ"),
            ("ve", "ゔぇ"),
            ("vo", "ゔぉ"),
            ("vya", "ゔゃ"),
            ("vyi", "ゔぃ"),
            ("vyu", "ゔゅ"),
            ("vye", "ゔぇ"),
            ("vyo", "ゔょ"),
        ];

        for (input, expected) in cases {
            assert_converted(input, expected, input.len());
        }
    }

    // ============================================================
    // 小書き文字
    // ============================================================

    #[test]
    fn test_small_vowels_and_kana() {
        let cases = [
            ("xa", "ぁ"),
            ("xi", "ぃ"),
            ("xu", "ぅ"),
            ("xe", "ぇ"),
            ("xo", "ぉ"),
            ("la", "ぁ"),
            ("li", "ぃ"),
            ("lu", "ぅ"),
            ("le", "ぇ"),
            ("lo", "ぉ"),
            ("xya", "ゃ"),
            ("xyu", "ゅ"),
            ("xyo", "ょ"),
            ("lya", "ゃ"),
            ("lyu", "ゅ"),
            ("lyo", "ょ"),
            ("xwa", "ゎ"),
            ("lwa", "ゎ"),
            ("xka", "ゕ"),
            ("lka", "ゕ"),
            ("xke", "ゖ"),
            ("lke", "ゖ"),
            ("lyi", "ぃ"),
            ("xyi", "ぃ"),
            ("lye", "ぇ"),
            ("xye", "ぇ"),
        ];

        for (input, expected) in cases {
            assert_converted(input, expected, input.len());
        }
    }

    // ============================================================
    // 「ん」
    // ============================================================

    #[test]
    fn test_nn() {
        assert_converted("nn", "ん", 2);
        assert_converted("n'", "ん", 2);
        assert_converted("xn", "ん", 2);
    }

    #[test]
    fn test_n_before_consonant() {
        assert_converted("nka", "ん", 1);
        assert_converted("nsa", "ん", 1);
        assert_converted("nta", "ん", 1);
        assert_converted("nma", "ん", 1);
        assert_converted("nra", "ん", 1);
    }

    #[test]
    fn test_n_before_symbol() {
        assert_converted("n.", "ん", 1);
        assert_converted("n,", "ん", 1);
        assert_converted("n-", "ん", 1);
        assert_converted("n[", "ん", 1);
        assert_converted("n]", "ん", 1);
    }

    #[test]
    fn test_single_n_is_pending() {
        assert_pending("n");
    }

    #[test]
    fn test_n_before_vowel_or_y() {
        assert_converted("na", "な", 2);
        assert_converted("ni", "に", 2);
        assert_converted("nu", "ぬ", 2);
        assert_converted("ne", "ね", 2);
        assert_converted("no", "の", 2);

        assert_pending("n");
        assert_pending("ny");
    }

    // ============================================================
    // 「っ」
    // ============================================================

    #[test]
    fn test_sokuon_by_double_consonant() {
        assert_converted("kk", "っ", 1);
        assert_converted("ss", "っ", 1);
        assert_converted("tt", "っ", 1);
        assert_converted("pp", "っ", 1);
        assert_converted("gg", "っ", 1);
        assert_converted("dd", "っ", 1);
        assert_converted("bb", "っ", 1);
        assert_converted("cc", "っ", 1);
        assert_converted("jj", "っ", 1);
        assert_converted("zz", "っ", 1);
    }

    #[test]
    fn test_explicit_sokuon() {
        assert_converted("ltu", "っ", 3);
        assert_converted("xtu", "っ", 3);
        assert_converted("ltsu", "っ", 4);
        assert_converted("xtsu", "っ", 4);
    }

    #[test]
    fn test_tcha_sokuon() {
        assert_converted("tcha", "っちゃ", 4);
        assert_converted("tchi", "っち", 4);
        assert_converted("tchu", "っちゅ", 4);
        assert_converted("tche", "っちぇ", 4);
        assert_converted("tcho", "っちょ", 4);
    }

    // ============================================================
    // 部分入力 → Pending
    // ============================================================

    #[test]
    fn test_pending_partial_input() {
        let cases = [
            "k",
            "ky",
            "s",
            "sh",
            "t",
            "ch",
            "ts",
            "n",
            "h",
            "f",
            "m",
            "y",
            "r",
            "w",
            "g",
            "z",
            "d",
            "b",
            "p",
            "v",
            "x",
            "l",
            "q",
            "wh",
            "kw",
            "gw",
        ];

        for input in cases {
            assert_pending(input);
        }
    }

    // ============================================================
    // 無効入力
    // ============================================================

    #[test]
    fn test_invalid_input() {
        assert_invalid("qz");
        assert_invalid("jx");
        assert_invalid("bk");
        assert_invalid("dv");
    }

    // ============================================================
    // 最長一致・消費文字数
    // ============================================================

    #[test]
    fn test_longest_match() {
        assert_converted("kya", "きゃ", 3);
        assert_converted("shi", "し", 3);
        assert_converted("chi", "ち", 3);
        assert_converted("tsu", "つ", 3);
        assert_converted("tcha", "っちゃ", 4);
        assert_converted("hwyu", "ふゅ", 4);
    }

    #[test]
    fn test_consumed_length() {
        assert_converted("a", "あ", 1);
        assert_converted("ka", "か", 2);
        assert_converted("kya", "きゃ", 3);
        assert_converted("hwyu", "ふゅ", 4);
        assert_converted("nn", "ん", 2);
        assert_converted("nka", "ん", 1);
        assert_converted("kk", "っ", 1);
    }

    // ============================================================
    // 記号
    // ============================================================

    #[test]
    fn test_punctuation() {
        assert_converted(".", "。", 1);
        assert_converted(",", "、", 1);
        assert_converted("-", "ー", 1);
        assert_converted("~", "～", 1);
        assert_converted("[", "「", 1);
        assert_converted("]", "」", 1);
    }

    #[test]
    fn test_full_width_symbols() {
        let cases = [
            ("!", "！"),
            ("?", "？"),
            (":", "："),
            (";", "；"),
            ("(", "（"),
            (")", "）"),
            ("{", "｛"),
            ("}", "｝"),
            ("<", "＜"),
            (">", "＞"),
            ("/", "／"),
            ("\\", "＼"),
            ("|", "｜"),
            ("@", "＠"),
            ("#", "＃"),
            ("$", "＄"),
            ("%", "％"),
            ("&", "＆"),
            ("*", "＊"),
            ("+", "＋"),
            ("=", "＝"),
            ("^", "＾"),
            ("`", "｀"),
            ("_", "＿"),
            ("'", "＇"),
            ("\"", "＂"),
        ];

        for (input, expected) in cases {
            assert_converted(input, expected, input.len());
        }
    }

    // ============================================================
    // 全角数字
    // ============================================================

    #[test]
    fn test_full_width_numbers() {
        let cases = [
            ("0", "０"),
            ("1", "１"),
            ("2", "２"),
            ("3", "３"),
            ("4", "４"),
            ("5", "５"),
            ("6", "６"),
            ("7", "７"),
            ("8", "８"),
            ("9", "９"),
        ];

        for (input, expected) in cases {
            assert_converted(input, expected, 1);
        }
    }

    // ============================================================
    // Zコマンド
    // ============================================================

    #[test]
    fn test_z_commands() {
        let cases = [
            ("z/", "・"),
            ("z.", "…"),
            ("z,", "‥"),
            ("zh", "←"),
            ("zj", "↓"),
            ("zk", "↑"),
            ("zl", "→"),
            ("z-", "～"),
            ("z[", "『"),
            ("z]", "』"),
        ];

        for (input, expected) in cases {
            assert_converted(input, expected, 2);
        }
    }

    // ============================================================
    // 空文字
    // ============================================================

    #[test]
    fn test_empty_input() {
        assert_pending("");
    }
}