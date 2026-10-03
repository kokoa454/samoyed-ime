use crate::composition::Composition;
use crate::romaji_input::RomajiInput;

/// 入力モード
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Hiragana, // ひらがな
    FullWidthKatakana, // 全角カタカナ
    HalfWidthKatakana, // 半角カタカナ
    FullWidthAlphanumeric, // 全角英数字
    HalfWidthAlphanumeric, // 半角英数字
}

/// IMEの状態を管理する構造体
pub struct ImeState {
    input_mode: InputMode, // 現在の入力モード
    composition: Composition, // 現在の未確定文字列
    romaji_input: RomajiInput, // ローマ字入力
}

impl ImeState {
    /// コンストラクタ
    pub fn new() -> Self {
        Self {
            input_mode: InputMode::Hiragana,
            composition: Composition::new(),
            romaji_input: RomajiInput::new(),
        }
    }

    /// キーボードから文字を入力する。
    /// 
    /// 引数
    /// * `input`: 入力された文字
    /// 
    /// 戻り値
    /// * `Vec<char>`: 入力された文字をひらがなに変換した文字列
    pub fn input_char(&mut self, input: char) {
        let kana = self.romaji_input.input(input);

        for ch in kana.chars() {
            self.composition.insert(ch);
        }
    }
}


/// テスト
#[cfg(test)]
mod tests {
    use super::*;

    fn composition_to_string(state: &ImeState) -> String {
        state.composition.get_content_units().iter().collect()
    }

    // ============================================================
    // 基本入力
    // ============================================================

    #[test]
    fn test_single_vowel() {
        let mut state = ImeState::new();

        state.input_char('a');

        assert_eq!(composition_to_string(&state), "あ");
    }

    #[test]
    fn test_basic_kana() {
        let mut state = ImeState::new();

        state.input_char('k');
        state.input_char('a');

        assert_eq!(composition_to_string(&state), "か");
    }

    #[test]
    fn test_youon() {
        let mut state = ImeState::new();

        state.input_char('k');
        state.input_char('y');
        state.input_char('a');

        assert_eq!(composition_to_string(&state), "きゃ");
    }

    // ============================================================
    // 連続入力
    // ============================================================

    #[test]
    fn test_continuous_input() {
        let mut state = ImeState::new();

        for ch in "watashi".chars() {
            state.input_char(ch);
        }

        assert_eq!(composition_to_string(&state), "わたし");
    }

    #[test]
    fn test_multiple_kana() {
        let mut state = ImeState::new();

        for ch in "kakikukeko".chars() {
            state.input_char(ch);
        }

        assert_eq!(composition_to_string(&state), "かきくけこ");
    }

    // ============================================================
    // 促音
    // ============================================================

    #[test]
    fn test_sokuon() {
        let mut state = ImeState::new();

        for ch in "kka".chars() {
            state.input_char(ch);
        }

        assert_eq!(composition_to_string(&state), "っか");
    }

    #[test]
    fn test_mixed_sokuon() {
        let mut state = ImeState::new();

        for ch in "katta".chars() {
            state.input_char(ch);
        }

        assert_eq!(composition_to_string(&state), "かった");
    }

    // ============================================================
    // 「ん」
    // ============================================================

    #[test]
    fn test_nn() {
        let mut state = ImeState::new();

        for ch in "nn".chars() {
            state.input_char(ch);
        }

        assert_eq!(composition_to_string(&state), "ん");
    }

    #[test]
    fn test_n_before_consonant() {
        let mut state = ImeState::new();

        for ch in "nka".chars() {
            state.input_char(ch);
        }

        assert_eq!(composition_to_string(&state), "んか");
    }

    #[test]
    fn test_nna() {
        let mut state = ImeState::new();

        for ch in "nna".chars() {
            state.input_char(ch);
        }

        assert_eq!(composition_to_string(&state), "んあ");
    }

    // ============================================================
    // ヘボン式
    // ============================================================

    #[test]
    fn test_hebon_style() {
        let mut state = ImeState::new();

        for ch in "shi".chars() {
            state.input_char(ch);
        }

        assert_eq!(composition_to_string(&state), "し");
    }

    #[test]
    fn test_chi() {
        let mut state = ImeState::new();

        for ch in "chi".chars() {
            state.input_char(ch);
        }

        assert_eq!(composition_to_string(&state), "ち");
    }

    // ============================================================
    // 記号
    // ============================================================

    #[test]
    fn test_symbols() {
        let mut state = ImeState::new();

        state.input_char('.');
        state.input_char(',');

        assert_eq!(composition_to_string(&state), "。、");
    }

    // ============================================================
    // 小書き文字
    // ============================================================

    #[test]
    fn test_small_kana() {
        let mut state = ImeState::new();

        for ch in "xya".chars() {
            state.input_char(ch);
        }

        assert_eq!(composition_to_string(&state), "ゃ");
    }

    // ============================================================
    // Compositionへの反映
    // ============================================================

    #[test]
    fn test_pending_input_does_not_update_composition() {
        let mut state = ImeState::new();

        state.input_char('k');

        assert_eq!(composition_to_string(&state), "");
    }

    #[test]
    fn test_pending_input_is_updated_after_completion() {
        let mut state = ImeState::new();

        state.input_char('k');
        assert_eq!(composition_to_string(&state), "");

        state.input_char('a');
        assert_eq!(composition_to_string(&state), "か");
    }

    // ============================================================
    // 長い入力
    // ============================================================

    #[test]
    fn test_sentence_like_input() {
        let mut state = ImeState::new();

        for ch in "watashihagakuseidesu".chars() {
            state.input_char(ch);
        }

        assert_eq!(composition_to_string(&state), "わたしはがくせいです");
    }
}