mod accessibility;
mod appkit;
mod cursor;
mod display;
mod window;

#[allow(unused_imports)]
use objc::{msg_send, sel, sel_impl};
use objc::runtime::{Class, Object, BOOL};
use core_graphics::display::CGRect;
use core_graphics::geometry::CGPoint;
use std::process;

pub struct DisplayInfo {
    pub bounds: CGRect,
}

impl DisplayInfo {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        self.bounds.contains(&CGPoint::new(x, y))
    }
}

pub struct WindowInfo {
    pub pid: i32,
    pub owner_name: String,
    pub title: String,
    pub bounds: CGRect,
}

fn rect_center(r: &CGRect) -> (f64, f64) {
    (
        r.origin.x + r.size.width / 2.0,
        r.origin.y + r.size.height / 2.0,
    )
}

fn on_display(w: &WindowInfo, d: &DisplayInfo) -> bool {
    let (cx, cy) = rect_center(&w.bounds);
    d.contains(cx, cy)
}

fn display_index_for_point(displays: &[DisplayInfo], x: f64, y: f64) -> usize {
    displays.iter().position(|d| d.contains(x, y)).unwrap_or(0)
}

fn display_label(idx: usize) -> &'static str {
    if idx == 0 { "first(メイン)" } else { "second(サブ)" }
}

pub fn debug_log(msg: &str) {
    if std::env::var_os("FOD_DEBUG").is_some() {
        eprintln!("[FOD_DEBUG] {msg}");
    }
}

fn main() {
    // 0. 引数パース
    let args: Vec<String> = std::env::args().collect();
    let target_arg: Option<&str> = args.get(1).map(|s| s.as_str());
    if let Some(t) = target_arg {
        if t != "first" && t != "second" {
            eprintln!("Usage: focus_other_display [first|second]");
            eprintln!("  first  = メインディスプレイ(メニューバーのある画面)");
            eprintln!("  second = サブディスプレイ");
            eprintln!("  引数なし = 反対側のディスプレイへトグル");
            process::exit(1);
        }
    }

    // 1. ディスプレイ情報
    let displays = display::get_displays();
    if displays.len() < 2 {
        eprintln!("ERROR: ディスプレイが1枚しか検出されません");
        process::exit(1);
    }

    // 2. フロントアプリ
    let (front_pid, front_app_name) = appkit::get_frontmost_app();

    // 3. 全ウィンドウ取得 (Z-order順)
    let windows = window::get_windows();

    // 4. 現在のディスプレイ判定(トグルの方向決定と結果表示に使う)
    //    優先: AXFocusedWindow(キーボードフォーカスを持つウィンドウ)
    //    フォールバック: CGWindowList 上のフロントアプリ最上位ウィンドウ(AX 権限なし・未対応アプリ用)
    debug_log(&format!("front app: {front_app_name} (pid {front_pid})"));
    let current_display_idx = accessibility::get_focused_window_bounds(front_pid)
        .map(|b| {
            let (cx, cy) = rect_center(&b);
            let idx = display_index_for_point(&displays, cx, cy);
            debug_log(&format!(
                "現在地: AXFocusedWindow center=({cx:.0},{cy:.0}) → display {idx}"
            ));
            idx
        })
        .or_else(|| {
            windows.iter().find(|w| w.pid == front_pid).map(|w| {
                let (cx, cy) = rect_center(&w.bounds);
                let idx = display_index_for_point(&displays, cx, cy);
                debug_log(&format!(
                    "現在地: CGWindowList フォールバック center=({cx:.0},{cy:.0}) → display {idx}"
                ));
                idx
            })
        })
        .unwrap_or_else(|| {
            debug_log("現在地: 判定不能 → display 0 とみなす");
            0
        });

    // 5. ターゲットディスプレイ
    //    first/second 明示時は現在地判定に依存せず固定する。空のデスクトップが見えていても
    //    キーボードフォーカスは別ディスプレイのウィンドウに残っていることがあり、
    //    「既にターゲットにいる」と判断して何もしないと、その状態から抜け出せなくなるため
    let target_display_idx = match target_arg {
        Some("first") => 0,
        Some("second") => 1,
        _ => if current_display_idx == 0 { 1 } else { 0 },
    };
    let target_display = &displays[target_display_idx];

    // 6. ターゲットウィンドウ検索
    //    (windows は Z-order 順なので find = そのディスプレイ上の最上位)
    let target = windows.iter().find(|w| on_display(w, target_display));

    let target = match target {
        Some(t) => t,
        None => {
            eprintln!("ERROR: ターゲットディスプレイにウィンドウが見つかりません");
            process::exit(1);
        }
    };
    debug_log(&format!(
        "ターゲット: display {target_display_idx} 最前面 [{}] {}",
        target.owner_name, target.title
    ));

    // 7. マウス移動
    let (center_x, center_y) = rect_center(&target.bounds);
    cursor::move_mouse(center_x, center_y);

    // 8. AXRaise + frontmost
    if !accessibility::ax_raise_window(target.pid, &target.bounds) {
        // フォールバック: NSRunningApplication.activateWithOptions
        unsafe {
            let cls = Class::get("NSRunningApplication").unwrap();
            let app: *mut Object =
                msg_send![cls, runningApplicationWithProcessIdentifier: target.pid];
            if !app.is_null() {
                let _: BOOL = msg_send![app, activateWithOptions: 3u64];
            }
        }
    }

    // 9. 結果出力
    let src_name = display_label(current_display_idx);
    let dst_name = display_label(target_display_idx);
    let title = if target.title.is_empty() {
        "(untitled)"
    } else {
        &target.title
    };
    println!(
        "OK: {} [{}] → {} [{} - {}]",
        src_name, front_app_name, dst_name, target.owner_name, title
    );
}
