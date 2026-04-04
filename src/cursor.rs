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
