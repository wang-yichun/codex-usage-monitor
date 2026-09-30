use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use serde::Deserialize;
use crate::account_panel::{self, Row, Key};
use crate::localization::LanguageId;
use windows::core::{w, PCWSTR};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Controls::{SetScrollPos, SetScrollInfo};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE};
use windows::Win32::Foundation::*;

use windows::Win32::UI::WindowsAndMessaging::*;


pub const TIMER_BUBBLE: usize = 6101;
static BUBBLE: Mutex<(isize, Vec<u16>, Option<Instant>, bool)> = Mutex::new((0, Vec::new(), None, false));
static RUNNING: AtomicBool = AtomicBool::new(false);
static TOTAL: Mutex<Option<(Usage, usize, usize)>> = Mutex::new(None);


#[derive(Clone, Copy, Default, Deserialize, Debug, PartialEq)]
#[serde(default)]
struct Usage {
    input_tokens: u64,
    cached_input_tokens: u64,
    output_tokens: u64,
    reasoning_output_tokens: u64,
    total_tokens: u64,
}

fn read_usage(reader: impl BufRead) -> Option<Usage> {
    let mut latest = None;
    for line in reader.lines().map_while(Result::ok) {
        if !line.contains("\"token_count\"") { continue; }
        let Ok(record) = serde_json::from_str::<serde_json::Value>(&line) else { continue; };
        if record["type"] != "event_msg" || record["payload"]["type"] != "token_count" { continue; }
        if let Ok(usage) = serde_json::from_value(record["payload"]["info"]["total_token_usage"].clone()) {
            latest = Some(usage);
        }
    }
    latest
}

fn scan() -> Option<(Usage, usize, usize)> {
    let local = crate::native_interop::system_time_to_local(SystemTime::now())?;
    let root = std::env::var_os("CODEX_HOME").map(std::path::PathBuf::from)
        .or_else(|| dirs::home_dir().map(|p| p.join(".codex")))?;
    let mut sessions = HashMap::new();
    let mut unreadable = 0;
    let date_path = format!("{:04}/{:02}/{:02}", local.wYear, local.wMonth, local.wDay);
    let prefix = format!("rollout-{:04}-{:02}-{:02}T", local.wYear, local.wMonth, local.wDay);
    for dir in [root.join("sessions").join(&date_path), root.join("archived_sessions").join(&date_path), root.join("archived_sessions")] {
        let Ok(entries) = std::fs::read_dir(dir) else { continue; };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("jsonl") { continue; }
            if !entry.file_name().to_string_lossy().starts_with(&prefix) { continue; }
            match File::open(&path) {
                Ok(file) => {
                    if let Some(usage) = read_usage(BufReader::new(file)) {
                        let previous = sessions.entry(entry.file_name()).or_insert(usage);
                        if usage.total_tokens > previous.total_tokens { *previous = usage; }
                    }
                }
                Err(_) => unreadable += 1,
            }
        }
    }
    let mut total = Usage::default();
    for usage in sessions.values() {
        total.input_tokens += usage.input_tokens;
        total.cached_input_tokens += usage.cached_input_tokens;
        total.output_tokens += usage.output_tokens;
        total.reasoning_output_tokens += usage.reasoning_output_tokens;
        total.total_tokens += usage.total_tokens;
    }
    Some((total, sessions.len(), unreadable))
}

pub fn start(_hwnd: HWND) {
    RUNNING.store(true, Ordering::Relaxed);

    std::thread::spawn(move || {
        while RUNNING.load(Ordering::Relaxed) {
            *TOTAL.lock().unwrap() = scan();

            for _ in 0..60 {
                if !RUNNING.load(Ordering::Relaxed) { return; }
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    });
}

pub fn show(hwnd: HWND, language: LanguageId) {
    let mut rows = vec![account_panel::section(language, Key::Title), account_panel::section(language, Key::Local)];
    match *TOTAL.lock().unwrap() {
        Some((u, count, errors)) => {
            for (key, n) in [(Key::Input, u.input_tokens), (Key::Cached, u.cached_input_tokens),
                (Key::Output, u.output_tokens), (Key::Reasoning, u.reasoning_output_tokens)] {
                rows.push(account_panel::row(language, key, n.to_string()));
            }
            let unit = match language {
                LanguageId::SimplifiedChinese => "亿",
                LanguageId::TraditionalChinese => "億",
                LanguageId::Japanese => "億",
                LanguageId::Korean => "억",
                _ => "×100M",
            };
            rows.push(account_panel::row(language, Key::Total, format!("{} ({:.3} {})", u.total_tokens, u.total_tokens as f64 / 100_000_000.0, unit)));
            rows.push(account_panel::row(language, Key::Sessions, count.to_string()));
            rows.push(account_panel::row(language, Key::Unreadable, errors.to_string()));
        }
        None => rows.push(account_panel::row(language, Key::Local, account_panel::label(language, Key::NoData).into())),
    }
    rows.extend(account_panel::rows(language));
    let text = serde_json::to_string(&rows).unwrap_or_default();    unsafe {
        let instance = GetModuleHandleW(None).unwrap_or_default();
        let class = WNDCLASSW {
            style: CS_DROPSHADOW,
            lpfnWndProc: Some(bubble_proc),
            hInstance: instance.into(),
            lpszClassName: w!("CodexTokenBubble"),
            ..Default::default()
        };
        RegisterClassW(&class);
        let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        let scale = GetDpiForWindow(hwnd).max(96) as i32;

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let monitor = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        let mut monitor_info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let _ = GetMonitorInfoW(monitor, &mut monitor_info);
        let work = monitor_info.rcWork;
        let width = (1080 * scale / 96).min((work.right - work.left - 32).max(300));
        let dc = GetDC(hwnd);
        let font = make_font(scale, true);
        let old = SelectObject(dc, font);
        let (_, content_height) = layout(dc, &rows, width, scale);
        SelectObject(dc, old);
        let _ = DeleteObject(font);
        ReleaseDC(hwnd, dc);
        let height = content_height.min((work.bottom - work.top - 32).max(180));
        let x = pt.x.clamp(work.left, (work.right - width).max(work.left));
        let y = (pt.y - height - 12).clamp(work.top, (work.bottom - height).max(work.top));
        let window = {
            let mut bubble = BUBBLE.lock().unwrap();
            if bubble.0 == 0 {
                // No taskbar owner or parent: this is an independent top-level popup.
                let result = CreateWindowExW(WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    w!("CodexTokenBubble"), PCWSTR(wide.as_ptr()), WS_POPUP | WS_VSCROLL,
                    x, y, width, height, None, None, instance, None);
                let Ok(window) = result else {
                    crate::diagnose::log("Unable to create Token popup");
                    return;
                };
                bubble.0 = window.0 as isize;
            }
            bubble.2 = Some(Instant::now());
            bubble.3 = false;
            HWND(bubble.0 as *mut _)
        };
        let _ = SetWindowTextW(window, PCWSTR(wide.as_ptr()));
        SetScrollPos(window, SB_VERT, 0, false);
        let _ = SetWindowPos(window, HWND_TOPMOST, x, y, width, height, SWP_NOACTIVATE | SWP_SHOWWINDOW);
        let _ = InvalidateRect(window, None, false);
        let _ = UpdateWindow(window);
        SetTimer(hwnd, TIMER_BUBBLE, 100, None);
    }
}

unsafe fn make_font(scale: i32, bold: bool) -> HFONT {
    CreateFontW(-13 * scale / 96, 0, 0, 0, if bold { FW_SEMIBOLD.0 } else { FW_NORMAL.0 } as i32,
        0, 0, 0, DEFAULT_CHARSET.0 as u32, OUT_DEFAULT_PRECIS.0 as u32,
        CLIP_DEFAULT_PRECIS.0 as u32, CLEARTYPE_QUALITY.0 as u32, 0, w!("Segoe UI"))
}

unsafe fn measure(dc: HDC, text: &str, width: i32) -> i32 {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    let mut rect = RECT { right: width, ..Default::default() };
    DrawTextW(dc, &mut wide, &mut rect, DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX);
    (rect.bottom - rect.top).max(16)
}

// Balance complete title/value pairs across columns. Each pair wraps independently.
unsafe fn layout(dc: HDC, rows: &[Row], width: i32, scale: i32) -> (Vec<(RECT, RECT)>, i32) {
    let pad = 18 * scale / 96;
    let columns = if width >= 850 * scale / 96 { 3 } else { 2 };
    let col_width = (width - pad * (columns + 1)) / columns;
    let heights: Vec<(i32, i32)> = rows.iter().map(|r| (
        measure(dc, &r.title, col_width),
        if r.section { 0 } else { measure(dc, &r.value, col_width) },
    )).collect();
    let sum: i32 = heights.iter().map(|(a,b)| a + b + pad).sum();
    let target = (sum + columns - 1) / columns;
    let mut col = 0;
    let mut y = pad;
    let mut bottom = y;
    let mut positions = Vec::new();
    for (i, (label_height, value_height)) in heights.into_iter().enumerate() {
        let h = label_height + value_height + pad;
        if y > pad && y + h - pad > target && col < columns - 1 && !rows[i - 1].section {
            col += 1;
            y = pad;
        }
        let x = pad + col * (col_width + pad);
        positions.push((
            RECT { left: x, top: y, right: x + col_width, bottom: y + label_height },
            RECT { left: x, top: y + label_height + 3 * scale / 96, right: x + col_width, bottom: y + label_height + value_height + 3 * scale / 96 },
        ));
        y += h;
        bottom = bottom.max(y);
    }
    (positions, bottom)
}

unsafe fn draw(dc: HDC, text: &str, rect: &mut RECT) {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    DrawTextW(dc, &mut wide, rect, DT_WORDBREAK | DT_NOPREFIX);
}

unsafe extern "system" fn bubble_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            let dc = BeginPaint(hwnd, &mut paint);
            let brush = CreateSolidBrush(COLORREF(0x00D6F6FF));
            let mut client = RECT::default();
            let _ = GetClientRect(hwnd, &mut client);
            FillRect(dc, &client, brush);
            let _ = DeleteObject(brush);
            let scale = GetDpiForWindow(hwnd).max(96) as i32;
            let normal = make_font(scale, false);
            let bold = make_font(scale, true);
            let old = SelectObject(dc, bold);
            SetBkMode(dc, TRANSPARENT);
            let mut text = vec![0u16; GetWindowTextLengthW(hwnd) as usize + 1];
            let len = GetWindowTextW(hwnd, &mut text);
            let json = String::from_utf16_lossy(&text[..len as usize]);
            let rows: Vec<Row> = serde_json::from_str(&json).unwrap_or_default();
            let (positions, height) = layout(dc, &rows, client.right, scale);
            let scroll = SCROLLINFO { cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
                fMask: SIF_RANGE | SIF_PAGE, nMin: 0, nMax: height - 1,
                nPage: client.bottom.max(0) as u32, ..Default::default() };
            SetScrollInfo(hwnd, SB_VERT, &scroll, true);
            let offset = GetScrollPos(hwnd, SB_VERT);
            for (row, (mut title, mut value)) in rows.iter().zip(positions) {
                title.top -= offset; title.bottom -= offset;
                value.top -= offset; value.bottom -= offset;
                if value.bottom < 0 || title.top > client.bottom { continue; }
                SelectObject(dc, if row.section { bold } else { normal });
                SetTextColor(dc, COLORREF(if row.section { 0x003A5D79 } else { 0x005D6D78 }));
                draw(dc, &row.title, &mut title);
                if row.section {
                    let line = RECT { left: title.left, top: title.bottom + 5, right: title.right, bottom: title.bottom + 6 };
                    let line_brush = CreateSolidBrush(COLORREF(0x00A8CEDC));
                    FillRect(dc, &line, line_brush);
                    let _ = DeleteObject(line_brush);
                } else {
                    SelectObject(dc, bold);
                    SetTextColor(dc, COLORREF(0x00222222));
                    draw(dc, &row.value, &mut value);
                }
            }
            SelectObject(dc, old);
            let _ = DeleteObject(normal);
            let _ = DeleteObject(bold);
            let _ = EndPaint(hwnd, &paint);
            LRESULT(0)
        }
        WM_MOUSEWHEEL | WM_VSCROLL => {
            let current = GetScrollPos(hwnd, SB_VERT);
            let mut info = SCROLLINFO { cbSize: std::mem::size_of::<SCROLLINFO>() as u32, fMask: SIF_ALL, ..Default::default() };
            let _ = GetScrollInfo(hwnd, SB_VERT, &mut info);
            let next = if msg == WM_MOUSEWHEEL {
                current - ((wparam.0 >> 16) as u16 as i16 as i32) / 120 * 60
            } else {
                match wparam.0 as u16 as i32 {
                    0 => current - 30, 1 => current + 30,
                    2 => current - info.nPage as i32, 3 => current + info.nPage as i32,
                    4 | 5 => info.nTrackPos, 6 => 0, 7 => info.nMax,
                    _ => current,
                }
            };
            SetScrollPos(hwnd, SB_VERT, next.max(0), true);
            let _ = InvalidateRect(hwnd, None, false);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub fn hide(hwnd: HWND) {
    unsafe {
        let mut bubble = BUBBLE.lock().unwrap();
        if bubble.0 != 0 { let _ = ShowWindow(HWND(bubble.0 as *mut _), SW_HIDE); }
        bubble.2 = None;
        let _ = KillTimer(hwnd, TIMER_BUBBLE);
    }
}

pub fn tick(hwnd: HWND) {
    unsafe {
        let mut point = POINT::default();
        let mut rect = RECT::default();
        let outside = GetCursorPos(&mut point).is_err() || GetWindowRect(hwnd, &mut rect).is_err()
            || point.x < rect.left || point.x >= rect.right || point.y < rect.top || point.y >= rect.bottom;
        let dismiss = {
            let mut bubble = BUBBLE.lock().unwrap();
            let mut bubble_rect = RECT::default();
            let in_bubble = bubble.0 != 0 && GetWindowRect(HWND(bubble.0 as *mut _), &mut bubble_rect).is_ok()
                && point.x >= bubble_rect.left && point.x < bubble_rect.right
                && point.y >= bubble_rect.top && point.y < bubble_rect.bottom;
            if !outside || in_bubble { bubble.2 = Some(Instant::now()); }
            outside && !in_bubble && bubble.2.is_some_and(|t| t.elapsed() >= Duration::from_millis(300))
        };
        if dismiss || GetAsyncKeyState(VK_ESCAPE.0 as i32) < 0 { hide(hwnd); }
    }
}
pub fn stop() {
    RUNNING.store(false, Ordering::Relaxed);
    let mut bubble = BUBBLE.lock().unwrap();
    if bubble.0 != 0 {
        unsafe { let _ = DestroyWindow(HWND(bubble.0 as *mut _)); }
        bubble.0 = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn translated_title_value_pairs_fit_columns_without_overlapping() {
        unsafe {
            let dc = CreateCompatibleDC(HDC::default());
            let font = make_font(96, true);
            let old = SelectObject(dc, font);
            for language in LanguageId::ALL {
                let rows: Vec<Row> = (0..60).map(|i| account_panel::row(language,
                    if i % 2 == 0 { Key::Reasoning } else { Key::RedeemStart },
                    "2026-10-04 21:39 · 2728083 (0.027 ×100M)".into())).collect();
                for width in [600, 1080] {
                    let (positions, height) = layout(dc, &rows, width, 96);
                    assert_eq!(positions.len(), rows.len());
                    assert!(height > 0);
                    for (title, value) in positions {
                        assert!(title.left >= 0 && value.right <= width);
                        assert!(value.top >= title.bottom);
                        assert!(value.bottom < height);
                    }
                }
            }
            SelectObject(dc, old);
            let _ = DeleteObject(font);
            let _ = DeleteDC(dc);
        }
    }

    #[test]
    fn cumulative_events_are_not_added_twice_and_null_events_preserve_total() {
        let log = concat!(
            "{\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":100,\"total_tokens\":120}}}}\n",
            "{\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":200,\"cached_input_tokens\":150,\"output_tokens\":30,\"total_tokens\":230}}}}\n",
            "{\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":null}}\n",
            "{incomplete"
        );
        let usage = read_usage(log.as_bytes()).unwrap();
        assert_eq!(usage.total_tokens, 230);
        assert_eq!(usage.cached_input_tokens, 150);
    }
}
