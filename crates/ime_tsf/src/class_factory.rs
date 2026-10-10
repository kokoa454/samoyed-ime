use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::atomic::Ordering::SeqCst;

use windows::core::{BOOL, GUID, implement, Interface, IUnknown, Result as WinResult, Ref};
use windows::Win32::Foundation::{CLASS_E_NOAGGREGATION, E_POINTER};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};

use crate::{LIVE_OBJECT_COUNT, SERVER_LOCK_COUNT};
use crate::text_input_processor::TextService;

/// TSFを実装するためのClass Factory
#[implement(IClassFactory)]
pub struct ClassFactory;

impl ClassFactory {
    /// コンストラクタ
    pub fn new() -> Self {
        LIVE_OBJECT_COUNT.fetch_add(1, SeqCst);
        Self
    }
}

impl Drop for ClassFactory {
    /// ClassFactoryのDrop実装
    ///
    /// 引数
    /// * `_self`: Drop対象のClassFactory
    ///
    /// 戻り値
    /// * `()`: Drop処理
    fn drop(&mut self) {
        LIVE_OBJECT_COUNT.fetch_sub(1, SeqCst);
    }
}

impl IClassFactory_Impl for ClassFactory_Impl {
    /// COMオブジェクトを生成する。
    ///
    /// 引数
    /// * punkouter: COM Aggregationで使うIUnknown。
    /// * riid: 要求するインターフェースのGUID。
    /// * ppvobject: COMオブジェクトへのポインタ。
    /// 
    /// 戻り値
    /// * Ok(()): 生成に成功した場合
    /// * Err(CLASS_E_NOAGGREGATION): COM Aggregationがサポートされていない場合
    /// * Err(E_POINTER): 引数が無効な場合
    fn CreateInstance(
        &self,
        punkouter: Ref<'_, IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut c_void,
    ) -> WinResult<()> {
        unsafe {
            // 出力先ポインタのチェック
            if ppvobject.is_null() {
                return Err(E_POINTER.into());
            }

            *ppvobject = null_mut();

            // COM Aggregationは今回サポートしない
            if !punkouter.is_null() {
                return Err(CLASS_E_NOAGGREGATION.into());
            }

            // IIDのチェック
            if riid.is_null() {
                return Err(E_POINTER.into());
            }

            // DisplayAttributeProviderが要求された場合
            if *riid == windows::Win32::UI::TextServices::ITfDisplayAttributeProvider::IID {
                let provider = crate::display_attribute::DisplayAttributeProvider::new();
                let unknown: IUnknown = provider.into();
                unknown.query(riid, ppvobject).ok()?;
                return Ok(());
            }

            // TextInputProcessorを生成
            let text_input_processor = TextService::new();

            // COMインターフェースとしてIUnknownを取得
            let unknown: IUnknown = text_input_processor.into();

            // 要求されたIIDをQueryInterfaceで取得
            unknown.query(riid, ppvobject).ok()?;

            Ok(())
        }
    }

    /// COMサーバーをロック / アンロックする。
    /// 
    /// 引数
    /// * flock: ロックする場合はTRUE、アンロックする場合はFALSE。
    /// 
    /// 戻り値
    /// * Ok(()): ロック／アンロックに成功した場合
    fn LockServer(&self, flock: BOOL) -> WinResult<()> {
        if flock.as_bool() {
            // ロック
            SERVER_LOCK_COUNT.fetch_add(1, SeqCst);
        } else {
            // アンロック
            SERVER_LOCK_COUNT.fetch_sub(1, SeqCst);
        }

        Ok(())
    }
}