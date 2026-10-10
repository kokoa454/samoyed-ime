use std::mem::ManuallyDrop;
use std::ptr::null;
use std::sync::{Arc, Mutex};

use windows::core::{implement, Ref, Result as WinResult, Interface};
use windows::Win32::UI::TextServices::{
    ITfComposition, ITfCompositionSink, ITfCompositionSink_Impl, ITfContext, ITfContextComposition, ITfEditSession, ITfEditSession_Impl,
    TF_ANCHOR_END, TF_ANCHOR_START, TF_SELECTION,
};
use ime_core::{ImeState, InputMode, ModeCommand};

use crate::text_input_processor::SharedState;
use crate::logging::log;


/// EditSessionで実行する操作
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditAction {
    InputChar(char), // ローマ字を入力
    Backspace, // カーソルの直前の文字を削除
    Delete, // カーソルの直後の文字を削除
    MoveCursorLeft, // カーソルを左に移動
    MoveCursorRight, // カーソルを右に移動
    MoveCursorToHead, // カーソルを先頭に移動
    MoveCursorToTail, // カーソルを末尾に移動
    Commit, // 現在のCompositionを確定
    Clear, // 現在のCompositionを削除
    SetOpen(bool, Option<InputMode>), // IMEの開閉状態を変更
    ApplyModeCommand(ModeCommand), // モードコマンドを適用
    ConvertComposition(InputMode), // ファンクションキーによるComposition変換
    InputSpace(bool), // スペースを入力 (is_shift)
}

/// Composition終了通知Sink
#[implement(ITfCompositionSink)]
pub struct SamoyedIMECompositionSink {
    state: Arc<Mutex<SharedState>>,
}

impl SamoyedIMECompositionSink {
    pub fn new(
        state: Arc<Mutex<SharedState>>,
    ) -> Self {
        Self {
            state,
        }
    }
}

impl ITfCompositionSink_Impl
    for SamoyedIMECompositionSink_Impl
{
    /// Compositionが終了したときに呼ばれる。
    ///
    /// # 引数
    /// * `_ecwrite`: EditContextのライトフラグ
    /// * `_pcomposition`: Compositionオブジェクト
    ///
    /// # 戻り値
    /// * `Ok(())`: 成功した場合
    /// * `Err(E_FAIL)`: 失敗した場合
    fn OnCompositionTerminated(
        &self,
        _ecwrite: u32,
        _pcomposition: Ref<'_, ITfComposition>,
    ) -> WinResult<()> {
        log("[SamoyedIME] OnCompositionTerminated");

        let mut state = self.state.lock().unwrap();

        if state.composition.is_some() {
            state.composition = None;
            state.ime_state.clear();
        }

        Ok(())
    }
}

/// EditSession本体
#[implement(ITfEditSession)]
pub struct SamoyedIMEEditSession {
    context: ITfContext,
    state: Arc<Mutex<SharedState>>,
    action: EditAction,
}

impl SamoyedIMEEditSession {
    /// コンストラクタ
    pub fn new(
        context: ITfContext,
        state: Arc<Mutex<SharedState>>,
        action: EditAction,
    ) -> Self {
        Self {
            context,
            state,
            action,
        }
    }
}

impl ITfEditSession_Impl for SamoyedIMEEditSession_Impl {
    /// EditSession本体
    ///
    /// # 引数
    /// * `ec`: EditContext
    ///
    /// # 戻り値
    /// * `Ok(())`: 成功した場合
    /// * `Err(E_FAIL)`: 失敗した場合
    fn DoEditSession(
        &self,
        ec: u32,
    ) -> WinResult<()> {
        unsafe {
            match &self.action {
                // キー入力
                EditAction::InputChar(input) => {
                    self.update_state_and_text(
                        ec,
                        |ime_state| {
                            ime_state.input_char(*input);
                        },
                    )
                }

                // Backspace
                EditAction::Backspace => {
                    self.update_state_and_text(
                        ec,
                        |ime_state| {
                            ime_state.backspace();
                        },
                    )
                }

                // Delete
                EditAction::Delete => {
                    self.update_state_and_text(
                        ec,
                        |ime_state| {
                            ime_state.delete();
                        },
                    )
                }

                // カーソル左移動
                EditAction::MoveCursorLeft => {
                    self.update_state_and_text(
                        ec,
                        |ime_state| {
                            ime_state.move_cursor_left();
                        },
                    )
                }

                // カーソル右移動
                EditAction::MoveCursorRight => {
                    self.update_state_and_text(
                        ec,
                        |ime_state| {
                            ime_state.move_cursor_right();
                        },
                    )
                }

                // カーソルを先頭に移動
                EditAction::MoveCursorToHead => {
                    self.update_state_and_text(
                        ec,
                        |ime_state| {
                            ime_state.move_cursor_to_head();
                        },
                    )
                }

                // カーソルを末尾に移動
                EditAction::MoveCursorToTail => {
                    self.update_state_and_text(
                        ec,
                        |ime_state| {
                            ime_state.move_cursor_to_tail();
                        },
                    )
                }

                // 現在のCompositionを確定
                EditAction::Commit => {
                    self.commit(ec)
                }

                // 現在のCompositionを削除
                EditAction::Clear => {
                    self.clear(ec)
                }

                // IMEの開閉状態を変更
                EditAction::SetOpen(is_open, target_mode) => {
                    self.set_open(ec, *is_open, *target_mode)
                }

                // モードコマンドを適用
                EditAction::ApplyModeCommand(command) => {
                    let result = self.update_state_and_text(
                        ec,
                        |ime_state| {
                            ime_state.apply_mode_command(*command);
                        },
                    );
                    SharedState::notify_lang_bar_update(&self.state);
                    result
                }

                // ファンクションキーによるComposition変換
                EditAction::ConvertComposition(target_mode) => {
                    self.update_state_and_text(
                        ec,
                        |ime_state| {
                            ime_state.convert_composition(*target_mode);
                        },
                    )
                }

                // スペース入力
                EditAction::InputSpace(is_shift) => {
                    self.update_state_and_text(
                        ec,
                        |ime_state| {
                            ime_state.input_space(*is_shift);
                        },
                    )
                }
            }
        }
    }
}

impl SamoyedIMEEditSession {
    /// IME状態を更新し、Compositionへ反映する。
    ///
    /// # 引数
    /// * `ec`: EditContext
    /// * `update`: IME状態を更新する処理
    ///
    /// # 戻り値
    /// * `Ok(())`: 成功した場合
    /// * `Err(E_FAIL)`: 失敗した場合
    unsafe fn update_state_and_text<F>(
        &self,
        ec: u32,
        update: F,
    ) -> WinResult<()>
    where
        F: FnOnce(&mut ImeState),
    {
        let (current_state, current_composition) = {
            let state = self.state.lock().unwrap();

            (
                state.ime_state.clone(),
                state.composition.clone(),
            )
        };

        let mut next_state = current_state.clone();

        update(&mut next_state);

        let current_text = current_state.get_display_text();
        let current_cursor_pos = current_state.get_cursor_pos();

        let next_text = next_state.get_display_text();
        let next_cursor_pos = next_state.get_cursor_pos();

        let text_changed = current_text != next_text;
        let cursor_changed = current_cursor_pos != next_cursor_pos;
        let state_changed = current_state != next_state;

        if !state_changed {
            return Ok(());
        }

        // Compositionがまだ無い場合
        if current_composition.is_none() {
            // Pending入力だけが変化した場合
            if next_text.is_empty() {
                let mut state = self.state.lock().unwrap();
                state.ime_state = next_state;

                log("[SamoyedIME] IME state updated without Composition");

                return Ok(());
            }

            let composition = unsafe {
                self.start_composition(
                    ec,
                    &next_text,
                    next_cursor_pos,
                )?
            };

            let mut state = self.state.lock().unwrap();

            state.composition = Some(composition);
            state.ime_state = next_state;

            return Ok(());
        }

        let composition = current_composition.unwrap();

        // Compositionの文字列が空になった場合
        if next_text.is_empty() {
            unsafe {
                self.clear_composition_text(
                    ec,
                    &composition,
                )?
            };

            let mut state = self.state.lock().unwrap();

            state.composition = None;
            state.ime_state = next_state;

            return Ok(());
        }

        // テキストが変化した場合だけCompositionの文字列を更新する
        if text_changed {
            unsafe {
                self.set_composition_text(
                    ec,
                    &composition,
                    &next_text,
                )?;
            }
        }

        // テキストまたはカーソルが変化した場合にSelectionを更新する
        if text_changed || cursor_changed {
            let cursor_pos_utf16 =
                Self::cursor_pos_to_utf16(
                    &next_text,
                    next_cursor_pos,
                );

            unsafe {
                Self::update_selection(
                    &self.context,
                    ec,
                    &composition,
                    cursor_pos_utf16,
                )?;
            }
        }

        let mut state = self.state.lock().unwrap();

        state.composition = Some(composition);
        state.ime_state = next_state;

        Ok(())
    }

    /// Compositionを開始する。
    ///
    /// # 引数
    /// * `ec`: EditContext
    /// * `text`: 未確定文字列
    /// * `cursor_pos`: カーソル位置
    ///
    /// # 戻り値
    /// * `ITfComposition`: 開始したComposition
    unsafe fn start_composition(
        &self,
        ec: u32,
        text: &str,
        cursor_pos: usize,
    ) -> WinResult<ITfComposition> {
        // InsertTextAtSelection(QUERYONLY)で、コンポジション開始用の範囲を取得する
        let insert_at_selection: windows::Win32::UI::TextServices::ITfInsertAtSelection =
            self.context.cast()?;

        let range = unsafe {
            insert_at_selection.InsertTextAtSelection(
                ec,
                windows::Win32::UI::TextServices::TF_IAS_QUERYONLY,
                &[],
            )?
        };

        // Compositionを開始
        let context_composition: ITfContextComposition = self.context.cast()?;

        let sink: ITfCompositionSink =
            SamoyedIMECompositionSink::new(self.state.clone()).into();

        let composition = unsafe {
            context_composition.StartComposition(ec, &range, &sink)?
        };

        log("[SamoyedIME] StartComposition succeeded");

        // 初期テキストを書き込む
        let utf16_text: Vec<u16> = text.encode_utf16().collect();

        let composition_range = unsafe { composition.GetRange()? };

        unsafe {
            composition_range.SetText(ec, 0, &utf16_text)?
        };

        log(&format!(
            "[SamoyedIME] Initial composition text inserted: '{}'",
            text
        ));

        // カーソル位置を設定する
        let cursor_pos_utf16 = Self::cursor_pos_to_utf16(text, cursor_pos);

        unsafe {
            Self::update_selection(&self.context, ec, &composition, cursor_pos_utf16)?
        };

        Ok(composition)
    }

    /// Compositionのテキストを更新する。
    ///
    /// # 引数
    /// * `ec`: EditContext
    /// * `composition`: Composition
    /// * `text`: 未確定文字列
    ///
    /// # 戻り値
    /// * `Ok(())`: 成功した場合
    /// * `Err(E_FAIL)`: 失敗した場合
    unsafe fn set_composition_text(
        &self,
        ec: u32,
        composition: &ITfComposition,
        text: &str,
    ) -> WinResult<()> {
        let utf16_text: Vec<u16> =
            text.encode_utf16().collect();

        let range = unsafe {
            composition.GetRange()?
        };

        unsafe {
            range.SetText(
                ec,
                0,
                &utf16_text,
            )?
        };

        log(&format!(
            "[SamoyedIME] Composition text updated: '{}'",
            text
        ));

        Ok(())
    }

    /// Compositionのテキストを削除する。
    ///
    /// # 引数
    /// * `ec`: EditContext
    /// * `composition`: Composition
    ///
    /// # 戻り値
    /// * `Ok(())`: 成功した場合
    /// * `Err(E_FAIL)`: 失敗した場合
    unsafe fn clear_composition_text(
        &self,
        ec: u32,
        composition: &ITfComposition,
    ) -> WinResult<()> {
        let range = unsafe { composition.GetRange()? };

        unsafe { range.SetText(ec, 0, &[],)? };
        unsafe { composition.EndComposition(ec)? };

        Ok(())
    }

    /// Selectionを更新する。
    ///
    /// # Arguments
    /// * `context` - Context
    /// * `ec` - EditContext
    /// * `composition` - Composition
    /// * `cursor_pos_utf16` - UTF-16単位のカーソル位置
    ///
    /// # Returns
    /// * `Ok(())` - Success
    /// * `Err(E_FAIL)` - Failure
    unsafe fn update_selection(
        context: &ITfContext,
        ec: u32,
        composition: &ITfComposition,
        cursor_pos_utf16: usize,
    ) -> WinResult<()> {
        let range = unsafe {
            composition.GetRange()?
        };

        let cursor_range = unsafe {
            range.Clone()?
        };

        unsafe {
            cursor_range.Collapse(
                ec,
                TF_ANCHOR_START,
            )?
        };

        let mut shifted = 0;

        unsafe {
            cursor_range.ShiftEnd(
                ec,
                cursor_pos_utf16 as i32,
                &mut shifted,
                null(),
            )?
        };

        if shifted != cursor_pos_utf16 as i32 {
            log(&format!(
                "[SamoyedIME] Cursor shift mismatch: requested={}, actual={}",
                cursor_pos_utf16,
                shifted
            ));
        }

        unsafe {
            cursor_range.Collapse(
                ec,
                TF_ANCHOR_END,
            )?
        };

        let selection = TF_SELECTION {
            range: ManuallyDrop::new(
                Some(cursor_range)
            ),
            style: Default::default(),
        };

        unsafe {
            context.SetSelection(
                ec,
                &[selection],
            )?
        };

        log(&format!(
            "[SamoyedIME] Selection updated: cursor_pos_utf16={}",
            cursor_pos_utf16
        ));

        Ok(())
    }

    /// 文字単位のカーソル位置をUTF-16単位へ変換する。
    ///
    /// # 引数
    /// * `text`: テキスト
    /// * `cursor_pos`: 文字単位のカーソル位置
    ///
    /// # 戻り値
    /// * `usize`: UTF-16単位のカーソル位置
    fn cursor_pos_to_utf16(
        text: &str,
        cursor_pos: usize,
    ) -> usize {
        text.chars()
            .take(cursor_pos)
            .map(char::len_utf16)
            .sum()
    }

    /// テキストを確定する。
    ///
    /// # 引数
    /// * `ec`: EditContext
    ///
    /// # 戻り値
    /// * `Ok(())`: 成功した場合
    /// * `Err(_)`: 失敗した場合
    unsafe fn commit(&self, ec: u32) -> WinResult<()> {
        let composition = {
            let state = self.state.lock().unwrap();
            state.composition.clone()
        };

        if let Some(composition) = composition {
            unsafe {
                composition.EndComposition(ec)?;
            }
        }

        // Compositionが無い場合でも未変換ローマ字は破棄
        let mut state = self.state.lock().unwrap();
        state.composition = None;
        state.ime_state.clear();

        Ok(())
    }

    /// テキストを削除する。
    ///
    /// # Arguments
    /// * `ec` - EditContext
    ///
    /// # Returns
    /// * `Ok(())` - Success
    /// * `Err(E_FAIL)` - Failure
    unsafe fn clear(
        &self,
        ec: u32,
    ) -> WinResult<()> {
        let composition = {
            let state = self.state.lock().unwrap();
            state.composition.clone()
        };

        if let Some(composition) = composition {
            let range = unsafe { composition.GetRange()? };

            unsafe { range.SetText(ec, 0, &[],)? };
            unsafe { composition.EndComposition(ec)? };
        }

        let mut state = self.state.lock().unwrap();

        state.composition = None;
        state.ime_state.clear();

        Ok(())
    }

    /// IMEの開閉状態を変更する。
    ///
    /// # 引数
    /// * `ec`: EditContext
    /// * `is_open`: 新しい開閉状態
    ///
    /// # 戻り値
    /// * `Ok(())`: 成功した場合
    /// * `Err(_)`: 確定またはコンパートメント更新に失敗した場合
    unsafe fn set_open(
        &self,
        ec: u32,
        is_open: bool,
        target_mode: Option<InputMode>,
    ) -> WinResult<()> {
        // 閉じる場合は未確定文字列を確定してから状態を切り替える
        if !is_open {
            unsafe { self.commit(ec)? };
        } else {
            let mut state = self.state.lock().unwrap();
            let mode = target_mode.unwrap_or(InputMode::Hiragana);
            state.ime_state.set_input_mode(mode);
        }

        // コンパートメントと is_open を同時に更新する
        SharedState::set_keyboard_open(&self.state, is_open)?;

        log(&format!("[SamoyedIME] SetOpen: is_open={}, mode={:?}", is_open, target_mode));

        Ok(())
    }
}