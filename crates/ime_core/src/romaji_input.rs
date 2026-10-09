use crate::romaji_converter::{convert, RomajiConversionResult};


/// 未確定のローマ字入力を管理する構造体
#[derive(Clone, PartialEq, Eq)]
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
        let mut output = String::new();

        // 前回までに「変換できない」と判定したローマ字を確定
        while matches!(convert(&self.pending), RomajiConversionResult::Invalid) {
            output.push(self.pending.remove(0));
        }

        self.pending.push(input);

        loop {
            match convert(&self.pending) {
                // 変換できた場合
                RomajiConversionResult::Converted { kana, consumed } => {
                    output.push_str(&kana);
                    self.pending.drain(..consumed);
                }

                // 変換に必要な文字が足りない場合と、変換できない場合は保留
                RomajiConversionResult::Pending | RomajiConversionResult::Invalid => {
                    break;
                }
            }
        }

        output
    }

    /// 未変換のローマ字を1文字削除する。
    ///
    /// 戻り値
    /// * `true`: 削除した場合
    /// * `false`: 未変換文字列が空の場合
    pub(crate) fn backspace(&mut self) -> bool {
        self.pending.pop().is_some()
    }

    /// 未変換のローマ字入力が存在するかを取得する。
    ///
    /// 戻り値
    /// * `true`: 未変換文字列が存在する場合
    /// * `false`: 未変換文字列が空の場合
    pub(crate) fn has_pending_input(&self) -> bool {
        !self.pending.is_empty()
    }

    /// 未変換のローマ字入力をクリアする。
    pub(crate) fn clear(&mut self) {
        self.pending.clear();
    }

    /// 未変換のローマ字入力を取得する。
    ///
    /// 戻り値
    /// * `&str`: 未変換のローマ字入力
    pub(crate) fn get_pending_input(&self) -> &str {
        &self.pending
    }

    /// 入力文字をローマ字変換で受け付けられるかを判定する。
    ///
    /// 引数
    /// * `input`: 入力文字
    ///
    /// 戻り値
    /// * `true`: 入力を受け付けられる場合
    /// * `false`: 入力を受け付けない場合
    pub(crate) fn can_accept_input(&self, input: char) -> bool {
        let input = if input.is_ascii_alphabetic() {
            input.to_ascii_lowercase()
        } else {
            input
        };

        !matches!(
            convert(&input.to_string()),
            RomajiConversionResult::Invalid
        )
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

        assert_eq!(input.get_pending_input(), "ky");
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

        assert_eq!(input.input('q'), "");
        assert_eq!(input.input('x'), "");

        assert_eq!(input.input('a'), "qぁ");
    }

    #[test]
    fn test_invalid_input_can_recover() {
        let mut input = RomajiInput::new();

        assert_eq!(input.input('q'), "");
        assert_eq!(input.input('z'), "");

        assert_eq!(input.input('a'), "qざ");
    }

    #[test]
    fn test_invalid_pair_is_kept_as_pending() {
        let mut input = RomajiInput::new();

        assert_eq!(input.input('k'), "");
        assert_eq!(input.input('l'), "");

        assert_eq!(input.get_pending_input(), "kl");
    }

    #[test]
    fn test_backspace_after_invalid_pair_keeps_leading_char() {
        let mut input = RomajiInput::new();

        assert_eq!(input.input('k'), "");
        assert_eq!(input.input('l'), "");

        assert_eq!(input.backspace(), true);
        assert_eq!(input.get_pending_input(), "k");

        assert_eq!(input.input('e'), "け");
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

    // ============================================================
    // Backspace
    // ============================================================

    #[test]
    fn test_backspace_pending() {
        let mut input = RomajiInput::new();

        assert_eq!(input.input('k'), "");
        assert_eq!(input.input('y'), "");

        assert_eq!(input.backspace(), true);
        assert_eq!(input.input('a'), "か");
    }

    #[test]
    fn test_backspace_empty_pending() {
        let mut input = RomajiInput::new();

        assert_eq!(input.backspace(), false);
    }
}