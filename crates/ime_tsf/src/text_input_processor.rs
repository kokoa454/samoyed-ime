use std::collections::HashSet;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Mutex};

use ime_core::ImeState;
use windows::core::{implement, Interface, Ref, Result as WinResult, GUID};
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::TextServices::{
    GUID_COMPARTMENT_KEYBOARD_OPENCLOSE, ITfCompartmentMgr, ITfComposition, ITfContext,
    ITfDisplayAttributeInfo, ITfDisplayAttributeProvider, ITfDisplayAttributeProvider_Impl,
    ITfEditSession, ITfKeyEventSink, ITfKeystrokeMgr, ITfLangBarItem, ITfLangBarItemMgr,
    ITfLangBarItemSink, ITfTextInputProcessor, ITfTextInputProcessor_Impl,
    ITfThreadMgr, TF_ES_ASYNCDONTCARE, TF_ES_READWRITE, IEnumTfDisplayAttributeInfo,
};

use crate::display_attribute::{DisplayAttributeProvider, EnumDisplayAttributeInfo};
use crate::edit_session::{EditAction, SamoyedIMEEditSession};
use crate::key_event_sink::KeyEventSink;
use crate::lang_bar::SamoyedLangBarItem;
use crate::logging::log;
use crate::LIVE_OBJECT_COUNT;


/// TSF Text Service本体
#[implement(ITfTextInputProcessor, ITfDisplayAttributeProvider)]
pub struct TextService {
    state: Arc<Mutex<SharedState>>, // TextServiceが共有する状態
    lang_bar_item: Mutex<Option<ITfLangBarItem>>, // LangBarItemへのポインタ
}

impl TextService {
    /// コンストラクタ
    pub fn new() -> Self {
        LIVE_OBJECT_COUNT.fetch_add(1, SeqCst);

        log("[SamoyedIME] TextService::Create");

        Self {
            state: Arc::new(Mutex::new(SharedState {
                client_id: None,
                thread_mgr: None,
                keystroke_mgr: None,
                ime_state: ImeState::new(),
                composition: None,
                eaten_keys: HashSet::new(),
                is_open: true,
                lang_bar_item_sink: None,
            })),
            lang_bar_item: Mutex::new(None),
        }
    }
}

impl Drop for TextService {
    /// TextServiceのDrop実装
    ///
    /// # 戻り値
    /// * `Ok(())`: 成功した場合
    /// * `Err(E_UNEXPECTED)`: 失敗した場合
    fn drop(&mut self) {
        LIVE_OBJECT_COUNT.fetch_sub(1, SeqCst);

        log("[SamoyedIME] TextService::Drop");
    }
}

impl ITfTextInputProcessor_Impl for TextService_Impl {
    /// Text Serviceを有効化する。
    ///
    /// # 引数
    /// * `ptim`: Thread Manager*
    /// * `tid`: Thread ID*
    ///
    /// # 戻り値
    /// * `Ok(())`: 成功した場合
    /// * `Err(E_INVALIDARG)`: 引数が無効な場合
    /// * `Err(E_UNEXPECTED)`: 予期しないエラーが発生した場合
    fn Activate(
        &self,
        ptim: Ref<'_, ITfThreadMgr>,
        tid: u32,
    ) -> WinResult<()> {
        log("[SamoyedIME] TextService::Activate");

        // Thread Managerの取得
        let thread_mgr = ptim.ok()?;

        // ITfThreadMgrからITfKeystrokeMgrを取得
        let keystroke_mgr: ITfKeystrokeMgr = thread_mgr.cast()?;

        // TextServiceの状態を更新
        {
            let mut state = self.state.lock().unwrap();

            state.client_id = Some(tid);
            state.thread_mgr = Some(thread_mgr.clone());
            state.keystroke_mgr = Some(keystroke_mgr.clone());
            state.ime_state.clear();
            state.composition = None;
            state.eaten_keys.clear();
        }

        // 初期状態をコンパートメントに反映する(デフォルトはON)
        SharedState::set_keyboard_open(&self.state, true)?;

        // KeyEventSinkを生成
        let sink: ITfKeyEventSink = KeyEventSink::new(self.state.clone()).into();

        // KeyEventSinkをTSFへ登録
        unsafe { keystroke_mgr.AdviseKeyEventSink(tid, &sink, true)?; }

        // LangBarItemを登録する
        let samoyed_lang_bar = SamoyedLangBarItem::new(self.state.clone());
        let lang_bar_item_unk: ITfLangBarItem = samoyed_lang_bar.into();

        if let Ok(lang_bar_item_mgr) = thread_mgr.cast::<ITfLangBarItemMgr>() {
            unsafe {
                if let Err(e) = lang_bar_item_mgr.AddItem(&lang_bar_item_unk) {
                    log(&format!("[SamoyedIME] AddItem failed: {:?}", e));
                } else {
                    *self.lang_bar_item.lock().unwrap() = Some(lang_bar_item_unk);
                    log("[SamoyedIME] SamoyedLangBarItem registered successfully");
                }
            }
        }

        log("[SamoyedIME] KeyEventSink::Advised");

        Ok(())
    }

    /// Text Serviceを無効化する。
    ///
    /// # 戻り値
    /// * `Ok(())`: 成功した場合
    /// * `Err(E_UNEXPECTED)`: 失敗した場合
    fn Deactivate(&self) -> WinResult<()> {
        log("[SamoyedIME] TextService::Deactivate");

        // LangBarItemをTSFから削除
        let thread_mgr = {
            let state = self.state.lock().unwrap();
            state.thread_mgr.clone()
        };

        if let Some(lang_bar_item) = self.lang_bar_item.lock().unwrap().take() {
            if let Some(thread_mgr) = &thread_mgr {
                if let Ok(lang_bar_item_mgr) = thread_mgr.cast::<ITfLangBarItemMgr>() {
                    let _ = unsafe { lang_bar_item_mgr.RemoveItem(&lang_bar_item) };
                }
            }
        }

        // 状態をクリア
        let (client_id, keystroke_mgr) = {
            let mut state = self.state.lock().unwrap();

            let client_id = state.client_id.take();
            let keystroke_mgr = state.keystroke_mgr.take();

            state.thread_mgr = None;
            state.ime_state.clear();
            state.composition = None;
            state.eaten_keys.clear();

            (client_id, keystroke_mgr)
        };

        // KeyEventSinkの登録を解除
        if let (Some(client_id), Some(keystroke_mgr)) = (client_id, keystroke_mgr) {
            unsafe {
                keystroke_mgr.UnadviseKeyEventSink(client_id)?;
            }

            log("[SamoyedIME] KeyEventSink::Unadvised");
        }

        Ok(())
    }
}

impl ITfDisplayAttributeProvider_Impl for TextService_Impl {
    /// EnumDisplayAttributeInfoを取得する。
    ///
    /// # 戻り値
    /// * `Ok(IEnumTfDisplayAttributeInfo)`: EnumDisplayAttributeInfo
    /// * `Err(E_FAIL)`: 失敗した場合
    fn EnumDisplayAttributeInfo(&self) -> WinResult<IEnumTfDisplayAttributeInfo> {
        Ok(EnumDisplayAttributeInfo::new().into())
    }

    /// DisplayAttributeInfoを取得する。
    ///
    /// # 引数
    /// * `guid`: GUID
    ///
    /// # 戻り値
    /// * `Ok(ITfDisplayAttributeInfo)`: DisplayAttribute
    /// * `Err(E_FAIL)`: 失敗した場合
    fn GetDisplayAttributeInfo(&self, guid: *const GUID) -> WinResult<ITfDisplayAttributeInfo> {
        DisplayAttributeProvider::get_info(guid)
    }
}

/// TextServiceが共有する状態
pub struct SharedState {
    pub client_id: Option<u32>, // TSFシステムが管理するクライアントID
    pub thread_mgr: Option<ITfThreadMgr>, // TSF Thread Manager
    pub keystroke_mgr: Option<ITfKeystrokeMgr>, // KeystrokeMgrへのポインタ
    pub ime_state: ImeState, // IMEの状態
    pub composition: Option<ITfComposition>, // 変換中のコンポジション
    pub eaten_keys: HashSet<u16>, // 入力済みのキー
    pub is_open: bool, // キーボードが開いているかどうか
    pub lang_bar_item_sink: Option<ITfLangBarItemSink>, // LangBarItem
}

impl SharedState {
    /// TSFのキーボード開閉状態を設定し、ローカル状態も同時に更新する。
    ///
    /// コンパートメントの更新に失敗した場合はローカル状態も変更しない。
    /// 両者が食い違うと、Windowsの表示と実際の入力動作が一致しなくなる。
    ///
    /// # 引数
    /// * `state`: TextServiceの共有状態
    /// * `is_open`: 新しいキーボード開閉状態
    ///
    /// # 戻り値
    /// * `Ok(())`: 成功した場合
    /// * `Err(E_FAIL)`: Client IDまたはThread Managerが未設定の場合
    pub(crate) fn set_keyboard_open(
        state: &Arc<Mutex<Self>>,
        is_open: bool,
    ) -> WinResult<()> {
        let (thread_mgr, client_id) = {
            let state = state.lock().unwrap();

            let Some(thread_mgr) = state.thread_mgr.clone() else {
                return Err(E_FAIL.into());
            };

            let Some(client_id) = state.client_id else {
                return Err(E_FAIL.into());
            };

            (thread_mgr, client_id)
        };

        let compartment_mgr: ITfCompartmentMgr = thread_mgr.cast()?;

        let compartment = unsafe {
            compartment_mgr.GetCompartment(
                &GUID_COMPARTMENT_KEYBOARD_OPENCLOSE,
            )?
        };

        let value = VARIANT::from(if is_open { 1i32 } else { 0i32 });

        unsafe {
            compartment.SetValue(client_id, &value)?;
        }

        // TSF側の更新が成功したときだけローカル状態を合わせる
        let mut state = state.lock().unwrap();
        state.is_open = is_open;

        Ok(())
    }

    /// EditSessionを要求する。
    ///
    /// # 引数
    /// * `state`: TextServiceの共有状態
    /// * `context`: Context
    /// * `action`: EditSessionで実行する操作
    ///
    /// # 戻り値
    /// * `Ok(())`: EditSessionの要求に成功した場合
    /// * `Err(E_FAIL)`: Client IDが設定されていない場合
    pub(crate) fn request_edit_session(
        state: Arc<Mutex<Self>>,
        context: &ITfContext,
        action: EditAction,
    ) -> WinResult<()> {
        let client_id = {
            let state = state.lock().unwrap();

            let Some(client_id) = state.client_id else {
                log("[SamoyedIME] request_edit_session::Unavailable client_id");
                return Err(E_FAIL.into());
            };

            client_id
        };

        let edit_session: ITfEditSession =
            SamoyedIMEEditSession::new(
                context.clone(),
                state,
                action,
            ).into();

        let hr_session = unsafe {
            context.RequestEditSession(
                client_id,
                &edit_session,
                TF_ES_ASYNCDONTCARE | TF_ES_READWRITE,
            )?
        };

        log(&format!(
            "[SamoyedIME] RequestEditSession: HRESULT=0x{:08X}",
            hr_session.0 as u32
        ));

        hr_session.ok()?;

        Ok(())
    }

    /// 言語バーの表示更新通知をOS（TSF）へ送る。
    ///
    /// # 引数
    /// * `state`: TextServiceの共有状態
    pub(crate) fn notify_lang_bar_update(state: &Arc<Mutex<Self>>) {
        let state = state.lock().unwrap();
        if let Some(sink) = state.lang_bar_item_sink.as_ref() {
            const TF_LBI_ICON: u32 = 2;
            const TF_LBI_TEXT: u32 = 4;
            const TF_LBI_TOOLTIP: u32 = 8;
            const TF_LBI_STATUS: u32 = 1;

            if let Err(e) = unsafe {
                sink.OnUpdate(TF_LBI_STATUS | TF_LBI_ICON | TF_LBI_TEXT | TF_LBI_TOOLTIP)
            } {
                log(&format!("[SamoyedIME] OnUpdate failed: {:?}", e));
            }
        }
    }
}