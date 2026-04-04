#[allow(unused_imports)]
use objc::{msg_send, sel, sel_impl};
use objc::runtime::{Class, Object};
use core_graphics::display::CGRect;

use crate::DisplayInfo;

pub fn get_displays() -> Vec<DisplayInfo> {
    unsafe {
        let ns_screen_class = Class::get("NSScreen").unwrap();
        let screens: *mut Object = msg_send![ns_screen_class, screens];
        let count: usize = msg_send![screens, count];

        let mut displays = Vec::new();
        for i in 0..count {
            let screen: *mut Object = msg_send![screens, objectAtIndex: i];
            let frame: CGRect = msg_send![screen, frame];
            displays.push(DisplayInfo {
                x: frame.origin.x,
                w: frame.size.width,
            });
        }
        displays.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
        displays
    }
}
