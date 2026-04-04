use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::display::CGRect;
use core_graphics::geometry::CGPoint;
use std::ffi::c_void;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
    fn AXUIElementCopyAttributeValue(
        element: CFTypeRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXUIElementPerformAction(element: CFTypeRef, action: CFStringRef) -> i32;
    fn AXUIElementSetAttributeValue(
        element: CFTypeRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> i32;
    fn AXValueGetValue(value: CFTypeRef, value_type: u32, value_ptr: *mut c_void) -> bool;
}

const AX_VALUE_TYPE_CGPOINT: u32 = 1;
const AX_VALUE_TYPE_CGSIZE: u32 = 2;

pub fn ax_raise_window(pid: i32, target_bounds: &CGRect) -> bool {
    unsafe {
        let app = AXUIElementCreateApplication(pid);
        if app.is_null() {
            return false;
        }

        let attr = CFString::new("AXWindows");
        let mut windows_ref: CFTypeRef = std::ptr::null();
        let err = AXUIElementCopyAttributeValue(app, attr.as_concrete_TypeRef(), &mut windows_ref);
        if err != 0 || windows_ref.is_null() {
            CFRelease(app);
            return false;
        }

        let count = CFArrayGetCount(windows_ref as CFArrayRef);
        let mut found = false;

        for i in 0..count {
            let win = CFArrayGetValueAtIndex(windows_ref as CFArrayRef, i);

            // Get AXPosition
            let pos_attr = CFString::new("AXPosition");
            let mut pos_ref: CFTypeRef = std::ptr::null();
            if AXUIElementCopyAttributeValue(win, pos_attr.as_concrete_TypeRef(), &mut pos_ref) != 0
            {
                continue;
            }
            let mut pos = CGPoint::new(0.0, 0.0);
            if !AXValueGetValue(
                pos_ref,
                AX_VALUE_TYPE_CGPOINT,
                &mut pos as *mut CGPoint as *mut c_void,
            ) {
                CFRelease(pos_ref);
                continue;
            }
            CFRelease(pos_ref);

            // Get AXSize
            let size_attr = CFString::new("AXSize");
            let mut size_ref: CFTypeRef = std::ptr::null();
            if AXUIElementCopyAttributeValue(win, size_attr.as_concrete_TypeRef(), &mut size_ref)
                != 0
            {
                continue;
            }
            let mut size = core_graphics::geometry::CGSize::new(0.0, 0.0);
            if !AXValueGetValue(
                size_ref,
                AX_VALUE_TYPE_CGSIZE,
                &mut size as *mut core_graphics::geometry::CGSize as *mut c_void,
            ) {
                CFRelease(size_ref);
                continue;
            }
            CFRelease(size_ref);

            // Match by position (within tolerance)
            let dx = (pos.x - target_bounds.origin.x).abs();
            let dy = (pos.y - target_bounds.origin.y).abs();
            let dw = (size.width - target_bounds.size.width).abs();
            let dh = (size.height - target_bounds.size.height).abs();

            if dx < 5.0 && dy < 5.0 && dw < 5.0 && dh < 5.0 {
                // AXRaise
                let raise_action = CFString::new("AXRaise");
                AXUIElementPerformAction(win, raise_action.as_concrete_TypeRef());

                // Set app frontmost
                let frontmost_attr = CFString::new("AXFrontmost");
                let true_val =
                    core_foundation::boolean::CFBoolean::true_value().as_CFTypeRef();
                AXUIElementSetAttributeValue(
                    app,
                    frontmost_attr.as_concrete_TypeRef(),
                    true_val,
                );

                found = true;
                break;
            }
        }

        CFRelease(windows_ref);
        CFRelease(app);
        found
    }
}
