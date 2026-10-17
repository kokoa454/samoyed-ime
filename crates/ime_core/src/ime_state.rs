use std::collections::HashSet;

use crate::composition::Composition;
use crate::dictionary::Dictionary;
use crate::mode_converter::{
    full_width_alphanumeric_to_half_width_alphanumeric, full_width_katakana_to_hiragana,
    half_width_alphanumeric_to_full_width_alphanumeric, half_width_katakana_to_full_width_katakana,
    hiragana_to_full_width_katakana, hiragana_to_half_width_katakana, hiragana_to_romaji,
    romaji_to_hiragana,
};
use crate::romaji_input::RomajiInput;
use crate::connection_costs::ConnectionCosts;
use crate::kanji_converter::{KanjiConversionResult, KanjiConverter};


const FULL_WIDTH_ZERO: char = '０'; // 全角の0
const FULL_WIDTH_NINE: char = '９'; // 全角の9
const HALF_WIDTH_SPACE: char = ' '; // 半角スペース
const FULL_WIDTH_SPACE: char = '　'; // 全角スペース
const HIRAGANA_N: &str = "ん"; // 「ん」
const HALF_ALPHANUMERIC_N: &str = "n"; // 「n」


/// 入力モード
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Hiragana,              // ひらがな
    FullWidthKatakana,     // 全角カタカナ
    HalfWidthKatakana,     // 半角カタカナ
    FullWidthAlphanumeric, // 全角英数字
    HalfWidthAlphanumeric, // 半角英数字
}

/// 入力モード切替コマンド
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeCommand {
    SetInputMode(InputMode), // 指定した入力モードへ変更
    SwitchKanaType,          // 無変換キー：かな種別を巡回
    ToggleAlphanumeric,      // 英数キー
}

/// 無効なローマ字が確定された直前の入力状態
#[derive(Clone, PartialEq, Eq)]
struct RomajiFallbackUndo {
    composition: Composition,
    romaji_input: RomajiInput,
}

/// IMEの状態を管理する構造体
#[derive(Clone, PartialEq, Eq)]
pub struct ImeState {
    input_mode: InputMode, // 現在の入力モード
    conversion_mode: Option<InputMode>, // ファンクションキー等による一時変換モード
    dictionary_reading: Option<String>, // 辞書変換前の読み
    dictionary_candidates: Vec<String>, // 辞書変換候補
    dictionary_candidate_index: usize, // 現在の候補位置
    composition: Composition, // 現在の未確定文字列
    romaji_input: RomajiInput, // ローマ字入力
    last_romaji_fallback_undo: Option<RomajiFallbackUndo>, // 無効なローマ字が確定された直前の状態
    temporary_ascii_input: bool, // 一時英字入力中かどうか
    preserve_composition_text: bool, // 入力済み文字列を再変換せず表示するか
    dictionary_conversion: Option<KanjiConversionResult>, // Viterbiによる変換結果
    dictionary_segment_index: usize, // 現在操作している文節のインデックス
}

impl ImeState {
    /// コンストラクタ
    pub fn new() -> Self {
        Self {
            input_mode: InputMode::Hiragana,
            conversion_mode: None,
            dictionary_reading: None,
            dictionary_candidates: Vec::new(),
            dictionary_candidate_index: 0,
            composition: Composition::new(),
            romaji_input: RomajiInput::new(),
            last_romaji_fallback_undo: None,
            temporary_ascii_input: false,
            preserve_composition_text: false,
            dictionary_conversion: None,
            dictionary_segment_index: 0,
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
        self.last_romaji_fallback_undo = None;
        self.temporary_ascii_input = false;
        self.preserve_composition_text = false;
        self.input_mode = mode;
    }

    /// モード切替コマンドを適用する。
    ///
    /// 引数
    /// * `command`: モード切替コマンド
    pub fn apply_mode_command(&mut self, command: ModeCommand) {
        self.last_romaji_fallback_undo = None;
        self.temporary_ascii_input = false;
        self.preserve_composition_text = false;

        // 未変換ローマ字がある間はモードを変更しない
        if self.has_pending_input() {
            return;
        }

        self.cancel_dictionary_conversion();
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
        self.last_romaji_fallback_undo = None;

        if !self.has_active_input() {
            return;
        }

        self.temporary_ascii_input = false;
        self.preserve_composition_text = false;

        self.cancel_dictionary_conversion();

        // 未変換ローマ字（pending）がある場合、解決してCompositionに取り込む
        if self.romaji_input.has_pending_input() {
            let pending = self.romaji_input.get_pending_input();
            let resolved = if pending == HALF_ALPHANUMERIC_N {
                HIRAGANA_N.to_string()
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

    /// モードに応じて文字列を変換する。スペース文字（半角/全角）はそのまま保持する。
    ///
    /// 引数
    /// * `text`: 変換する文字列
    ///
    /// 戻り値
    /// * `String`: 変換後の文字列
    fn convert_text(&self, text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut non_space_buf = String::new();

        let flush_buf = |buf: &mut String, res: &mut String| {
            if !buf.is_empty() {
                let normalized = {
                    let full_width_kana = half_width_katakana_to_full_width_katakana(buf);
                    let hiragana = full_width_katakana_to_hiragana(&full_width_kana);
                    let full_width = half_width_alphanumeric_to_full_width_alphanumeric(&hiragana);

                    if self.input_mode == InputMode::FullWidthAlphanumeric {
                        full_width
                    } else {
                        full_width
                            .chars()
                            .map(|ch| match ch {
                                FULL_WIDTH_ZERO..=FULL_WIDTH_NINE => char::from_u32(ch as u32 - 0xFEE0).unwrap_or(ch),
                                _ => ch,
                            })
                            .collect()
                    }
                };

                let converted = match self.input_mode {
                    InputMode::Hiragana => normalized,
                    InputMode::FullWidthKatakana => hiragana_to_full_width_katakana(&normalized),
                    InputMode::HalfWidthKatakana => hiragana_to_half_width_katakana(&normalized),
                    InputMode::FullWidthAlphanumeric => normalized,
                    InputMode::HalfWidthAlphanumeric => full_width_alphanumeric_to_half_width_alphanumeric(&normalized),
                };

                res.push_str(&converted);
                buf.clear();
            }
        };

        for ch in text.chars() {
            if ch == HALF_WIDTH_SPACE || ch == FULL_WIDTH_SPACE {
                flush_buf(&mut non_space_buf, &mut result);
                result.push(ch);
            } else {
                non_space_buf.push(ch);
            }
        }
        flush_buf(&mut non_space_buf, &mut result);

        result
    }
}

/// モード切替時にCompositionテキストを対象モード向けに変換する。
/// 
/// 引数:
/// * `text`: 変換する文字列
/// * `target_mode`: 変換先の入力モード
/// 
/// 戻り値:
/// * `String`: 変換後の文字列
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
    /// 一時英字入力を開始する。
    ///
    /// Shift付きの英字キーから呼び出す。
    ///
    /// 引数:
    /// * `first_char`: 最初に入力する英字
    ///
    /// 戻り値:
    /// * `String`: 入力した英字
    pub fn start_temporary_ascii_input(&mut self, first_char: char) -> String {
        self.last_romaji_fallback_undo = None;
        self.cancel_dictionary_conversion();
        self.conversion_mode = None;

        // 未変換ローマ字が残っている場合は、先にCompositionへ反映
        if self.romaji_input.has_pending_input() {
            let pending = self.romaji_input.get_pending_input();
            let resolved = if pending == HALF_ALPHANUMERIC_N {
                HIRAGANA_N.to_string()
            } else {
                pending.to_string()
            };

            let converted = self.convert_text(&resolved);

            for ch in converted.chars() {
                self.composition.insert(ch);
            }

            self.romaji_input.clear();
        }

        self.temporary_ascii_input = true;
        self.preserve_composition_text = true;

        // 最初の大文字をそのまま挿入
        self.composition.insert(first_char);

        first_char.to_string()
    }

    /// 一時英字入力中かどうかを取得する。
    ///
    /// 戻り値:
    /// * `bool`: 一時英字入力中ならtrue
    pub fn is_temporary_ascii_input(&self) -> bool {
        self.temporary_ascii_input
    }

    /// 文字を入力する。
    ///
    /// 引数:
    /// * `input`: 入力文字
    ///
    /// 戻り値:
    /// * `String`: 変換後の文字
    pub fn input_char(&mut self, input: char) -> String {
        // 前回の復元情報は、今回の入力に対して一度だけ使用
        self.last_romaji_fallback_undo = None;

        // 一時英字入力中は、ローマ字変換をせず文字をそのまま入力
        if self.temporary_ascii_input && input.is_ascii_graphic() {
            self.cancel_dictionary_conversion();
            self.conversion_mode = None;
            self.composition.insert(input);

            return input.to_string();
        }

        // ASCII英字以外の文字が入力された場合は一時英字入力を終了
        self.temporary_ascii_input = false;

        self.cancel_dictionary_conversion();
        self.conversion_mode = None;

        // 入力前の状態を保存
        let composition_before = self.composition.clone();
        let romaji_before = self.romaji_input.clone();

        let is_kana_mode = matches!(
            self.input_mode,
            InputMode::Hiragana
                | InputMode::FullWidthKatakana
                | InputMode::HalfWidthKatakana
        );

        let raw = match self.input_mode {
            // 英数モードはローマ字変換せず、そのまま入力
            InputMode::FullWidthAlphanumeric | InputMode::HalfWidthAlphanumeric => {
                input.to_string()
            }

            // かな系モードはローマ字変換
            _ => self.romaji_input.input(input),
        };

        let has_romaji_fallback = is_kana_mode
            && raw.chars().any(|ch| ch.is_ascii_alphabetic());

        let converted = self.convert_text(&raw);

        for ch in converted.chars() {
            self.composition.insert(ch);
        }

        // 無効なローマ字を通常文字として出力した場合だけ、Backspaceによる状態復元を可能にする
        if has_romaji_fallback {
            self.last_romaji_fallback_undo = Some(RomajiFallbackUndo {
                composition: composition_before,
                romaji_input: romaji_before,
            });
        }

        converted
    }

    /// 現在の表示文字列を取得する。
    ///
    /// 戻り値
    /// * `String`: 画面に表示する文字列
    pub fn get_display_text(&self) -> String {
        // 辞書候補または一時変換の結果は、再度モード変換しない
        if self.conversion_mode.is_some() || self.dictionary_reading.is_some() {
            return self.composition.get_content_units().iter().collect();
        }

        // 一時英字入力を開始したCompositionは、入力済み文字を再変換しない
        if self.preserve_composition_text {
            let content_units = self.composition.get_content_units();
            let cursor_pos = self.composition.get_cursor_pos().min(content_units.len());

            let before_cursor: String = content_units[..cursor_pos].iter().collect();
            let after_cursor: String = content_units[cursor_pos..].iter().collect();

            let mut text = before_cursor;
            text.push_str(&self.convert_text(self.romaji_input.get_pending_input()));
            text.push_str(&after_cursor);

            return text;
        }

        let content_units = self.composition.get_content_units();
        let cursor_pos = self.composition.get_cursor_pos().min(content_units.len());
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
        // 辞書候補または一時変換の結果は、再度モード変換しない
        if self.conversion_mode.is_some() || self.dictionary_reading.is_some() {
            return self.composition.get_cursor_pos();
        }

        // 一時英字入力中は、未確定ローマ字分だけカーソル位置をずらす
        if self.preserve_composition_text {
            return self.composition.get_cursor_pos()
                + self.convert_text(self.romaji_input.get_pending_input())
                    .chars()
                    .count();
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

    /// スペースを入力する。
    ///
    /// 通常（Shiftなし）時の全角/半角：
    /// - ひらがな / 全角カタカナ / 全角英数: 全角スペース
    /// - 半角カタカナ / 半角英数: 半角スペース
    ///
    /// Shiftあり時は全角/半角が反転する。
    ///
    /// 引数
    /// * `is_shift`: Shiftキーが押下されているかどうか
    ///
    /// 戻り値
    /// * `String`: 入力されたスペース文字列
    pub fn input_space(&mut self, is_shift: bool) -> String {
        self.last_romaji_fallback_undo = None;

        // 一時英字入力中のSpaceは半角スペースを入力して、かな入力へ戻す
        if self.temporary_ascii_input {
            self.temporary_ascii_input = false;
            self.composition.insert(' ');

            return " ".to_string();
        }

        // 変換候補があれば次の候補へ
        if !is_shift && self.dictionary_reading.is_some() {
            self.next_dictionary_candidate();
            return self.get_display_text();
        }

        // 一時変換中かどうかを、解除する前に保存
        let can_start_dictionary_conversion = self.conversion_mode.is_none();

        self.cancel_dictionary_conversion();
        self.conversion_mode = None;

        // 未変換ローマ字があれば解決してCompositionに追加
        if self.romaji_input.has_pending_input() {
            let pending = self.romaji_input.get_pending_input();
            let resolved = if pending == HALF_ALPHANUMERIC_N {
                HIRAGANA_N.to_string()
            } else {
                pending.to_string()
            };
            for ch in resolved.chars() {
                self.composition.insert(ch);
            }
            self.romaji_input.clear();
        }

        if !is_shift && self.input_mode == InputMode::Hiragana && can_start_dictionary_conversion && self.has_composition() {
            let reading: String = self.composition.get_content_units().iter().collect();

            // 実際の辞書と連接コストを使ってViterbi変換を試みる
            let conversion = ConnectionCosts::embedded().ok().and_then(
                |connection_costs| {
                    let converter = KanjiConverter::new(
                        Dictionary::embedded(),
                        &connection_costs,
                    );

                    converter.convert(&reading).ok()
                },
            );

            if let Some(result) =
                conversion.filter(|result| !result.segments.is_empty())
            {
                // 読みとViterbiの結果を保持する
                self.dictionary_reading = Some(reading);
                self.dictionary_conversion = Some(result);
                self.dictionary_segment_index = 0;

                // 現在の文節に対する候補リストを更新する
                self.update_dictionary_candidate_view();

                // 各文節の選択候補を連結してCompositionへ反映する
                let surface = self
                    .dictionary_conversion
                    .as_ref()
                    .expect("conversion result was just assigned")
                    .surface();

                self.replace_composition(&surface);

                return self.get_display_text();
            }

            // Viterbiで変換できない場合は、従来の完全一致検索に戻す
            // 既存動作を維持するためのフォールバック
            let mut seen_surfaces = HashSet::new();

            let candidates: Vec<String> = Dictionary::embedded()
                .lookup(&reading)
                .iter()
                .filter_map(|entry| {
                    let surface = entry.surface.clone();
                    seen_surfaces
                        .insert(surface.clone())
                        .then_some(surface)
                })
                .collect();

            // 候補がない場合は、読みを維持して空白を挿入しない
            if candidates.is_empty() {
                return String::new();
            }

            self.dictionary_reading = Some(reading);
            self.dictionary_candidates = candidates;
            self.dictionary_candidate_index = 0;

            let candidate = self.dictionary_candidates[0].clone();
            self.replace_composition(&candidate);

            return self.get_display_text();
        }

        // 通常スペースの種別（全角か半角か）
        let normally_full_width = matches!(
            self.input_mode,
            InputMode::Hiragana | InputMode::FullWidthKatakana | InputMode::FullWidthAlphanumeric
        );

        // Shiftありの場合は反転
        let is_full_width = if is_shift {
            !normally_full_width
        } else {
            normally_full_width
        };

        let ch = if is_full_width { FULL_WIDTH_SPACE } else { HALF_WIDTH_SPACE };
        self.composition.insert(ch);
        ch.to_string()
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
        if self.temporary_ascii_input {
            return input.is_ascii_graphic();
        }

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

    /// 現在の辞書変換候補を取得する。
    pub fn get_dictionary_candidates(&self) -> &[String] {
        &self.dictionary_candidates
    }

    /// 現在選択されている辞書候補の位置を取得する。
    ///
    /// 戻り値:
    /// * `Option<usize>`: 候補選択中の場合はその位置、それ以外はNone
    pub fn get_dictionary_candidate_index(&self) -> Option<usize> {
        self.dictionary_reading
            .as_ref()
            .map(|_| self.dictionary_candidate_index)
    }

    /// Viterbiによる文節変換中かどうかを取得する。
    /// 
    /// 戻り値:
    /// * `bool`: 変換中の場合はtrue
    pub fn has_dictionary_conversion(&self) -> bool {
        self.dictionary_conversion.is_some()
    }

    /// 辞書変換中の候補を次へ進める。
    ///
    /// 戻り値:
    /// * `String`: 選択された候補。候補選択中でない場合は空文字列
    pub fn select_next_dictionary_candidate(&mut self) -> String {
        self.advance_dictionary_candidate(true)
    }

    /// 辞書変換中の候補を前へ戻す。
    /// 
    /// 戻り値:
    /// * `String`: 選択された候補。候補選択中でない場合は空文字列
    pub fn select_previous_dictionary_candidate(&mut self) -> String {
        self.advance_dictionary_candidate(false)
    }

    /// 次の文節へ選択対象を移動する。
    ///
    /// 戻り値:
    /// * `true`: 移動成功
    /// * `false`: 変換中ではない、または次の文節が存在しない
    pub fn move_to_next_dictionary_segment(&mut self) -> bool {
        let Some(conversion) = self.dictionary_conversion.as_ref() else {
            return false;
        };

        let next_index = self.dictionary_segment_index.saturating_add(1);

        if next_index >= conversion.segments.len() {
            return false;
        }

        self.dictionary_segment_index = next_index;
        self.update_dictionary_candidate_view();

        true
    }

    /// 前の文節へ選択対象を移動する。
    ///
    /// 戻り値:
    /// * `true`: 移動成功
    /// * `false`: 変換中ではない、または前の文節が存在しない
    pub fn move_to_previous_dictionary_segment(&mut self) -> bool {
        if self.dictionary_conversion.is_none()
            || self.dictionary_segment_index == 0
        {
            return false;
        }

        self.dictionary_segment_index -= 1;
        self.update_dictionary_candidate_view();

        true
    }

    /// 候補選択を前後に進める。
    /// Viterbi結果がある場合は現在の文節の候補を切り替え、全文節の選択候補を連結してCompositionを更新。
    /// 
    /// 引数
    /// * `forward`: trueの場合は次へ、falseの場合は前へ
    ///
    /// 戻り値
    /// * `String`: 選択された候補。候補選択中でない場合は空文字列
    fn advance_dictionary_candidate(&mut self, forward: bool) -> String {
        if self.dictionary_reading.is_none() {
            return String::new();
        }

        // Viterbiによる文節単位の候補切替
        if let Some(result) = self.dictionary_conversion.as_mut() {
            let Some(segment) =
                result.segments.get_mut(self.dictionary_segment_index)
            else {
                return String::new();
            };

            if segment.candidates.is_empty() {
                return String::new();
            }

            let count = segment.candidates.len();

            segment.selected_index = if forward {
                (segment.selected_index + 1) % count
            } else if segment.selected_index == 0 {
                count - 1
            } else {
                segment.selected_index - 1
            };

            let selected_candidate =
                segment.candidates[segment.selected_index].surface.clone();

            // 選択された候補を反映した全文字列を取得
            let surface = result.surface();

            self.replace_composition(&surface);
            self.update_dictionary_candidate_view();

            return selected_candidate;
        }

        // フォールバック：従来の読み全体に対する候補切替
        if self.dictionary_candidates.is_empty() {
            return String::new();
        }

        let count = self.dictionary_candidates.len();

        self.dictionary_candidate_index = if forward {
            (self.dictionary_candidate_index + 1) % count
        } else if self.dictionary_candidate_index == 0 {
            count - 1
        } else {
            self.dictionary_candidate_index - 1
        };

        let candidate =
            self.dictionary_candidates[self.dictionary_candidate_index].clone();

        self.replace_composition(&candidate);

        candidate
    }

    /// Spaceで次の辞書候補へ進める。
    ///
    /// 戻り値:
    /// * `String`: 選択された候補。候補選択中でない場合は空文字列
    fn next_dictionary_candidate(&mut self) -> String {
        self.advance_dictionary_candidate(true)
    }

    /// Backspaceを実行する。
    pub fn backspace(&mut self) {
        // 辞書変換中はEscと同じように変換をキャンセル
        // 元の読みを復元した後、文字の削除は行わない
        if self.dictionary_reading.is_some() {
            self.cancel_dictionary_conversion_and_restore_reading();
            return;
        }

        // 直前の入力で無効なローマ字が確定されていた場合は、入力前のCompositionとローマ字入力状態を復元
        if let Some(undo) = self.last_romaji_fallback_undo.take() {
            self.composition = undo.composition;
            self.romaji_input = undo.romaji_input;
            return;
        }

        self.cancel_dictionary_conversion();

        // 未確定ローマ字がある場合は、ローマ字入力状態を戻す
        if self.romaji_input.has_pending_input() {
            self.romaji_input.backspace();
            return;
        }

        // 通常の入力中だけ、Compositionの文字を削除
        self.composition.backspace();
    }

    /// Deleteを実行する。
    pub fn delete(&mut self) {
        self.last_romaji_fallback_undo = None;
        self.cancel_dictionary_conversion();
        if self.romaji_input.has_pending_input() {
            return;
        }

        self.composition.delete();
    }

    // カーソルを左へ移動する。
    pub fn move_cursor_left(&mut self) {
        self.last_romaji_fallback_undo = None;
        self.cancel_dictionary_conversion();
        if self.romaji_input.has_pending_input() {
            return;
        }

        self.composition.move_cursor_left();
    }

    // カーソルを右へ移動する。
    pub fn move_cursor_right(&mut self) {
        self.last_romaji_fallback_undo = None;
        self.cancel_dictionary_conversion();
        if self.romaji_input.has_pending_input() {
            return;
        }

        self.composition.move_cursor_right();
    }

    // カーソルを先頭へ移動する。
    pub fn move_cursor_to_head(&mut self) {
        self.last_romaji_fallback_undo = None;
        self.cancel_dictionary_conversion();
        if self.romaji_input.has_pending_input() {
            return;
        }

        self.composition.move_cursor_to_head();
    }

    // カーソルを末尾へ移動する。
    pub fn move_cursor_to_tail(&mut self) {
        self.last_romaji_fallback_undo = None;
        self.cancel_dictionary_conversion();
        if self.romaji_input.has_pending_input() {
            return;
        }

        self.composition.move_cursor_to_tail();
    }

    // 入力状態をすべてクリアする。
    pub fn clear(&mut self) {
        self.last_romaji_fallback_undo = None;
        self.dictionary_reading = None;
        self.dictionary_candidates.clear();
        self.dictionary_candidate_index = 0;
        self.composition.clear();
        self.romaji_input.clear();
        self.conversion_mode = None;
        self.temporary_ascii_input = false;
        self.preserve_composition_text = false;
        self.dictionary_conversion = None;
        self.dictionary_segment_index = 0;
    }

    /// 辞書変換をキャンセルし、元の読みをCompositionへ戻す。
    ///
    /// 候補選択中にEscが押された場合に使用する。
    ///
    /// 戻り値:
    /// * なし
    pub fn cancel_dictionary_conversion_and_restore_reading(&mut self) {
        // 古い入力復元情報を破棄
        self.last_romaji_fallback_undo = None;

        // 選択中の候補を解除し、元の読みをCompositionへ戻す
        self.cancel_dictionary_conversion();

        // 一時的な変換モードも解除
        self.conversion_mode = None;
    }

    /// Compositionを指定した文字列に置き換える。
    /// 
    /// 引数:
    /// * `text`: 置き換える文字列
    fn replace_composition(&mut self, text: &str) {
        self.composition.clear();
        for ch in text.chars() {
            self.composition.insert(ch);
        }
    }

    /// 現在の文節の候補を既存の候補一覧へ反映する。
    fn update_dictionary_candidate_view(&mut self) {
        let Some((candidates, selected_index)) = self
            .dictionary_conversion
            .as_ref()
            .and_then(|result| {
                result.segments.get(self.dictionary_segment_index)
            })
            .map(|segment| {
                (
                    segment
                        .candidates
                        .iter()
                        .map(|candidate| candidate.surface.clone())
                        .collect::<Vec<_>>(),
                    segment.selected_index,
                )
            })
        else {
            return;
        };

        self.dictionary_candidates = candidates;
        self.dictionary_candidate_index = selected_index;
    }

    /// 辞書変換を解除し、元の読みをCompositionへ戻す。
    fn cancel_dictionary_conversion(&mut self) {
        if let Some(reading) = self.dictionary_reading.take() {
            self.replace_composition(&reading);
        }

        self.dictionary_conversion = None;
        self.dictionary_segment_index = 0;
        self.dictionary_candidates.clear();
        self.dictionary_candidate_index = 0;
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
            cursor,
            display.chars().count(),
            display
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

        assert_eq!(composition_to_string(&state), "。、！？：；");
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

        assert_eq!(composition_to_string(&state), "・…‥");
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
        assert_eq!(
            cursor,
            display.chars().count(),
            "カーソルは表示テキストの末尾({}文字目)にあるべき",
            display.chars().count()
        );
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
        assert_eq!(
            cursor,
            display.chars().count(),
            "カーソルは末尾({})にあるべきだが{}だった",
            display.chars().count(),
            cursor
        );
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
        assert_eq!(
            cursor, 2,
            "カーソルはｶﾞの後ろ(2)にあるべき、実際: {}",
            cursor
        );
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

        assert_eq!(
            state.get_input_mode(),
            InputMode::HalfWidthAlphanumeric,
            "ひらがな→Caps→半角英数になるべき"
        );
    }

    /// 半角英数でToggleAlphanumericを押すとひらがなに戻る
    #[test]
    fn test_toggle_alphanumeric_back_to_hiragana() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::HalfWidthAlphanumeric);

        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);

        assert_eq!(
            state.get_input_mode(),
            InputMode::Hiragana,
            "半角英数→Caps→ひらがなに戻るべき"
        );
    }

    /// 全角カタカナでToggleAlphanumericを押すとひらがなに戻る（Mozc準拠）
    #[test]
    fn test_toggle_alphanumeric_from_full_width_katakana() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::FullWidthKatakana);

        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);

        assert_eq!(
            state.get_input_mode(),
            InputMode::Hiragana,
            "全角カタカナ→Caps→ひらがなに戻るべき(半角英数ではない)"
        );
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

        assert_eq!(
            state.get_display_text(),
            "hello",
            "半角英数モードでは 'hello' がそのまま入るべき"
        );
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
        assert_eq!(
            state.get_display_text(),
            "ka",
            "切替時にCompositionも半角英数に変換されるべき"
        );

        // 切替後の入力はASCIIになる
        state.input_char('k');
        state.input_char('a');
        assert_eq!(
            state.get_display_text(),
            "kaka",
            "モード切替後はASCIIが入るべき"
        );
    }

    /// 半角英数モードで 'n' を単独入力してもローマ字変換されない
    #[test]
    fn test_half_width_alphanumeric_no_romaji_pending() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::HalfWidthAlphanumeric);

        state.input_char('n');

        // ひらがなモードなら「n」は pending になるが、英数モードでは直接確定
        assert!(
            !state.has_pending_input(),
            "英数モードではpendin入力にならない"
        );
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
        assert_eq!(
            state.get_input_mode(),
            InputMode::Hiragana,
            "全角英数→無変換→ひらがなに戻るべき"
        );
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
                assert_eq!(
                    state.get_input_mode(),
                    expected,
                    "{}周目 {}回目のSwitchKanaType後",
                    cycle + 1,
                    i + 1
                );
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
            assert_eq!(
                state.get_input_mode(),
                InputMode::FullWidthKatakana,
                "{:?}→カタカナキー→全角カタカナになるべき",
                mode
            );
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

        assert_eq!(
            state.get_input_mode(),
            InputMode::Hiragana,
            "全角英数→Caps→ひらがなに戻るべき(半角英数ではない)"
        );
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

        assert_eq!(
            state.get_display_text(),
            "カ",
            "全角カタカナモードで 'ka' → 'カ' になるべき"
        );
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

        assert_eq!(
            state.get_display_text(),
            "ｶ",
            "半角カタカナモードで 'ka' → 'ｶ' になるべき"
        );
    }

    /// カタカナキーで全角カタカナに切替後、入力が全角カタカナで出る
    #[test]
    fn test_input_after_katakana_key() {
        let mut state = ImeState::new();

        state.apply_mode_command(ModeCommand::SetInputMode(InputMode::FullWidthKatakana));

        for ch in "sakura".chars() {
            state.input_char(ch);
        }

        assert_eq!(
            state.get_display_text(),
            "サクラ",
            "全角カタカナモードで 'sakura' → 'サクラ' になるべき"
        );
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

        assert_eq!(
            state.get_display_text(),
            "か",
            "ひらがなキーで戻した後は 'ka' → 'か' になるべき"
        );
    }

    // ============================================================
    // 複合シナリオ：実際のユーザー操作をシミュレート
    // ============================================================

    /// ひらがな→英数→ひらがな→カタカナ→ひらがな と切替ながら入力する
    #[test]
    fn test_mode_switching_scenario() {
        let mut state = ImeState::new();

        // ひらがなで入力
        for ch in "a".chars() {
            state.input_char(ch);
        }
        assert_eq!(state.get_display_text(), "あ");

        // 英数キーで半角英数に切替 ("あ" → "a")
        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);
        state.input_char('b');
        // 半角英数モードなので 'a' + 'b' で "ab"
        assert_eq!(state.get_display_text(), "ab");

        // 英数キーでひらがなに戻す ("ab" → ひらがな表示では英字が全角になり "あｂか")
        state.apply_mode_command(ModeCommand::ToggleAlphanumeric);
        for ch in "ka".chars() {
            state.input_char(ch);
        }
        assert_eq!(state.get_display_text(), "あｂか");

        // カタカナキーで全角カタカナに切替 ("あｂか" → "アｂカサ")
        state.apply_mode_command(ModeCommand::SetInputMode(InputMode::FullWidthKatakana));
        for ch in "sa".chars() {
            state.input_char(ch);
        }
        assert_eq!(state.get_display_text(), "アｂカサ");

        // ひらがなキーでひらがなに戻す
        state.apply_mode_command(ModeCommand::SetInputMode(InputMode::Hiragana));
        for ch in "ta".chars() {
            state.input_char(ch);
        }
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
        for ch in "tesuto".chars() {
            state.input_char(ch);
        }
        state.convert_composition(InputMode::Hiragana);
        assert_eq!(state.get_display_text(), "てすと");
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);

        // F7: 全角カタカナ「テスト」
        let mut state = ImeState::new();
        for ch in "tesuto".chars() {
            state.input_char(ch);
        }
        state.convert_composition(InputMode::FullWidthKatakana);
        assert_eq!(state.get_display_text(), "テスト");
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);

        // F8: 半角カタカナ「ﾃｽﾄ」
        let mut state = ImeState::new();
        for ch in "tesuto".chars() {
            state.input_char(ch);
        }
        state.convert_composition(InputMode::HalfWidthKatakana);
        assert_eq!(state.get_display_text(), "ﾃｽﾄ");
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);

        // F9: 全角英数「ｔｅｓｕｔｏ」
        let mut state = ImeState::new();
        for ch in "tesuto".chars() {
            state.input_char(ch);
        }
        state.convert_composition(InputMode::FullWidthAlphanumeric);
        assert_eq!(state.get_display_text(), "ｔｅｓｕｔｏ");
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);

        // F10: 半角英数「tesuto」
        let mut state = ImeState::new();
        for ch in "tesuto".chars() {
            state.input_char(ch);
        }
        state.convert_composition(InputMode::HalfWidthAlphanumeric);
        assert_eq!(state.get_display_text(), "tesuto");
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);
    }

    #[test]
    fn test_convert_composition_cycles() {
        let mut state = ImeState::new();
        for ch in "tesuto".chars() {
            state.input_char(ch);
        }

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
        for ch in "kan".chars() {
            state.input_char(ch);
        }
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
        for ch in "tesuto".chars() {
            state.input_char(ch);
        }
        state.convert_composition(InputMode::FullWidthKatakana);
        assert_eq!(state.get_display_text(), "テスト");

        state.clear();
        assert_eq!(state.get_display_text(), "");
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);

        // 次の入力はひらがなモードのまま
        state.input_char('a');
        assert_eq!(state.get_display_text(), "あ");
    }

    // ============================================================
    // スペースキー入力
    // ============================================================

    fn assert_space_for_mode(
        mode: InputMode,
        is_shift: bool,
        expected: &str,
    ) {
        let mut state = ImeState::new();

        // 入力状態を初期化し、テスト対象の入力モードを設定する。
        state.clear();
        state.set_input_mode(mode);

        // Spaceを入力する。
        let actual = state.input_space(is_shift);

        // 入力されたスペースを確認する。
        assert_eq!(
            actual,
            expected,
            "input_space() の戻り値が不正: mode={mode:?}, shift={is_shift}"
        );

        // Compositionの表示内容も確認する。
        assert_eq!(
            state.get_display_text(),
            expected,
            "表示文字列が不正: mode={mode:?}, shift={is_shift}"
        );
    }

    #[test]
    fn test_space_input_per_mode() {
        // ひらがなモード
        assert_space_for_mode(InputMode::Hiragana, false, "　");
        assert_space_for_mode(InputMode::Hiragana, true, " ");

        // 全角カタカナモード
        assert_space_for_mode(InputMode::FullWidthKatakana, false, "　");
        assert_space_for_mode(InputMode::FullWidthKatakana, true, " ");

        // 全角英数モード
        assert_space_for_mode(InputMode::FullWidthAlphanumeric, false, "　");
        assert_space_for_mode(InputMode::FullWidthAlphanumeric, true, " ");

        // 半角カタカナモード
        assert_space_for_mode(InputMode::HalfWidthKatakana, false, " ");
        assert_space_for_mode(InputMode::HalfWidthKatakana, true, "　");

        // 半角英数モード
        assert_space_for_mode(InputMode::HalfWidthAlphanumeric, false, " ");
        assert_space_for_mode(InputMode::HalfWidthAlphanumeric, true, "　");
    }

    #[test]
    fn test_dictionary_conversion_on_space() {
        let mut state = ImeState::new();
        for ch in "nihongo".chars() {
            state.input_char(ch);
        }

        state.input_space(false);

        assert_eq!(state.get_display_text(), "日本語");
    }

    #[test]
    fn test_dictionary_conversion_cycles_candidates() {
        let mut state = ImeState::new();
        for ch in "nihongo".chars() {
            state.input_char(ch);
        }

        state.input_space(false);
        let first = state.get_display_text();
        state.input_space(false);

        assert_eq!(first, "日本語");
        assert_ne!(state.get_display_text(), first);
    }

    #[test]
    fn test_backspace_cancels_dictionary_conversion_without_deleting_reading() {
        let mut state = ImeState::new();

        for ch in "nihongo".chars() {
            state.input_char(ch);
        }

        // 辞書変換を開始する。
        state.input_space(false);

        assert!(state.get_dictionary_candidate_index().is_some());

        // 1回目のBackspaceでは変換をキャンセルする。
        state.backspace();

        assert_eq!(state.get_display_text(), "にほんご");
        assert_eq!(state.get_dictionary_candidate_index(), None);
        assert!(state.get_dictionary_candidates().is_empty());

        // 2回目のBackspaceでは通常の文字削除を行う。
        state.backspace();

        assert_eq!(state.get_display_text(), "にほん");
    }

    #[test]
    fn test_backspace_restores_invalid_romaji_for_correction() {
        let mut state = ImeState::new();

        // 「しん」を入力する。
        for ch in "shinb".chars() {
            state.input_char(ch);
        }

        // 誤った「n」を入力する。
        state.input_char('n');
        assert_eq!(state.get_display_text(), "しんｂｎ");

        // uによって「b」が通常文字として出力され、
        // 「nu」が「ぬ」に変換される。
        state.input_char('u');
        assert_eq!(state.get_display_text(), "しんｂぬ");

        // 1回目のBackspaceで、uを入力する前の状態に戻る。
        state.backspace();
        assert_eq!(state.get_display_text(), "しんｂｎ");

        // 2回目のBackspaceで誤字のnを削除する。
        state.backspace();
        assert_eq!(state.get_display_text(), "しんｂ");

        // bにuを続けて、正しく「ぶ」に変換される。
        state.input_char('u');
        assert_eq!(state.get_display_text(), "しんぶ");
    }

    #[test]
    fn test_dictionary_candidate_navigation() {
        let mut state = ImeState::new();

        for ch in "nihon".chars() {
            state.input_char(ch);
        }

        state.input_space(false);

        let candidates = state.get_dictionary_candidates().to_vec();
        assert!(candidates.len() >= 2);

        assert_eq!(state.get_dictionary_candidate_index(), Some(0));

        let first_surface = state.get_display_text();
        let second_candidate = candidates[1].clone();

        // 次の候補を選択する。
        let selected = state.select_next_dictionary_candidate();

        assert_eq!(selected, second_candidate);
        assert_eq!(state.get_dictionary_candidate_index(), Some(1));
        assert_ne!(state.get_display_text(), first_surface);

        // 前の候補へ戻る。
        let selected = state.select_previous_dictionary_candidate();

        assert_eq!(selected, candidates[0]);
        assert_eq!(state.get_dictionary_candidate_index(), Some(0));
        assert_eq!(state.get_display_text(), first_surface);

        // 先頭からさらに前へ移動すると末尾へ進む。
        let last_index = candidates.len() - 1;
        let last_candidate = candidates[last_index].clone();

        let selected = state.select_previous_dictionary_candidate();

        assert_eq!(selected, last_candidate);
        assert_eq!(
            state.get_dictionary_candidate_index(),
            Some(last_index)
        );
    }

    #[test]
    fn test_cancel_dictionary_conversion_restores_reading() {
        let mut state = ImeState::new();

        // 「にほんご」を入力して辞書変換する。
        for ch in "nihongo".chars() {
            state.input_char(ch);
        }

        state.input_space(false);

        // 辞書候補を選択していることを確認する。
        assert!(state.get_dictionary_candidate_index().is_some());
        assert!(!state.get_dictionary_candidates().is_empty());

        // Esc相当の処理で変換をキャンセルする。
        state.cancel_dictionary_conversion_and_restore_reading();

        // 元の読みへ戻り、候補選択が終了している。
        assert_eq!(state.get_display_text(), "にほんご");
        assert_eq!(state.get_dictionary_candidate_index(), None);
        assert!(state.get_dictionary_candidates().is_empty());
    }

    #[test]
    fn test_space_without_dictionary_candidate_preserves_reading() {
        let mut state = ImeState::new();

        // 辞書にない長い読みを作る。
        // 「か」が16回続く読みを使用する。
        for _ in 0..16 {
            state.input_char('k');
            state.input_char('a');
        }

        state.input_char('!');

        let reading: String = state.composition.get_content_units().iter().collect();

        // テスト対象の読みが辞書に存在しないことを確認する。
        assert!(Dictionary::embedded().lookup(&reading).is_empty());

        let before = state.get_display_text();

        // Spaceで変換を試みる。
        state.input_space(false);

        // 候補がない場合、読みも表示文字列も維持する。
        assert_eq!(state.get_display_text(), before);
        assert_eq!(state.get_dictionary_candidate_index(), None);
    }

    /// 文節移動テスト用の候補を作成する。
    fn make_test_candidate(
        surface: &str,
        context_id: u16,
        cost: i32,
    ) -> crate::kanji_converter::KanjiCandidate {
        crate::kanji_converter::KanjiCandidate {
            surface: surface.to_string(),
            left_id: context_id,
            right_id: context_id,
            word_cost: cost,
            contextual_cost: i64::from(cost),
        }
    }

    /// 文節移動テスト用の2文節変換結果を設定する。
    ///
    /// 文節1: わたし → 私 / 渡し
    /// 文節2: は     → は / 派
    fn install_test_dictionary_conversion(state: &mut ImeState) {
        let first_segment = crate::kanji_converter::ConversionSegment {
            reading: "わたし".to_string(),
            start: 0,
            end: 3,
            candidates: vec![
                make_test_candidate("私", 1, 0),
                make_test_candidate("渡し", 1, 10),
            ],
            selected_index: 0,
        };

        let second_segment = crate::kanji_converter::ConversionSegment {
            reading: "は".to_string(),
            start: 3,
            end: 4,
            candidates: vec![
                make_test_candidate("は", 2, 0),
                make_test_candidate("派", 2, 10),
            ],
            selected_index: 0,
        };

        state.dictionary_reading = Some("わたしは".to_string());

        state.dictionary_conversion =
            Some(crate::kanji_converter::KanjiConversionResult {
                reading: "わたしは".to_string(),
                segments: vec![first_segment, second_segment],
                total_cost: 0,
            });

        state.dictionary_segment_index = 0;
        state.update_dictionary_candidate_view();

        let surface = state
            .dictionary_conversion
            .as_ref()
            .unwrap()
            .surface();

        state.replace_composition(&surface);
    }

    #[test]
    fn test_move_to_next_dictionary_segment_updates_candidates() {
        let mut state = ImeState::new();
        install_test_dictionary_conversion(&mut state);

        assert_eq!(state.get_display_text(), "私は");
        assert_eq!(state.dictionary_segment_index, 0);

        assert_eq!(
            state.get_dictionary_candidates()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["私", "渡し"]
        );

        assert!(state.move_to_next_dictionary_segment());

        assert_eq!(state.dictionary_segment_index, 1);

        assert_eq!(
            state.get_dictionary_candidates()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["は", "派"]
        );

        assert_eq!(state.get_dictionary_candidate_index(), Some(0));

        // 文節の選択対象を移動しても、変換結果全体は変わらない。
        assert_eq!(state.get_display_text(), "私は");
    }

    #[test]
    fn test_move_to_previous_dictionary_segment_updates_candidates() {
        let mut state = ImeState::new();
        install_test_dictionary_conversion(&mut state);

        assert!(state.move_to_next_dictionary_segment());
        assert!(state.move_to_previous_dictionary_segment());

        assert_eq!(state.dictionary_segment_index, 0);

        assert_eq!(
            state.get_dictionary_candidates()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["私", "渡し"]
        );

        assert_eq!(state.get_dictionary_candidate_index(), Some(0));
        assert_eq!(state.get_display_text(), "私は");
    }

    #[test]
    fn test_dictionary_segment_navigation_stops_at_boundaries() {
        let mut state = ImeState::new();
        install_test_dictionary_conversion(&mut state);

        // 最初の文節より前には移動しない。
        assert!(!state.move_to_previous_dictionary_segment());
        assert_eq!(state.dictionary_segment_index, 0);

        assert!(state.move_to_next_dictionary_segment());

        // 最後の文節より後ろには移動しない。
        assert!(!state.move_to_next_dictionary_segment());
        assert_eq!(state.dictionary_segment_index, 1);

        assert_eq!(state.get_display_text(), "私は");
    }

    #[test]
    fn test_candidate_change_after_segment_navigation_affects_only_target() {
        let mut state = ImeState::new();
        install_test_dictionary_conversion(&mut state);

        assert!(state.move_to_next_dictionary_segment());

        let selected = state.select_next_dictionary_candidate();

        assert_eq!(selected, "派");
        assert_eq!(state.get_display_text(), "私派");

        let conversion = state.dictionary_conversion.as_ref().unwrap();

        assert_eq!(
            conversion.segments[0]
                .selected_candidate()
                .unwrap()
                .surface,
            "私"
        );

        assert_eq!(
            conversion.segments[1]
                .selected_candidate()
                .unwrap()
                .surface,
            "派"
        );
    }

    #[test]
    fn test_dictionary_segment_navigation_without_conversion() {
        let mut state = ImeState::new();

        assert!(!state.move_to_next_dictionary_segment());
        assert!(!state.move_to_previous_dictionary_segment());

        assert_eq!(state.get_display_text(), "");
    }

    // ============================================================
    // 回帰テスト用ヘルパー
    // ============================================================

    /// ローマ字を入力し、Spaceで辞書変換した状態を取得する。
    ///
    /// 引数:
    /// * romaji: 入力するローマ字
    ///
    /// 戻り値:
    /// * ImeState: 変換操作後の状態
    fn convert_romaji_for_regression(romaji: &str) -> ImeState {
        let mut state = ImeState::new();

        for ch in romaji.chars() {
            state.input_char(ch);
        }

        state.input_space(false);

        state
    }

    // ============================================================
    // 文章変換の回帰テスト
    // ============================================================

    /// 実機で変換できた文章が、引き続き期待する結果になることを確認する。
    #[test]
    fn test_sentence_conversion_regression_cases() {
        let cases = [
            (
                "nihongowobenkyousuru",
                "日本語を勉強する",
            ),
            (
                "kyouhaiitenkidesu",
                "今日はいい天気です",
            ),
            (
                "ashitatoukyouheikimasu",
                "明日東京へ行きます",
            ),
            (
                "watashihanihongowobenkyoushiteimasu",
                "私は日本語を勉強しています",
            ),
            (
                "iitoomouyo",
                "いいと思うよ",
            ),
            (
                "watashihasushigasukidesu",
                "私は寿司が好きです",
            ),
        ];

        for (romaji, expected) in cases {
            let state = convert_romaji_for_regression(romaji);

            assert_eq!(
                state.get_display_text(),
                expected,
                "文章変換の結果が変わった: input='{}'",
                romaji
            );
        }
    }

    /// 代表的な文章でViterbiによる変換結果が取得できることを確認する。
    #[test]
    fn test_sentence_conversion_produces_viterbi_result_regression() {
        let cases = [
            "nihongowobenkyousuru",
            "kyouhaiitenkidesu",
            "ashitatokyouheikimasu",
            "watashihanihongowobenkyoushiteimasu",
            "iitoomouyo",
            "watashihasushigasukidesu",
            "kinouhaamedadattakeredokyouhaharedesu",
        ];

        for romaji in cases {
            let state = convert_romaji_for_regression(romaji);

            assert!(
                state.has_dictionary_conversion(),
                "Viterbi変換が行われていない: input='{}'",
                romaji
            );

            assert!(
                !state.get_display_text().is_empty(),
                "変換結果が空: input='{}'",
                romaji
            );
        }
    }

    /// 変換前の読みが辞書変換状態に保持されることを確認する。
    #[test]
    fn test_sentence_conversion_preserves_original_reading_regression() {
        let cases = [
            ("nihongowobenkyousuru", "にほんごをべんきょうする"),
            ("kyouhaiitenkidesu", "きょうはいいてんきです"),
            ("ashitatoukyouheikimasu", "あしたとうきょうへいきます"),
            (
                "watashihanihongowobenkyoushiteimasu",
                "わたしはにほんごをべんきょうしています",
            ),
            ("iitoomouyo", "いいとおもうよ"),
            ("watashihasushigasukidesu", "わたしはすしがすきです"),
        ];

        for (romaji, expected_reading) in cases {
            let state = convert_romaji_for_regression(romaji);

            assert_eq!(
                state.dictionary_reading.as_deref(),
                Some(expected_reading),
                "元の読みが保持されていない: input='{}'",
                romaji
            );
        }
    }

    // ============================================================
    // 文節解析の回帰テスト
    // ============================================================

    /// 文節の範囲が元の読みを漏れなく連続して覆っていることを確認する。
    #[test]
    fn test_sentence_conversion_segment_ranges_regression() {
        let cases = [
            "nihongowobenkyousuru",
            "kyouhaiitenkidesu",
            "ashitatokyouheikimasu",
            "watashihanihongowobenkyoushiteimasu",
            "iitoomouyo",
            "watashihasushigasukidesu",
            "kinouhaamedadattakeredokyouhaharedesu",
        ];

        for romaji in cases {
            let state = convert_romaji_for_regression(romaji);

            let reading = state
                .dictionary_reading
                .as_deref()
                .expect("変換前の読みが存在すること");

            let conversion = state
                .dictionary_conversion
                .as_ref()
                .expect("Viterbi変換結果が存在すること");

            let reading_chars: Vec<char> = reading.chars().collect();
            let mut expected_start = 0;

            assert!(
                !conversion.segments.is_empty(),
                "文節が存在しない: input='{}'",
                romaji
            );

            for segment in &conversion.segments {
                assert_eq!(
                    segment.start,
                    expected_start,
                    "文節の範囲が連続していない: input='{}'",
                    romaji
                );

                assert!(
                    segment.start < segment.end,
                    "文節の範囲が空、または不正: input='{}'",
                    romaji
                );

                assert!(
                    segment.end <= reading_chars.len(),
                    "文節の終端が読みの長さを超えている: input='{}'",
                    romaji
                );

                let expected_reading: String = reading_chars
                    [segment.start..segment.end]
                    .iter()
                    .collect();

                assert_eq!(
                    segment.reading,
                    expected_reading,
                    "文節の読みが範囲と一致しない: input='{}'",
                    romaji
                );

                assert!(
                    !segment.candidates.is_empty(),
                    "文節の候補が空: input='{}', segment='{}'",
                    romaji,
                    segment.reading
                );

                assert!(
                    segment.selected_index < segment.candidates.len(),
                    "選択中の候補インデックスが不正: input='{}'",
                    romaji
                );

                expected_start = segment.end;
            }

            assert_eq!(
                expected_start,
                reading_chars.len(),
                "文節が読み全体を覆っていない: input='{}'",
                romaji
            );
        }
    }

    /// 各文節の選択候補を連結した結果が、表示文字列と一致することを確認する。
    #[test]
    fn test_sentence_surface_matches_display_regression() {
        let cases = [
            "nihongowobenkyousuru",
            "kyouhaiitenkidesu",
            "ashitatokyouheikimasu",
            "watashihanihongowobenkyoushiteimasu",
            "iitoomouyo",
            "watashihasushigasukidesu",
            "kinouhaamedadattakeredokyouhaharedesu",
        ];

        for romaji in cases {
            let state = convert_romaji_for_regression(romaji);

            let conversion = state
                .dictionary_conversion
                .as_ref()
                .expect("Viterbi変換結果が存在すること");

            assert_eq!(
                state.get_display_text(),
                conversion.surface(),
                "表示文字列と選択中の文節候補が一致しない: input='{}'",
                romaji
            );
        }
    }

    // ============================================================
    // 同音異義語・候補生成の回帰テスト
    // ============================================================

    /// 「昨日」が長い文章のどこかの文節候補に含まれることを確認する。
    ///
    /// 先頭候補であることは要求しない。
    /// 候補順位の改善時に既存仕様を誤って固定しないため。
    #[test]
    fn test_kinou_candidate_is_available_in_sentence_regression() {
        let state = convert_romaji_for_regression(
            "kinouhaamedadattakeredokyouhaharedesu",
        );

        let conversion = state
            .dictionary_conversion
            .as_ref()
            .expect("Viterbi変換結果が存在すること");

        let found = conversion
            .segments
            .iter()
            .flat_map(|segment| segment.candidates.iter())
            .any(|candidate| candidate.surface.contains("昨日"));

        assert!(
            found,
            "「昨日」を含む候補が存在しない"
        );
    }

    /// 候補を切り替えても、全文字列と文節の選択状態が整合することを確認する。
    #[test]
    fn test_sentence_candidate_navigation_consistency_regression() {
        let mut state = convert_romaji_for_regression("kyouhaiitenkidesu");

        assert!(state.has_dictionary_conversion());

        let before = state.get_display_text();

        assert!(
            !state.get_dictionary_candidates().is_empty(),
            "最初の文節に候補が存在すること"
        );

        state.select_next_dictionary_candidate();

        let conversion = state
            .dictionary_conversion
            .as_ref()
            .expect("候補切り替え後も変換結果が存在すること");

        assert_eq!(
            state.get_display_text(),
            conversion.surface(),
            "次候補の選択後、表示と変換結果が一致すること"
        );

        state.select_previous_dictionary_candidate();

        let conversion = state
            .dictionary_conversion
            .as_ref()
            .expect("前候補の選択後も変換結果が存在すること");

        assert_eq!(
            state.get_display_text(),
            conversion.surface(),
            "前候補の選択後、表示と変換結果が一致すること"
        );

        assert_eq!(
            state.get_display_text(),
            before,
            "次候補、前候補の操作で元の候補へ戻ること"
        );
    }

    // ============================================================
    // 変換キャンセルの回帰テスト
    // ============================================================

    /// Esc相当の処理で元の読みと未変換状態を復元する。
    #[test]
    fn test_escape_cancel_restores_sentence_reading_regression() {
        let cases = [
            ("nihongowobenkyousuru", "にほんごをべんきょうする"),
            ("kyouhaiitenkidesu", "きょうはいいてんきです"),
            ("iitoomouyo", "いいとおもうよ"),
        ];

        for (romaji, expected_reading) in cases {
            let mut state = convert_romaji_for_regression(romaji);

            state.cancel_dictionary_conversion_and_restore_reading();

            assert_eq!(
                state.get_display_text(),
                expected_reading,
                "キャンセル後に元の読みへ戻らない: input='{}'",
                romaji
            );

            assert!(!state.has_dictionary_conversion());
            assert_eq!(state.get_dictionary_candidate_index(), None);
            assert!(state.get_dictionary_candidates().is_empty());
        }
    }

    /// 変換キャンセル後にもう一度変換できることを確認する。
    #[test]
    fn test_sentence_can_be_converted_again_after_cancel_regression() {
        let mut state = ImeState::new();

        for ch in "nihongowobenkyousuru".chars() {
            state.input_char(ch);
        }

        state.input_space(false);
        assert!(state.has_dictionary_conversion());

        state.cancel_dictionary_conversion_and_restore_reading();

        assert_eq!(
            state.get_display_text(),
            "にほんごをべんきょうする"
        );

        state.input_space(false);

        assert!(
            state.has_dictionary_conversion(),
            "キャンセル後に再変換できること"
        );

        assert_eq!(
            state.get_display_text(),
            "日本語を勉強する"
        );
    }

    // ============================================================
    // Backspace・Delete・カーソル操作の回帰テスト
    // ============================================================

    /// 未変換ローマ字がある場合、Backspaceでpending入力を削除する。
    #[test]
    fn test_backspace_removes_pending_romaji_regression() {
        let mut state = ImeState::new();

        state.input_char('k');

        assert!(state.has_pending_input());
        assert_eq!(state.get_display_text(), "ｋ");

        state.backspace();

        assert!(!state.has_pending_input());
        assert_eq!(state.get_display_text(), "");
    }

    /// カーソルが文字列の途中にある場合、Backspaceは左側の文字を削除する。
    #[test]
    fn test_backspace_at_middle_of_composition_regression() {
        let mut state = ImeState::new();

        for ch in "aiu".chars() {
            state.input_char(ch);
        }

        assert_eq!(state.get_display_text(), "あいう");

        state.move_cursor_left();

        assert_eq!(state.get_cursor_pos(), 2);

        state.backspace();

        assert_eq!(
            state.get_display_text(),
            "あう",
            "カーソルの左側の1文字だけ削除すること"
        );
        assert_eq!(state.get_cursor_pos(), 1);
    }

    /// カーソルが文字列の途中にある場合、Deleteは右側の文字を削除する。
    #[test]
    fn test_delete_at_middle_of_composition_regression() {
        let mut state = ImeState::new();

        for ch in "aiu".chars() {
            state.input_char(ch);
        }

        state.move_cursor_left();

        assert_eq!(state.get_cursor_pos(), 2);

        state.delete();

        assert_eq!(
            state.get_display_text(),
            "あい",
            "カーソルの右側の1文字だけ削除すること"
        );
        assert_eq!(state.get_cursor_pos(), 2);
    }

    /// 文字列の先頭で左移動・Backspaceを実行しても状態が壊れないことを確認する。
    #[test]
    fn test_backspace_at_head_is_safe_regression() {
        let mut state = ImeState::new();

        for ch in "aiu".chars() {
            state.input_char(ch);
        }

        state.move_cursor_to_head();

        assert_eq!(state.get_cursor_pos(), 0);

        state.move_cursor_left();
        state.backspace();

        assert_eq!(state.get_display_text(), "あいう");
        assert_eq!(state.get_cursor_pos(), 0);
    }

    /// 文字列の末尾で右移動・Deleteを実行しても状態が壊れないことを確認する。
    #[test]
    fn test_delete_at_tail_is_safe_regression() {
        let mut state = ImeState::new();

        for ch in "aiu".chars() {
            state.input_char(ch);
        }

        assert_eq!(state.get_cursor_pos(), 3);

        state.move_cursor_right();
        state.delete();

        assert_eq!(state.get_display_text(), "あいう");
        assert_eq!(state.get_cursor_pos(), 3);
    }

    /// 変換をBackspaceでキャンセルした後も入力を続けられることを確認する。
    #[test]
    fn test_input_continues_after_backspace_cancels_conversion_regression() {
        let mut state = ImeState::new();

        for ch in "nihongo".chars() {
            state.input_char(ch);
        }

        state.input_space(false);
        assert!(state.has_dictionary_conversion());

        state.backspace();

        assert_eq!(state.get_display_text(), "にほんご");
        assert!(!state.has_dictionary_conversion());

        for ch in "desu".chars() {
            state.input_char(ch);
        }

        assert_eq!(
            state.get_display_text(),
            "にほんごです",
            "キャンセル後の文字入力が正しく継続すること"
        );
    }

    // ============================================================
    // 多様な入力の基本不変条件
    // ============================================================

    /// さまざまなローマ字入力で表示文字列とカーソル位置が整合することを確認する。
    #[test]
    fn test_cursor_within_display_for_varied_romaji_regression() {
        let inputs = [
            "konnichiwa",
            "gakkou",
            "chotto",
            "shinjuku",
            "kyou",
            "kinou",
            "shinbun",
            "ginkou",
            "nihongo",
            "benkyou",
            "tokyo",
            "watashi",
            "sushi",
            "sukidesu",
            "arigatou",
            "toukyou",
            "ryokou",
            "byouin",
            "shigoto",
            "densha",
            "sensei",
            "tomodachi",
            "kippu",
            "issho",
            "qzx",
        ];

        for input in inputs {
            let mut state = ImeState::new();

            for ch in input.chars() {
                state.input_char(ch);
                assert_cursor_within_display(&state);
            }

            state.move_cursor_to_head();
            assert_cursor_within_display(&state);

            state.move_cursor_to_tail();
            assert_cursor_within_display(&state);

            state.backspace();
            assert_cursor_within_display(&state);
        }
    }

    /// 変換を確定・キャンセルした後、次の入力が可能であることを確認する。
    #[test]
    fn test_state_can_be_reused_after_clear_regression() {
        let mut state = convert_romaji_for_regression("nihongo");

        assert!(state.has_active_input());

        state.clear();

        assert_eq!(state.get_display_text(), "");
        assert!(!state.has_active_input());
        assert!(!state.has_dictionary_conversion());

        for ch in "ai".chars() {
            state.input_char(ch);
        }

        assert_eq!(state.get_display_text(), "あい");
        assert_cursor_within_display(&state);
    }

    // ============================================================
    // Viterbi変換経路の診断
    // ============================================================

    /// 長文変換について、文節・候補・コスト・選択結果を出力する。
    ///
    /// このテストは変換結果を診断するためのものであり、
    /// 特定の候補が最上位であることは要求しない。
    #[test]
    fn diagnose_kinou_sentence_conversion() {
        let reading = "きのうはあめだったけれどきょうははれです";

        let connection_costs = ConnectionCosts::embedded()
            .ok()
            .expect("接続コストデータを読み込めること");

        let converter =
            KanjiConverter::new(Dictionary::embedded(), &connection_costs);

        let result = converter
            .convert(reading)
            .ok()
            .expect("文章のViterbi変換が成功すること");

        println!("\n========== Viterbi診断 ==========");
        println!("読み: {}", reading);
        println!("変換結果: {}", result.surface());
        println!("合計コスト: {}", result.total_cost);
        println!("文節数: {}", result.segments.len());

        for (segment_index, segment) in result.segments.iter().enumerate() {
            let selected_surface = segment
                .candidates
                .get(segment.selected_index)
                .map(|candidate| candidate.surface.as_str())
                .unwrap_or("<invalid selected_index>");

            println!(
                "\n文節[{}]: reading='{}', range={}..{}",
                segment_index,
                segment.reading,
                segment.start,
                segment.end
            );

            println!(
                "  選択候補: [{}] '{}'",
                segment.selected_index,
                selected_surface
            );

            println!("  候補数: {}", segment.candidates.len());

            for (candidate_index, candidate) in
                segment.candidates.iter().enumerate()
            {
                let marker = if candidate_index == segment.selected_index {
                    ">"
                } else {
                    " "
                };

                println!(
                    "  {} [{}] surface='{}', \
                     word_cost={}, contextual_cost={}, \
                     left_id={}, right_id={}",
                    marker,
                    candidate_index,
                    candidate.surface,
                    candidate.word_cost,
                    candidate.contextual_cost,
                    candidate.left_id,
                    candidate.right_id
                );
            }
        }

        println!("\n========== 診断終了 ==========\n");
    }

    #[test]
    fn test_digits_are_half_width_except_full_width_alphanumeric_mode() {
        let modes = [
            InputMode::Hiragana,
            InputMode::FullWidthKatakana,
            InputMode::HalfWidthKatakana,
            InputMode::HalfWidthAlphanumeric,
        ];

        for mode in modes {
            let mut state = ImeState::new();
            state.set_input_mode(mode);

            for ch in "123".chars() {
                state.input_char(ch);
            }

            assert_eq!(
                state.get_display_text(),
                "123",
                "数字は半角であること: mode={mode:?}",
            );
        }

        let mut state = ImeState::new();
        state.set_input_mode(InputMode::FullWidthAlphanumeric);

        for ch in "123".chars() {
            state.input_char(ch);
        }

        assert_eq!(state.get_display_text(), "１２３");
    }
}
