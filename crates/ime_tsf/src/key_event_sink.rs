use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use windows::core::{implement, BOOL, GUID, Ref, Result as WinResult};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetKeyboardLayout, GetKeyboardState, ToUnicodeEx,
    VK_BACK, VK_CAPITAL, VK_CONTROL, VK_DELETE, VK_END, VK_ESCAPE, VK_HOME,
    VK_KANA, VK_KANJI, VK_LEFT, VK_LWIN, VK_MENU, VK_NONCONVERT,
    VK_OEM_AUTO, VK_OEM_ENLW, VK_RETURN, VK_RIGHT, VK_RWIN, VK_SHIFT,
    VK_DBE_ALPHANUMERIC, VK_DBE_HIRAGANA, VK_DBE_KATAKANA,
};
use windows::Win32::UI::TextServices::{ITfContext, ITfKeyEventSink, ITfKeyEventSink_Impl};

use crate::edit_session::EditAction;
use crate::logging::log;
use crate::text_input_processor::SharedState;
use ime_core::{InputMode, ModeCommand};


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
            last_toggle: Mutex::new(None),
        }
    }

    /// 直近のトグルから十分な時間が経っていれば true を返し、記録を更新する。
    /// 
    /// # 戻り値
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
    /// # 引数
    /// * `vk`: Virtual Key code
    /// * `lparam`: キーイベントのLPARAM
    ///
    /// # 戻り値
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

        // モード遷移キーをチェック
        if let Some(action) = route_edit_key(vk, state.is_open, ime_state.has_active_input()) {
            return Some(action);
        }

        // 文字入力以外でShiftを含む修飾キーはIMEで処理しない
        if has_blocking_modifier() {
            return None;
        }

        // 文字入力
        if let Some(ch) = vk_to_input_char(vk, lparam) {
            if ime_state.can_input_char(ch) {
                return Some(EditAction::InputChar(ch));
            }
        }

        None
    }

    /// IMEの開閉状態を切り替える。
    ///
    /// # 引数
    /// * `pic`: Context
    /// * `target_mode`: 開く際に設定する入力モード
    fn toggle_open(&self, pic: Ref<'_, ITfContext>, target_mode: Option<InputMode>) {
        let target = {
            let state = self.state.lock().unwrap();
            !state.is_open
        };

        let Some(context) = pic.as_ref() else {
            // Contextが無い場合はTSFを操作できない
            log("[SamoyedIME] toggle_open: no context, state-only update");
            {
                let mut state = self.state.lock().unwrap();
                state.ime_state.clear();
                state.is_open = target;
                if target {
                    let mode = target_mode.unwrap_or(ime_core::InputMode::Hiragana);
                    state.ime_state.set_input_mode(mode);
                }
            }
            SharedState::notify_lang_bar_update(&self.state);
            return;
        };

        if let Err(e) = SharedState::request_edit_session(
            self.state.clone(),
            context,
            EditAction::SetOpen(target, target_mode),
        ) {
            // EditSessionを要求できなかった場合は状態を変えない
            log(&format!(
                "[SamoyedIME] toggle_open: RequestEditSession failed: {:?}",
                e
            ));
            return;
        }

        SharedState::notify_lang_bar_update(&self.state);
        log(&format!("[SamoyedIME] Toggled is_open={} mode={:?}", target, target_mode));
    }
}

/// ITfKeyEventSinkの実装
impl ITfKeyEventSink_Impl for KeyEventSink_Impl {
    /// キーが押されたときに呼ばれる。
    ///
    /// # 引数
    /// * `pic`: Context
    /// * `wparam`: WPARAM
    /// * `lparam`: LPARAM
    ///
    /// # 戻り値
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

        // モード切替キーとIME ONキーの処理
        {
            let shift = is_shift_down();
            let state = self.state.lock().unwrap();
            if state.is_open {
                if let Some(command) = route_mode_key(vk, true, shift) {
                    drop(state);
                    let Some(context) = pic.as_ref() else {
                        return Ok(BOOL(1));
                    };
                    if let Err(e) = SharedState::request_edit_session(
                        self.state.clone(),
                        context,
                        EditAction::ApplyModeCommand(command),
                    ) {
                        log(&format!("[SamoyedIME] RequestEditSession failed: {:?}", e));
                        return Ok(BOOL(1));
                    }
                    {
                        {
                            let mut state = self.state.lock().unwrap();
                            state.eaten_keys.insert(vk);
                            if vk == VK_DBE_ALPHANUMERIC.0 {
                                state.eaten_keys.insert(VK_DBE_HIRAGANA.0);
                            } else if vk == VK_DBE_HIRAGANA.0 {
                                state.eaten_keys.insert(VK_DBE_ALPHANUMERIC.0);
                            }
                        }

                        SharedState::notify_lang_bar_update(&self.state);
                        return Ok(BOOL(1));
                    }
                }
            } else if let Some(target_mode) = ime_on_target_mode(vk, shift) {
                drop(state);
                if self.should_toggle_now() {
                    self.toggle_open(pic, Some(target_mode));
                }

                {
                    let mut state = self.state.lock().unwrap();
                    state.eaten_keys.insert(vk);
                    if vk == VK_DBE_ALPHANUMERIC.0 {
                        state.eaten_keys.insert(VK_DBE_HIRAGANA.0);
                    } else if vk == VK_DBE_HIRAGANA.0 {
                        state.eaten_keys.insert(VK_DBE_ALPHANUMERIC.0);
                    }
                }
                return Ok(BOOL(1));
            }
        }

        if is_toggle_key(vk) {
            if self.should_toggle_now() {
                self.toggle_open(pic, None);
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

        if let Err(e) = SharedState::request_edit_session(
            self.state.clone(),
            context,
            action,
        ) {
            log(&format!("[SamoyedIME] RequestEditSession failed: {:?}", e));
            return Ok(BOOL(1));
        }

        let mut state = self.state.lock().unwrap();
        state.eaten_keys.insert(vk);

        Ok(BOOL(1))
    }

    /// キーが離されたときに呼ばれる。
    ///
    /// # 引数
    /// * `pic`: Context
    /// * `wparam`: WPARAM
    /// * `_lparam`: LPARAM
    ///
    /// # 戻り値
    /// * `Ok(BOOL(0))`: キーを処理しなかった場合
    /// * `Ok(BOOL(1))`: キーを処理した場合
    /// * `Err(E_UNEXPECTED)`: 予期しないエラーが発生した場合
    fn OnKeyUp(
        &self,
        _pic: Ref<'_, ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> WinResult<BOOL> {
        let vk = wparam.0 as u16;

        log(&format!("[SamoyedIME] OnKeyUp: VK=0x{:X}", vk));

        // キーが押されたときに処理したキーを削除
        let eaten = {
            let mut state = self.state.lock().unwrap();
            let removed_self = state.eaten_keys.remove(&vk);
            let removed_pair = if vk == VK_DBE_ALPHANUMERIC.0 {
                state.eaten_keys.remove(&VK_DBE_HIRAGANA.0)
            } else if vk == VK_DBE_HIRAGANA.0 {
                state.eaten_keys.remove(&VK_DBE_ALPHANUMERIC.0)
            } else {
                false
            };
            removed_self || removed_pair
        };

        if eaten {
            return Ok(BOOL(1));
        }

        let shift = is_shift_down();
        if is_toggle_key(vk) || is_ime_on_key(vk, shift) || route_mode_key(vk, true, shift).is_some() {
            return Ok(BOOL(1));
        }

        Ok(BOOL(0))
    }

    /// キーがIMEによって処理されるかをテストする。
    ///
    /// # 引数
    /// * `_pic`: Context
    /// * `wparam`: WPARAM
    /// * `lparam`: LPARAM
    ///
    /// # 戻り値
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

        {
            let shift = is_shift_down();
            let state = self.state.lock().unwrap();
            if state.is_open && route_mode_key(vk, true, shift).is_some() {
                return Ok(BOOL(1));
            } else if !state.is_open && is_ime_on_key(vk, shift) {
                return Ok(BOOL(1));
            }
        }

        if self.get_key_action(vk, lparam).is_some() {
            return Ok(BOOL(1));
        }

        Ok(BOOL(0))
    }

    /// KeyUpの処理対象かをテストする。
    ///
    /// # 引数
    /// * `_pic`: Context
    /// * `wparam`: WPARAM
    /// * `_lparam`: LPARAM
    ///
    /// # 戻り値
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

        if state.eaten_keys.contains(&vk)
            || (vk == VK_DBE_ALPHANUMERIC.0 && state.eaten_keys.contains(&VK_DBE_HIRAGANA.0))
            || (vk == VK_DBE_HIRAGANA.0 && state.eaten_keys.contains(&VK_DBE_ALPHANUMERIC.0))
        {
            return Ok(BOOL(1));
        }

        let shift = is_shift_down();
        if is_toggle_key(vk) || is_ime_on_key(vk, shift) || route_mode_key(vk, true, shift).is_some() {
            return Ok(BOOL(1));
        }

        Ok(BOOL(0))
    }

    /// Preserved Keyが押されたときに呼ばれる。
    ///
    /// # 引数
    /// * `_pic`: Context
    /// * `_rguid`: Preserved KeyのGUID
    ///
    /// # 戻り値
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
    /// # 引数
    /// * `_fforeground`: TRUEならフォアグラウンド、FALSEならバックグラウンド
    ///
    /// # 戻り値
    /// * `Ok(())`: 成功した場合
    /// * `Err(E_UNEXPECTED)`: 失敗した場合
    fn OnSetFocus(&self, _fforeground: BOOL) -> WinResult<()> {
        Ok(())
    }
}

/// Virtual Key codeから実際の入力文字へ変換する。
///
/// # 引数
/// * `vk`: Virtual Key code
/// * `lparam`: キーイベントのLPARAM
///
/// # 戻り値
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
/// # 戻り値
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
/// # 戻り値
/// * `true`: Shiftキーが押下されている場合
/// * `false`: Shiftキーが押下されていない場合
fn is_shift_down() -> bool {
    unsafe {
        GetKeyState(VK_SHIFT.0 as i32) < 0
    }
}

/// 修飾キーが押下されているかを取得する。
///
/// # 戻り値
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
/// # 戻り値
/// IMEの開閉を切り替えるキーかどうかを判定する。
///
/// # 引数
/// * `vk`: Virtual Key code
///
/// # 戻り値
/// * `true`: IMEの開閉を切り替えるキーの場合
/// * `false`: IMEの開閉を切り替えないキーの場合
fn is_toggle_key(vk: u16) -> bool {
    vk == VK_KANJI.0 || vk == VK_OEM_AUTO.0 || vk == VK_OEM_ENLW.0
}

/// キーをIMEが処理すべきかを判定する。
///
/// TSFやWindows APIに依存しない純粋関数にしておくことで、
/// キー割り当ての仕様を単体テストで固定できる。
///
/// # 引数
/// * `vk`: Virtual Key code
/// * `is_open`: IMEがONかどうか
/// * `has_active_input`: 未確定文字列または未変換ローマ字があるか
///
/// # 戻り値
/// * `Some(EditAction)`: IMEが処理するキーの場合
/// * `None`: アプリへ渡すキーの場合
pub(crate) fn route_edit_key(
    vk: u16,
    is_open: bool,
    has_active_input: bool,
) -> Option<EditAction> {
    // IMEがOFFのときは何もしない
    if !is_open {
        return None;
    }

    // 未確定文字列が無いときはアプリの操作を邪魔しない
    if !has_active_input {
        return None;
    }

    // 未確定文字列がある間は、編集キーをアプリへ渡してはならない。
    // アプリ側のテキストはCompositionそのものなので、
    // アプリに処理させると内部状態と表示が食い違う。
    // 実際に変化するかどうかは ime_core 側が no-op で吸収する。
    match vk {
        x if x == VK_BACK.0 => Some(EditAction::Backspace),
        x if x == VK_DELETE.0 => Some(EditAction::Delete),
        x if x == VK_LEFT.0 => Some(EditAction::MoveCursorLeft),
        x if x == VK_RIGHT.0 => Some(EditAction::MoveCursorRight),
        x if x == VK_HOME.0 => Some(EditAction::MoveCursorToHead),
        x if x == VK_END.0 => Some(EditAction::MoveCursorToTail),
        x if x == VK_RETURN.0 => Some(EditAction::Commit),
        x if x == VK_ESCAPE.0 => Some(EditAction::Clear),
        _ => None,
    }
}

/// VKコードをモード切替コマンドへ変換する。
///
/// # 引数
/// * `vk`: Virtual Key code
/// * `is_open`: IMEがONかどうか
/// * `is_shift`: Shiftキーが押下されているかどうか
///
/// # 戻り値
/// * `Some(ModeCommand)`: モード切替コマンド
/// * `None`: モード切替ではないキーの場合
pub(crate) fn route_mode_key(vk: u16, is_open: bool, is_shift: bool) -> Option<ModeCommand> {
    if !is_open {
        return None;
    }

    match vk {
        // ひらがな（かなキー）：どのモードからでもひらがなに戻る
        x if x == VK_KANA.0 || x == VK_DBE_HIRAGANA.0 => {
            Some(ModeCommand::SetInputMode(
                ime_core::InputMode::Hiragana,
            ))
        }

        // 全角カタカナ
        x if x == VK_DBE_KATAKANA.0 => {
            Some(ModeCommand::SetInputMode(
                ime_core::InputMode::FullWidthKatakana,
            ))
        }

        // 無変換：かな種別を巡回（ひらがな→全角カタカナ→半角カタカナ）
        x if x == VK_NONCONVERT.0 => {
            Some(ModeCommand::SwitchKanaType)
        }

        // 英数キー（よくCaps）：半角英数 ⇔ ひらがな の切り替え
        // Shiftキーが押されていない場合のみ英数キーとして処理する
        x if (x == VK_CAPITAL.0 && !is_shift) || x == VK_DBE_ALPHANUMERIC.0 => {
            Some(ModeCommand::ToggleAlphanumeric)
        }

        _ => None,
    }
}

/// IMEがOFFのときに、IMEをONにするキーと開く対象の入力モードを判定する。
///
/// # 引数
/// * `vk`: Virtual Key code
/// * `is_shift`: Shiftキーが押下されているかどうか
///
/// # 戻り値
/// * `Some(InputMode)`: IMEをONにし、設定すべき入力モード
/// * `None`: IMEをONにしないキーの場合
pub(crate) fn ime_on_target_mode(vk: u16, is_shift: bool) -> Option<ime_core::InputMode> {
    match vk {
        // 英数キー（Capsキー Shiftなし）：半角英数モードでIME ON
        x if (x == VK_CAPITAL.0 && !is_shift) || x == VK_DBE_ALPHANUMERIC.0 => {
            Some(ime_core::InputMode::HalfWidthAlphanumeric)
        }
        // 全角カタカナ
        x if x == VK_DBE_KATAKANA.0 => {
            Some(ime_core::InputMode::FullWidthKatakana)
        }
        // ひらがな
        x if x == VK_KANA.0 || x == VK_DBE_HIRAGANA.0 => {
            Some(ime_core::InputMode::Hiragana)
        }
        _ => None,
    }
}

/// IMEがOFFのときに、IMEをONにするキーかどうか。
///
/// # 引数
/// * `vk`: Virtual Key code
/// * `is_shift`: Shiftキーが押下されているかどうか
///
/// # 戻り値
/// * `true`: IMEをONにするキーの場合
/// * `false`: IMEをONにしないキーの場合
pub(crate) fn is_ime_on_key(vk: u16, is_shift: bool) -> bool {
    ime_on_target_mode(vk, is_shift).is_some()
}


/// テスト
#[cfg(test)]
mod tests {
    use ime_core::{ImeState, InputMode};
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_F1, VK_SPACE, VK_UP};

    use super::*;

    #[test]
    fn test_can_accept_input() {
        assert_eq!(route_edit_key(VK_DELETE.0, false, true), None);
        assert_eq!(route_edit_key(VK_RETURN.0, false, true), None);
    }

    #[test]
    fn test_no_pending_input() {
        assert_eq!(route_edit_key(VK_DELETE.0, true, false), None);
        assert_eq!(route_edit_key(VK_LEFT.0, true, false), None);
        assert_eq!(route_edit_key(VK_BACK.0, true, false), None);
    }

    #[test]
    fn test_delete_key_with_pending() {
        assert_eq!(
            route_edit_key(VK_DELETE.0, true, true),
            Some(EditAction::Delete)
        );
    }

    #[test]
    fn test_cursor_keys_with_pending() {
        assert_eq!(route_edit_key(VK_LEFT.0, true, true), Some(EditAction::MoveCursorLeft));
        assert_eq!(route_edit_key(VK_RIGHT.0, true, true), Some(EditAction::MoveCursorRight));
        assert_eq!(route_edit_key(VK_HOME.0, true, true), Some(EditAction::MoveCursorToHead));
        assert_eq!(route_edit_key(VK_END.0, true, true), Some(EditAction::MoveCursorToTail));
    }

    #[test]
    fn test_edit_keys_with_pending() {
        assert_eq!(route_edit_key(VK_BACK.0, true, true), Some(EditAction::Backspace));
        assert_eq!(route_edit_key(VK_RETURN.0, true, true), Some(EditAction::Commit));
        assert_eq!(route_edit_key(VK_ESCAPE.0, true, true), Some(EditAction::Clear));
    }

    #[test]
    fn test_edit_keys_when_closed() {
        assert_eq!(route_edit_key(VK_BACK.0, false, true), None);
        assert_eq!(route_edit_key(VK_RETURN.0, false, true), None);
        assert_eq!(route_edit_key(VK_ESCAPE.0, false, true), None);
    }

    #[test]
    fn test_other_keys_not_handled() {
        assert_eq!(route_edit_key(VK_UP.0, true, true), None);
        assert_eq!(route_edit_key(VK_F1.0, true, true), None);
    }

    #[test]
    fn test_is_toggle_key() {
        assert!(is_toggle_key(VK_KANJI.0));
        assert!(is_toggle_key(VK_OEM_AUTO.0));
        assert!(is_toggle_key(VK_OEM_ENLW.0));
        assert!(!is_toggle_key(VK_KANA.0));
        assert!(!is_toggle_key(VK_CAPITAL.0));
        assert!(!is_toggle_key(VK_DBE_ALPHANUMERIC.0));
        assert!(!is_toggle_key(VK_F1.0));
    }

    #[test]
    fn test_route_mode_key() {
        // かなキー：どのモードからでもひらがなに戻るコマンド
        assert_eq!(
            route_mode_key(VK_KANA.0, true, false),
            Some(ModeCommand::SetInputMode(InputMode::Hiragana))
        );
        assert_eq!(
            route_mode_key(VK_DBE_HIRAGANA.0, true, false),
            Some(ModeCommand::SetInputMode(InputMode::Hiragana))
        );
        assert_eq!(route_mode_key(VK_KANA.0, false, false), None);

        // 全角カタカナ
        assert_eq!(
            route_mode_key(VK_DBE_KATAKANA.0, true, false),
            Some(ModeCommand::SetInputMode(InputMode::FullWidthKatakana))
        );

        // 無変換キー：かな巡回
        assert_eq!(
            route_mode_key(VK_NONCONVERT.0, true, false),
            Some(ModeCommand::SwitchKanaType)
        );

        // 英数キー（Capsキー Shiftなし）：半角英数 ⇔ ひらがな切り替え
        assert_eq!(
            route_mode_key(VK_CAPITAL.0, true, false),
            Some(ModeCommand::ToggleAlphanumeric)
        );
        // Shift+CapsはCapsLockなのでIMEでフックしない
        assert_eq!(route_mode_key(VK_CAPITAL.0, true, true), None);

        // VK_DBE_ALPHANUMERIC
        assert_eq!(
            route_mode_key(VK_DBE_ALPHANUMERIC.0, true, false),
            Some(ModeCommand::ToggleAlphanumeric)
        );

        // IMEがOFFのときはモード切替キーにならない
        assert_eq!(route_mode_key(VK_CAPITAL.0, false, false), None);
        assert_eq!(route_mode_key(VK_DBE_ALPHANUMERIC.0, false, false), None);
        assert_eq!(route_mode_key(VK_DBE_HIRAGANA.0, false, false), None);
        assert_eq!(route_mode_key(VK_DBE_KATAKANA.0, false, false), None);
        assert_eq!(route_mode_key(VK_NONCONVERT.0, false, false), None);

        // その他
        assert_eq!(route_mode_key(VK_F1.0, true, false), None);
        assert_eq!(route_mode_key(VK_SPACE.0, true, false), None);
    }

    #[test]
    fn test_ime_on_target_mode() {
        // Capsキー（Shiftなし）または英数キーで半角英数モードでIME ON
        assert_eq!(
            ime_on_target_mode(VK_CAPITAL.0, false),
            Some(InputMode::HalfWidthAlphanumeric)
        );
        assert_eq!(
            ime_on_target_mode(VK_DBE_ALPHANUMERIC.0, false),
            Some(InputMode::HalfWidthAlphanumeric)
        );
        // Shift+Capsは除外（Windows標準のCapsLock動作）
        assert_eq!(ime_on_target_mode(VK_CAPITAL.0, true), None);

        // カタカナキーで全角カタカナモードでIME ON
        assert_eq!(
            ime_on_target_mode(VK_DBE_KATAKANA.0, false),
            Some(InputMode::FullWidthKatakana)
        );

        // かなキーでひらがなモードでIME ON
        assert_eq!(
            ime_on_target_mode(VK_KANA.0, false),
            Some(InputMode::Hiragana)
        );
        assert_eq!(
            ime_on_target_mode(VK_DBE_HIRAGANA.0, false),
            Some(InputMode::Hiragana)
        );

        // 無関係なキー
        assert_eq!(ime_on_target_mode(VK_F1.0, false), None);
        assert_eq!(ime_on_target_mode(VK_SPACE.0, false), None);
    }

    #[test]
    fn test_is_ime_on_key() {
        assert!(is_ime_on_key(VK_KANA.0, false));
        assert!(is_ime_on_key(VK_DBE_HIRAGANA.0, false));
        assert!(is_ime_on_key(VK_DBE_KATAKANA.0, false));
        assert!(is_ime_on_key(VK_DBE_ALPHANUMERIC.0, false));
        assert!(is_ime_on_key(VK_CAPITAL.0, false));
        assert!(!is_ime_on_key(VK_CAPITAL.0, true)); // Shift+Capsは除外
        assert!(!is_ime_on_key(VK_F1.0, false));
        assert!(!is_ime_on_key(VK_SPACE.0, false));
    }

    // ============================================================
    // route_mode_key の詳細テスト
    // ============================================================

    /// IME ON中にCapsキー(Shiftなし)でToggleAlphanumericが返る
    #[test]
    fn test_route_mode_key_caps_no_shift_returns_toggle() {
        let result = route_mode_key(VK_CAPITAL.0, true, false);
        assert_eq!(
            result,
            Some(ModeCommand::ToggleAlphanumeric),
            "IME ON + Caps(Shiftなし) → ToggleAlphanumeric"
        );
    }

    /// IME ON中にShift+CapsはNone (CapsLock切り替えをIMEが横取りしない)
    #[test]
    fn test_route_mode_key_caps_with_shift_returns_none() {
        let result = route_mode_key(VK_CAPITAL.0, true, true);
        assert_eq!(result, None, "Shift+Caps は CapsLock のため IME が処理しない");
    }

    /// IME OFF 中はどのモードキーもNone
    #[test]
    fn test_route_mode_key_all_return_none_when_ime_off() {
        let mode_keys = [
            VK_KANA.0,
            VK_DBE_HIRAGANA.0,
            VK_DBE_KATAKANA.0,
            VK_DBE_ALPHANUMERIC.0,
            VK_CAPITAL.0,
            VK_NONCONVERT.0,
        ];
        for &vk in &mode_keys {
            assert_eq!(
                route_mode_key(vk, false, false),
                None,
                "IME OFF 時は VK=0x{:X} がモード切替になってはいけない", vk
            );
        }
    }

    /// VK_DBE_ALPHANUMERIC でも ToggleAlphanumeric が返る
    #[test]
    fn test_route_mode_key_dbe_alphanumeric_is_toggle() {
        assert_eq!(
            route_mode_key(VK_DBE_ALPHANUMERIC.0, true, false),
            Some(ModeCommand::ToggleAlphanumeric),
            "VK_DBE_ALPHANUMERIC(英数キー) → ToggleAlphanumeric"
        );
        // Shiftあり・なしどちらでも効く(VK_DBE_ALPHANUMERIC はShift関係ない)
        assert_eq!(
            route_mode_key(VK_DBE_ALPHANUMERIC.0, true, true),
            Some(ModeCommand::ToggleAlphanumeric),
        );
    }

    /// かな系キーのルーティングを全パターン確認
    #[test]
    fn test_route_mode_key_kana_variants() {
        // VK_KANA と VK_DBE_HIRAGANA は両方ひらがな
        for &vk in &[VK_KANA.0, VK_DBE_HIRAGANA.0] {
            assert_eq!(
                route_mode_key(vk, true, false),
                Some(ModeCommand::SetInputMode(InputMode::Hiragana)),
                "VK=0x{:X} → Hiragana", vk
            );
        }

        // VK_DBE_KATAKANA は全角カタカナ
        assert_eq!(
            route_mode_key(VK_DBE_KATAKANA.0, true, false),
            Some(ModeCommand::SetInputMode(InputMode::FullWidthKatakana)),
        );

        // VK_NONCONVERT は SwitchKanaType
        assert_eq!(
            route_mode_key(VK_NONCONVERT.0, true, false),
            Some(ModeCommand::SwitchKanaType),
        );
    }

    // ============================================================
    // ime_on_target_mode の詳細テスト
    // ============================================================

    /// IME OFF→ON のターゲットモードがキーによって正しく決まる
    #[test]
    fn test_ime_on_target_mode_all_keys() {
        // Caps(Shiftなし) / 英数キー → 半角英数でON
        assert_eq!(
            ime_on_target_mode(VK_CAPITAL.0, false),
            Some(InputMode::HalfWidthAlphanumeric),
        );
        assert_eq!(
            ime_on_target_mode(VK_DBE_ALPHANUMERIC.0, false),
            Some(InputMode::HalfWidthAlphanumeric),
        );

        // かなキー → ひらがなでON
        assert_eq!(
            ime_on_target_mode(VK_KANA.0, false),
            Some(InputMode::Hiragana),
        );
        assert_eq!(
            ime_on_target_mode(VK_DBE_HIRAGANA.0, false),
            Some(InputMode::Hiragana),
        );

        // カタカナキー → 全角カタカナでON
        assert_eq!(
            ime_on_target_mode(VK_DBE_KATAKANA.0, false),
            Some(InputMode::FullWidthKatakana),
        );

        // Shift+Caps は CapsLock なので対象外
        assert_eq!(ime_on_target_mode(VK_CAPITAL.0, true), None);

        // 関係ないキー
        assert_eq!(ime_on_target_mode(VK_F1.0, false), None);
        assert_eq!(ime_on_target_mode(VK_SPACE.0, false), None);
    }

    // ============================================================
    // route_edit_key の詳細テスト
    // ============================================================

    /// 無変換キーはIME ONトリガーにならない
    #[test]
    fn test_ime_on_target_mode_nonconvert_is_not_ime_on_key() {
        assert_eq!(
            ime_on_target_mode(VK_NONCONVERT.0, false),
            None,
            "無変換キーはIME OFFからIME ONにするキーではない"
        );
        assert!(!is_ime_on_key(VK_NONCONVERT.0, false));
    }

    /// 無変換キーはIMEトグルキーでもない
    #[test]
    fn test_nonconvert_is_not_toggle_key() {
        assert!(
            !is_toggle_key(VK_NONCONVERT.0),
            "無変換キーはIME ON/OFFトグルキーではない"
        );
    }

    /// 未確定文字列がある場合のみ編集キーをIMEが処理する
    #[test]
    fn test_route_edit_key_only_when_has_active_input() {
        let edit_keys = [
            (VK_BACK.0, Some(EditAction::Backspace)),
            (VK_DELETE.0, Some(EditAction::Delete)),
            (VK_LEFT.0, Some(EditAction::MoveCursorLeft)),
            (VK_RIGHT.0, Some(EditAction::MoveCursorRight)),
            (VK_HOME.0, Some(EditAction::MoveCursorToHead)),
            (VK_END.0, Some(EditAction::MoveCursorToTail)),
            (VK_RETURN.0, Some(EditAction::Commit)),
            (VK_ESCAPE.0, Some(EditAction::Clear)),
        ];

        for (vk, expected) in &edit_keys {
            // IME ON + 未確定あり → IMEが処理
            assert_eq!(
                route_edit_key(*vk, true, true),
                *expected,
                "VK=0x{:X}: IME ON+active_input時はIMEが処理すべき", vk
            );

            // IME ON + 未確定なし → アプリへ渡す
            assert_eq!(
                route_edit_key(*vk, true, false),
                None,
                "VK=0x{:X}: 未確定なし時はアプリへ渡すべき", vk
            );

            // IME OFF → アプリへ渡す
            assert_eq!(
                route_edit_key(*vk, false, true),
                None,
                "VK=0x{:X}: IME OFF時はアプリへ渡すべき", vk
            );
        }
    }

    // ============================================================
    // エンドツーエンド: キールーティング → モード遷移の統合テスト
    // ============================================================

    /// ひらがなモードでCapsキーを押すと半角英数モードになる
    #[test]
    fn test_e2e_caps_from_hiragana_switches_to_half_width_alphanumeric() {
        let mut state = ImeState::new();
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);

        let command = route_mode_key(VK_CAPITAL.0, true, false)
            .expect("Caps(Shiftなし)はモード切替キーであるべき");
        state.apply_mode_command(command);

        assert_eq!(state.get_input_mode(), InputMode::HalfWidthAlphanumeric,
            "ひらがな→Caps→半角英数になるべき");
    }

    /// 半角英数モードでCapsキーを押すとひらがなモードに戻る
    #[test]
    fn test_e2e_caps_from_half_width_alphanumeric_switches_to_hiragana() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::HalfWidthAlphanumeric);

        let command = route_mode_key(VK_CAPITAL.0, true, false)
            .expect("Caps(Shiftなし)はモード切替キーであるべき");
        state.apply_mode_command(command);

        assert_eq!(state.get_input_mode(), InputMode::Hiragana,
            "半角英数→Caps→ひらがなに戻るべき");
    }

    /// 英数キー（VK_DBE_ALPHANUMERIC）でも同じくひらがな↔半角英数がトグルする
    #[test]
    fn test_e2e_dbe_alphanumeric_toggles_hiragana_and_half_width() {
        let mut state = ImeState::new();

        // ひらがな → 半角英数
        let cmd = route_mode_key(VK_DBE_ALPHANUMERIC.0, true, false).unwrap();
        state.apply_mode_command(cmd);
        assert_eq!(state.get_input_mode(), InputMode::HalfWidthAlphanumeric);

        // 半角英数 → ひらがな
        let cmd = route_mode_key(VK_DBE_ALPHANUMERIC.0, true, false).unwrap();
        state.apply_mode_command(cmd);
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);
    }

    /// カタカナモードからCapsキーを押すとひらがなに戻る（半角英数ではない）
    #[test]
    fn test_e2e_caps_from_katakana_returns_to_hiragana() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::FullWidthKatakana);

        let cmd = route_mode_key(VK_CAPITAL.0, true, false).unwrap();
        state.apply_mode_command(cmd);

        assert_eq!(state.get_input_mode(), InputMode::Hiragana,
            "全角カタカナ→Caps→ひらがなに戻るべき");
    }

    /// 全角英数モードからCapsキーを押すとひらがなに戻る
    #[test]
    fn test_e2e_caps_from_full_width_alphanumeric_returns_to_hiragana() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::FullWidthAlphanumeric);

        let cmd = route_mode_key(VK_CAPITAL.0, true, false).unwrap();
        state.apply_mode_command(cmd);

        assert_eq!(state.get_input_mode(), InputMode::Hiragana,
            "全角英数→Caps→ひらがなに戻るべき");
    }

    /// 半角カタカナモードからCapsキーを押すとひらがなに戻る
    #[test]
    fn test_e2e_caps_from_half_width_katakana_returns_to_hiragana() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::HalfWidthKatakana);

        let cmd = route_mode_key(VK_CAPITAL.0, true, false).unwrap();
        state.apply_mode_command(cmd);

        assert_eq!(state.get_input_mode(), InputMode::Hiragana,
            "半角カタカナ→Caps→ひらがなに戻るべき");
    }

    /// 半角英数モードでaaaaと入力した状態でCapsキーを押すと「ああああ」になり、再度押すと「aaaa」に戻る
    #[test]
    fn test_e2e_caps_toggles_composition_aaaa_and_hiragana() {
        let mut state = ImeState::new();
        state.set_input_mode(InputMode::HalfWidthAlphanumeric);

        for ch in "aaaa".chars() {
            state.input_char(ch);
        }
        assert_eq!(state.get_display_text(), "aaaa");

        // Capsキー押下
        let cmd = route_mode_key(VK_CAPITAL.0, true, false).unwrap();
        state.apply_mode_command(cmd);
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);
        assert_eq!(state.get_display_text(), "ああああ");

        // 再度Capsキー押下
        let cmd = route_mode_key(VK_CAPITAL.0, true, false).unwrap();
        state.apply_mode_command(cmd);
        assert_eq!(state.get_input_mode(), InputMode::HalfWidthAlphanumeric);
        assert_eq!(state.get_display_text(), "aaaa");
    }

    /// ひらがなモードでああああと入力した状態でVK_DBE_ALPHANUMERICを押すと「aaaa」になり、再度押すと「ああああ」に戻る
    #[test]
    fn test_e2e_dbe_alphanumeric_toggles_composition_hiragana_and_aaaa() {
        let mut state = ImeState::new();
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);

        for ch in "aaaa".chars() {
            state.input_char(ch);
        }
        assert_eq!(state.get_display_text(), "ああああ");

        // VK_DBE_ALPHANUMERIC押下
        let cmd = route_mode_key(VK_DBE_ALPHANUMERIC.0, true, false).unwrap();
        state.apply_mode_command(cmd);
        assert_eq!(state.get_input_mode(), InputMode::HalfWidthAlphanumeric);
        assert_eq!(state.get_display_text(), "aaaa");

        // 再度VK_DBE_ALPHANUMERIC押下
        let cmd = route_mode_key(VK_DBE_ALPHANUMERIC.0, true, false).unwrap();
        state.apply_mode_command(cmd);
        assert_eq!(state.get_input_mode(), InputMode::Hiragana);
        assert_eq!(state.get_display_text(), "ああああ");
    }
}