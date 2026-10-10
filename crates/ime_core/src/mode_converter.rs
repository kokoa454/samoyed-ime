use crate::half_width_katakana_table::HALF_WIDTH_KATAKANA_TABLE;
use crate::romaji_input::RomajiInput;
use crate::romaji_table::ROMAJI_TABLE;


/// 半角英数字を全角英数字に変換する。
///
/// 引数
/// * `text`: 変換する文字列
///
/// 戻り値
/// * `String`: 変換後の文字列
pub fn half_width_alphanumeric_to_full_width_alphanumeric(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            ' ' => '　',
            c if c.is_ascii_graphic() => {
                char::from_u32(c as u32 + 0xFEE0).unwrap_or(c)
            }
            c => c,
        })
        .collect()
}

/// ひらがなをカタカナに変換する。
///
/// 引数
/// * `text`: 変換する文字列
///
/// 戻り値
/// * `String`: 変換後の文字列
pub fn hiragana_to_full_width_katakana(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            '\u{3041}'..='\u{3096}' | '\u{309D}'..='\u{309E}' => {
                char::from_u32(ch as u32 + 0x60).unwrap_or(ch)
            }
            c => c,
        })
        .collect()
}

/// 全角英数字の1文字を半角英数字に変換する。
///
/// 引数
/// * `ch`: 変換する文字
///
/// 戻り値
/// * `char`: 変換後の文字
fn to_half_width_alphanumeric_char(ch: char) -> char {
    match ch {
        ' ' | '　' => ' ',
        '\u{FF01}'..='\u{FF5E}' => char::from_u32(ch as u32 - 0xFEE0).unwrap_or(ch),
        c => c,
    }
}

/// 全角英数字を半角英数字に変換する。
///
/// 引数
/// * `text`: 変換する文字列
///
/// 戻り値
/// * `String`: 変換後の文字列
pub fn full_width_alphanumeric_to_half_width_alphanumeric(text: &str) -> String {
    text.chars().map(to_half_width_alphanumeric_char).collect()
}

/// ひらがなを半角カタカナに変換する。
///
/// 引数
/// * `text`: 変換する文字列
///
/// 戻り値
/// * `String`: 変換後の文字列
pub fn hiragana_to_half_width_katakana(text: &str) -> String {
    let mut result = String::with_capacity(text.len() * 2);
    
    for ch in text.chars() {
        if let Some(&(_, half)) = HALF_WIDTH_KATAKANA_TABLE.iter().find(|&&(kana, _)| kana == ch) {
            result.push_str(half);
        } else {
            result.push(to_half_width_alphanumeric_char(ch));
        }
    }
    
    result
}

/// カタカナをひらがなに変換する。
///
/// 引数
/// * `text`: 変換する文字列
///
/// 戻り値
/// * `String`: 変換後の文字列
pub fn full_width_katakana_to_hiragana(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            '\u{30A1}'..='\u{30F6}' | '\u{30FD}'..='\u{30FE}' => {
                char::from_u32(ch as u32 - 0x60).unwrap_or(ch)
            }
            c => c,
        })
        .collect()
}

/// 半角カタカナを全角カタカナに変換する。
///
/// 引数
/// * `text`: 変換する文字列
///
/// 戻り値
/// * `String`: 変換後の文字列
pub fn half_width_katakana_to_full_width_katakana(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        let next = chars.peek().copied();

        let mapped = match next {
            Some('ﾞ') => {
                chars.next();
                match ch {
                    'ｶ' => 'ガ', 'ｷ' => 'ギ', 'ｸ' => 'グ', 'ｹ' => 'ゲ', 'ｺ' => 'ゴ',
                    'ｻ' => 'ザ', 'ｼ' => 'ジ', 'ｽ' => 'ズ', 'ｾ' => 'ゼ', 'ｿ' => 'ゾ',
                    'ﾀ' => 'ダ', 'ﾁ' => 'ヂ', 'ﾂ' => 'ヅ', 'ﾃ' => 'デ', 'ﾄ' => 'ド',
                    'ﾊ' => 'バ', 'ﾋ' => 'ビ', 'ﾌ' => 'ブ', 'ﾍ' => 'ベ', 'ﾎ' => 'ボ',
                    'ｳ' => 'ヴ', _ => ch,
                }
            }
            Some('ﾟ') => {
                chars.next();
                match ch {
                    'ﾊ' => 'パ', 'ﾋ' => 'ピ', 'ﾌ' => 'プ', 'ﾍ' => 'ペ', 'ﾎ' => 'ポ',
                    _ => ch,
                }
            }
            _ => {
                // half が厳密に1文字 かつ その文字が ch に一致する場合のみマッチ
                HALF_WIDTH_KATAKANA_TABLE.iter()
                    .find_map(|&(full, half)| {
                        let mut half_chars = half.chars();
                        let first = half_chars.next();
                        let is_single = half_chars.next().is_none();
                        if is_single && first == Some(ch) {
                            Some(full)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(ch)
            }
        };
        result.push(mapped);
    }

    result
}

/// ひらがなをローマ字（半角英数）に変換する。
///
/// # 引数
/// * `text`: 変換する文字列
///
/// # 戻り値
/// * `String`: 変換後のローマ字文字列
pub fn hiragana_to_romaji(text: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == 'っ' {
            if i + 1 < chars.len() {
                let next_two: String = chars[i + 1..].iter().take(2).collect();
                let next_one: String = chars[i + 1..].iter().take(1).collect();

                let next_romaji = ROMAJI_TABLE
                    .iter()
                    .find(|&&(kana, _)| kana == next_two)
                    .or_else(|| {
                        ROMAJI_TABLE
                            .iter()
                            .find(|&&(kana, _)| kana == next_one)
                    })
                    .map(|&(_, romaji)| romaji);

                if let Some(romaji) = next_romaji {
                    let first = romaji.chars().next().unwrap_or('x');
                    if first.is_ascii_alphabetic() && !"aeiou".contains(first) {
                        let sokuon_char = if romaji.starts_with("ch") { 't' } else { first };
                        result.push(sokuon_char);
                        i += 1;
                        continue;
                    }
                }
            }
            result.push_str("xtsu");
            i += 1;
            continue;
        }

        // 2文字マッチを試す
        if i + 1 < chars.len() {
            let two_chars: String = chars[i..i + 2].iter().collect();
            if let Some(&(_, romaji)) = ROMAJI_TABLE.iter().find(|&&(kana, _)| kana == two_chars) {
                result.push_str(romaji);
                i += 2;
                continue;
            }
        }

        // 1文字マッチを試す
        let one_char: String = chars[i..i + 1].iter().collect();
        if let Some(&(_, romaji)) = ROMAJI_TABLE.iter().find(|&&(kana, _)| kana == one_char) {
            result.push_str(romaji);
            i += 1;
            continue;
        }

        // マッチしない文字（全角英数なら半角化、その他はそのまま）
        let ch = chars[i];
        result.push(to_half_width_alphanumeric_char(ch));
        i += 1;
    }

    result
}

/// ローマ字（半角英数）をひらがなに変換する。
///
/// # 引数
/// * `text`: 変換する文字列
///
/// # 戻り値
/// * `String`: 変換後のひらがな文字列
pub fn romaji_to_hiragana(text: &str) -> String {
    let mut romaji_input = RomajiInput::new();
    let mut result = String::new();

    for ch in text.chars() {
        let ch_lower = ch.to_ascii_lowercase();
        if ch_lower.is_ascii_alphabetic() {
            result.push_str(&romaji_input.input(ch_lower));
        } else {
            let pending = romaji_input.get_pending_input();
            if pending == "n" {
                result.push('ん');
            } else {
                result.push_str(pending);
            }
            romaji_input.clear();
            result.push(ch);
        }
    }

    let pending = romaji_input.get_pending_input();
    if pending == "n" {
        result.push('ん');
    } else {
        result.push_str(pending);
    }

    result
}


/// テスト
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hiragana_to_romaji_basic() {
        assert_eq!(hiragana_to_romaji("ああああ"), "aaaa");
        assert_eq!(hiragana_to_romaji("かきくけこ"), "kakikukeko");
        assert_eq!(hiragana_to_romaji("さくら"), "sakura");
        assert_eq!(hiragana_to_romaji("とうきょう"), "toukyou");
        assert_eq!(hiragana_to_romaji("がっこう"), "gakkou");
        assert_eq!(hiragana_to_romaji("にほん"), "nihon");
    }

    #[test]
    fn test_romaji_to_hiragana_basic() {
        assert_eq!(romaji_to_hiragana("aaaa"), "ああああ");
        assert_eq!(romaji_to_hiragana("kakikukeko"), "かきくけこ");
        assert_eq!(romaji_to_hiragana("sakura"), "さくら");
        assert_eq!(romaji_to_hiragana("toukyou"), "とうきょう");
        assert_eq!(romaji_to_hiragana("gakkou"), "がっこう");
        assert_eq!(romaji_to_hiragana("nihon"), "にほん");
    }

    #[test]
    fn test_bidirectional_conversion() {
        assert_eq!(romaji_to_hiragana(&hiragana_to_romaji("ああああ")), "ああああ");
        assert_eq!(hiragana_to_romaji(&romaji_to_hiragana("aaaa")), "aaaa");
    }
}