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
static BUBBLE_ROWS: Mutex<Vec<Row>> = Mutex::new(Vec::new());
static BUBBLE_LANGUAGE: Mutex<Option<LanguageId>> = Mutex::new(None);
static LAST_CONTENT_REFRESH: Mutex<Option<Instant>> = Mutex::new(None);
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

fn format_count(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 { grouped.push(','); }
        grouped.push(ch);
    }
    grouped
}

fn current_rows(language: LanguageId) -> Vec<Row> {
    let mut rows = Vec::new();
    match *TOTAL.lock().unwrap() {
        Some((u, count, errors)) => {
            rows.push(account_panel::section(language, Key::Local));
            for (key, n) in [(Key::Input, u.input_tokens), (Key::Cached, u.cached_input_tokens),
                (Key::Output, u.output_tokens), (Key::Reasoning, u.reasoning_output_tokens)] {
                rows.push(account_panel::row(language, key, format_count(n)));
            }
            let summary = match language {
                LanguageId::SimplifiedChinese => "亿",
                LanguageId::TraditionalChinese => "億",
                LanguageId::Japanese => "億",
                LanguageId::Korean => "억",
                _ => "M",
            };
            let total = if matches!(language, LanguageId::SimplifiedChinese | LanguageId::TraditionalChinese | LanguageId::Japanese | LanguageId::Korean) {
                format!("{} ({:.1} {})", format_count(u.total_tokens), u.total_tokens as f64 / 100_000_000.0, summary)
            } else {
                format!("{} ({:.2}M)", format_count(u.total_tokens), u.total_tokens as f64 / 1_000_000.0)
            };
            rows.push(account_panel::row(language, Key::Total, total));
            rows.push(account_panel::row(language, Key::Sessions, format_count(count as u64)));
            rows.push(account_panel::row(language, Key::Unreadable, format_count(errors as u64)));
        }
        None => {}
    }
    rows.extend(account_panel::rows(language));
    if !rows.is_empty() {
        rows.insert(0, Row {
            title: account_panel::label(language, Key::TimeZone).into(),
            value: String::new(),
            section: false,
            column: 2,
            card: false,
            badge: String::new(),
            badge_tone: 0,
            refresh_line: None,
        });
    }
    rows
}

fn refresh_only_line_changed(old: &Row, new: &Row) -> Option<usize> {
    let line_index = old.refresh_line?;
    if new.refresh_line != Some(line_index) { return None; }
    let mut old_normalized = old.clone();
    let mut new_normalized = new.clone();
    let mut old_lines: Vec<String> = old.value.lines().map(str::to_owned).collect();
    let mut new_lines: Vec<String> = new.value.lines().map(str::to_owned).collect();
    if line_index >= old_lines.len() || line_index >= new_lines.len() { return None; }
    old_lines[line_index] = "<refresh-line>".into();
    new_lines[line_index] = "<refresh-line>".into();
    old_normalized.value = old_lines.join("\n");
    new_normalized.value = new_lines.join("\n");
    (old_normalized == new_normalized).then_some(line_index)
}

pub fn show(hwnd: HWND, language: LanguageId) {
    let rows = current_rows(language);
    if rows.is_empty() {
        hide(hwnd);
        return;
    }
    *BUBBLE_ROWS.lock().unwrap() = rows.clone();
    unsafe {
        let instance = GetModuleHandleW(None).unwrap_or_default();
        let class = WNDCLASSW {
            style: CS_DROPSHADOW,
            lpfnWndProc: Some(bubble_proc),
            hInstance: instance.into(),
            lpszClassName: w!("CodexTokenBubble"),
            ..Default::default()
        };
        RegisterClassW(&class);
        let scale = GetDpiForWindow(hwnd).max(96) as i32;

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let monitor = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        let mut monitor_info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let _ = GetMonitorInfoW(monitor, &mut monitor_info);
        let work = monitor_info.rcWork;
        let width = (920 * scale / 96).min((work.right - work.left - 32).max(300));
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
                    w!("CodexTokenBubble"), PCWSTR::null(), WS_POPUP | WS_VSCROLL,
                    x, y, width, height, None, None, instance, None);
                let Ok(window) = result else {
                    crate::diagnose::log("Unable to create Token popup");
                    return;
                };
                bubble.0 = window.0 as isize;
            }
            bubble.2 = Some(Instant::now());
            bubble.3 = false;
            *BUBBLE_LANGUAGE.lock().unwrap() = Some(language);
            *LAST_CONTENT_REFRESH.lock().unwrap() = Some(Instant::now());
            HWND(bubble.0 as *mut _)
        };
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

unsafe fn measure_width(dc: HDC, text: &str) -> i32 {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    let mut rect = RECT::default();
    DrawTextW(dc, &mut wide, &mut rect, DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX);
    rect.right - rect.left
}

// Keep the reset-card list in a dedicated right column and let each column grow independently.
unsafe fn layout(dc: HDC, rows: &[Row], width: i32, scale: i32) -> (Vec<(RECT, RECT)>, i32) {
    let pad = 16 * scale / 96;
    let has_right_column = rows.iter().any(|row| row.column == 1);
    let columns = if has_right_column && width >= 640 * scale / 96 { 2 } else { 1 };
    let gutter = 20 * scale / 96;
    let col_width = if columns == 2 { (width - pad * 2 - gutter) / 2 } else { width - pad * 2 };
    let label_gap = 8 * scale / 96;
    let label_width = (col_width * 42 / 100).max(100 * scale / 96).min(col_width - label_gap);
    let value_width = col_width - label_width - label_gap;
    let card_pad = 12 * scale / 96;
    let heading_gap = 12 * scale / 96;
    let mut column_bottoms = [pad, pad];
    let mut positions = Vec::new();
    for row in rows {
        if row.column == 2 {
            let y = *column_bottoms.iter().max().unwrap_or(&pad);
            let title_height = measure(dc, &row.title, width - pad * 2);
            let title = RECT { left: pad, top: y, right: width - pad, bottom: y + title_height };
            positions.push((title, RECT::default()));
            let bottom = title.bottom + 8 * scale / 96;
            column_bottoms = [bottom, bottom];
            continue;
        }
        let column = if columns == 2 { row.column.min(1) as usize } else { 0 };
        let x = pad + column as i32 * (col_width + gutter);
        let y = &mut column_bottoms[column];
        if *y > pad && (row.section || row.card) { *y += heading_gap; }

        if row.card {
            let inner_width = col_width - card_pad * 2;
            let title_height = measure(dc, &row.title, inner_width);
            let value_height = measure(dc, &row.value, inner_width);
            let title = RECT {
                left: x + card_pad,
                top: *y + card_pad,
                right: x + col_width - card_pad,
                bottom: *y + card_pad + title_height,
            };
            let value_top = title.bottom + 6 * scale / 96;
            let value = RECT {
                left: title.left,
                top: value_top,
                right: title.right,
                bottom: value_top + value_height,
            };
            positions.push((title, value));
            *y += card_pad * 2 + title_height + 6 * scale / 96 + value_height;
            *y += 8 * scale / 96;
            continue;
        }

        let title_width = if row.section { col_width } else { label_width };
        let title_height = measure(dc, &row.title, title_width);
        let value_height = if row.section { 0 } else { measure(dc, &row.value, value_width) };
        let value_x = if row.section { x + col_width } else { x + label_width + label_gap };
        positions.push((
            RECT { left: x, top: *y, right: x + title_width, bottom: *y + title_height },
            RECT { left: value_x, top: *y, right: x + col_width, bottom: *y + value_height },
        ));
        if row.section { *y += title_height + 11 * scale / 96; }
        else { *y += title_height.max(value_height) + 5 * scale / 96; }
    }
    let content_bottom = *column_bottoms.iter().max().unwrap_or(&pad);
    (positions, content_bottom + pad)
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
            let card_background = CreateSolidBrush(COLORREF(0x00E8F7FF));
            let card_border = CreateSolidBrush(COLORREF(0x00B8D5E6));
            let old = SelectObject(dc, bold);
            SetBkMode(dc, TRANSPARENT);
            let rows = BUBBLE_ROWS.lock().unwrap().clone();
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
                if row.card {
                    let inset = 12 * scale / 96;
                    let card_rect = RECT {
                        left: title.left - inset,
                        top: title.top - inset,
                        right: value.right + inset,
                        bottom: value.bottom + inset,
                    };
                    FillRect(dc, &card_rect, card_background);
                    FrameRect(dc, &card_rect, card_border);
                    SelectObject(dc, bold);
                    SetTextColor(dc, COLORREF(0x003A5D79));
                    if !row.badge.is_empty() {
                        let text_width = measure_width(dc, &row.badge);
                        let badge_width = text_width + 16 * scale / 96;
                        let badge_height = 21 * scale / 96;
                        let badge = RECT { left: value.right - badge_width, top: title.top - 1 * scale / 96,
                            right: value.right, bottom: title.top + badge_height };
                        let (background, foreground) = match row.badge_tone {
                            1 => (COLORREF(0x00E4F3E6), COLORREF(0x002B6B3F)),
                            2 => (COLORREF(0x00DDEEFF), COLORREF(0x00694714)),
                            3 => (COLORREF(0x00E7E9EB), COLORREF(0x00535B61)),
                            _ => (COLORREF(0x00E8F0F4), COLORREF(0x003A5D79)),
                        };
                        let badge_brush = CreateSolidBrush(background);
                        FillRect(dc, &badge, badge_brush);
                        let _ = DeleteObject(badge_brush);
                        let mut badge_text = badge;
                        SetTextColor(dc, foreground);
                        let mut wide: Vec<u16> = row.badge.encode_utf16().collect();
                        DrawTextW(dc, &mut wide, &mut badge_text, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
                        title.right = badge.left - 8 * scale / 96;
                    }
                    draw(dc, &row.title, &mut title);
                    SelectObject(dc, normal);
                    SetTextColor(dc, COLORREF(0x00222222));
                    draw(dc, &row.value, &mut value);
                } else if row.section {
                    SelectObject(dc, bold);
                    SetTextColor(dc, COLORREF(0x003A5D79));
                    draw(dc, &row.title, &mut title);
                    let line = RECT { left: title.left, top: title.bottom + 5, right: title.right, bottom: title.bottom + 6 };
                    let line_brush = CreateSolidBrush(COLORREF(0x00A8CEDC));
                    FillRect(dc, &line, line_brush);
                    let _ = DeleteObject(line_brush);
                } else if row.column == 2 {
                    SelectObject(dc, normal);
                    SetTextColor(dc, COLORREF(0x007B858B));
                    draw(dc, &row.title, &mut title);
                } else {
                    SelectObject(dc, normal);
                    SetTextColor(dc, COLORREF(0x005D6D78));
                    draw(dc, &row.title, &mut title);
                    SelectObject(dc, bold);
                    SetTextColor(dc, COLORREF(0x00222222));
                    draw(dc, &row.value, &mut value);
                }
            }
            SelectObject(dc, old);
            let _ = DeleteObject(card_background);
            let _ = DeleteObject(card_border);
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
        *BUBBLE_LANGUAGE.lock().unwrap() = None;
        *LAST_CONTENT_REFRESH.lock().unwrap() = None;
        BUBBLE_ROWS.lock().unwrap().clear();
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
        else {
            let refresh = {
                let mut last = LAST_CONTENT_REFRESH.lock().unwrap();
                if last.is_some_and(|t| t.elapsed() >= Duration::from_secs(1)) {
                    *last = Some(Instant::now());
                    true
                } else { false }
            };
            if refresh {
                if let Some(language) = *BUBBLE_LANGUAGE.lock().unwrap() {
                    let rows = current_rows(language);
                    if !rows.is_empty() {
                        let (full_redraw, changed_rows) = {
                            let mut current = BUBBLE_ROWS.lock().unwrap();
                            if *current == rows { (false, Vec::new()) }
                            else {
                                let indices = if current.len() == rows.len() {
                                    Some(current.iter().zip(&rows).enumerate()
                                        .filter_map(|(index, (old, new))| {
                                            (old != new).then_some((index, refresh_only_line_changed(old, new)))
                                        })
                                        .collect::<Vec<_>>())
                                } else { None };
                                *current = rows.clone();
                                match indices {
                                    Some(indices) => (false, indices),
                                    None => (true, Vec::new()),
                                }
                            }
                        };
                        let popup = BUBBLE.lock().unwrap().0;
                        if popup != 0 && (full_redraw || !changed_rows.is_empty()) {
                            let popup = HWND(popup as *mut _);
                            if full_redraw {
                                let _ = InvalidateRect(popup, None, false);
                            } else {
                                let mut client = RECT::default();
                                if GetClientRect(popup, &mut client).is_ok() {
                                    let scale = GetDpiForWindow(popup).max(96) as i32;
                                    let dc = GetDC(popup);
                                    let font = make_font(scale, true);
                                    let old = SelectObject(dc, font);
                                    let (positions, _) = layout(dc, &rows, client.right, scale);
                                    let scroll_offset = GetScrollPos(popup, SB_VERT);
                                    let mut dirty_regions = Vec::new();
                                    for (index, refresh_line) in changed_rows {
                                        let Some((mut title, mut value)) = positions.get(index).copied() else { continue; };
                                        title.top -= scroll_offset;
                                        title.bottom -= scroll_offset;
                                        value.top -= scroll_offset;
                                        value.bottom -= scroll_offset;
                                        let scale_margin = 2 * scale / 96;
                                        let dirty = if rows[index].card {
                                            if let Some(line_index) = refresh_line {
                                                let lines: Vec<&str> = rows[index].value.lines().collect();
                                                let Some(line) = lines.get(line_index) else { continue; };
                                                let line_width = value.right - value.left;
                                                let preceding_height: i32 = lines.iter().take(line_index)
                                                    .map(|text| measure(dc, text, line_width)).sum();
                                                let line_height = measure(dc, line, line_width);
                                                let line_width = measure_width(dc, line).min(line_width);
                                                RECT {
                                                    left: value.left - scale_margin,
                                                    top: value.top + preceding_height - scale_margin,
                                                    right: value.left + line_width + scale_margin,
                                                    bottom: value.top + preceding_height + line_height + scale_margin,
                                                }
                                            } else {
                                                let inset = 12 * scale / 96;
                                                RECT { left: title.left - inset, top: title.top - inset,
                                                    right: value.right + inset, bottom: value.bottom + inset }
                                            }
                                        } else {
                                            RECT { left: title.left, top: title.top - scale_margin,
                                                right: value.right.max(title.right),
                                                bottom: title.bottom.max(value.bottom) + scale_margin }
                                        };
                                        let mut dirty = dirty;
                                        dirty.left = dirty.left.max(0);
                                        dirty.top = dirty.top.max(0);
                                        dirty.right = dirty.right.min(client.right);
                                        dirty.bottom = dirty.bottom.min(client.bottom);
                                        if dirty.left < dirty.right && dirty.top < dirty.bottom {
                                            dirty_regions.push(dirty);
                                        }
                                    }
                                    SelectObject(dc, old);
                                    let _ = DeleteObject(font);
                                    ReleaseDC(popup, dc);
                                    for dirty in dirty_regions {
                                        let _ = InvalidateRect(popup, Some(&dirty), false);
                                    }
                                }
                            }
                            let _ = UpdateWindow(popup);
                        }
                    }
                }
            }
        }
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
                        assert!(title.left >= 0 && title.right <= width && value.right <= width);
                        assert_eq!(value.top, title.top);
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
