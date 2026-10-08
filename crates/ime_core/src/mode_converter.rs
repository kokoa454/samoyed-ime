use crate::half_width_katakana_table::HALF_WIDTH_KATAKANA_TABLE;

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
        ' ' => ' ',
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
                HALF_WIDTH_KATAKANA_TABLE.iter()
                    .find_map(|&(full, half)| {
                        if half.chars().next() == Some(ch) {
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