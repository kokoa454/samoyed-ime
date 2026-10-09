use std::sync::{Arc, Mutex};

use windows::core::{implement, Ref, Result as WinResult, BOOL, GUID, Interface};
use windows::Win32::Foundation::{COLORREF, HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::TextServices::{
    ITfLangBarItem, ITfLangBarItem_Impl, ITfLangBarItemButton, ITfLangBarItemButton_Impl,
    ITfLangBarItemSink, ITfMenu, ITfSource, ITfSource_Impl, TF_LANGBARITEMINFO, TF_LBI_DESC_MAXLEN,
    TF_LBI_ICON, TF_LBI_TEXT, TF_LBI_TOOLTIP, TF_LBI_STYLE_BTN_BUTTON, 
    TF_LBI_STYLE_SHOWNINTRAY, TfLBIClick,
};
use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, HICON, ICONINFO};

use ime_core::InputMode;

use crate::logging::log;
use crate::text_input_processor::SharedState;
use crate::CLSID_SAMOYED_IME;


const LF_FACESIZE: usize = 32; // フォントフェイス名の最大バイト数


/// 入力モードに対応する表示情報
///
/// # 引数
/// * `mode`: 入力モード
/// * `is_open`: IMEがONかどうか
///
/// # 戻り値
/// * `(&'static str, &'static str)`: (表示用テキスト, ツールチップ文字列)
pub(crate) fn mode_to_info(
    mode: InputMode,
    is_open: bool,
) -> (&'static str, &'static str) {
    if !is_open {
        return ("A", "IMEオフ");
    }

    match mode {
        InputMode::Hiragana => ("あ", "ひらがな"),
        InputMode::FullWidthKatakana => ("ア", "全角カタカナ"),
        InputMode::HalfWidthKatakana => ("ｱ", "半角カタカナ"),
        InputMode::FullWidthAlphanumeric => ("Ａ", "全角英数"),
        InputMode::HalfWidthAlphanumeric => ("_A", "半角英数"),
    }
}
/// Windows 公式の入力モード用言語バーアイテム GUID (`GUID_LBI_INPUTMODE`)
pub const GUID_LBI_INPUTMODE: GUID = GUID::from_u128(0x2c77a81e_41cc_4178_a3a7_5f8a987568e6);

#[implement(ITfLangBarItem, ITfLangBarItemButton, ITfSource)]
pub struct SamoyedLangBarItem {
    state: Arc<Mutex<SharedState>>, // バーの状態
    guid: GUID, // バーのGUID
}

impl SamoyedLangBarItem {
    pub fn new(state: Arc<Mutex<SharedState>>) -> Self {
        Self {
            state,
            guid: GUID_LBI_INPUTMODE,
        }
    }

    /// 変更時に呼び出して表示を更新する。
    pub fn update(&self) {
        let sink = {
            let state = self.state.lock().unwrap();
            state.lang_bar_item_sink.clone()
        };

        if let Some(sink) = sink {
            if let Err(e) = unsafe {
                sink.OnUpdate(TF_LBI_ICON | TF_LBI_TEXT | TF_LBI_TOOLTIP)
            } {
                log(&format!(
                    "[SamoyedIME] OnUpdate failed: {:?}",
                    e
                ));
            }
        }
    }
}

impl ITfLangBarItem_Impl for SamoyedLangBarItem_Impl {
    /// LangBarItem の情報を取得する。
    /// 
    /// pinfo: LangBarItem の情報を格納するポインタ。
    /// 
    /// 戻り値:
    /// * `Ok(())`: 成功した場合
    /// * `Err(E_POINTER)`: pinfo が NULL の場合
    fn GetInfo(&self, pinfo: *mut TF_LANGBARITEMINFO) -> WinResult<()> {
        log("[SamoyedIME] LangBarItem::GetInfo called");
        unsafe {
            (*pinfo).clsidService = CLSID_SAMOYED_IME; // TextService の CLSID
            (*pinfo).guidItem = self.guid;
            (*pinfo).dwStyle = TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_SHOWNINTRAY;
            (*pinfo).ulSort = 0;

            let state = self.state.lock().unwrap();
            let (text, _) = mode_to_info(state.ime_state.get_input_mode(), state.is_open);

            let mut label = [0u16; TF_LBI_DESC_MAXLEN as usize];
            for (i, c) in text.encode_utf16().enumerate() {
                if i < label.len() - 1 {
                    label[i] = c;
                }
            }
            (*pinfo).szDescription = label;
        }

        Ok(())
    }

    /// LangBarItem のステータスを取得する。
    /// 
    /// 戻り値:
    /// * `Ok(0)`: 成功した場合
    fn GetStatus(&self) -> WinResult<u32> {
        log("[SamoyedIME] LangBarItem::GetStatus called");
        Ok(0)
    }

    /// LangBarItem を表示するかどうかを設定する。
    /// 
    /// 戻り値:
    /// * `Ok(())`: 成功した場合
    fn Show(&self, _bshow: BOOL) -> WinResult<()> {
        Ok(())
    }

    /// LangBarItem のツールチップ文字列を取得する。
    /// 
    /// 戻り値:
    /// * `Ok(tooltip)`: 成功した場合
    fn GetTooltipString(&self) -> WinResult<windows::core::BSTR> {
        let state = self.state.lock().unwrap();
        let (_, tooltip) = mode_to_info(state.ime_state.get_input_mode(), state.is_open);
        Ok(tooltip.into())
    }
}

impl ITfLangBarItemButton_Impl for SamoyedLangBarItem_Impl {
    /// LangBarItem がクリックされたときの処理。
    ///
    /// _click: クリックの種類。
    /// _pt: クリックした位置。
    ///
    /// 戻り値:
    /// * `Ok(())`: 成功した場合
    /// * `Err(E_POINTER)`: _pt が NULL の場合
    fn OnClick(
        &self,
        _click: TfLBIClick,
        _pt: &POINT,
        _prcarea: *const RECT,
    ) -> WinResult<()> {
        let mut state = self.state.lock().unwrap();
        if state.is_open {
            state.ime_state.apply_mode_command(ime_core::ModeCommand::SwitchKanaType);
        }
        drop(state);

        self.update();
        Ok(())
    }

    /// LangBarItem のメニューが初期化されたときの処理。
    /// 
    /// _pmenu: メニューのポインタ
    ///
    /// 戻り値:
    /// * `Ok(())`: 成功した場合
    fn InitMenu(&self, _pmenu: Ref<'_, ITfMenu>) -> WinResult<()> {
        Ok(())
    }

    /// LangBarItem のメニューが選択されたときの処理。
    ///
    /// _wid: メニューID。
    ///
    /// 戻り値:
    /// * `Ok(())`: 成功した場合
    fn OnMenuSelect(&self, _wid: u32) -> WinResult<()> {
        Ok(())
    }

    /// LangBarItem のアイコンを取得する。
    ///
    /// 戻り値:
    /// * `Ok(icon)`: 成功した場合
    fn GetIcon(&self) -> WinResult<HICON> {
        log("[SamoyedIME] LangBarItem::GetIcon called");
        let state = self.state.lock().unwrap();
        let (text, _) = mode_to_info(state.ime_state.get_input_mode(), state.is_open);

        let hicon = create_text_icon(text);
        if hicon.0.is_null() {
            return Err(windows::core::Error::from(windows::Win32::Foundation::E_FAIL));
        }
        Ok(hicon)
    }

    /// LangBarItem のテキストを取得する。
    ///
    /// 戻り値:
    /// * `Ok(text)`: 成功した場合
    fn GetText(&self) -> WinResult<windows::core::BSTR> {
        log("[SamoyedIME] LangBarItem::GetText called");
        let state = self.state.lock().unwrap();
        let (text, _) = mode_to_info(state.ime_state.get_input_mode(), state.is_open);
        Ok(text.into())
    }
}

impl ITfSource_Impl for SamoyedLangBarItem_Impl {
    /// Sink を追加する。
    ///
    /// riid: Sink のインターフェースID。
    /// punk: Sink のポインタ。
    /// pdwcookie: Cookie を格納するポインタ。
    ///
    /// 戻り値:
    /// * `Ok(())`: 成功した場合
    /// * `Err(E_INVALIDARG)`: riid が ITfLangBarItemSink::IID でない場合
    /// * `Err(E_INVALIDARG)`: punk が NULL の場合
    fn AdviseSink(
        &self,
        riid: *const GUID,
        punk: Ref<'_, windows::core::IUnknown>,
    ) -> WinResult<u32> {
        log("[SamoyedIME] LangBarItem::AdviseSink called");

        let riid = unsafe { *riid };
        if riid != ITfLangBarItemSink::IID {
            return Err(windows::core::Error::new(
                windows::Win32::Foundation::E_INVALIDARG,
                "Unsupported interface",
            ));
        }

        let sink = punk
            .as_ref()
            .ok_or_else(|| {
                windows::core::Error::new(
                    windows::Win32::Foundation::E_INVALIDARG,
                    "null punk",
                )
            })?
            .cast::<ITfLangBarItemSink>()?;

        let mut state = self.state.lock().unwrap();
        state.lang_bar_item_sink = Some(sink);

        Ok(1)
    }

    /// Sink を削除する。
    ///
    /// dwcookie: Cookie。
    ///
    /// 戻り値:
    /// * `Ok(())`: 成功した場合
    fn UnadviseSink(&self, _dwcookie: u32) -> WinResult<()> {
        let mut state = self.state.lock().unwrap();
        state.lang_bar_item_sink = None;
        Ok(())
    }
}

/// 文字を表示するアイコンを作成する。
/// 
/// 引数
/// * `text`: 表示する文字列
/// 
/// 戻り値
/// * `HICON`: 作成したアイコン
fn create_text_icon(text: &str) -> HICON {
    const W: i32 = 16;
    const H: i32 = 16;

    unsafe {
        let hwnd = Some(HWND(std::ptr::null_mut()));
        let hdc_screen = GetDC(hwnd);
        if hdc_screen.0.is_null() {
            return HICON(std::ptr::null_mut());
        }

        let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
        if hdc_mem.0.is_null() {
            let _ = ReleaseDC(hwnd, hdc_screen);
            return HICON(std::ptr::null_mut());
        }

        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: W,
                biHeight: -H,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: 0,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [Default::default(); 1],
        };

        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let hbm_color = match CreateDIBSection(
            Some(hdc_screen),
            &bmi,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        ) {
            Ok(bmp) => bmp,
            Err(_) => {
                let _ = DeleteDC(hdc_mem);
                let _ = ReleaseDC(hwnd, hdc_screen);
                return HICON(std::ptr::null_mut());
            }
        };

        let hbm_mask = CreateBitmap(W, H, 1, 1, None);
        if hbm_mask.0.is_null() {
            let _ = DeleteObject(hbm_color.into());
            let _ = DeleteDC(hdc_mem);
            let _ = ReleaseDC(hwnd, hdc_screen);
            return HICON(std::ptr::null_mut());
        }

        let _ = ReleaseDC(hwnd, hdc_screen);

        let mut face = [0u16; LF_FACESIZE];
        let face_str = "MS Gothic";
        for (i, c) in face_str.encode_utf16().enumerate().take(LF_FACESIZE) {
            face[i] = c;
        }

        let lf = LOGFONTW {
            lfHeight: -11,
            lfWidth: 0,
            lfEscapement: 0,
            lfOrientation: 0,
            lfWeight: 700,
            lfItalic: 0,
            lfUnderline: 0,
            lfStrikeOut: 0,
            lfCharSet: FONT_CHARSET(128),
            lfOutPrecision: FONT_OUTPUT_PRECISION(0),
            lfClipPrecision: FONT_CLIP_PRECISION(0),
            lfQuality: FONT_QUALITY(0),
            lfPitchAndFamily: 0,
            lfFaceName: face,
        };

        let h_font = CreateFontIndirectW(&lf);
        if h_font.0.is_null() {
            let _ = DeleteObject(hbm_color.into());
            let _ = DeleteObject(hbm_mask.into());
            let _ = DeleteDC(hdc_mem);
            return HICON(std::ptr::null_mut());
        }

        let old_bmp = SelectObject(hdc_mem, HGDIOBJ(hbm_color.0));
        let old_font = SelectObject(hdc_mem, HGDIOBJ(h_font.0));

        let h_black = CreateSolidBrush(COLORREF(0x00000000));
        let rc = RECT { left: 0, top: 0, right: W, bottom: H };
        FillRect(hdc_mem, &rc, h_black);
        let _ = DeleteObject(HGDIOBJ(h_black.0));

        let _ = SetBkMode(hdc_mem, TRANSPARENT);
        let _ = SetTextColor(hdc_mem, COLORREF(0x00FFFFFF));

        let mut text_wide: Vec<u16> = text.encode_utf16().collect();
        let mut rc_text = RECT { left: 0, top: 0, right: W, bottom: H };
        DrawTextW(
            hdc_mem,
            &mut text_wide,
            &mut rc_text,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );

        let _ = SelectObject(hdc_mem, old_font);
        let _ = SelectObject(hdc_mem, old_bmp);

        if !bits.is_null() {
            let p = bits as *mut u32;
            for i in 0..(W * H) as usize {
                let pixel = *p.add(i);
                if (pixel & 0x00FFFFFF) != 0 {
                    *p.add(i) = pixel | 0xFF000000;
                } else {
                    *p.add(i) = 0x00000000;
                }
            }
        }

        let hdc_mask = CreateCompatibleDC(Some(hdc_mem));
        let old_mask_bmp = SelectObject(hdc_mask, HGDIOBJ(hbm_mask.0));
        let old_mask_font = SelectObject(hdc_mask, HGDIOBJ(h_font.0));

        let h_white_mask = CreateSolidBrush(COLORREF(0x00FFFFFF));
        FillRect(hdc_mask, &rc, h_white_mask);
        let _ = DeleteObject(HGDIOBJ(h_white_mask.0));

        let _ = SetBkMode(hdc_mask, TRANSPARENT);
        let _ = SetTextColor(hdc_mask, COLORREF(0x00000000));

        let mut rc_mask = RECT { left: 0, top: 0, right: W, bottom: H };
        DrawTextW(
            hdc_mask,
            &mut text_wide,
            &mut rc_mask,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );

        let _ = SelectObject(hdc_mask, old_mask_font);
        let _ = SelectObject(hdc_mask, old_mask_bmp);
        let _ = DeleteDC(hdc_mask);
        let _ = DeleteObject(HGDIOBJ(h_font.0));

        let icon_info = ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: hbm_mask,
            hbmColor: hbm_color,
        };

        let hicon = match CreateIconIndirect(&icon_info) {
            Ok(icon) => icon,
            Err(_) => HICON(std::ptr::null_mut()),
        };

        let _ = DeleteObject(hbm_color.into());
        let _ = DeleteObject(hbm_mask.into());
        let _ = DeleteDC(hdc_mem);

        hicon
    }
}


//. テスト
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_to_info_when_open() {
        assert_eq!(mode_to_info(InputMode::Hiragana, true), ("あ", "ひらがな"));
        assert_eq!(mode_to_info(InputMode::FullWidthKatakana, true), ("ア", "全角カタカナ"));
        assert_eq!(mode_to_info(InputMode::HalfWidthKatakana, true), ("ｱ", "半角カタカナ"));
        assert_eq!(mode_to_info(InputMode::FullWidthAlphanumeric, true), ("Ａ", "全角英数"));
        assert_eq!(mode_to_info(InputMode::HalfWidthAlphanumeric, true), ("_A", "半角英数"));
    }

    #[test]
    fn test_mode_to_info_when_closed() {
        assert_eq!(mode_to_info(InputMode::Hiragana, false), ("A", "IMEオフ"));
        assert_eq!(mode_to_info(InputMode::FullWidthKatakana, false), ("A", "IMEオフ"));
        assert_eq!(mode_to_info(InputMode::HalfWidthKatakana, false), ("A", "IMEオフ"));
        assert_eq!(mode_to_info(InputMode::FullWidthAlphanumeric, false), ("A", "IMEオフ"));
        assert_eq!(mode_to_info(InputMode::HalfWidthAlphanumeric, false), ("A", "IMEオフ"));
    }
}