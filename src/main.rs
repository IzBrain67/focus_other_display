mod accessibility;
mod appkit;
mod cursor;
mod display;
mod window;

#[allow(unused_imports)]
use objc::{msg_send, sel, sel_impl};
use objc::runtime::{Class, Object, BOOL};
use core_graphics::display::CGRect;
use std::process;

pub struct DisplayInfo {
    pub x: f64,
    pub w: f64,
}

pub struct WindowInfo {
    pub pid: i32,
    pub owner_name: String,
    pub title: String,
    pub bounds: CGRect,
}

fn display_index_for_x(displays: &[DisplayInfo], center_x: f64) -> usize {
    for (i, d) in displays.iter().enumerate() {
        if center_x >= d.x && center_x < d.x + d.w {
            return i;
        }
    }
    0
}

fn main() {
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
            display_index_for_x(&displays, cx)
        })
        .unwrap_or(0);

    // 5. ターゲットディスプレイ
    let target_display_idx = if current_display_idx == 0 { 1 } else { 0 };
    let target_display = &displays[target_display_idx];

    // 6. ターゲットウィンドウ検索
    let target = windows.iter().find(|w| {
        let cx = w.bounds.origin.x + w.bounds.size.width / 2.0;
        let on_target = cx >= target_display.x && cx < target_display.x + target_display.w;

        // 現在のアプリのターゲットディスプレイ外のウィンドウはスキップ
        if w.pid == front_pid && !on_target {
            return false;
        }

        on_target
    });

    let target = match target {
        Some(t) => t,
        None => {
            eprintln!("ERROR: 反対側のディスプレイにウィンドウが見つかりません");
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
    let src_name = if current_display_idx == 0 { "左" } else { "右" };
    let dst_name = if target_display_idx == 0 { "左" } else { "右" };
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
