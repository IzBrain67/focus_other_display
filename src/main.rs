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

fn display_index_for_point(displays: &[DisplayInfo], x: f64, y: f64) -> usize {
    displays.iter().position(|d| d.contains(x, y)).unwrap_or(0)
}

fn display_label(idx: usize) -> &'static str {
    if idx == 0 { "first(メイン)" } else { "second(サブ)" }
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

    // 4. 現在のディスプレイ判定
    let current_display_idx = windows
        .iter()
        .find(|w| w.pid == front_pid)
        .map(|w| {
            let cx = w.bounds.origin.x + w.bounds.size.width / 2.0;
            let cy = w.bounds.origin.y + w.bounds.size.height / 2.0;
            display_index_for_point(&displays, cx, cy)
        })
        .unwrap_or(0);

    // 5. ターゲットディスプレイ
    let target_display_idx = match target_arg {
        Some("first") => 0,
        Some("second") => 1,
        _ => if current_display_idx == 0 { 1 } else { 0 },
    };

    if target_display_idx == current_display_idx {
        println!("既にターゲットディスプレイにいます");
        process::exit(0);
    }

    let target_display = &displays[target_display_idx];

    // 6. ターゲットウィンドウ検索
    let target = windows.iter().find(|w| {
        let cx = w.bounds.origin.x + w.bounds.size.width / 2.0;
        let cy = w.bounds.origin.y + w.bounds.size.height / 2.0;
        target_display.contains(cx, cy)
    });

    let target = match target {
        Some(t) => t,
        None => {
            eprintln!("ERROR: ターゲットディスプレイにウィンドウが見つかりません");
            process::exit(1);
        }
    };

    // 7. マウス移動
    let center_x = target.bounds.origin.x + target.bounds.size.width / 2.0;
    let center_y = target.bounds.origin.y + target.bounds.size.height / 2.0;
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
