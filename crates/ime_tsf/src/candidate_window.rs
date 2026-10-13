use std::sync::{Mutex, OnceLock};

use windows::core::{w, BOOL, Result as WinResult};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreatePen, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect,
    InvalidateRect, RoundRect, SelectObject, SetBkMode, SetTextColor,
    CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, DEFAULT_CHARSET, DRAW_TEXT_FORMAT, HGDIOBJ,
    OUT_DEFAULT_PRECIS, PS_SOLID, TRANSPARENT,
};
use windows::Win32::UI::TextServices::{ITfComposition, ITfContext};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, IsWindow, PostMessageW,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, GWLP_WNDPROC, HWND_MESSAGE, HWND_TOPMOST,
    SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_SHOWWINDOW, WM_APP, WM_ERASEBKGND,
    WM_NCDESTROY, WM_PAINT, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::logging::log;

const MAX_CANDIDATE_HEIGHT: i32 = 270; // 候補ウィンドウの最大高さ
const CANDIDATE_WIDTH: i32 = 300; // 候補ウィンドウの幅
const CANDIDATE_ROW_HEIGHT: i32 = 30; // 候補行の高さ
const CANDIDATE_VERTICAL_PADDING: i32 = 6; // 候補一覧の上下余白
const MAX_VISIBLE_CANDIDATES: usize = 8; // 一度に表示する候補の最大数
const WM_UPDATE_CANDIDATE_WINDOW: u32 = WM_APP + 1; // 候補ウィンドウの更新メッセージ
const WM_HIDE_CANDIDATE_WINDOW: u32 = WM_APP + 2; // 候補ウィンドウの非表示メッセージ

/// 候補ウィンドウの状態
struct WindowState {
    task_window: Option<isize>, // タスクウィンドウのハンドル
    candidate_window: Option<isize>, // 候補ウィンドウのハンドル
    rect: RECT, // 候補ウィンドウの位置とサイズ
    candidates: Vec<String>, // 候補文字列のリスト
    selection: usize, // 選択中の候補インデックス
}

static STATE: OnceLock<Mutex<WindowState>> = OnceLock::new(); // 候補ウィンドウの状態を保持するシングルトン

/// 候補ウィンドウの状態を取得する。
///
/// 戻り値:
/// * `&'static Mutex<WindowState>`: 候補ウィンドウの状態
fn state() -> &'static Mutex<WindowState> {
    STATE.get_or_init(|| {
        Mutex::new(WindowState {
            task_window: None,
            candidate_window: None,
            rect: RECT::default(),
            candidates: Vec::new(),
            selection: 0,
        })
    })
}

/// 候補ウィンドウを初期化する。
///
/// 戻り値:
/// * `WinResult<()>`: 成功時はOk、失敗時はErr
pub fn initialize() -> WinResult<()> {
    let mut state = state().lock().unwrap();

    if let (Some(task), Some(candidate)) = (state.task_window, state.candidate_window) {
        if unsafe { IsWindow(Some(HWND(task as *mut _))).as_bool() }
            && unsafe { IsWindow(Some(HWND(candidate as *mut _))).as_bool() }
        {
            return Ok(());
        }

        state.task_window = None;
        state.candidate_window = None;
    }

    unsafe {
        let task_window = CreateWindowExW(
            Default::default(),
            w!("STATIC"),
            w!(""),
            Default::default(),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            None,
            None,
        )?;

        SetWindowLongPtrW(
            task_window,
            GWLP_WNDPROC,
            task_window_proc as *const () as isize,
        );

        let candidate_window = match CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
            w!("STATIC"),
            w!("Samoyed IME candidates"),
            WS_POPUP,
            0,
            0,
            CANDIDATE_WIDTH,
            CANDIDATE_ROW_HEIGHT,
            None,
            None,
            None,
            None,
        ) {
            Ok(window) => window,
            Err(error) => {
                let _ = DestroyWindow(task_window);
                return Err(error);
            }
        };

        SetWindowLongPtrW(
            candidate_window,
            GWLP_WNDPROC,
            candidate_window_proc as *const () as isize,
        );

        state.task_window = Some(task_window.0 as isize);
        state.candidate_window = Some(candidate_window.0 as isize);
    }

    Ok(())
}

/// 候補ウィンドウを表示する。
///
/// 引数:
/// * `context`: ITfContextオブジェクト
/// * `edit_cookie`: 編集クッキー
/// * `composition`: ITfCompositionオブジェクト
/// * `candidates`: 候補文字列のリスト
/// * `selection`: 選択中の候補インデックス
///
/// 戻り値:
/// * `WinResult<()>`: 成功時はOk、失敗時はErr
pub fn show(
    context: &ITfContext,
    edit_cookie: u32,
    composition: &ITfComposition,
    candidates: &[String],
    selection: usize,
) -> WinResult<()> {
    log("[SamoyedIME] CandidateWindow::show begin");

    log("[SamoyedIME] Before composition_rect");
    let rect = composition_rect(context, edit_cookie, composition)?;

    log(&format!(
        "[SamoyedIME] composition_rect succeeded: left={}, top={}, right={}, bottom={}",
        rect.left, rect.top, rect.right, rect.bottom
    ));

    let mut state = state().lock().unwrap();
    let Some(task_window) = state.task_window else {
        return Ok(());
    };

    state.rect = rect;
    state.candidates = candidates.to_vec();
    state.selection = selection.min(candidates.len().saturating_sub(1));
    drop(state);

    unsafe {
        PostMessageW(
            Some(HWND(task_window as *mut _)),
            WM_UPDATE_CANDIDATE_WINDOW,
            WPARAM(0),
            LPARAM(0),
        )?;
    }

    Ok(())
}

/// 候補ウィンドウを非表示にする。
pub fn hide() {
    let state = state().lock().unwrap();
    if let Some(task_window) = state.task_window {
        unsafe {
            let _ = PostMessageW(
                Some(HWND(task_window as *mut _)),
                WM_HIDE_CANDIDATE_WINDOW,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
}

/// 候補ウィンドウを終了する。
pub fn shutdown() {
    let state = state().lock().unwrap();
    if let Some(candidate_window) = state.candidate_window {
        unsafe {
            let _ = ShowWindow(HWND(candidate_window as *mut _), SW_HIDE);
        }
    }
}

/// 候補ウィンドウのタスクウィンドウのメッセージプロシージャ
///
/// 引数:
/// * `hwnd`: ウィンドウハンドル
/// * `message`: メッセージID
/// * `wparam`: WPARAM
/// * `lparam`: LPARAM
///
/// 戻り値:
/// * `LRESULT`: メッセージ処理結果
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn task_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_UPDATE_CANDIDATE_WINDOW => {
            log("[SamoyedIME] Candidate update message begin");
            refresh_candidate_window(true);
            log("[SamoyedIME] Candidate update message end");
            LRESULT(0)
        }
        WM_HIDE_CANDIDATE_WINDOW => {
            refresh_candidate_window(false);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

/// 候補ウィンドウのメッセージプロシージャ
///
/// 引数:
/// * `hwnd`: ウィンドウハンドル
/// * `message`: メッセージID
/// * `wparam`: WPARAM
/// * `lparam`: LPARAM
///
/// 戻り値:
/// * `LRESULT`: メッセージ処理結果
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn candidate_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_PAINT => {
            log("[SamoyedIME] candidate_window_proc: WM_PAINT begin");
            let mut paint = Default::default();
            let hdc = BeginPaint(hwnd, &mut paint);

            log("[SamoyedIME] candidate_window_proc: before paint_candidates");
            paint_candidates(hdc, hwnd);
            log("[SamoyedIME] candidate_window_proc: after paint_candidates");

            let _ = EndPaint(hwnd, &paint);
            log("[SamoyedIME] candidate_window_proc: WM_PAINT end");
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_NCDESTROY => DefWindowProcW(hwnd, message, wparam, lparam),
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

/// 候補ウィンドウを更新する。
///
/// 引数:
/// * `show`: 候補ウィンドウを表示するかどうか
fn refresh_candidate_window(show: bool) {
    log("[SamoyedIME] refresh_candidate_window: begin");

    let (handle, rect, candidates, selection) = {
        let state = state().lock().unwrap();
        (
            state.candidate_window,
            state.rect,
            state.candidates.clone(),
            state.selection,
        )
    };

    let Some(handle) = handle else {
        return;
    };

    let hwnd = HWND(handle as *mut _);

    unsafe {
        if !show {
            let _ = ShowWindow(hwnd, SW_HIDE);
            return;
        }

        // 表示する候補数に応じて高さを計算
        // ヘッダーとフッターは設けず、候補一覧をコンパクトに表示
        let visible_count = candidates.len().min(MAX_VISIBLE_CANDIDATES).max(1);
        let height = (visible_count as i32 * CANDIDATE_ROW_HEIGHT
            + CANDIDATE_VERTICAL_PADDING * 2)
            .clamp(
                CANDIDATE_ROW_HEIGHT + CANDIDATE_VERTICAL_PADDING * 2,
                MAX_CANDIDATE_HEIGHT,
            );

        log("[SamoyedIME] refresh: before SetWindowPos");
        let result = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            rect.left,
            rect.bottom,
            CANDIDATE_WIDTH,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        log(&format!(
            "[SamoyedIME] refresh: after SetWindowPos, result={:?}",
            result
        ));

        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = InvalidateRect(Some(hwnd), None, false);
    }

    let _ = selection;
}

/// RGBの各成分からCOLORREFを生成する。
///
/// 引数:
/// * `r`: 赤成分
/// * `g`: 緑成分
/// * `b`: 青成分
///
/// 戻り値:
/// * `COLORREF`: RGB色
const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
}

/// テキストを指定した矩形内に描画する。
///
/// 引数:
/// * `hdc`: デバイスコンテキストのハンドル
/// * `value`: 描画する文字列
/// * `rect`: 描画先の矩形
/// * `flags`: DrawTextWの描画フラグ
fn draw_text_line(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    value: &str,
    rect: &mut RECT,
    flags: u32,
) {
    let mut wide: Vec<u16> = value.encode_utf16().collect();

    unsafe {
        let _ = DrawTextW(hdc, &mut wide, rect, DRAW_TEXT_FORMAT(flags));
    }
}

/// 角丸の矩形を描画する。
///
/// 引数:
/// * `hdc`: デバイスコンテキストのハンドル
/// * `rect`: 描画先の矩形
/// * `color`: 塗りつぶし色
/// * `radius`: 角丸の大きさ
fn fill_rounded_rect(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    rect: &RECT,
    color: COLORREF,
    radius: i32,
) {
    unsafe {
        let brush = CreateSolidBrush(color);
        let pen = CreatePen(PS_SOLID, 1, color);

        let old_brush = SelectObject(hdc, HGDIOBJ(brush.0));
        let old_pen = SelectObject(hdc, HGDIOBJ(pen.0));

        let _ = RoundRect(
            hdc,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            radius,
            radius,
        );

        let _ = SelectObject(hdc, old_pen);
        let _ = SelectObject(hdc, old_brush);

        let _ = DeleteObject(HGDIOBJ(brush.0));
        let _ = DeleteObject(HGDIOBJ(pen.0));
    }
}

/// 候補ウィンドウを描画する。
///
/// 引数:
/// * `hdc`: デバイスコンテキストのハンドル
/// * `hwnd`: ウィンドウハンドル
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn paint_candidates(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    hwnd: HWND,
) {
    let mut client = RECT::default();
    let _ = GetClientRect(hwnd, &mut client);

    // 描画中にMutexを保持しないよう、必要な状態をコピー
    let (candidates, selection) = {
        let state = state().lock().unwrap();
        (state.candidates.clone(), state.selection)
    };

    // 背景と選択行の色を抑えめに
    let background_color = rgb(25, 27, 32);
    let selected_color = rgb(43, 49, 62);
    let accent_color = rgb(112, 145, 226);
    let border_color = rgb(48, 52, 61);
    let text_color = rgb(239, 241, 246);
    let muted_color = rgb(132, 140, 154);
    let selected_number_color = rgb(174, 193, 245);

    // 背景を塗りつぶす
    let background = CreateSolidBrush(background_color);
    let _ = FillRect(hdc, &client, background);
    let _ = DeleteObject(HGDIOBJ(background.0));

    // ウィンドウの輪郭を細く描き、背景との境界を控えめにする
    let border = CreateSolidBrush(border_color);
    let _ = windows::Win32::Graphics::Gdi::FrameRect(hdc, &client, border);
    let _ = DeleteObject(HGDIOBJ(border.0));

    // 候補文字列用と番号用のフォントを作成
    // フォント高さを16px程度に抑え、候補ウィンドウをコンパクトにする
    let font = CreateFontW(
        -16,
        0,
        0,
        0,
        400,
        0,
        0,
        0,
        DEFAULT_CHARSET,
        OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS,
        CLEARTYPE_QUALITY,
        0,
        w!("Segoe UI"),
    );
    let number_font = CreateFontW(
        -13,
        0,
        0,
        0,
        400,
        0,
        0,
        0,
        DEFAULT_CHARSET,
        OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS,
        CLEARTYPE_QUALITY,
        0,
        w!("Segoe UI"),
    );

    let old_font = SelectObject(hdc, HGDIOBJ(font.0));
    let _ = SetBkMode(hdc, TRANSPARENT);

    // 表示領域に合わせて候補数を制限
    // 選択中の候補が表示範囲外にならないよう開始位置を調整
    let visible_rows = ((client.bottom - CANDIDATE_VERTICAL_PADDING * 2)
        / CANDIDATE_ROW_HEIGHT)
        .max(1) as usize;
    let visible_count = candidates
        .len()
        .min(visible_rows)
        .min(MAX_VISIBLE_CANDIDATES);
    let start_index = if candidates.len() > visible_count && visible_count > 0 {
        selection
            .saturating_sub(visible_count.saturating_sub(1))
            .min(candidates.len() - visible_count)
    } else {
        0
    };

    for (visible_index, (index, candidate)) in candidates
        .iter()
        .enumerate()
        .skip(start_index)
        .take(visible_count)
        .enumerate()
    {
        let top = CANDIDATE_VERTICAL_PADDING
            + visible_index as i32 * CANDIDATE_ROW_HEIGHT;
        let selected = index == selection;

        // 選択行だけに薄い背景と細いアクセントを付ける
        if selected {
            let row_rect = RECT {
                left: 5,
                top,
                right: client.right - 5,
                bottom: top + CANDIDATE_ROW_HEIGHT - 2,
            };
            fill_rounded_rect(hdc, &row_rect, selected_color, 7);

            let accent_rect = RECT {
                left: row_rect.left + 1,
                top: row_rect.top + 7,
                right: row_rect.left + 3,
                bottom: row_rect.bottom - 7,
            };
            let accent_brush = CreateSolidBrush(accent_color);
            let _ = FillRect(hdc, &accent_rect, accent_brush);
            let _ = DeleteObject(HGDIOBJ(accent_brush.0));
        }

        // 番号はバッジにせず、左側に小さく控えめに配置
        let mut number_rect = RECT {
            left: 12,
            top: top + 1,
            right: 29,
            bottom: top + CANDIDATE_ROW_HEIGHT - 1,
        };
        let _ = SelectObject(hdc, HGDIOBJ(number_font.0));
        let _ = SetTextColor(
            hdc,
            if selected {
                selected_number_color
            } else {
                muted_color
            },
        );
        draw_text_line(
            hdc,
            &(index + 1).to_string(),
            &mut number_rect,
            0x20 | 0x04 | 0x01,
        );

        // 候補文字列は番号と間隔を空ける
        let mut candidate_rect = RECT {
            left: 38,
            top: top + 1,
            right: client.right - 14,
            bottom: top + CANDIDATE_ROW_HEIGHT - 1,
        };
        let _ = SelectObject(hdc, HGDIOBJ(font.0));
        let _ = SetTextColor(hdc, text_color);
        draw_text_line(
            hdc,
            candidate,
            &mut candidate_rect,
            0x20 | 0x04 | 0x8000,
        );
    }

    // 描画後に元のフォントを復元して、作成したフォントを解放
    let _ = SelectObject(hdc, old_font);
    let _ = DeleteObject(HGDIOBJ(font.0));
    let _ = DeleteObject(HGDIOBJ(number_font.0));
}

/// 候補ウィンドウの位置を取得する。
///
/// 引数:
/// * `context`: ITfContextオブジェクト
/// * `edit_cookie`: 編集クッキー
/// * `composition`: ITfCompositionオブジェクト
///
/// 戻り値:
/// * `WinResult<RECT>`: 候補ウィンドウの位置とサイズ
fn composition_rect(
    context: &ITfContext,
    edit_cookie: u32,
    composition: &ITfComposition,
) -> WinResult<RECT> {
    let view = unsafe { context.GetActiveView()? };
    let range = unsafe { composition.GetRange()? };
    let mut rect = RECT::default();
    let mut clipped = BOOL(0);

    unsafe {
        view.GetTextExt(edit_cookie, &range, &mut rect, &mut clipped)?;
    }

    Ok(rect)
}
