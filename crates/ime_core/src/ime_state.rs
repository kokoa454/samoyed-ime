use crate::composition::Composition;
use crate::mode_converter::{
    full_width_alphanumeric_to_half_width_alphanumeric,
    full_width_katakana_to_hiragana,
    half_width_alphanumeric_to_full_width_alphanumeric,
    half_width_katakana_to_full_width_katakana,
    hiragana_to_full_width_katakana,
    hiragana_to_half_width_katakana,
};
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

/// 入力モード切替コマンド
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeCommand {
    Hiragana, // ひらがなキー
    FullWidthKatakana, // Shift+ひらがな / カタカナキー
    SwitchKanaType, // 無変換キー
    ToggleAlphanumeric, // 英数キー
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

    /// 現在の入力モードを取得する。
    ///
    /// 戻り値
    /// * `InputMode`: 現在の入力モード
    pub fn get_input_mode(&self) -> InputMode {
        self.input_mode
    }

    /// 現在の入力モードを設定する。
    ///
    /// 引数
    /// * `mode`: 新しい入力モード
    pub fn set_input_mode(&mut self, mode: InputMode) {
        self.input_mode = mode;
    }

    /// モード切替コマンドを適用する。
    ///
    /// 引数
    /// * `command`: モード切替コマンド
    pub fn apply_mode_command(&mut self, command: ModeCommand) {
        match command {
            // ひらがなキー: ひらがなモードへ
            ModeCommand::Hiragana => self.input_mode = InputMode::Hiragana,

            // カタカナキー: 全角カタカナモードへ
            ModeCommand::FullWidthKatakana => {
                self.input_mode = InputMode::FullWidthKatakana
            }

            // 無変換キー: かな種別を巡回
            ModeCommand::SwitchKanaType => self.switch_kana_type(),

            // 英数キー: ひらがな ⇔ 半角英数
            ModeCommand::ToggleAlphanumeric => self.toggle_alphanumeric_mode(),
        }
    }

    /// かな種別を巡回させる（無変換キー）。
    ///
    /// 引数
    /// * `mode`: 新しい入力モード
    fn switch_kana_type(&mut self) {
        self.input_mode = match self.input_mode {
            InputMode::Hiragana => InputMode::FullWidthKatakana,
            InputMode::FullWidthKatakana => InputMode::HalfWidthKatakana,
            InputMode::HalfWidthKatakana => InputMode::Hiragana,
            InputMode::FullWidthAlphanumeric | InputMode::HalfWidthAlphanumeric => {
                self.input_mode
            }
        };
    }

    /// ひらがなモードと半角英数モードをトグルする。
    ///
    /// 引数
    /// * `mode`: 新しい入力モード
    fn toggle_alphanumeric_mode(&mut self) {
        self.input_mode = match self.input_mode {
            InputMode::Hiragana => InputMode::HalfWidthAlphanumeric,
            _ => InputMode::Hiragana,
        };
    }

    /// モードに応じて文字列を変換する。
    ///
    /// 引数
    /// * `text`: 変換する文字列
    ///
    /// 戻り値
    /// * `String`: 変換後の文字列
    fn convert_text(&self, text: &str) -> String {
        //半角カタカナを全角カタカナに、ひらがなに変換
        let normalized = {
            let full_width_kana = half_width_katakana_to_full_width_katakana(text);
            let hiragana = full_width_katakana_to_hiragana(&full_width_kana);
            half_width_alphanumeric_to_full_width_alphanumeric(&hiragana)
        };

        match self.input_mode {
            // ひらがな
            InputMode::Hiragana => normalized,
            // 全角カタカナ
            InputMode::FullWidthKatakana => hiragana_to_full_width_katakana(&normalized),
            // 半角カタカナ
            InputMode::HalfWidthKatakana => hiragana_to_half_width_katakana(&normalized),
            // 全角英数
            InputMode::FullWidthAlphanumeric => normalized.clone(),
            // 半角英数
            InputMode::HalfWidthAlphanumeric => full_width_alphanumeric_to_half_width_alphanumeric(&normalized),
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
        let raw = match self.input_mode {
            // 英数モードはローマ字変換せず、そのまま入れる
            InputMode::FullWidthAlphanumeric | InputMode::HalfWidthAlphanumeric => {
                input.to_string()
            }
            // かな系モードはローマ字変換する
            _ => self.romaji_input.input(input),
        };

        let converted = self.convert_text(&raw);

        for ch in converted.chars() {
            self.composition.insert(ch);
        }

        converted
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

        // カーソル位置までの確定文字をモード変換し直す
        let before_cursor: String = content_units[..cursor_pos].iter().collect();
        text.push_str(&self.convert_text(&before_cursor));

        // 未確定ローマ字もモードに応じて変換して表示する
        text.push_str(&self.convert_text(pending));

        // カーソル位置以降の確定文字をモード変換し直す
        let after_cursor: String = content_units[cursor_pos..].iter().collect();
        text.push_str(&self.convert_text(&after_cursor));

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
        match self.input_mode {
            InputMode::FullWidthAlphanumeric | InputMode::HalfWidthAlphanumeric => true,
            _ => input.is_ascii_alphabetic() || self.romaji_input.can_accept_input(input),
        }
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

        assert_eq!(state.get_display_text(), "ｋ");
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

        assert_eq!(state.get_display_text(), "あいｋう");
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

    // ============================================================
    // 無効なローマ字入力
    // ============================================================

    #[test]
    fn test_backspace_after_invalid_romaji_pair_removes_only_pending_char() {
        let mut state = ImeState::new();

        for ch in "soreda".chars() {
            state.input_char(ch);
        }
        state.input_char('k');
        state.input_char('l');

        assert_eq!(state.get_display_text(), "それだｋｌ");

        state.backspace();

        assert_eq!(state.get_display_text(), "それだｋ");

        state.input_char('e');

        assert_eq!(state.get_display_text(), "それだけ");
    }

    #[test]
    fn test_invalid_input_does_not_stuck() {
        let mut state = ImeState::new();
        state.input_char('q');
        state.input_char('x');
        assert_eq!(state.input_char('a'), "ｑぁ");
    }

    #[test]
    fn test_invalid_input_can_recover() {
        let mut state = ImeState::new();
        state.input_char('q');
        state.input_char('z');
        assert_eq!(state.input_char('a'), "ｑざ");
    }

    // ============================================================
    // 入力モード切替
    // ============================================================

    #[test]
    fn test_switch_kana_type_cycles_kana() {
        let mut state = ImeState::new();

        state.apply_mode_command(ModeCommand::SwitchKanaType);
        assert_eq!(state.get_input_mode(), InputMode::FullWidthKatakana);

        state.apply_mode_command(ModeCommand::SwitchKanaType);
        assert_eq!(state.get_input_mode(), InputMode::HalfWidthKatakana);

        state.apply_mode_command(ModeCommand::SwitchKanaType);
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);
    }

    #[test]
    fn test_switch_kana_type_does_nothing_with_active_input() {
        let mut state = ImeState::new();

        state.input_char('k'); // 未確定ローマ字あり

        state.apply_mode_command(ModeCommand::SwitchKanaType);

        assert_eq!(state.get_input_mode(), InputMode::Hiragana);
    }

    #[test]
    fn test_switch_kana_type_keeps_alphanumeric() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::HalfWidthAlphanumeric);

        state.apply_mode_command(ModeCommand::SwitchKanaType);

        assert_eq!(state.get_input_mode(), InputMode::HalfWidthAlphanumeric);
    }

    #[test]
    fn test_toggle_alphanumeric_from_hiragana() {
        let mut state = ImeState::new();

        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);
        assert_eq!(state.get_input_mode(), InputMode::HalfWidthAlphanumeric);

        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);
    }

    #[test]
    fn test_toggle_alphanumeric_from_katakana_returns_to_hiragana() {
        let mut state = ImeState::new();

        state.apply_mode_command(ModeCommand::FullWidthKatakana);
        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);

        // カタカナから英数を押すとひらがなに戻る（Mozc の ToggleInputMode と同じ）
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);
    }

    #[test]
    fn test_hiragana_command_sets_hiragana() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::FullWidthKatakana);

        state.apply_mode_command(ModeCommand::Hiragana);

        assert_eq!(state.get_input_mode(), InputMode::Hiragana);
    }

    // ============================================================
    // モード別の文字変換
    // ============================================================

    #[test]
    fn test_pending_is_full_width_in_full_width_katakana() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::FullWidthKatakana);

        state.input_char('k');

        assert_eq!(state.get_display_text(), "ｋ");
    }

    #[test]
    fn test_pending_is_half_width_in_half_width_katakana() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::HalfWidthKatakana);

        state.input_char('k');

        // 半角カタカナモードは半角英数と同じ見た目になる
        assert_eq!(state.get_display_text(), "k");
    }

    #[test]
    fn test_half_width_katakana_converts_voiced_kana() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::HalfWidthKatakana);

        for ch in "ga".chars() {
            state.input_char(ch);
        }

        assert_eq!(composition_to_string(&state), "ｶﾞ");
    }

    #[test]
    fn test_full_width_alphanumeric_mode() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::FullWidthAlphanumeric);

        state.input_char('a');

        assert_eq!(composition_to_string(&state), "ａ");
    }
}