use crate::composition::Composition;
use crate::mode_converter::{
    full_width_alphanumeric_to_half_width_alphanumeric,
    full_width_katakana_to_hiragana,
    half_width_alphanumeric_to_full_width_alphanumeric,
    half_width_katakana_to_full_width_katakana,
    hiragana_to_full_width_katakana,
    hiragana_to_half_width_katakana,
    hiragana_to_romaji,
    romaji_to_hiragana,
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
    SetInputMode(InputMode), // 指定した入力モードへ変更
    SwitchKanaType, // 無変換キー：かな種別を巡回
    ToggleAlphanumeric, // 英数キー
}

/// IMEの状態を管理する構造体
#[derive(Clone, PartialEq, Eq)]
pub struct ImeState {
    input_mode: InputMode, // 現在の入力モード
    conversion_mode: Option<InputMode>, // ファンクションキー等による一時変換モード
    composition: Composition, // 現在の未確定文字列
    romaji_input: RomajiInput, // ローマ字入力
}

impl ImeState {
    /// コンストラクタ
    pub fn new() -> Self {
        Self {
            input_mode: InputMode::Hiragana,
            conversion_mode: None,
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
        // 未変換ローマ字がある間はモードを変更しない
        if self.has_pending_input() {
            return;
        }

        self.conversion_mode = None;

        match command {
            // 指定された入力モードへ変更
            ModeCommand::SetInputMode(mode) => {
                self.input_mode = mode;
            }

            // 無変換キー：かな種別を巡回
            ModeCommand::SwitchKanaType => {
                self.switch_kana_type();
            }

            // 英数キー：ひらがな ⇔ 半角英数
            ModeCommand::ToggleAlphanumeric => {
                self.toggle_alphanumeric_mode();
            }
        }

        // Compositionが存在する場合、新しい入力モードに合わせてCompositionを変換
        if self.has_composition() {
            let current: String = self.composition.get_content_units().iter().collect();
            let converted = convert_text_to_mode(&current, self.input_mode);
            self.composition.clear();
            for ch in converted.chars() {
                self.composition.insert(ch);
            }
        }
    }

    /// 未確定文字列を指定された入力モードの文字種に変換する（ファンクションキー変換）。
    ///
    /// 引数
    /// * `target_mode`: 変換先の入力モード（文字種）
    pub fn convert_composition(&mut self, target_mode: InputMode) {
        if !self.has_active_input() {
            return;
        }

        // 未変換ローマ字（pending）がある場合、解決してCompositionに取り込む
        if self.romaji_input.has_pending_input() {
            let pending = self.romaji_input.get_pending_input();
            let resolved = if pending == "n" {
                "ん".to_string()
            } else {
                pending.to_string()
            };
            for ch in resolved.chars() {
                self.composition.insert(ch);
            }
            self.romaji_input.clear();
        }

        let current: String = self.composition.get_content_units().iter().collect();
        let converted = convert_text_to_mode(&current, target_mode);
        self.composition.clear();
        for ch in converted.chars() {
            self.composition.insert(ch);
        }

        self.conversion_mode = Some(target_mode);
    }

    /// かな種別を巡回させる（無変換キー）。
    fn switch_kana_type(&mut self) {
        self.input_mode = match self.input_mode {
            InputMode::Hiragana => InputMode::FullWidthKatakana,
            InputMode::FullWidthKatakana => InputMode::HalfWidthKatakana,
            InputMode::HalfWidthKatakana => InputMode::Hiragana,
            InputMode::FullWidthAlphanumeric | InputMode::HalfWidthAlphanumeric => {
                InputMode::Hiragana
            }
        };
    }

    /// ひらがなモードと半角英数モードをトグルする。
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
}

/// モード切替時にCompositionテキストを対象モード向けに変換する。
fn convert_text_to_mode(text: &str, target_mode: InputMode) -> String {
    let hiragana = {
        let half_kata = half_width_katakana_to_full_width_katakana(text);
        let full_kata_to_hira = full_width_katakana_to_hiragana(&half_kata);
        let half_alpha = full_width_alphanumeric_to_half_width_alphanumeric(&full_kata_to_hira);
        romaji_to_hiragana(&half_alpha)
    };

    match target_mode {
        InputMode::Hiragana => hiragana,
        InputMode::FullWidthKatakana => hiragana_to_full_width_katakana(&hiragana),
        InputMode::HalfWidthKatakana => hiragana_to_half_width_katakana(&hiragana),
        InputMode::HalfWidthAlphanumeric => hiragana_to_romaji(&hiragana),
        InputMode::FullWidthAlphanumeric => {
            let romaji = hiragana_to_romaji(&hiragana);
            half_width_alphanumeric_to_full_width_alphanumeric(&romaji)
        }
    }
}

impl ImeState {

    /// 文字を入力する。
    ///
    /// 引数
    /// * `input`: 入力文字
    ///
    /// 戻り値
    /// * `String`: 変換後の文字
    pub fn input_char(&mut self, input: char) -> String {
        self.conversion_mode = None;

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
        if self.conversion_mode.is_some() {
            return self.composition.get_content_units().iter().collect();
        }

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
    /// **表示テキスト上の** 文字数単位で返す。
    /// 半角カタカナモードでは が→ｶﾞ のように1ソース文字が複数表示文字になるため、
    /// ソース文字数ではなく convert_text 後の表示文字数を返す必要がある。
    ///
    /// 戻り値
    /// * `usize`: 表示テキスト上のカーソル位置（文字数単位）
    pub fn get_cursor_pos(&self) -> usize {
        if self.conversion_mode.is_some() {
            return self.composition.get_cursor_pos();
        }

        let content_units = self.composition.get_content_units();
        let source_cursor_pos = self.composition.get_cursor_pos().min(content_units.len());

        // カーソル位置までの確定文字をモード変換して表示文字数を数える
        let before_cursor: String = content_units[..source_cursor_pos].iter().collect();
        let before_display = self.convert_text(&before_cursor);

        // 未確定ローマ字もモードに応じて変換して表示文字数を数える
        let pending_display = self.convert_text(self.romaji_input.get_pending_input());

        before_display.chars().count() + pending_display.chars().count()
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

    // Backspaceを実行する。
    pub fn backspace(&mut self) {
        if self.romaji_input.has_pending_input() {
            self.romaji_input.backspace();
            return;
        }

        self.composition.backspace();
    }

    // Deleteを実行する。
    pub fn delete(&mut self) {
        if self.romaji_input.has_pending_input() {
            return;
        }

        self.composition.delete();
    }

    // カーソルを左へ移動する。
    pub fn move_cursor_left(&mut self) {
        if self.romaji_input.has_pending_input() {
            return;
        }

        self.composition.move_cursor_left();
    }

    // カーソルを右へ移動する。
    pub fn move_cursor_right(&mut self) {
        if self.romaji_input.has_pending_input() {
            return;
        }

        self.composition.move_cursor_right();
    }

    // カーソルを先頭へ移動する。
    pub fn move_cursor_to_head(&mut self) {
        if self.romaji_input.has_pending_input() {
            return;
        }

        self.composition.move_cursor_to_head();
    }

    // カーソルを末尾へ移動する。
    pub fn move_cursor_to_tail(&mut self) {
        if self.romaji_input.has_pending_input() {
            return;
        }

        self.composition.move_cursor_to_tail();
    }

    // 入力状態をすべてクリアする。
    pub fn clear(&mut self) {
        self.composition.clear();
        self.romaji_input.clear();
        self.conversion_mode = None;
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
    // ヘルパー: カーソル位置の一貫性チェック
    // ============================================================

    // get_cursor_pos() が get_display_text() の文字数を超えないことを確認する
    fn assert_cursor_within_display(state: &ImeState) {
        let display = state.get_display_text();
        let cursor = state.get_cursor_pos();
        assert!(
            cursor <= display.chars().count(),
            "cursor({}) > display_len({}) in '{}'",
            cursor, display.chars().count(), display
        );
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
    fn test_switch_kana_type_from_alphanumeric_returns_to_hiragana() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::HalfWidthAlphanumeric);

        state.apply_mode_command(ModeCommand::SwitchKanaType);
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);

        state.apply_mode_command(ModeCommand::SwitchKanaType);
        assert_eq!(state.get_input_mode(), InputMode::FullWidthKatakana);
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

        state.apply_mode_command(ModeCommand::SetInputMode(InputMode::FullWidthKatakana));
        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);

        // カタカナから英数を押すとひらがなに戻る（Mozc の ToggleInputMode と同じ）
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);
    }

    #[test]
    fn test_hiragana_command_from_any_mode() {
        let modes = [
            InputMode::FullWidthKatakana,
            InputMode::HalfWidthKatakana,
            InputMode::FullWidthAlphanumeric,
            InputMode::HalfWidthAlphanumeric,
            InputMode::Hiragana,
        ];

        for mode in modes {
            let mut state = ImeState::new();
            state.set_input_mode(mode);
            state.apply_mode_command(ModeCommand::SetInputMode(InputMode::Hiragana));
            assert_eq!(state.get_input_mode(), InputMode::Hiragana);
        }
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

    #[test]
    fn test_half_width_alphanumeric_mode_input_and_operations() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::HalfWidthAlphanumeric);

        assert_eq!(state.get_input_mode(), InputMode::HalfWidthAlphanumeric);

        // アルファベット入力
        for ch in "hello".chars() {
            state.input_char(ch);
        }
        assert_eq!(state.get_display_text(), "hello");

        // Backspace
        state.backspace();
        assert_eq!(state.get_display_text(), "hell");

        // カーソル移動と文字削除
        state.move_cursor_left();
        state.delete();
        assert_eq!(state.get_display_text(), "hel");

        // 確定（表示テキストを取り出してクリア）
        let committed = state.get_display_text();
        assert_eq!(committed, "hel");
        state.clear();
        assert_eq!(state.get_display_text(), "");
        assert!(!state.has_active_input());
    }

    #[test]
    fn test_half_width_alphanumeric_mode_symbols_and_numbers() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::HalfWidthAlphanumeric);

        for ch in "123!-_".chars() {
            state.input_char(ch);
        }
        assert_eq!(state.get_display_text(), "123!-_");
    }

    #[test]
    fn test_toggle_alphanumeric_from_all_modes() {
        let mut state = ImeState::new();

        // Hiragana -> HalfWidthAlphanumeric
        state.set_input_mode(InputMode::Hiragana);
        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);
        assert_eq!(state.get_input_mode(), InputMode::HalfWidthAlphanumeric);

        // HalfWidthAlphanumeric -> Hiragana
        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);

        // HalfWidthKatakana -> Hiragana
        state.set_input_mode(InputMode::HalfWidthKatakana);
        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);

        // FullWidthAlphanumeric -> Hiragana
        state.set_input_mode(InputMode::FullWidthAlphanumeric);
        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);
    }

    #[test]
    fn test_apply_mode_command_blocked_when_pending_romaji() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::Hiragana);

        // 'k' を入力して未変換ローマ字がある状態にする
        state.input_char('k');
        assert!(state.has_pending_input());

        // 未変換ローマ字がある間はモード切替コマンドが無視される
        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);

        state.apply_mode_command(ModeCommand::SwitchKanaType);
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);

        state.apply_mode_command(ModeCommand::SetInputMode(InputMode::HalfWidthAlphanumeric));
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);

        // ローマ字を完成させる
        state.input_char('a');
        assert!(!state.has_pending_input());
        assert_eq!(state.get_display_text(), "か");

        // 未変換ローマ字が無くなればモード切替可能
        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);
        assert_eq!(state.get_input_mode(), InputMode::HalfWidthAlphanumeric);
    }

    // ============================================================
    // カーソル位置 — 半角カタカナの多文字変換
    // ============================================================

    /// 濁点付きかな(が→ｶﾞ=2文字)のカーソル位置が表示テキスト基準になっているかを検証
    #[test]
    fn test_cursor_pos_half_width_katakana_voiced() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::HalfWidthKatakana);

        // が → ｶﾞ (表示2文字)
        for ch in "ga".chars() {
            state.input_char(ch);
        }

        let display = state.get_display_text();
        let cursor = state.get_cursor_pos();

        assert_eq!(display, "ｶﾞ", "表示テキストが正しいこと");
        assert_eq!(cursor, display.chars().count(),
            "カーソルは表示テキストの末尾({}文字目)にあるべき", display.chars().count());
        assert_cursor_within_display(&state);
    }

    /// 半濁点付きかな(ぱ→ﾊﾟ=2文字)のカーソル位置
    #[test]
    fn test_cursor_pos_half_width_katakana_semi_voiced() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::HalfWidthKatakana);

        // pa → ぱ → ﾊﾟ (表示2文字)
        for ch in "pa".chars() {
            state.input_char(ch);
        }

        let display = state.get_display_text();
        let cursor = state.get_cursor_pos();

        assert_eq!(display, "ﾊﾟ");
        assert_eq!(cursor, display.chars().count());
        assert_cursor_within_display(&state);
    }

    /// 無声かな(か→ｶ=1文字)のカーソル位置
    #[test]
    fn test_cursor_pos_half_width_katakana_unvoiced() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::HalfWidthKatakana);

        // ka → か → ｶ (表示1文字)
        for ch in "ka".chars() {
            state.input_char(ch);
        }

        assert_eq!(state.get_display_text(), "ｶ");
        assert_eq!(state.get_cursor_pos(), 1);
        assert_cursor_within_display(&state);
    }

    /// 複数文字入力後のカーソル位置: 濁音と清音の混在
    #[test]
    fn test_cursor_pos_half_width_katakana_mixed() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::HalfWidthKatakana);

        // がか → ｶﾞｶ (表示3文字)
        for ch in "gaka".chars() {
            state.input_char(ch);
        }

        let display = state.get_display_text();
        let cursor = state.get_cursor_pos();

        assert_eq!(display, "ｶﾞｶ", "濁音+清音の表示が正しいこと");
        assert_eq!(cursor, display.chars().count(),
            "カーソルは末尾({})にあるべきだが{}だった", display.chars().count(), cursor);
        assert_cursor_within_display(&state);
    }

    /// カーソルを中間に移動したときの表示カーソル位置
    #[test]
    fn test_cursor_pos_half_width_katakana_cursor_in_middle() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::HalfWidthKatakana);

        // がか → ｶﾞｶ の間でカーソルを左へ1つ移動する
        // (ソース: が|か) → 表示: ｶﾞ|ｶ → カーソルは表示の2文字目
        for ch in "gaka".chars() {
            state.input_char(ch);
        }
        state.move_cursor_left(); // ソース単位で1つ左 → が|か

        let display = state.get_display_text();
        let cursor = state.get_cursor_pos();

        assert_eq!(display, "ｶﾞｶ");
        // ｶﾞ(2文字) + カーソル + ｶ(1文字)
        assert_eq!(cursor, 2, "カーソルはｶﾞの後ろ(2)にあるべき、実際: {}", cursor);
        assert_cursor_within_display(&state);
    }

    /// カーソル位置がディスプレイ文字数以内であることを全モードで検証する
    #[test]
    fn test_cursor_always_within_display_all_modes() {
        let modes = [
            InputMode::Hiragana,
            InputMode::FullWidthKatakana,
            InputMode::HalfWidthKatakana,
            InputMode::FullWidthAlphanumeric,
            InputMode::HalfWidthAlphanumeric,
        ];

        let test_inputs = ["ga", "ka", "pa", "ne", "sa"];

        for &mode in &modes {
            for &input in &test_inputs {
                let mut state = ImeState::new();
                state.set_input_mode(mode);

                for ch in input.chars() {
                    state.input_char(ch);
                    assert_cursor_within_display(&state);
                }

                // カーソル移動後も範囲内であることを確認
                state.move_cursor_left();
                assert_cursor_within_display(&state);
                state.move_cursor_right();
                assert_cursor_within_display(&state);
            }
        }
    }

    // ============================================================
    // カーソル位置 — ひらがな/全角カタカナ
    // ============================================================

    #[test]
    fn test_cursor_pos_hiragana_tail() {
        let mut state = ImeState::new();

        for ch in "aka".chars() {
            state.input_char(ch);
        }

        assert_eq!(state.get_display_text(), "あか");
        assert_eq!(state.get_cursor_pos(), 2);
        assert_cursor_within_display(&state);
    }

    #[test]
    fn test_cursor_pos_full_width_katakana_voiced() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::FullWidthKatakana);

        // ga → が → ガ (1文字)
        for ch in "ga".chars() {
            state.input_char(ch);
        }

        assert_eq!(state.get_display_text(), "ガ");
        assert_eq!(state.get_cursor_pos(), 1);
        assert_cursor_within_display(&state);
    }

    // ============================================================
    // モード切替 (ToggleAlphanumeric) の網羅的テスト
    // ============================================================

    /// ひらがなモードでToggleAlphanumericを押すと半角英数になる（Capsキーの動作）
    #[test]
    fn test_toggle_alphanumeric_hiragana_to_half_width() {
        let mut state = ImeState::new();
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);

        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);

        assert_eq!(state.get_input_mode(), InputMode::HalfWidthAlphanumeric,
            "ひらがな→Caps→半角英数になるべき");
    }

    /// 半角英数でToggleAlphanumericを押すとひらがなに戻る
    #[test]
    fn test_toggle_alphanumeric_back_to_hiragana() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::HalfWidthAlphanumeric);

        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);

        assert_eq!(state.get_input_mode(), InputMode::Hiragana,
            "半角英数→Caps→ひらがなに戻るべき");
    }

    /// 全角カタカナでToggleAlphanumericを押すとひらがなに戻る（Mozc準拠）
    #[test]
    fn test_toggle_alphanumeric_from_full_width_katakana() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::FullWidthKatakana);

        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);

        assert_eq!(state.get_input_mode(), InputMode::Hiragana,
            "全角カタカナ→Caps→ひらがなに戻るべき(半角英数ではない)");
    }

    /// 半角カタカナでToggleAlphanumericを押すとひらがなに戻る
    #[test]
    fn test_toggle_alphanumeric_from_half_width_katakana() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::HalfWidthKatakana);

        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);

        assert_eq!(state.get_input_mode(), InputMode::Hiragana);
    }

    /// ToggleAlphanumericを繰り返す: Hiragana→AlphaNum→Hiragana→AlphaNum
    #[test]
    fn test_toggle_alphanumeric_repeated() {
        let mut state = ImeState::new();

        for i in 0..4 {
            state.apply_mode_command(ModeCommand::ToggleAlphanumeric);
            let expected = if i % 2 == 0 {
                InputMode::HalfWidthAlphanumeric
            } else {
                InputMode::Hiragana
            };
            assert_eq!(state.get_input_mode(), expected, "{}回目のトグル後", i + 1);
        }
    }

    // ============================================================
    // 半角英数モードの入力内容
    // ============================================================

    /// 半角英数モードでローマ字を入力するとそのまま半角で入る
    #[test]
    fn test_half_width_alphanumeric_input_is_raw_ascii() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::HalfWidthAlphanumeric);

        for ch in "hello".chars() {
            state.input_char(ch);
        }

        assert_eq!(state.get_display_text(), "hello",
            "半角英数モードでは 'hello' がそのまま入るべき");
        assert_eq!(state.get_cursor_pos(), 5);
        assert_cursor_within_display(&state);
    }

    /// 半角英数モードで数字・記号が直接入力できる
    #[test]
    fn test_half_width_alphanumeric_input_numbers_and_symbols() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::HalfWidthAlphanumeric);

        for ch in "abc123!@#".chars() {
            state.input_char(ch);
        }

        assert_eq!(state.get_display_text(), "abc123!@#");
        assert_eq!(state.get_cursor_pos(), 9);
    }

    // ============================================================
    // モード切替後の入力動作
    // ============================================================

    /// ひらがな→Caps→半角英数モードで入力するとひらがなではなくASCIIが入る
    #[test]
    fn test_input_after_toggle_to_alphanumeric() {
        let mut state = ImeState::new();

        // まずひらがなで入力
        state.input_char('k');
        state.input_char('a');
        assert_eq!(composition_to_string(&state), "か");

        // Capsでモード切替(ひらがな→半角英数)
        // ※未確定ローマ字がないのでモード切替可能
        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);
        assert_eq!(state.get_input_mode(), InputMode::HalfWidthAlphanumeric);
        assert_eq!(state.get_display_text(), "ka", "切替時にCompositionも半角英数に変換されるべき");

        // 切替後の入力はASCIIになる
        state.input_char('k');
        state.input_char('a');
        assert_eq!(state.get_display_text(), "kaka",
            "モード切替後はASCIIが入るべき");
    }

    /// 半角英数モードで 'n' を単独入力してもローマ字変換されない
    #[test]
    fn test_half_width_alphanumeric_no_romaji_pending() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::HalfWidthAlphanumeric);

        state.input_char('n');

        // ひらがなモードなら「n」は pending になるが、英数モードでは直接確定
        assert!(!state.has_pending_input(), "英数モードではpendin入力にならない");
        assert_eq!(state.get_display_text(), "n");
    }

    // ============================================================
    // 無変換キー（SwitchKanaType）の網羅テスト
    // ============================================================

    /// 全角英数モードから無変換キーを押すとひらがなに戻る
    #[test]
    fn test_switch_kana_type_from_full_width_alphanumeric_returns_to_hiragana() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::FullWidthAlphanumeric);

        state.apply_mode_command(ModeCommand::SwitchKanaType);
        assert_eq!(state.get_input_mode(), InputMode::Hiragana,
            "全角英数→無変換→ひらがなに戻るべき");
    }

    /// かな巡回を2周回しても安定して動作する
    #[test]
    fn test_switch_kana_type_two_full_cycles() {
        let mut state = ImeState::new();

        let expected_cycle = [
            InputMode::FullWidthKatakana,
            InputMode::HalfWidthKatakana,
            InputMode::Hiragana,
        ];

        for cycle in 0..2 {
            for (i, &expected) in expected_cycle.iter().enumerate() {
                state.apply_mode_command(ModeCommand::SwitchKanaType);
                assert_eq!(state.get_input_mode(), expected,
                    "{}周目 {}回目のSwitchKanaType後", cycle + 1, i + 1);
            }
        }
    }

    // ============================================================
    // カタカナキー（SetInputMode(FullWidthKatakana)）の全モード遷移
    // ============================================================

    /// カタカナキーはどのモードからでも全角カタカナに遷移する
    #[test]
    fn test_katakana_command_from_any_mode() {
        let modes = [
            InputMode::Hiragana,
            InputMode::FullWidthKatakana,
            InputMode::HalfWidthKatakana,
            InputMode::FullWidthAlphanumeric,
            InputMode::HalfWidthAlphanumeric,
        ];

        for mode in modes {
            let mut state = ImeState::new();
            state.set_input_mode(mode);
            state.apply_mode_command(ModeCommand::SetInputMode(InputMode::FullWidthKatakana));
            assert_eq!(state.get_input_mode(), InputMode::FullWidthKatakana,
                "{:?}→カタカナキー→全角カタカナになるべき", mode);
        }
    }

    // ============================================================
    // 英数キー（ToggleAlphanumeric）の全角英数からの遷移
    // ============================================================

    /// 全角英数モードでToggleAlphanumericを押すとひらがなに戻る
    #[test]
    fn test_toggle_alphanumeric_from_full_width_alphanumeric() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::FullWidthAlphanumeric);

        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);

        assert_eq!(state.get_input_mode(), InputMode::Hiragana,
            "全角英数→Caps→ひらがなに戻るべき(半角英数ではない)");
    }

    // ============================================================
    // モード切替後の入力内容が正しいモードで変換される
    // ============================================================

    /// 無変換キーで全角カタカナに切替後、入力が全角カタカナで出る
    #[test]
    fn test_input_after_switch_to_full_width_katakana() {
        let mut state = ImeState::new();

        // 無変換キーで全角カタカナに切替
        state.apply_mode_command(ModeCommand::SwitchKanaType);
        assert_eq!(state.get_input_mode(), InputMode::FullWidthKatakana);

        for ch in "ka".chars() {
            state.input_char(ch);
        }

        assert_eq!(state.get_display_text(), "カ",
            "全角カタカナモードで 'ka' → 'カ' になるべき");
    }

    /// 無変換キー2回で半角カタカナに切替後、入力が半角カタカナで出る
    #[test]
    fn test_input_after_switch_to_half_width_katakana() {
        let mut state = ImeState::new();

        state.apply_mode_command(ModeCommand::SwitchKanaType); // → 全角カタカナ
        state.apply_mode_command(ModeCommand::SwitchKanaType); // → 半角カタカナ
        assert_eq!(state.get_input_mode(), InputMode::HalfWidthKatakana);

        for ch in "ka".chars() {
            state.input_char(ch);
        }

        assert_eq!(state.get_display_text(), "ｶ",
            "半角カタカナモードで 'ka' → 'ｶ' になるべき");
    }

    /// カタカナキーで全角カタカナに切替後、入力が全角カタカナで出る
    #[test]
    fn test_input_after_katakana_key() {
        let mut state = ImeState::new();

        state.apply_mode_command(ModeCommand::SetInputMode(InputMode::FullWidthKatakana));

        for ch in "sakura".chars() {
            state.input_char(ch);
        }

        assert_eq!(state.get_display_text(), "サクラ",
            "全角カタカナモードで 'sakura' → 'サクラ' になるべき");
    }

    /// ひらがなキーでひらがなに戻した後、入力がひらがなで出る
    #[test]
    fn test_input_after_hiragana_key_from_katakana() {
        let mut state = ImeState::new();

        // カタカナモードに切替→ひらがなキーで戻す
        state.apply_mode_command(ModeCommand::SetInputMode(InputMode::FullWidthKatakana));
        state.apply_mode_command(ModeCommand::SetInputMode(InputMode::Hiragana));

        for ch in "ka".chars() {
            state.input_char(ch);
        }

        assert_eq!(state.get_display_text(), "か",
            "ひらがなキーで戻した後は 'ka' → 'か' になるべき");
    }

    // ============================================================
    // 複合シナリオ：実際のユーザー操作をシミュレート
    // ============================================================

    /// ひらがな→英数→ひらがな→カタカナ→ひらがな と切替ながら入力する
    #[test]
    fn test_mode_switching_scenario() {
        let mut state = ImeState::new();

        // ひらがなで入力
        for ch in "a".chars() { state.input_char(ch); }
        assert_eq!(state.get_display_text(), "あ");

        // 英数キーで半角英数に切替 ("あ" → "a")
        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);
        state.input_char('b');
        // 半角英数モードなので 'a' + 'b' で "ab"
        assert_eq!(state.get_display_text(), "ab");

        // 英数キーでひらがなに戻す ("ab" → ひらがな表示では英字が全角になり "あｂか")
        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);
        for ch in "ka".chars() { state.input_char(ch); }
        assert_eq!(state.get_display_text(), "あｂか");

        // カタカナキーで全角カタカナに切替 ("あｂか" → "アｂカサ")
        state.apply_mode_command(ModeCommand::SetInputMode(InputMode::FullWidthKatakana));
        for ch in "sa".chars() { state.input_char(ch); }
        assert_eq!(state.get_display_text(), "アｂカサ");

        // ひらがなキーでひらがなに戻す
        state.apply_mode_command(ModeCommand::SetInputMode(InputMode::Hiragana));
        for ch in "ta".chars() { state.input_char(ch); }
        assert_eq!(state.get_display_text(), "あｂかさた");
    }

    /// 半角英数でaaaaと入力してCapsキーを押したらああああになり、再度押すとaaaaに戻る
    #[test]
    fn test_toggle_alphanumeric_aaaa_to_hiragana_and_back() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::HalfWidthAlphanumeric);

        for ch in "aaaa".chars() {
            state.input_char(ch);
        }
        assert_eq!(state.get_display_text(), "aaaa");
        assert_eq!(state.get_input_mode(), InputMode::HalfWidthAlphanumeric);

        // Capsキー（ToggleAlphanumeric）を押す → ひらがなモード＆「ああああ」になる
        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);
        assert_eq!(state.get_display_text(), "ああああ");

        // 再度Capsキーを押す → 半角英数モード＆「aaaa」に戻る
        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);
        assert_eq!(state.get_input_mode(), InputMode::HalfWidthAlphanumeric);
        assert_eq!(state.get_display_text(), "aaaa");
    }

    /// ひらがなでああああ（aaaa）と入力してCapsキーを押したらaaaaになり、再度押すとああああに戻る
    #[test]
    fn test_toggle_alphanumeric_hiragana_to_aaaa_and_back() {
        let mut state = ImeState::new();
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);

        for ch in "aaaa".chars() {
            state.input_char(ch);
        }
        assert_eq!(state.get_display_text(), "ああああ");

        // Capsキーを押す → 半角英数モード＆「aaaa」になる
        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);
        assert_eq!(state.get_input_mode(), InputMode::HalfWidthAlphanumeric);
        assert_eq!(state.get_display_text(), "aaaa");

        // 再度Capsキーを押す → ひらがなモード＆「ああああ」に戻る
        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);
        assert_eq!(state.get_display_text(), "ああああ");
    }

    /// カタカナや全角英数からCapsキーを押すとひらがなモードになり、未確定文字列もひらがなになる
    #[test]
    fn test_toggle_alphanumeric_from_katakana_and_full_width() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::FullWidthKatakana);

        for ch in "ka".chars() {
            state.input_char(ch);
        }
        assert_eq!(state.get_display_text(), "カ");

        // カタカナからCapsを押すとひらがなモードへ
        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);
        assert_eq!(state.get_display_text(), "か");

        // 全角カタカナへ切替
        state.apply_mode_command(ModeCommand::SetInputMode(InputMode::FullWidthKatakana));
        assert_eq!(state.get_display_text(), "カ");

        // 全角英数へ切替
        state.apply_mode_command(ModeCommand::SetInputMode(InputMode::FullWidthAlphanumeric));
        assert_eq!(state.get_display_text(), "ｋａ");

        // 全角英数からCapsを押すとひらがなモードへ
        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);
        assert_eq!(state.get_display_text(), "か");
    }

    // ============================================================
    // ファンクションキー変換 (convert_composition: F6〜F10)
    // ============================================================

    #[test]
    fn test_convert_composition_f6_to_f10() {
        // F6: ひらがな「てすと」
        let mut state = ImeState::new();
        for ch in "tesuto".chars() { state.input_char(ch); }
        state.convert_composition(InputMode::Hiragana);
        assert_eq!(state.get_display_text(), "てすと");
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);

        // F7: 全角カタカナ「テスト」
        let mut state = ImeState::new();
        for ch in "tesuto".chars() { state.input_char(ch); }
        state.convert_composition(InputMode::FullWidthKatakana);
        assert_eq!(state.get_display_text(), "テスト");
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);

        // F8: 半角カタカナ「ﾃｽﾄ」
        let mut state = ImeState::new();
        for ch in "tesuto".chars() { state.input_char(ch); }
        state.convert_composition(InputMode::HalfWidthKatakana);
        assert_eq!(state.get_display_text(), "ﾃｽﾄ");
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);

        // F9: 全角英数「ｔｅｓｕｔｏ」
        let mut state = ImeState::new();
        for ch in "tesuto".chars() { state.input_char(ch); }
        state.convert_composition(InputMode::FullWidthAlphanumeric);
        assert_eq!(state.get_display_text(), "ｔｅｓｕｔｏ");
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);

        // F10: 半角英数「tesuto」
        let mut state = ImeState::new();
        for ch in "tesuto".chars() { state.input_char(ch); }
        state.convert_composition(InputMode::HalfWidthAlphanumeric);
        assert_eq!(state.get_display_text(), "tesuto");
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);
    }

    #[test]
    fn test_convert_composition_cycles() {
        let mut state = ImeState::new();
        for ch in "tesuto".chars() { state.input_char(ch); }

        state.convert_composition(InputMode::FullWidthKatakana);
        assert_eq!(state.get_display_text(), "テスト");

        state.convert_composition(InputMode::HalfWidthKatakana);
        assert_eq!(state.get_display_text(), "ﾃｽﾄ");

        state.convert_composition(InputMode::FullWidthAlphanumeric);
        assert_eq!(state.get_display_text(), "ｔｅｓｕｔｏ");

        state.convert_composition(InputMode::HalfWidthAlphanumeric);
        assert_eq!(state.get_display_text(), "tesuto");

        state.convert_composition(InputMode::Hiragana);
        assert_eq!(state.get_display_text(), "てすと");

        // ベースの入力モードは変更されていないこと
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);
    }

    #[test]
    fn test_convert_composition_resolves_pending_romaji() {
        let mut state = ImeState::new();
        for ch in "kan".chars() { state.input_char(ch); }
        assert!(state.has_pending_input());

        state.convert_composition(InputMode::FullWidthKatakana);
        assert_eq!(state.get_display_text(), "カン");
        assert!(!state.has_pending_input());
    }

    #[test]
    fn test_convert_composition_noop_without_active_input() {
        let mut state = ImeState::new();
        state.convert_composition(InputMode::FullWidthKatakana);
        assert_eq!(state.get_display_text(), "");
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);
    }

    #[test]
    fn test_convert_composition_clear_restores_state() {
        let mut state = ImeState::new();
        for ch in "tesuto".chars() { state.input_char(ch); }
        state.convert_composition(InputMode::FullWidthKatakana);
        assert_eq!(state.get_display_text(), "テスト");

        state.clear();
        assert_eq!(state.get_display_text(), "");
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);

        // 次の入力はひらがなモードのまま
        state.input_char('a');
        assert_eq!(state.get_display_text(), "あ");
    }
}