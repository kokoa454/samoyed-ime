use crate::romaji_converter::{convert, RomajiConversionResult};

/// 未確定のローマ字入力を管理する構造体
pub struct RomajiInput {
    pending: String, // ひらがなとして確定していないローマ字
}

impl RomajiInput {
    /// コンストラクタ
    pub fn new() -> Self {
        Self {
            pending: String::new(),
        }
    }

    /// ローマ字を1文字入力する。
    /// 
    /// 引数
    /// * `input`: 入力されたローマ字
    /// 
    /// 戻り値
    /// * `String`: 入力されたローマ字をひらがなに変換した文字列
    pub fn input(&mut self, input: char) -> String {
        self.pending.push(input);

        let mut output = String::new();

        loop {
            match convert(&self.pending) {
                // 変換できた場合
                RomajiConversionResult::Converted { kana, consumed } => {
                    output.push_str(&kana);
                    self.pending.drain(..consumed);
                }

                // 変換するローマ字が足りない場合
                RomajiConversionResult::Pending => {
                    break;
                }

                // 変換できないローマ字の場合
                RomajiConversionResult::Invalid => {
                    output.push(self.pending.remove(0));
                }
            }
        }

        output
    }
}

/// テスト
#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // 基本入力
    // ============================================================

    #[test]
    fn test_single_vowels() {
        let mut input = RomajiInput::new();

        assert_eq!(input.input('a'), "あ");
        assert_eq!(input.input('i'), "い");
        assert_eq!(input.input('u'), "う");
        assert_eq!(input.input('e'), "え");
        assert_eq!(input.input('o'), "お");
    }

    #[test]
    fn test_basic_kana() {
        let mut input = RomajiInput::new();

        assert_eq!(input.input('k'), "");
        assert_eq!(input.input('a'), "か");
    }

    #[test]
    fn test_youon() {
        let mut input = RomajiInput::new();

        assert_eq!(input.input('k'), "");
        assert_eq!(input.input('y'), "");
        assert_eq!(input.input('a'), "きゃ");
    }

    #[test]
    fn test_hebon_style() {
        let mut input = RomajiInput::new();

        assert_eq!(input.input('s'), "");
        assert_eq!(input.input('h'), "");
        assert_eq!(input.input('i'), "し");
    }

    // ============================================================
    // 促音
    // ============================================================

    #[test]
    fn test_sokuon() {
        let mut input = RomajiInput::new();

        assert_eq!(input.input('k'), "");
        assert_eq!(input.input('k'), "っ");
        assert_eq!(input.input('a'), "か");
    }

    #[test]
    fn test_explicit_sokuon() {
        let mut input = RomajiInput::new();

        assert_eq!(input.input('x'), "");
        assert_eq!(input.input('t'), "");
        assert_eq!(input.input('s'), "");
        assert_eq!(input.input('u'), "っ");
    }

    #[test]
    fn test_tcha() {
        let mut input = RomajiInput::new();

        assert_eq!(input.input('t'), "");
        assert_eq!(input.input('c'), "");
        assert_eq!(input.input('h'), "");
        assert_eq!(input.input('a'), "っちゃ");
    }

    // ============================================================
    // 「ん」
    // ============================================================

    #[test]
    fn test_nn() {
        let mut input = RomajiInput::new();

        assert_eq!(input.input('n'), "");
        assert_eq!(input.input('n'), "ん");
    }

    #[test]
    fn test_n_before_consonant() {
        let mut input = RomajiInput::new();

        assert_eq!(input.input('n'), "");
        assert_eq!(input.input('k'), "ん");
        assert_eq!(input.input('a'), "か");
    }

    #[test]
    fn test_n_apostrophe() {
        let mut input = RomajiInput::new();

        assert_eq!(input.input('n'), "");
        assert_eq!(input.input('\''), "ん");
    }

    #[test]
    fn test_xn() {
        let mut input = RomajiInput::new();

        assert_eq!(input.input('x'), "");
        assert_eq!(input.input('n'), "ん");
    }

    // ============================================================
    // 未確定状態
    // ============================================================

    #[test]
    fn test_pending_input() {
        let mut input = RomajiInput::new();

        assert_eq!(input.input('k'), "");
        assert_eq!(input.input('y'), "");
    }

    #[test]
    fn test_pending_is_resolved_by_next_input() {
        let mut input = RomajiInput::new();

        assert_eq!(input.input('k'), "");
        assert_eq!(input.input('y'), "");
        assert_eq!(input.input('a'), "きゃ");
    }

    // ============================================================
    // 連続した入力
    // ============================================================

    #[test]
    fn test_continuous_input() {
        let mut input = RomajiInput::new();

        let mut output = String::new();

        output.push_str(&input.input('w'));
        output.push_str(&input.input('a'));
        output.push_str(&input.input('t'));
        output.push_str(&input.input('a'));
        output.push_str(&input.input('s'));
        output.push_str(&input.input('h'));
        output.push_str(&input.input('i'));

        assert_eq!(output, "わたし");
    }

    #[test]
    fn test_multiple_kana() {
        let mut input = RomajiInput::new();

        let mut output = String::new();

        output.push_str(&input.input('k'));
        output.push_str(&input.input('a'));
        output.push_str(&input.input('k'));
        output.push_str(&input.input('i'));
        output.push_str(&input.input('k'));
        output.push_str(&input.input('u'));

        assert_eq!(output, "かきく");
    }

    #[test]
    fn test_mixed_sokuon_and_kana() {
        let mut input = RomajiInput::new();

        let mut output = String::new();

        output.push_str(&input.input('k'));
        output.push_str(&input.input('a'));
        output.push_str(&input.input('t'));
        output.push_str(&input.input('t'));
        output.push_str(&input.input('e'));

        assert_eq!(output, "かって");
    }

    // ============================================================
    // 記号
    // ============================================================

    #[test]
    fn test_symbols() {
        let mut input = RomajiInput::new();

        assert_eq!(input.input('.'), "。");
        assert_eq!(input.input(','), "、");
        assert_eq!(input.input('-'), "ー");
        assert_eq!(input.input('['), "「");
        assert_eq!(input.input(']'), "」");
    }

    // ============================================================
    // Invalid
    // ============================================================

    #[test]
    fn test_invalid_input_does_not_stuck() {
        let mut input = RomajiInput::new();

        // "qx" は無効
        // q をそのまま出力して、x は次の入力に持ち越す
        assert_eq!(input.input('q'), "");
        assert_eq!(input.input('x'), "q");

        // 残った "x" に "a" が続くと "ぁ" になる
        assert_eq!(input.input('a'), "ぁ");
    }

    #[test]
    fn test_invalid_input_can_recover() {
        let mut input = RomajiInput::new();

        assert_eq!(input.input('q'), "");
        assert_eq!(input.input('z'), "q");

        // 残った z を使って次の入力を続ける
        assert_eq!(input.input('a'), "ざ");
    }

    // ============================================================
    // 複数文字を1回の入力で生成
    // ============================================================

    #[test]
    fn test_kka() {
        let mut input = RomajiInput::new();

        assert_eq!(input.input('k'), "");
        assert_eq!(input.input('k'), "っ");
        assert_eq!(input.input('a'), "か");
    }

    #[test]
    fn test_nka() {
        let mut input = RomajiInput::new();

        assert_eq!(input.input('n'), "");
        assert_eq!(input.input('k'), "ん");
        assert_eq!(input.input('a'), "か");
    }

    #[test]
    fn test_nna() {
        let mut input = RomajiInput::new();

        assert_eq!(input.input('n'), "");
        assert_eq!(input.input('n'), "ん");
        assert_eq!(input.input('a'), "あ");
    }
}