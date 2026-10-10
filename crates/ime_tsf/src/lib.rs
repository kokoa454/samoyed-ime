mod class_factory;
mod key_event_sink;
mod text_input_processor;
mod logging;
mod edit_session;
mod lang_bar;
mod display_attribute;

use std::ffi::c_void;
use std::ptr::null_mut;
use std::slice::from_raw_parts;
use std::sync::atomic::{AtomicUsize, Ordering};

use class_factory::ClassFactory;
use windows::core::{GUID, HRESULT, Interface, IUnknown, PCWSTR, Result as WinResult};
use windows::Win32::Foundation::{CLASS_E_CLASSNOTAVAILABLE, E_FAIL, E_POINTER, HMODULE, S_FALSE, S_OK};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
use windows::Win32::System::LibraryLoader::{
    GetModuleFileNameW, GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
    GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW, HKEY_CLASSES_ROOT,
    KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ,
};
use windows::Win32::UI::TextServices::{
    ITfCategoryMgr, ITfInputProcessorProfiles, CLSID_TF_CategoryMgr,
    CLSID_TF_InputProcessorProfiles, GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER, GUID_TFCAT_DISPLAYATTRIBUTEPROPERTY,
};

use crate::display_attribute::{
    GUID_SAMOYED_DISPLAY_ATTR_INPUT,
    GUID_SAMOYED_DISPLAY_ATTR_TARGET_CONVERTED,
    GUID_SAMOYED_DISPLAY_ATTR_CONVERTED,
};

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

/// 自身のDLLファイルパスを取得する。
fn get_dll_path() -> WinResult<Vec<u16>> {
    let mut hmodule = HMODULE::default();
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(get_dll_path as *const u16),
            &mut hmodule,
        )?;
        let mut path = vec![0u16; 1024];
        let len = GetModuleFileNameW(Some(hmodule), &mut path);
        if len == 0 {
            return Err(E_FAIL.into());
        }
        path.truncate((len + 1) as usize);
        Ok(path)
    }
}

/// COMサーバー（CLSID / InprocServer32）をレジストリへ書き込む。
unsafe fn register_com_server(clsid_str: &str, dll_path: &[u16]) -> WinResult<()> {
    let clsid_key = format!("CLSID\\{{{}}}", clsid_str);
    let inproc_key = format!("CLSID\\{{{}}}\\InprocServer32", clsid_str);

    let mut hkey = Default::default();

    // HKCR\CLSID\{GUID}
    let clsid_wide: Vec<u16> = clsid_key.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { RegCreateKeyExW(
            HKEY_CLASSES_ROOT,
            PCWSTR(clsid_wide.as_ptr()),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        ).ok()?;
    }

    let name_wide: Vec<u16> = "Samoyed IME".encode_utf16().chain(std::iter::once(0)).collect();
    let name_bytes = unsafe { from_raw_parts(name_wide.as_ptr() as *const u8, name_wide.len() * 2) };
    unsafe {
        RegSetValueExW(
            hkey,
            PCWSTR::null(),
            None,
            REG_SZ,
            Some(name_bytes),
        ).ok()?
    };
    let _ = unsafe { RegCloseKey(hkey) };

    // HKCR\CLSID\{GUID}\InprocServer32
    let inproc_wide: Vec<u16> = inproc_key.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        RegCreateKeyExW(
            HKEY_CLASSES_ROOT,
            PCWSTR(inproc_wide.as_ptr()),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        ).ok()?
    };

    let path_bytes = unsafe { from_raw_parts(dll_path.as_ptr() as *const u8, dll_path.len() * 2) };
    unsafe {
        RegSetValueExW(
            hkey,
            PCWSTR::null(),
            None,
            REG_SZ,
            Some(path_bytes),
        ).ok()?
    };

    let model_wide: Vec<u16> = "Apartment".encode_utf16().chain(std::iter::once(0)).collect();
    let model_bytes = unsafe { from_raw_parts(model_wide.as_ptr() as *const u8, model_wide.len() * 2) };
    let threading_wide: Vec<u16> = "ThreadingModel".encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        RegSetValueExW(
            hkey,
            PCWSTR(threading_wide.as_ptr()),
            None,
            REG_SZ,
            Some(model_bytes),
        ).ok()?
    };

    let _ = unsafe { RegCloseKey(hkey) };
    Ok(())
}

/// COMサーバーをレジストリから削除する。
unsafe fn unregister_com_server(clsid_str: &str) {
    let clsid_key = format!("CLSID\\{{{}}}", clsid_str);
    let clsid_wide: Vec<u16> = clsid_key.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = unsafe { RegDeleteTreeW(HKEY_CLASSES_ROOT, PCWSTR(clsid_wide.as_ptr())) };
}

/// DLLをレジストリおよびTSFへ登録する。
/// 
/// 戻り値
/// * `S_OK`: 登録に成功した場合
/// * `E_FAIL`: 登録に失敗した場合
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllRegisterServer() -> HRESULT {
    log("[SamoyedIME] DllRegisterServer called");

    let clsid_str = "cedc333b-8783-4e6e-9bf5-45310d7bffae";

    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

    // 1. DLLパスを取得し、COMサーバー（InprocServer32）をレジストリ登録
    let dll_path = match get_dll_path() {
        Ok(p) => p,
        Err(e) => {
            log(&format!("[SamoyedIME] get_dll_path failed: {:?}", e));
            return E_FAIL;
        }
    };

    if let Err(e) = unsafe { register_com_server(clsid_str, &dll_path) } {
        log(&format!("[SamoyedIME] register_com_server failed: {:?}", e));
        return E_FAIL;
    }

    // 2. TSF プロファイルの登録
    let profiles: ITfInputProcessorProfiles = match unsafe { CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER) } {
        Ok(p) => p,
        Err(e) => {
            log(&format!("[SamoyedIME] CoCreateInstance ITfInputProcessorProfiles failed: {:?}", e));
            return E_FAIL;
        }
    };

    if let Err(e) = unsafe { profiles.Register(&CLSID_SAMOYED_IME) } {
        log(&format!("[SamoyedIME] Register failed: {:?}", e));
        return E_FAIL;
    }

    let profile_name: Vec<u16> = "Samoyed IME".encode_utf16().chain(std::iter::once(0)).collect();
    if let Err(e) = unsafe {
        profiles.AddLanguageProfile(
            &CLSID_SAMOYED_IME,
            0x0411,
            &PROFILE_GUID_SAMOYED_IME,
            &profile_name,
            &[],
            0,
        )
    } {
        log(&format!("[SamoyedIME] AddLanguageProfile failed: {:?}", e));
        return E_FAIL;
    }

    // 3. TSF カテゴリ登録
    let category_mgr: ITfCategoryMgr = match unsafe { CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER) } {
        Ok(cm) => cm,
        Err(e) => {
            log(&format!("[SamoyedIME] CoCreateInstance ITfCategoryMgr failed: {:?}", e));
            return E_FAIL;
        }
    };

    if let Err(e) = unsafe {
        category_mgr.RegisterCategory(
            &CLSID_SAMOYED_IME,
            &GUID_TFCAT_TIP_KEYBOARD,
            &CLSID_SAMOYED_IME,
        )
    } {
        log(&format!("[SamoyedIME] RegisterCategory TIP_KEYBOARD failed: {:?}", e));
        return E_FAIL;
    }

    // Display Attribute Provider の登録
    if let Err(e) = unsafe {
        category_mgr.RegisterCategory(
            &CLSID_SAMOYED_IME,
            &GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
            &CLSID_SAMOYED_IME,
        )
    } {
        log(&format!("[SamoyedIME] RegisterCategory DISPLAYATTRIBUTEPROVIDER failed: {:?}", e));
        return E_FAIL;
    }

    // 各属性 GUID の登録
    let attr_guids = [
        GUID_SAMOYED_DISPLAY_ATTR_INPUT,
        GUID_SAMOYED_DISPLAY_ATTR_TARGET_CONVERTED,
        GUID_SAMOYED_DISPLAY_ATTR_CONVERTED,
    ];
    for guid in &attr_guids {
        if let Err(e) = unsafe {
            category_mgr.RegisterCategory(
                &CLSID_SAMOYED_IME,
                &GUID_TFCAT_DISPLAYATTRIBUTEPROPERTY,
                guid,
            )
        } {
            log(&format!("[SamoyedIME] RegisterCategory DISPLAYATTRIBUTEPROPERTY {:?} failed: {:?}", guid, e));
            return E_FAIL;
        }
    }

    log("[SamoyedIME] DllRegisterServer succeeded");
    S_OK
}

/// DLLの登録を解除する。
/// 
/// 戻り値
/// * `S_OK`: 登録解除に成功した場合
/// * `E_FAIL`: 登録解除に失敗した場合
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllUnregisterServer() -> HRESULT {
    log("[SamoyedIME] DllUnregisterServer called");

    let clsid_str = "cedc333b-8783-4e6e-9bf5-45310d7bffae";

    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

    if let Ok(category_mgr) = unsafe { CoCreateInstance::<_, ITfCategoryMgr>(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER) } {
        let attr_guids = [
            GUID_SAMOYED_DISPLAY_ATTR_INPUT,
            GUID_SAMOYED_DISPLAY_ATTR_TARGET_CONVERTED,
            GUID_SAMOYED_DISPLAY_ATTR_CONVERTED,
        ];
        for guid in &attr_guids {
            let _ = unsafe {
                category_mgr.UnregisterCategory(
                    &CLSID_SAMOYED_IME,
                    &GUID_TFCAT_DISPLAYATTRIBUTEPROPERTY,
                    guid,
                )
            };
        }

        let _ = unsafe {
            category_mgr.UnregisterCategory(
                &CLSID_SAMOYED_IME,
                &GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
                &CLSID_SAMOYED_IME,
            )
        };

        let _ = unsafe {
            category_mgr.UnregisterCategory(
                &CLSID_SAMOYED_IME,
                &GUID_TFCAT_TIP_KEYBOARD,
                &CLSID_SAMOYED_IME,
            )
        };
    }

    if let Ok(profiles) = unsafe { CoCreateInstance::<_, ITfInputProcessorProfiles>(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER) } {
        let _ = unsafe { profiles.Unregister(&CLSID_SAMOYED_IME) };
    }

    unsafe { unregister_com_server(clsid_str) };

    log("[SamoyedIME] DllUnregisterServer succeeded");
    S_OK
}