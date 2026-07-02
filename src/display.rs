use core_graphics::display::CGDisplay;

use crate::DisplayInfo;

/// index 0 = メインディスプレイ(メニューバーのある画面)、index 1 以降 = サブ
pub fn get_displays() -> Vec<DisplayInfo> {
    let main_id = CGDisplay::main().id;
    let mut ids = CGDisplay::active_displays().unwrap_or_default();
    ids.sort_by_key(|&id| if id == main_id { 0 } else { 1 });
    ids.into_iter()
        .map(|id| DisplayInfo {
            bounds: CGDisplay::new(id).bounds(),
        })
        .collect()
}
