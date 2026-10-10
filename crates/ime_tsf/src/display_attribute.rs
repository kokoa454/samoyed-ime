use std::cell::Cell;
use std::mem::ManuallyDrop;

use windows::core::{implement, BOOL, Error, GUID, Result as WinResult, BSTR};
use windows::Win32::Foundation::{E_INVALIDARG, E_NOTIMPL, S_FALSE};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::System::Variant::{VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_I4};
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, ITfCategoryMgr, ITfContext, ITfProperty, ITfRange,
    ITfDisplayAttributeProvider, ITfDisplayAttributeProvider_Impl,
    ITfDisplayAttributeInfo, ITfDisplayAttributeInfo_Impl,
    IEnumTfDisplayAttributeInfo, IEnumTfDisplayAttributeInfo_Impl,
    TF_DISPLAYATTRIBUTE, TF_DA_COLOR, TF_CT_NONE, TF_LS_DOT, TF_LS_SOLID,
    TF_ATTR_INPUT, TF_ATTR_TARGET_CONVERTED, TF_ATTR_CONVERTED,
    GUID_PROP_ATTRIBUTE,
};

use crate::logging::log;


pub const GUID_SAMOYED_DISPLAY_ATTR_INPUT: GUID = GUID::from_u128(0xe6d97c36_3918_4a57_8ba5_a8c6ebae0651); // 未確定入力用属性GUID（点線下線）
pub const GUID_SAMOYED_DISPLAY_ATTR_TARGET_CONVERTED: GUID = GUID::from_u128(0xa4f8d227_d779_4bc2_849a_e1bc13e3db67); // 変換中・注目文節用属性GUID（太実線下線）
pub const GUID_SAMOYED_DISPLAY_ATTR_CONVERTED: GUID = GUID::from_u128(0xf1f8da8c_5f80_4cf9_9db8_028f80cb5a92); // 変換中・非注目文節用属性GUID（細実線下線）


/// 属性データ定義
#[derive(Clone)]
struct AttributeData {
    guid: GUID,
    description: &'static str,
    attribute_info: TF_DISPLAYATTRIBUTE,
}

/// DisplayAttributeInfo実装
#[implement(ITfDisplayAttributeInfo)]
pub struct DisplayAttributeInfo {
    data: AttributeData, // 属性データ
}

impl DisplayAttributeInfo {
    /// コンストラクタ
    pub fn new() -> Self {
        Self {
            data: AttributeData {
                guid: GUID_SAMOYED_DISPLAY_ATTR_INPUT,
                description: "Samoyed IME Input",
                attribute_info: TF_DISPLAYATTRIBUTE {
                    crText: TF_DA_COLOR { r#type: TF_CT_NONE, Anonymous: Default::default() },
                    crBk: TF_DA_COLOR { r#type: TF_CT_NONE, Anonymous: Default::default() },
                    lsStyle: TF_LS_DOT,
                    fBoldLine: BOOL(0),
                    crLine: TF_DA_COLOR { r#type: TF_CT_NONE, Anonymous: Default::default() },
                    bAttr: TF_ATTR_INPUT,
                },
            },
        }
    }

    /// 変換中・注目文節（太実線下線）の属性を生成する。
    pub fn new_target_converted() -> Self {
        Self {
            data: AttributeData {
                guid: GUID_SAMOYED_DISPLAY_ATTR_TARGET_CONVERTED,
                description: "Samoyed IME Target Converted",
                attribute_info: TF_DISPLAYATTRIBUTE {
                    crText: TF_DA_COLOR { r#type: TF_CT_NONE, Anonymous: Default::default() },
                    crBk: TF_DA_COLOR { r#type: TF_CT_NONE, Anonymous: Default::default() },
                    lsStyle: TF_LS_SOLID,
                    fBoldLine: BOOL(1),
                    crLine: TF_DA_COLOR { r#type: TF_CT_NONE, Anonymous: Default::default() },
                    bAttr: TF_ATTR_TARGET_CONVERTED,
                },
            },
        }
    }

    /// 変換中・非注目文節（細実線下線）の属性を生成する。
    pub fn new_converted() -> Self {
        Self {
            data: AttributeData {
                guid: GUID_SAMOYED_DISPLAY_ATTR_CONVERTED,
                description: "Samoyed IME Converted",
                attribute_info: TF_DISPLAYATTRIBUTE {
                    crText: TF_DA_COLOR { r#type: TF_CT_NONE, Anonymous: Default::default() },
                    crBk: TF_DA_COLOR { r#type: TF_CT_NONE, Anonymous: Default::default() },
                    lsStyle: TF_LS_SOLID,
                    fBoldLine: BOOL(0),
                    crLine: TF_DA_COLOR { r#type: TF_CT_NONE, Anonymous: Default::default() },
                    bAttr: TF_ATTR_CONVERTED,
                },
            },
        }
    }
}

impl ITfDisplayAttributeInfo_Impl for DisplayAttributeInfo_Impl {
    /// GUIDを取得する。
    ///
    /// # 戻り値
    /// * `Ok(GUID)`: GUID
    /// * `Err(E_FAIL)`: 失敗した場合
    fn GetGUID(&self) -> WinResult<GUID> {
        Ok(self.data.guid)
    }

    /// Descriptionを取得する。
    /// 
    /// # 戻り値
    /// * `Ok(BSTR)`: Description
    /// * `Err(E_FAIL)`: 失敗した場合
    fn GetDescription(&self) -> WinResult<BSTR> {
        Ok(BSTR::from(self.data.description))
    }

    /// DisplayAttributeを取得する。
    ///
    /// # 引数
    /// * `pda`: DisplayAttribute
    ///
    /// # 戻り値
    /// * `Ok(())`: 成功した場合
    /// * `Err(E_FAIL)`: 失敗した場合
    fn GetAttributeInfo(&self, pda: *mut TF_DISPLAYATTRIBUTE) -> WinResult<()> {
        if pda.is_null() {
            return Err(E_INVALIDARG.into());
        }
        unsafe {
            *pda = self.data.attribute_info;
        }
        Ok(())
    }

    /// DisplayAttributeを設定する。
    ///
    /// # 引数
    /// * `_pda`: DisplayAttribute
    ///
    /// # 戻り値
    /// * `Ok(())`: 成功した場合
    /// * `Err(E_NOTIMPL)`: 失敗した場合
    fn SetAttributeInfo(&self, _pda: *const TF_DISPLAYATTRIBUTE) -> WinResult<()> {
        Err(E_NOTIMPL.into())
    }

    /// Resetする。
    ///
    /// # 戻り値
    /// * `Ok(())`: 成功した場合
    /// * `Err(E_FAIL)`: 失敗した場合
    fn Reset(&self) -> WinResult<()> {
        Ok(())
    }
}

/// EnumDisplayAttributeInfo実装
#[implement(IEnumTfDisplayAttributeInfo)]
pub struct EnumDisplayAttributeInfo {
    index: Cell<usize>, // 現在位置
    items: Vec<AttributeData>, // 属性データ
}

impl EnumDisplayAttributeInfo {
    /// コンストラクタ
    pub fn new() -> Self {
        Self {
            index: Cell::new(0),
            items: vec![
                DisplayAttributeInfo::new().data,
                DisplayAttributeInfo::new_target_converted().data,
                DisplayAttributeInfo::new_converted().data,
            ],
        }
    }
}

impl IEnumTfDisplayAttributeInfo_Impl for EnumDisplayAttributeInfo_Impl {
    /// Cloneする。
    ///
    /// # 戻り値
    /// * `Ok(IEnumTfDisplayAttributeInfo)`: Cloneしたオブジェクト
    /// * `Err(E_FAIL)`: 失敗した場合
    fn Clone(&self) -> WinResult<IEnumTfDisplayAttributeInfo> {
        Ok(EnumDisplayAttributeInfo {
            index: Cell::new(self.index.get()),
            items: self.items.clone(),
        }.into())
    }

    /// 次の要素を取得する。
    ///
    /// # 引数
    /// * `count`: 取得する要素数
    /// * `rginfo`: 要素を格納するバッファ
    /// * `pcfetched`: 実際に取得した要素数
    ///
    /// # 戻り値
    /// * `Ok(())`: 成功した場合
    /// * `Err(E_FAIL)`: 失敗した場合
    fn Next(
        &self,
        count: u32,
        rginfo: *mut Option<ITfDisplayAttributeInfo>,
        pcfetched: *mut u32,
    ) -> WinResult<()> {
        if rginfo.is_null() {
            return Err(E_INVALIDARG.into());
        }

        let mut fetched = 0u32;
        let cur = self.index.get();
        for i in 0..(count as usize) {
            if cur + i >= self.items.len() {
                break;
            }
            let info: ITfDisplayAttributeInfo = DisplayAttributeInfo {
                data: self.items[cur + i].clone(),
            }.into();
            unsafe {
                *rginfo.add(i) = Some(info);
            }
            fetched += 1;
        }
        self.index.set(cur + fetched as usize);

        if !pcfetched.is_null() {
            unsafe {
                *pcfetched = fetched;
            }
        }

        if fetched == count {
            Ok(())
        } else {
            Err(Error::from(S_FALSE))
        }
    }

    /// リセットする。
    ///
    /// # 戻り値
    /// * `Ok(())`: 成功した場合
    /// * `Err(E_FAIL)`: 失敗した場合
    fn Reset(&self) -> WinResult<()> {
        self.index.set(0);
        Ok(())
    }

    /// スキップする。
    ///
    /// # 引数
    /// * `count`: スキップする要素数
    ///
    /// # 戻り値
    /// * `Ok(())`: 成功した場合
    /// * `Err(E_FAIL)`: 失敗した場合
    fn Skip(&self, count: u32) -> WinResult<()> {
        let cur = self.index.get();
        let new_index = cur + count as usize;
        if new_index <= self.items.len() {
            self.index.set(new_index);
            Ok(())
        } else {
            self.index.set(self.items.len());
            Err(Error::from(S_FALSE))
        }
    }
}

/// DisplayAttributeProvider実装
#[implement(ITfDisplayAttributeProvider)]
pub struct DisplayAttributeProvider;

impl DisplayAttributeProvider {
    /// コンストラクタ
    pub fn new() -> Self {
        Self
    }

    /// 指定されたGUIDに対応するDisplayAttributeを取得する。
    ///
    /// # 引数
    /// * `guid`: GUID
    ///
    /// # 戻り値
    /// * `Ok(ITfDisplayAttributeInfo)`: DisplayAttribute
    /// * `Err(E_FAIL)`: 失敗した場合
    pub fn get_info(guid: *const GUID) -> WinResult<ITfDisplayAttributeInfo> {
        if guid.is_null() {
            return Err(E_INVALIDARG.into());
        }
        let target_guid = unsafe { *guid };
        if target_guid == GUID_SAMOYED_DISPLAY_ATTR_INPUT {
            Ok(DisplayAttributeInfo::new().into())
        } else if target_guid == GUID_SAMOYED_DISPLAY_ATTR_TARGET_CONVERTED {
            Ok(DisplayAttributeInfo::new_target_converted().into())
        } else if target_guid == GUID_SAMOYED_DISPLAY_ATTR_CONVERTED {
            Ok(DisplayAttributeInfo::new_converted().into())
        } else {
            Err(E_INVALIDARG.into())
        }
    }
}

impl ITfDisplayAttributeProvider_Impl for DisplayAttributeProvider_Impl {
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

/// CompositionのRangeに指定したDisplay Attributeを設定する。
///
/// # 引数
/// * `context`: Context
/// * `ec`: EditContext
/// * `range`: Range
/// * `attr_guid`: Attribute GUID
///
/// # 戻り値
/// * `Ok(())`: 成功した場合
/// * `Err(E_FAIL)`: 失敗した場合
pub fn set_display_attribute(
    context: &ITfContext,
    ec: u32,
    range: &ITfRange,
    attr_guid: &GUID,
) -> WinResult<()> {
    let category_mgr: ITfCategoryMgr = unsafe {
        CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?
    };
    let guid_atom = unsafe { category_mgr.RegisterGUID(attr_guid)? };
    let property: ITfProperty = unsafe { context.GetProperty(&GUID_PROP_ATTRIBUTE)? };

    let var = VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_I4,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 {
                    lVal: guid_atom as i32,
                },
            }),
        },
    };

    unsafe {
        property.SetValue(ec, range, &var)?;
    }
    log(&format!("[SamoyedIME] Set display attribute succeeded for guid: {:?}", attr_guid));
    Ok(())
}

/// CompositionのRangeからDisplay Attributeをクリアする。
///
/// # 引数
/// * `context`: Context
/// * `ec`: EditContext
/// * `range`: Range
///
/// # 戻り値
/// * `Ok(())`: 成功した場合
/// * `Err(E_FAIL)`: 失敗した場合
pub fn clear_display_attribute(
    context: &ITfContext,
    ec: u32,
    range: &ITfRange,
) -> WinResult<()> {
    let property: ITfProperty = unsafe { context.GetProperty(&GUID_PROP_ATTRIBUTE)? };
    unsafe {
        property.Clear(ec, range)?;
    }
    log("[SamoyedIME] Cleared display attribute");
    Ok(())
}
