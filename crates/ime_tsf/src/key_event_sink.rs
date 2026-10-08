use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use windows::core::{implement, BOOL, GUID, Ref, Result as WinResult};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetKeyboardLayout, GetKeyboardState, ToUnicodeEx,
    VK_BACK, VK_CONTROL, VK_DELETE, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT,
    VK_LWIN, VK_MENU, VK_RETURN, VK_RIGHT, VK_RWIN, VK_SHIFT, VK_KANJI,
    VK_KANA, VK_NONCONVERT,
    VK_DBE_ALPHANUMERIC, VK_DBE_HIRAGANA, VK_DBE_KATAKANA,
};

use ime_core::ModeCommand;
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
    fn toggle_open(&self, pic: Ref<'_, ITfContext>) {
        let target = {
            let state = self.state.lock().unwrap();
            !state.is_open
        };

        let Some(context) = pic.as_ref() else {
            // Contextが無い場合はTSFを操作できない
            log("[SamoyedIME] toggle_open: no context, state-only update");
            let mut state = self.state.lock().unwrap();
            state.ime_state.clear();
            state.is_open = target;
            return;
        };

        if let Err(e) = SharedState::request_edit_session(
            self.state.clone(),
            context,
            EditAction::SetOpen(target),
        ) {
            // EditSessionを要求できなかった場合は状態を変えない
            log(&format!(
                "[SamoyedIME] toggle_open: RequestEditSession failed: {:?}",
                e
            ));
            return;
        }

        log(&format!("[SamoyedIME] Toggled is_open={}", target));
    }

    /// LangBarItemButtonの表示を更新する。
    fn update_language_bar_item(&self) {
        // TODO: ITfLangBarItemButton 実装後に有効化
        // 現在の input_mode を読んで、入力インジケーターを更新する
    }
}

/// ITfKeyEventSinkの実装
impl ITfKeyEventSink_Impl for KeyEventSink_Impl {
    /// キーが押されたときに呼ばれる。
    ///
    /// 引数
    /// * `pic`: Context
    /// * `wparam`: WPARAM
    /// * `lparam`: LPARAM
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

        // モード切替キーとIME ONキーの処理
        {
            let state = self.state.lock().unwrap();
            if state.is_open {
                if let Some(command) = route_mode_key(vk, true) {
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
                    let mut state = self.state.lock().unwrap();
                    state.eaten_keys.insert(vk);
                    return Ok(BOOL(1));
                }
            } else if is_ime_on_key(vk) {
                drop(state);
                if self.should_toggle_now() {
                    self.toggle_open(pic);
                }
                return Ok(BOOL(1));
            }
        }

        if is_toggle_key(vk) {
            if self.should_toggle_now() {
                self.toggle_open(pic);
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

        // キーが押されたときに処理したキーを削除
        let eaten = {
            let mut state = self.state.lock().unwrap();
            state.eaten_keys.remove(&vk)
        };

        if eaten {
            return Ok(BOOL(1));
        }

        // IMEがOFFのときのひらがな／カタカナ／英数キーはIMEをON
        if is_ime_on_key(vk) {
            if self.should_toggle_now() {
                self.toggle_open(pic);
            }
            return Ok(BOOL(1));
        }

        // モード切替キー
        let mode_command = {
            let state = self.state.lock().unwrap();
            route_mode_key(vk, state.is_open)
        };

        if let Some(command) = mode_command {
            let mut state = self.state.lock().unwrap();
            state.ime_state.apply_mode_command(command);
            drop(state);
            self.update_language_bar_item();
            return Ok(BOOL(1));
        }

        if is_toggle_key(vk) {
            let eaten = {
                let mut state = self.state.lock().unwrap();
                state.eaten_keys.remove(&vk)
            };
            if !eaten && self.should_toggle_now() {
                self.toggle_open(pic);
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

        {
            let state = self.state.lock().unwrap();
            if state.is_open && route_mode_key(vk, true).is_some() {
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

        if state.is_open {
            if route_mode_key(vk, true).is_some() {
                return Ok(BOOL(1));
            }
        } else if is_ime_on_key(vk) {
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
/// 引数
/// * `vk`: Virtual Key code
/// * `is_open`: IMEがONかどうか
///
/// 戻り値
/// * `Some(ModeCommand)`: モード切替コマンド
/// * `None`: モード切替ではないキーの場合
pub(crate) fn route_mode_key(vk: u16, is_open: bool) -> Option<ModeCommand> {
    if !is_open {
        return None;
    }

    match vk {
        x if x == VK_KANA.0 || x == VK_DBE_HIRAGANA.0 => Some(ModeCommand::Hiragana),
        x if x == VK_DBE_KATAKANA.0 => Some(ModeCommand::FullWidthKatakana),
        x if x == VK_NONCONVERT.0 => Some(ModeCommand::SwitchKanaType),
        x if x == VK_DBE_ALPHANUMERIC.0 => Some(ModeCommand::ToggleAlphanumeric),
        _ => None,
    }
}

/// IMEがOFFのときに、IMEをONにするキーかどうか。
///
/// 引数
/// * `vk`: Virtual Key code
///
/// 戻り値
/// * `true`: IMEをONにするキーの場合
/// * `false`: IMEをONにしないキーの場合
pub(crate) fn is_ime_on_key(vk: u16) -> bool {
    vk == VK_KANA.0
        || vk == VK_DBE_HIRAGANA.0
        || vk == VK_DBE_KATAKANA.0
        || vk == VK_DBE_ALPHANUMERIC.0
}


/// テスト
#[cfg(test)]
mod tests {
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_F1, VK_UP};

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
    fn test_other_keys_not_handled() {
        assert_eq!(route_edit_key(VK_UP.0, true, true), None);
        assert_eq!(route_edit_key(VK_F1.0, true, true), None);
    }

    #[test]
    fn test_route_mode_key() {
        assert_eq!(route_mode_key(VK_KANA.0, true), Some(ModeCommand::Hiragana));
        assert_eq!(route_mode_key(VK_KANA.0, false), None);
        assert_eq!(
            route_mode_key(VK_NONCONVERT.0, true),
            Some(ModeCommand::SwitchKanaType)
        );
        assert_eq!(
            route_mode_key(VK_DBE_ALPHANUMERIC.0, true),
            Some(ModeCommand::ToggleAlphanumeric)
        );
        assert_eq!(route_mode_key(VK_F1.0, true), None);
    }

    #[test]
    fn test_is_ime_on_key() {
        assert!(is_ime_on_key(VK_KANA.0));
        assert!(is_ime_on_key(VK_DBE_ALPHANUMERIC.0));
        assert!(!is_ime_on_key(VK_F1.0));
    }
}