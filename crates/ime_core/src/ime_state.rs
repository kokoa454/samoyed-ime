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
#[derive(Clone, PartialEq, Eq)]
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

    /// 文字を入力する。
    ///
    /// 引数
    /// * `input`: 入力文字
    ///
    /// 戻り値
    /// * `String`: 変換後の文字
    pub fn input_char(&mut self, input: char) -> String {
        let kana = self.romaji_input.input(input);

        for ch in kana.chars() {
            self.composition.insert(ch);
        }

        kana
    }

    /// 現在の表示文字列を取得する。
    ///
    /// 戻り値
    /// * `String`: 画面に表示する文字列
    pub fn get_display_text(&self) -> String {
        let content_units = self.composition.get_content_units();
        let cursor_pos = self
            .composition
            .get_cursor_pos()
            .min(content_units.len());
        let pending = self.romaji_input.get_pending_input();

        let mut text = String::new();

        for ch in &content_units[..cursor_pos] {
            text.push(*ch);
        }

        text.push_str(pending);

        for ch in &content_units[cursor_pos..] {
            text.push(*ch);
        }

        text
    }

    /// 現在のカーソル位置を取得する。
    ///
    /// 戻り値
    /// * `usize`: カーソル位置
    pub fn get_cursor_pos(&self) -> usize {
        self.composition.get_cursor_pos()
            + self.romaji_input.get_pending_input().chars().count()
    }

    /// Compositionが存在するかどうかを取得する。
    ///
    /// 戻り値
    /// * `bool`: Compositionが存在する場合はtrue
    pub fn has_composition(&self) -> bool {
        self.composition.get_content_units_len() > 0
    }

    /// 未変換ローマ字が存在するかどうかを取得する。
    ///
    /// 戻り値
    /// * `bool`: 未変換ローマ字が存在する場合はtrue
    pub fn has_pending_input(&self) -> bool {
        self.romaji_input.has_pending_input()
    }

    /// 入力文字を受け付けられるかどうかを取得する。
    ///
    /// 引数
    /// * `input`: 入力文字
    ///
    /// 戻り値
    /// * `bool`: 入力を受け付けられる場合はtrue
    pub fn can_input_char(&self, input: char) -> bool {
        self.romaji_input.can_accept_input(input)
    }

    /// 入力中の文字が存在するかどうかを取得する。
    ///
    /// 戻り値
    /// * `bool`: 入力中の文字が存在する場合はtrue
    pub fn has_active_input(&self) -> bool {
        self.has_composition() || self.has_pending_input()
    }

    /// Backspaceを実行する。
    pub fn backspace(&mut self) {
        if self.romaji_input.has_pending_input() {
            self.romaji_input.backspace();
            return;
        }

        self.composition.backspace();
    }

    /// Deleteを実行する。
    pub fn delete(&mut self) {
        if self.romaji_input.has_pending_input() {
            return;
        }

        self.composition.delete();
    }

    /// カーソルを左へ移動する。
    pub fn move_cursor_left(&mut self) {
        if self.romaji_input.has_pending_input() {
            return;
        }

        self.composition.move_cursor_left();
    }

    /// カーソルを右へ移動する。
    pub fn move_cursor_right(&mut self) {
        if self.romaji_input.has_pending_input() {
            return;
        }

        self.composition.move_cursor_right();
    }

    /// カーソルを先頭へ移動する。
    pub fn move_cursor_to_head(&mut self) {
        if self.romaji_input.has_pending_input() {
            return;
        }

        self.composition.move_cursor_to_head();
    }

    /// カーソルを末尾へ移動する。
    pub fn move_cursor_to_tail(&mut self) {
        if self.romaji_input.has_pending_input() {
            return;
        }

        self.composition.move_cursor_to_tail();
    }

    /// 入力状態をすべてクリアする。
    pub fn clear(&mut self) {
        self.composition.clear();
        self.romaji_input.clear();
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
        state.input_char('!');
        state.input_char('?');
        state.input_char(':');
        state.input_char(';');

        assert_eq!(
            composition_to_string(&state),
            "。、！？：；"
        );
    }

    #[test]
    fn test_symbol_commands() {
        let mut state = ImeState::new();

        for ch in "z/".chars() {
            state.input_char(ch);
        }

        for ch in "z.".chars() {
            state.input_char(ch);
        }

        for ch in "z,".chars() {
            state.input_char(ch);
        }

        assert_eq!(
            composition_to_string(&state),
            "・…‥"
        );
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
    fn test_pending_input_is_displayed() {
        let mut state = ImeState::new();

        state.input_char('k');

        assert_eq!(state.get_display_text(), "k");
        assert_eq!(state.get_cursor_pos(), 1);
    }

    #[test]
    fn test_pending_input_is_updated_after_completion() {
        let mut state = ImeState::new();

        state.input_char('k');

        assert_eq!(composition_to_string(&state), "");

        state.input_char('a');

        assert_eq!(composition_to_string(&state), "か");
        assert_eq!(state.get_display_text(), "か");
        assert_eq!(state.get_cursor_pos(), 1);
    }

    #[test]
    fn test_pending_input_in_middle() {
        let mut state = ImeState::new();

        state.input_char('a');
        state.input_char('i');
        state.input_char('u');

        state.move_cursor_left();

        state.input_char('k');

        assert_eq!(state.get_display_text(), "あいkう");
        assert_eq!(state.get_cursor_pos(), 3);

        state.input_char('a');

        assert_eq!(state.get_display_text(), "あいかう");
        assert_eq!(state.get_cursor_pos(), 3);
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