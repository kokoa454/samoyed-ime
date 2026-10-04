use std::fs::OpenOptions;
use std::io::Write;
use std::env::temp_dir;

use windows::core::PCWSTR;
use windows::Win32::System::Diagnostics::Debug::OutputDebugStringW;

/// デバッグログを出力する。
///
/// 引数
/// * `msg`: ログメッセージ
///
/// 戻り値
/// * なし
pub fn log(msg: &str) {
    let wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        OutputDebugStringW(PCWSTR(wide.as_ptr()));
    }
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(temp_dir().join("samoyed_ime.log"))
    {
        let _ = writeln!(file, "{}", msg);
    }
}