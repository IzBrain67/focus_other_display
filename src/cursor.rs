use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;

pub fn move_mouse(x: f64, y: f64) {
    let point = CGPoint::new(x, y);
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState);
    if let Ok(source) = source {
        if let Ok(event) =
            CGEvent::new_mouse_event(source, CGEventType::MouseMoved, point, CGMouseButton::Left)
        {
            event.post(CGEventTapLocation::HID);
        }
    }
}

/// 現在のマウスカーソル位置(CG グローバル座標)
pub fn mouse_position() -> Option<(f64, f64)> {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).ok()?;
    let event = CGEvent::new(source).ok()?;
    let p = event.location();
    Some((p.x, p.y))
}

/// 左クリックを合成する。デスクトップのクリックでディスプレイ自体をフォーカスするために使う
pub fn click_mouse(x: f64, y: f64) {
    let point = CGPoint::new(x, y);
    for event_type in [CGEventType::LeftMouseDown, CGEventType::LeftMouseUp] {
        if let Ok(source) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
            if let Ok(event) =
                CGEvent::new_mouse_event(source, event_type, point, CGMouseButton::Left)
            {
                event.post(CGEventTapLocation::HID);
            }
        }
    }
}
