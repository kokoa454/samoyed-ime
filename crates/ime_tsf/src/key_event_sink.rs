use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use windows::core::{implement, BOOL, GUID, Ref, Result as WinResult};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetKeyboardLayout, GetKeyboardState, ToUnicodeEx,
    VK_BACK, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT, VK_LWIN, VK_MENU, VK_RETURN, VK_RIGHT, VK_RWIN, VK_SHIFT, VK_UP, VK_KANJI,
};
use windows::Win32::UI::TextServices::{ITfContext, ITfKeyEventSink, ITfKeyEventSink_Impl};

use crate::logging::log;
use crate::text_input_processor::SharedState;
use crate::edit_session::EditAction;


/// キーイベントを受け取るTSF Sink
#[implement(ITfKeyEventSink)]
pub struct KeyEventSink {
    state: Arc<Mutex<SharedState>>, // IMEの状態
    last_toggle: Mutex<Option<Instant>>, // 前回トグルした時刻
}

impl KeyEventSink {
    /// コンストラクタ
    pub fn new(state: Arc<Mutex<SharedState>>) -> Self {
        Self {
            state,
            last_toggle: Mutex::new(None)
        }
    }

    /// 直近のトグルから十分な時間が経っていれば true を返し、記録を更新する。
    /// 
    /// 戻り値
    /// * `true`: 十分な時間が経過している場合
    /// * `false`: 十分な時間が経過していない場合
    fn should_toggle_now(&self) -> bool {
        const DEBOUNCE: Duration = Duration::from_millis(150);
        let mut last = self.last_toggle.lock().unwrap();
        let now = Instant::now();

        match *last {
            Some(prev) if now.duration_since(prev) < DEBOUNCE => false,
            _ => {
                *last = Some(now);
                true
            }
        }
    }

    /// 現在のIME状態から、キーに対応するEditActionを取得する。
    ///
    /// 引数
    /// * `vk`: Virtual Key code
    /// * `lparam`: キーイベントのLPARAM
    ///
    /// 戻り値
    /// * `Some(EditAction)`: IMEが処理するキーの場合
    /// * `None`: IMEが処理しないキーの場合
    fn get_key_action(
        &self,
        vk: u16,
        lparam: LPARAM,
    ) -> Option<EditAction> {
        let state = self.state.lock().unwrap();

        // IMEがOFFの場合は、何もしない
        if !state.is_open {
            return None;
        }

        let ime_state = &state.ime_state;

        // 文字入力
        if let Some(ch) = vk_to_input_char(vk, lparam) {
            if ime_state.can_input_char(ch) {
                return Some(EditAction::InputChar(ch));
            }
        }

        // 文字入力以外でShiftを含む修飾キーはIMEで処理しない
        if has_blocking_modifier() {
            return None;
        }

        match vk {
            x if x == VK_BACK.0 => {
                if ime_state.has_active_input() {
                    Some(EditAction::Backspace)
                } else {
                    None
                }
            }

            x if x == VK_DELETE.0 => {
                if ime_state.has_composition()
                    && !ime_state.has_pending_input()
                    && ime_state.get_cursor_pos()
                        < ime_state.get_display_text().chars().count()
                {
                    Some(EditAction::Delete)
                } else {
                    None
                }
            }

            x if x == VK_LEFT.0 => {
                if ime_state.has_composition()
                    && !ime_state.has_pending_input()
                    && ime_state.get_cursor_pos() > 0
                {
                    Some(EditAction::MoveCursorLeft)
                } else {
                    None
                }
            }

            x if x == VK_RIGHT.0 => {
                if ime_state.has_composition()
                    && !ime_state.has_pending_input()
                    && ime_state.get_cursor_pos()
                        < ime_state.get_display_text().chars().count()
                {
                    Some(EditAction::MoveCursorRight)
                } else {
                    None
                }
            }

            x if x == VK_HOME.0 => {
                if ime_state.has_composition()
                    && !ime_state.has_pending_input()
                    && ime_state.get_cursor_pos() > 0
                {
                    Some(EditAction::MoveCursorToHead)
                } else {
                    None
                }
            }

            x if x == VK_END.0 => {
                if ime_state.has_composition()
                    && !ime_state.has_pending_input()
                    && ime_state.get_cursor_pos()
                        < ime_state.get_display_text().chars().count()
                {
                    Some(EditAction::MoveCursorToTail)
                } else {
                    None
                }
            }

            x if x == VK_RETURN.0 => {
                if ime_state.has_active_input() {
                    Some(EditAction::Commit)
                } else {
                    None
                }
            }

            x if x == VK_ESCAPE.0 => {
                if ime_state.has_active_input() {
                    Some(EditAction::Clear)
                } else {
                    None
                }
            }

            x if x == VK_UP.0 || x == VK_DOWN.0 => {
                None
            }

            _ => None,
        }
    }
}

/// ITfKeyEventSinkの実装
impl ITfKeyEventSink_Impl for KeyEventSink_Impl {
    /// キーが押されたときに呼ばれる。
    ///
    /// 引数
    /// * `pic`: Context
    /// * `wparam`: WPARAM
    /// * `_lparam`: LPARAM
    ///
    /// 戻り値
    /// * `Ok(BOOL(0))`: キーを処理しなかった場合
    /// * `Ok(BOOL(1))`: キーを処理した場合
    /// * `Err(E_UNEXPECTED)`: 予期しないエラーが発生した場合
    fn OnKeyDown(
        &self,
        pic: Ref<'_, ITfContext>,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> WinResult<BOOL> {
        let vk = wparam.0 as u16;

        log(&format!("[SamoyedIME] OnKeyDown: VK=0x{:X}", vk));

        if is_toggle_key(vk) {
            if self.should_toggle_now() {
                let (new_state, needs_commit) = {
                    let mut state = self.state.lock().unwrap();
                    state.is_open = !state.is_open;
                    let needs_commit = !state.is_open && state.ime_state.has_active_input();
                    (state.is_open, needs_commit)
                };

                if needs_commit {
                    if let Some(context) = pic.as_ref() {
                        let _ = SharedState::request_edit_session(
                            self.state.clone(),
                            context,
                            EditAction::Commit,
                        );
                    }
                }

                let _ = SharedState::set_keyboard_open(&self.state, new_state);
                log(&format!("[SamoyedIME] Toggled is_open={}", new_state));
            } else {
                log("[SamoyedIME] Toggle key ignored (debounced)");
            }

            let mut state = self.state.lock().unwrap();
            state.eaten_keys.insert(vk);

            return Ok(BOOL(1));
        }

        let Some(context) = pic.as_ref() else {
            return Ok(BOOL(0));
        };

        let Some(action) = self.get_key_action(vk, lparam) else {
            return Ok(BOOL(0));
        };

        SharedState::request_edit_session(
            self.state.clone(),
            context,
            action,
        )?;

        let mut state = self.state.lock().unwrap();
        state.eaten_keys.insert(vk);

        Ok(BOOL(1))
    }

    /// キーが離されたときに呼ばれる。
    ///
    /// 引数
    /// * `pic`: Context
    /// * `wparam`: WPARAM
    /// * `_lparam`: LPARAM
    ///
    /// 戻り値
    /// * `Ok(BOOL(0))`: キーを処理しなかった場合
    /// * `Ok(BOOL(1))`: キーを処理した場合
    /// * `Err(E_UNEXPECTED)`: 予期しないエラーが発生した場合
    fn OnKeyUp(
        &self,
        pic: Ref<'_, ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> WinResult<BOOL> {
        let vk = wparam.0 as u16;

        log(&format!("[SamoyedIME] OnKeyUp: VK=0x{:X}", vk));

        if is_toggle_key(vk) {
            let eaten = {
                let mut state = self.state.lock().unwrap();
                state.eaten_keys.remove(&vk)
            };

            if !eaten && self.should_toggle_now() {
                let (new_state, needs_commit) = {
                    let mut state = self.state.lock().unwrap();
                    state.is_open = !state.is_open;
                    let needs_commit = !state.is_open && state.ime_state.has_active_input();
                    (state.is_open, needs_commit)
                };

                if needs_commit {
                    if let Some(context) = pic.as_ref() {
                        let _ = SharedState::request_edit_session(
                            self.state.clone(),
                            context,
                            EditAction::Commit,
                        );
                    }
                }

                let _ = SharedState::set_keyboard_open(&self.state, new_state);
                log(&format!("[SamoyedIME] Toggled (via KeyUp fallback) is_open={}", new_state));
            }

            return Ok(BOOL(1));
        }

        let eaten = {
            let mut state = self.state.lock().unwrap();
            state.eaten_keys.remove(&vk)
        };

        if eaten {
            return Ok(BOOL(1));
        }

        Ok(BOOL(0))
    }

    /// キーがIMEによって処理されるかをテストする。
    ///
    /// 引数
    /// * `_pic`: Context
    /// * `wparam`: WPARAM
    /// * `lparam`: LPARAM
    ///
    /// 戻り値
    /// * `Ok(BOOL(0))`: キーを処理しなかった場合
    /// * `Ok(BOOL(1))`: キーを処理した場合
    /// * `Err(E_UNEXPECTED)`: 予期しないエラーが発生した場合
    fn OnTestKeyDown(
        &self,
        _pic: Ref<'_, ITfContext>,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> WinResult<BOOL> {
        let vk = wparam.0 as u16;

        log(&format!("[SamoyedIME] OnTestKeyDown: VK=0x{:X}", vk));

        if is_toggle_key(vk) {
            return Ok(BOOL(1));
        }

        if self.get_key_action(vk, lparam).is_some() {
            return Ok(BOOL(1));
        }

        Ok(BOOL(0))
    }

    /// KeyUpの処理対象かをテストする。
    ///
    /// 引数
    /// * `_pic`: Context
    /// * `wparam`: WPARAM
    /// * `_lparam`: LPARAM
    ///
    /// 戻り値
    /// * `Ok(BOOL(0))`: キーを処理しなかった場合
    /// * `Ok(BOOL(1))`: キーを処理した場合
    /// * `Err(E_UNEXPECTED)`: 予期しないエラーが発生した場合
    fn OnTestKeyUp(
        &self,
        _pic: Ref<'_, ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> WinResult<BOOL> {
        let vk = wparam.0 as u16;

        let state = self.state.lock().unwrap();

        if state.eaten_keys.contains(&vk) {
            return Ok(BOOL(1));
        }

        Ok(BOOL(0))
    }

    /// Preserved Keyが押されたときに呼ばれる。
    ///
    /// 引数
    /// * `_pic`: Context
    /// * `_rguid`: Preserved KeyのGUID
    ///
    /// 戻り値
    /// * `Ok(BOOL(0))`: キーを処理しなかった場合
    /// * `Ok(BOOL(1))`: キーを処理した場合
    /// * `Err(E_UNEXPECTED)`: 予期しないエラーが発生した場合
    fn OnPreservedKey(
        &self,
        _pic: Ref<'_, ITfContext>,
        _rguid: *const GUID,
    ) -> WinResult<BOOL> {
        Ok(BOOL(0))
    }

    /// フォーカスが変更されたときに呼ばれる。
    ///
    /// 引数
    /// * `_fforeground`: TRUEならフォアグラウンド、FALSEならバックグラウンド
    ///
    /// 戻り値
    /// * `Ok(())`: 成功した場合
    /// * `Err(E_UNEXPECTED)`: 失敗した場合
    fn OnSetFocus(&self, _fforeground: BOOL) -> WinResult<()> {
        Ok(())
    }
}

/// Virtual Key codeから実際の入力文字へ変換する。
///
/// 引数
/// * `vk`: Virtual Key code
/// * `lparam`: キーイベントのLPARAM
///
/// 戻り値
/// * `Some(char)`: 変換できた入力文字
/// * `None`: IMEで処理しないキーの場合
fn vk_to_input_char(
    vk: u16,
    lparam: LPARAM,
) -> Option<char> {
    // Ctrl / Alt / Windowsキーとの組み合わせはIMEで処理しない
    if has_non_shift_blocking_modifier() {
        return None;
    }

    let mut keyboard_state = [0u8; 256];

    unsafe {
        if GetKeyboardState(&mut keyboard_state).is_err() {
            return None;
        }

        let shift = GetKeyState(VK_SHIFT.0 as i32);
        let control = GetKeyState(VK_CONTROL.0 as i32);
        let alt = GetKeyState(VK_MENU.0 as i32);

        keyboard_state[VK_SHIFT.0 as usize] =
            (keyboard_state[VK_SHIFT.0 as usize] & 0x01)
                | if shift < 0 { 0x80 } else { 0 };

        keyboard_state[VK_CONTROL.0 as usize] =
            (keyboard_state[VK_CONTROL.0 as usize] & 0x01)
                | if control < 0 { 0x80 } else { 0 };

        keyboard_state[VK_MENU.0 as usize] =
            (keyboard_state[VK_MENU.0 as usize] & 0x01)
                | if alt < 0 { 0x80 } else { 0 };
    }

    // キーイベントのLPARAMからスキャンコードを取得
    let scan_code = ((lparam.0 >> 16) & 0xFF) as u32;

    // 現在のキーボードレイアウトを取得
    let keyboard_layout = unsafe {
        GetKeyboardLayout(0)
    };

    let mut buffer = [0u16; 8];

    let result = unsafe {
        ToUnicodeEx(
            vk as u32,
            scan_code,
            &keyboard_state,
            &mut buffer,
            0x0004,
            Some(keyboard_layout),
        )
    };

    if result <= 0 {
        return None;
    }

    let units = &buffer[..(result as usize).min(buffer.len())];

    let text = String::from_utf16_lossy(units);

    let mut chars = text.chars();

    let mut ch = chars.next()?;

    // 1回のキー入力で複数文字が返る場合は処理しない
    if chars.next().is_some() {
        return None;
    }

    // Shift + Aなどは通常のローマ字入力としてIMEに渡さない
    if ch.is_ascii_alphabetic() {
        if is_shift_down() {
            return None;
        }

        ch = ch.to_ascii_lowercase();
    }

    Some(ch)
}

/// Shift以外の修飾キーが押下されているかを取得する。
///
/// 戻り値
/// * `true`: 修飾キーが押下されている場合
/// * `false`: 修飾キーが押下されていない場合
fn has_non_shift_blocking_modifier() -> bool {
    unsafe {
        GetKeyState(VK_CONTROL.0 as i32) < 0
            || GetKeyState(VK_MENU.0 as i32) < 0
            || GetKeyState(VK_LWIN.0 as i32) < 0
            || GetKeyState(VK_RWIN.0 as i32) < 0
    }
}

/// Shiftキーが押下されているかを取得する。
///
/// 戻り値
/// * `true`: Shiftキーが押下されている場合
/// * `false`: Shiftキーが押下されていない場合
fn is_shift_down() -> bool {
    unsafe {
        GetKeyState(VK_SHIFT.0 as i32) < 0
    }
}

/// 修飾キーが押下されているかを取得する。
///
/// 戻り値
/// * `true`: 修飾キーが押下されている場合
/// * `false`: 修飾キーが押下されていない場合
fn has_blocking_modifier() -> bool {
    unsafe {
        GetKeyState(VK_CONTROL.0 as i32) < 0
            || GetKeyState(VK_MENU.0 as i32) < 0
            || GetKeyState(VK_LWIN.0 as i32) < 0
            || GetKeyState(VK_RWIN.0 as i32) < 0
            || GetKeyState(VK_SHIFT.0 as i32) < 0
    }
}

/// IMEの開閉を切り替えるキーかどうかを判定する。
/// 
/// # 引数
/// * `vk`: Virtual Key code
/// 
/// 戻り値
/// * `true`: IMEの開閉を切り替えるキーの場合
/// * `false`: IMEの開閉を切り替えないキーの場合
fn is_toggle_key(vk: u16) -> bool {
    vk == VK_KANJI.0 || vk == 0xF3 || vk == 0xF4
}