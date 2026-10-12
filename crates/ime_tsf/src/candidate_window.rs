use std::sync::{Mutex, OnceLock};

use windows::core::{w, BOOL, Result as WinResult};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::UI::TextServices::{ITfComposition, ITfContext};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, PostMessageW, SendMessageW, IsWindow,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, GWLP_WNDPROC, HWND_TOPMOST, LB_ADDSTRING, LB_RESETCONTENT, LB_SETCURSEL,
    LB_SETITEMHEIGHT, LBS_NOINTEGRALHEIGHT, LBS_NOTIFY, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE,
    SWP_SHOWWINDOW, WINDOW_STYLE, WM_APP, HWND_MESSAGE, WS_BORDER, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_POPUP, WS_VSCROLL,
};

const MAX_CANDIDATE_HEIGHT: i32 = 320; // 候補ウィンドウの最大高さ
const CANDIDATE_WIDTH: i32 = 320; // 候補ウィンドウの幅
const CANDIDATE_ROW_HEIGHT: i32 = 28; // 候補ウィンドウの1行の高さ
const WM_UPDATE_CANDIDATE_WINDOW: u32 = WM_APP + 1; // 候補ウィンドウの更新メッセージ
const WM_HIDE_CANDIDATE_WINDOW: u32 = WM_APP + 2; // 候補ウィンドウの非表示メッセージ


/// 候補ウィンドウの状態
struct WindowState {
    task_window: Option<isize>, // Task Windowのハンドル
    candidate_window: Option<isize>, // 候補ウィンドウのハンドル
    visible: bool, // 候補ウィンドウが表示されているかどうか
    rect: RECT, // 候補ウィンドウの位置とサイズ
    candidates: Vec<String>, // 候補文字列のリスト
    selection: usize, // 選択中の候補インデックス
}

static STATE: OnceLock<Mutex<WindowState>> = OnceLock::new(); // 候補ウィンドウの状態を保持するシングルトン

/// 候補ウィンドウの状態を取得する。
/// 
/// 戻り値:
/// * `&'static Mutex<WindowState>`: 候補ウィンドウの状態を保持するMutex
fn state() -> &'static Mutex<WindowState> {
    STATE.get_or_init(|| {
        Mutex::new(WindowState {
            task_window: None,
            candidate_window: None,
            visible: false,
            rect: RECT::default(),
            candidates: Vec::new(),
            selection: 0,
        })
    })
}

/// TSF Activate時に、候補PopupとTask Windowを同じTSFスレッドへ作成する。
/// 
/// 戻り値:
/// * `WinResult<()>`: 成功時はOk、失敗時はErr
pub fn initialize() -> WinResult<()> {
    crate::logging::log("[SamoyedIME] Candidate window initialize: begin");
    let mut state = state().lock().unwrap();
    if state.task_window.is_some() && state.candidate_window.is_some() {
        let task_valid = unsafe { IsWindow(Some(HWND(state.task_window.unwrap() as *mut _))).as_bool() };
        let candidate_valid = unsafe { IsWindow(Some(HWND(state.candidate_window.unwrap() as *mut _))).as_bool() };
        if task_valid && candidate_valid {
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
        );
        let task_window = match task_window {
            Ok(window) => window,
            Err(error) => {
                crate::logging::log(&format!("[SamoyedIME] Candidate task window failed: {:?}", error));
                return Err(error);
            }
        };
        crate::logging::log("[SamoyedIME] Candidate task window created");
        SetWindowLongPtrW(
            task_window,
            GWLP_WNDPROC,
            task_window_proc as *const () as isize,
        );
        let candidate_window = match CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
            w!("LISTBOX"),
            w!("Samoyed IME candidates"),
            WS_POPUP
                | WS_BORDER
                | WS_VSCROLL
                | WINDOW_STYLE((LBS_NOTIFY | LBS_NOINTEGRALHEIGHT) as u32),
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
                crate::logging::log(&format!("[SamoyedIME] Candidate popup window failed: {:?}", error));
                let _ = DestroyWindow(task_window);
                return Err(error);
            }
        };
        crate::logging::log("[SamoyedIME] Candidate popup window created");
        state.task_window = Some(task_window.0 as isize);
        state.candidate_window = Some(candidate_window.0 as isize);
    }
    crate::logging::log("[SamoyedIME] Candidate window initialize: complete");
    Ok(())
}

/// EditSessionから候補データだけを更新し、描画はTask Windowへ通知する。
/// 
/// 引数:
/// * `context`: TSFコンテキスト
/// * `edit_cookie`: TSF Edit Cookie
/// * `composition`: TSF Composition
/// * `candidates`: 候補文字列のリスト
/// * `selection`: 選択中の候補インデックス
/// 戻り値:
/// * `WinResult<()>`: 成功時はOk、失敗時はErr
pub fn show(
    context: &ITfContext,
    edit_cookie: u32,
    composition: &ITfComposition,
    candidates: &[String],
    selection: usize,
) -> WinResult<()> {
    let rect = composition_rect(context, edit_cookie, composition)?;
    let mut state = state().lock().unwrap();
    let Some(task_window) = state.task_window else {
        crate::logging::log("[SamoyedIME] Candidate show skipped: task window missing");
        return Ok(());
    };
    state.rect = rect;
    state.candidates = candidates.to_vec();
    state.selection = selection.min(candidates.len().saturating_sub(1));
    state.visible = true;
    drop(state);

    unsafe {
        PostMessageW(
            Some(HWND(task_window as *mut _)),
            WM_UPDATE_CANDIDATE_WINDOW,
            WPARAM(0),
            LPARAM(0),
        )?;
    }
    crate::logging::log("[SamoyedIME] Candidate show message posted");
    Ok(())
}

/// 候補PopupをTask Window経由で非表示にする。
pub fn hide() {
    let mut state = state().lock().unwrap();
    let Some(task_window) = state.task_window else {
        return;
    };
    state.visible = false;
    unsafe {
        let _ = PostMessageW(
            Some(HWND(task_window as *mut _)),
            WM_HIDE_CANDIDATE_WINDOW,
            WPARAM(0),
            LPARAM(0),
        );
    }
}

/// TSF Deactivate時にTask Windowと候補Popupを破棄する。
pub fn shutdown() {
    let mut state = state().lock().unwrap();
    state.visible = false;
    state.candidates.clear();
    if let Some(handle) = state.candidate_window {
        unsafe { let _ = ShowWindow(HWND(handle as *mut _), SW_HIDE); }
    }
}

/// Task Windowのウィンドウプロシージャ
/// 
/// 引数:
/// * `hwnd`: ウィンドウハンドル
/// * `message`: メッセージID
/// * `wparam`: メッセージパラメータ1
/// * `lparam`: メッセージパラメータ2
/// 戻り値:
/// * `LRESULT`: メッセージ処理結果
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn task_window_proc(
    hwnd: HWND,
    message: u32,
    _wparam: WPARAM,
    _lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_UPDATE_CANDIDATE_WINDOW => {
            crate::logging::log("[SamoyedIME] Candidate task window update received");
            refresh_candidate_window(true);
            LRESULT(0)
        }
        WM_HIDE_CANDIDATE_WINDOW => {
            refresh_candidate_window(false);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, message, WPARAM(0), LPARAM(0)),
    }
}

/// 候補Popupの内容を更新する。
/// 
/// 引数:
/// * `show`: 候補Popupを表示するかどうか
fn refresh_candidate_window(show: bool) {
    crate::logging::log(&format!("[SamoyedIME] Candidate popup refresh: show={}", show));
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
        crate::logging::log("[SamoyedIME] Candidate popup refresh skipped: popup missing");
        return;
    };
    let hwnd = HWND(handle as *mut _);

    unsafe {
        if !show {
            let _ = ShowWindow(hwnd, SW_HIDE);
            return;
        }
        SendMessageW(hwnd, LB_RESETCONTENT, None, None);
        for candidate in &candidates {
            let wide: Vec<u16> = candidate.encode_utf16().chain(std::iter::once(0)).collect();
            SendMessageW(hwnd, LB_ADDSTRING, None, Some(LPARAM(wide.as_ptr() as isize)));
        }
        SendMessageW(hwnd, LB_SETCURSEL, Some(WPARAM(selection)), None);
        SendMessageW(
            hwnd,
            LB_SETITEMHEIGHT,
            Some(WPARAM(0)),
            Some(LPARAM(CANDIDATE_ROW_HEIGHT as isize)),
        );
        let height = (candidates.len() as i32 * CANDIDATE_ROW_HEIGHT)
            .clamp(CANDIDATE_ROW_HEIGHT, MAX_CANDIDATE_HEIGHT);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            rect.left,
            rect.bottom,
            CANDIDATE_WIDTH,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
    crate::logging::log("[SamoyedIME] Candidate popup refresh complete");
}

/// Compositionの範囲から候補Popupの位置を計算する。
/// 
/// 引数:
/// * `context`: TSFコンテキスト
/// * `edit_cookie`: TSF Edit Cookie
/// * `composition`: TSF Composition
/// 戻り値:
/// * `WinResult<RECT>`: Compositionの範囲を表すRECT、またはエラー
fn composition_rect(
    context: &ITfContext,
    edit_cookie: u32,
    composition: &ITfComposition,
) -> WinResult<RECT> {
    let view = unsafe { context.GetActiveView()? };
    let range = unsafe { composition.GetRange()? };
    let mut rect = RECT::default();
    let mut clipped = BOOL(0);
    unsafe { view.GetTextExt(edit_cookie, &range, &mut rect, &mut clipped)? };
    Ok(rect)
}
