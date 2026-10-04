mod class_factory;
mod key_event_sink;
mod text_input_processor;
mod logging;
mod edit_session;

use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicUsize, Ordering};

use class_factory::ClassFactory;
use windows::core::{GUID, HRESULT, Interface, IUnknown};
use windows::Win32::Foundation::{CLASS_E_CLASSNOTAVAILABLE, E_POINTER, S_FALSE, S_OK};

use crate::logging::log;


pub(crate) static LIVE_OBJECT_COUNT: AtomicUsize = AtomicUsize::new(0); // 現在DLLが管理しているCOMオブジェクト数
pub(crate) static SERVER_LOCK_COUNT: AtomicUsize = AtomicUsize::new(0); // server lock数
pub const CLSID_SAMOYED_IME: GUID = GUID::from_u128(0xcedc333b_8783_4e6e_9bf5_45310d7bffae); // Samoyed IMEのCLSID
pub const PROFILE_GUID_SAMOYED_IME: GUID = GUID::from_u128(0xd75a0971_4669_49ea_8ffe_42dc31386891); // Samoyed IMEのTSFプロファイルGUID


/// COMクラスオブジェクトを取得する。
/// 
/// 引数
/// * `rclsid`: CLSID
/// * `riid`: IID
/// * `ppv`: COMオブジェクトへのポインタ
/// 
/// 戻り値
/// * `Ok(())`: 成功した場合
/// * `Err(CLASS_E_CLASSNOTAVAILABLE)`: CLSIDが異なる場合
/// * `Err(E_POINTER)`: 引数が無効な場合
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    log("[SamoyedIME] DllGetClassObject");

    if rclsid.is_null() || riid.is_null() || ppv.is_null() {
        log("[SamoyedIME] DllGetClassObject: invalid argument");
        return E_POINTER;
    }

    unsafe {
        *ppv = null_mut();

        if *rclsid != CLSID_SAMOYED_IME {
            log("[SamoyedIME] DllGetClassObject: class unavailable");
            return CLASS_E_CLASSNOTAVAILABLE;
        }

        let factory: IUnknown = ClassFactory::new().into();

        let hr = factory.query(&*riid, ppv);

        log(&format!("[SamoyedIME] DllGetClassObject QueryInterface: HRESULT=0x{:08X}", hr.0 as u32));

        hr
    }
}

/// DLLをアンロードしてよいか判定する。
/// 
/// 戻り値
/// * `Ok(S_OK)`: 成功した場合
/// * `Err(S_FALSE)`: DLLをアンロードできない場合
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if LIVE_OBJECT_COUNT.load(Ordering::SeqCst) == 0
        && SERVER_LOCK_COUNT.load(Ordering::SeqCst) == 0
    {
        S_OK
    } else {
        S_FALSE
    }
}