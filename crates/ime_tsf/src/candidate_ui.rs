use std::sync::{Arc, Mutex};

use windows::core::{implement, BOOL, BSTR, GUID, Result as WinResult};
use windows::Win32::Foundation::E_INVALIDARG;
use windows::Win32::UI::TextServices::{
    ITfCandidateListUIElement, ITfCandidateListUIElement_Impl, ITfDocumentMgr,
    ITfCandidateListUIElementBehavior, ITfCandidateListUIElementBehavior_Impl,
    ITfUIElement_Impl, TF_CLUIE_COUNT, TF_CLUIE_DOCUMENTMGR, TF_CLUIE_SELECTION,
    TF_CLUIE_STRING,
};

use crate::logging::log;


pub const GUID_SAMOYED_CANDIDATE_UI: GUID = GUID::from_u128(0x4d3c4d89_6d4f_4dc1_8e1a_4b6a0d9e12a1); // 候補ウィンドウのGUID


/// 候補ウィンドウの状態
#[derive(Debug)]
pub struct CandidateUiState {
    pub document_mgr: ITfDocumentMgr, // ドキュメントマネージャ
    pub candidates: Vec<String>, // 候補文字列のリスト
    pub selection: u32, // 選択中の候補インデックス
    pub shown: bool, // 候補ウィンドウが表示されているかどうか
    pub updated_flags: u32, // 更新されたフラグ
}

/// 候補ウィンドウのUI要素
#[implement(ITfCandidateListUIElement, ITfCandidateListUIElementBehavior)]
pub struct CandidateListUi {
    pub state: Arc<Mutex<CandidateUiState>>, // 候補ウィンドウの状態
}

impl CandidateListUi {
    /// コンストラクタ。
    pub fn new(
        document_mgr: ITfDocumentMgr,
        candidates: Vec<String>,
        selection: usize,
    ) -> (ITfCandidateListUIElement, Arc<Mutex<CandidateUiState>>) {
        let state = Arc::new(Mutex::new(CandidateUiState {
            document_mgr,
            candidates,
            selection: selection as u32,
            shown: false,
            updated_flags: TF_CLUIE_COUNT
                | TF_CLUIE_DOCUMENTMGR
                | TF_CLUIE_SELECTION
                | TF_CLUIE_STRING,
        }));
        let element = Self {
            state: state.clone(),
        };
        (element.into(), state)
    }

    /// 候補ウィンドウの状態を更新する。
    /// 
    /// 引数:
    /// * `state`: 候補ウィンドウの状態
    /// * `candidates`: 更新する候補文字列のリスト
    /// * `selection`: 更新する選択中の候補インデックス
    pub fn update(state: &Arc<Mutex<CandidateUiState>>, candidates: Vec<String>, selection: usize) {
        let mut state = state.lock().unwrap();
        state.candidates = candidates;
        state.selection = selection as u32;
        state.updated_flags = TF_CLUIE_COUNT
            | TF_CLUIE_DOCUMENTMGR
            | TF_CLUIE_SELECTION
            | TF_CLUIE_STRING;
    }
}

impl ITfUIElement_Impl for CandidateListUi_Impl {
    /// 候補ウィンドウの説明を取得する。
    /// 
    /// 戻り値:
    /// * `WinResult<BSTR>`: 候補ウィンドウの説明文字列
    fn GetDescription(&self) -> WinResult<BSTR> {
        Ok(BSTR::from("Samoyed IME candidates"))
    }

    /// 候補ウィンドウのGUIDを取得する。
    /// 
    /// 戻り値:
    /// * `WinResult<GUID>`: 候補ウィンドウのGUID
    fn GetGUID(&self) -> WinResult<GUID> {
        Ok(GUID_SAMOYED_CANDIDATE_UI)
    }

    /// 候補ウィンドウを表示または非表示にする。
    /// 
    /// 引数:
    /// * `bshow`: 表示する場合はTRUE、非表示にする場合はFALSE
    /// 戻り値:
    /// * `WinResult<()>`: 成功時はOk、失敗時はErr
    fn Show(&self, bshow: BOOL) -> WinResult<()> {
        log(&format!("[SamoyedIME] Candidate UI Show: {}", bshow.as_bool()));
        self.state.lock().unwrap().shown = bshow.as_bool();
        Ok(())
    }

    /// 候補ウィンドウが表示されているかどうかを取得する。
    /// 
    /// 戻り値:
    /// * `WinResult<BOOL>`: 表示されている場合はTRUE、非表示の場合はFALSE
    fn IsShown(&self) -> WinResult<BOOL> {
        Ok(BOOL::from(self.state.lock().unwrap().shown))
    }
}

impl ITfCandidateListUIElement_Impl for CandidateListUi_Impl {
    /// 候補ウィンドウの更新されたフラグを取得する。
    ///
    /// 戻り値:
    /// * `WinResult<u32>`: 更新されたフラグのビットマスク
    fn GetUpdatedFlags(&self) -> WinResult<u32> {
        let mut state = self.state.lock().unwrap();
        let flags = state.updated_flags;
        state.updated_flags = 0;
        Ok(flags)
    }

    /// ドキュメントマネージャを取得する。
    ///
    /// 戻り値:
    /// * `WinResult<ITfDocumentMgr>`: ドキュメントマネージャ
    fn GetDocumentMgr(&self) -> WinResult<ITfDocumentMgr> {
        Ok(self.state.lock().unwrap().document_mgr.clone())
    }

    /// 候補の数を取得する。
    ///
    /// 戻り値:
    /// * `WinResult<u32>`: 候補の数
    fn GetCount(&self) -> WinResult<u32> {
        let count = self.state.lock().unwrap().candidates.len() as u32;
        log(&format!("[SamoyedIME] Candidate UI GetCount: {}", count));
        Ok(count)
    }

    /// 選択中の候補インデックスを取得する。
    ///
    /// 戻り値:
    /// * `WinResult<u32>`: 選択中の候補インデックス
    fn GetSelection(&self) -> WinResult<u32> {
        Ok(self.state.lock().unwrap().selection)
    }

    /// 指定したインデックスの候補文字列を取得する。
    ///
    /// 引数:
    /// * `uindex`: 取得する候補のインデックス
    /// 戻り値:
    /// * `WinResult<BSTR>`: 指定したインデックスの候補文字列、またはエラー
    fn GetString(&self, uindex: u32) -> WinResult<BSTR> {
        let state = self.state.lock().unwrap();
        log(&format!("[SamoyedIME] Candidate UI GetString: {}", uindex));
        state
            .candidates
            .get(uindex as usize)
            .map(BSTR::from)
            .ok_or_else(|| windows::core::Error::from_hresult(E_INVALIDARG))
    }

    /// 候補ウィンドウのページインデックスを取得する。
    ///
    /// 引数:
    /// * `pindex`: ページインデックスを格納するポインタ
    /// * `page_count`: ページ数
    /// * `pupagecnt`: ページ数を格納するポインタ
    /// 戻り値:
    /// * `WinResult<()>`: 成功時はOk、失敗時はErr
    fn GetPageIndex(
        &self,
        pindex: *mut u32,
        page_capacity: u32,
        pupagecnt: *mut u32,
    ) -> WinResult<()> {
        let candidate_count = self.state.lock().unwrap().candidates.len() as u32;
        let page_size = 10u32;
        let page_count = candidate_count.div_ceil(page_size).max(1);
        let page_indexes_to_write = page_capacity.min(page_count);
        unsafe {
            if page_indexes_to_write > 0 && !pindex.is_null() {
                let indexes =
                    std::slice::from_raw_parts_mut(pindex, page_indexes_to_write as usize);
                for (page, index) in indexes.iter_mut().enumerate() {
                    *index = page as u32 * page_size;
                }
            }
            if !pupagecnt.is_null() {
                *pupagecnt = page_count;
            }
        }
        Ok(())
    }

    /// 候補ウィンドウのページインデックスを設定する。
    ///
    /// 引数:
    /// * `pindex`: 設定するページインデックスのポインタ
    /// * `upagecnt`: ページ数
    /// 戻り値:
    /// * `WinResult<()>`: 成功時はOk、失敗時はErr
    fn SetPageIndex(&self, _pindex: *const u32, _upagecnt: u32) -> WinResult<()> {
        Ok(())
    }

    /// 現在のページインデックスを取得する。
    ///
    /// 戻り値:
    /// * `WinResult<u32>`: 現在のページインデックス
    fn GetCurrentPage(&self) -> WinResult<u32> {
        Ok(0)
    }
}

impl ITfCandidateListUIElementBehavior_Impl for CandidateListUi_Impl {
    /// アプリケーションから選択候補を変更されたときに呼ばれる。
    /// 
    /// 引数:
    /// * `nindex`: 選択された候補のインデックス
    /// 戻り値:
    /// * `WinResult<()>`: 成功時はOk、失敗時はErr
    fn SetSelection(&self, nindex: u32) -> WinResult<()> {
        let mut state = self.state.lock().unwrap();
        if nindex as usize >= state.candidates.len() {
            return Err(windows::core::Error::from_hresult(E_INVALIDARG));
        }
        state.selection = nindex;
        log(&format!("[SamoyedIME] Candidate UI SetSelection: {}", nindex));
        Ok(())
    }

    /// アプリケーションから現在の候補を確定するときに呼ばれる。
    /// 
    /// 戻り値:
    /// * `WinResult<()>`: 成功時はOk、失敗時はErr
    fn Finalize(&self) -> WinResult<()> {
        log("[SamoyedIME] Candidate UI Finalize");
        self.state.lock().unwrap().shown = false;
        Ok(())
    }

    /// アプリケーションから候補一覧を中止するときに呼ばれる。
    /// 
    /// 戻り値:
    /// * `WinResult<()>`: 成功時はOk、失敗時はErr
    fn Abort(&self) -> WinResult<()> {
        log("[SamoyedIME] Candidate UI Abort");
        self.state.lock().unwrap().shown = false;
        Ok(())
    }
}
