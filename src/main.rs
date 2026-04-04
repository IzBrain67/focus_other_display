use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::dictionary::CFDictionaryRef;
use core_foundation::number::CFNumberRef;
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::display::CGRect;
use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use core_graphics::window::{
    kCGNullWindowID, kCGWindowBounds, kCGWindowLayer, kCGWindowListExcludeDesktopElements,
    kCGWindowListOptionOnScreenOnly, kCGWindowName, kCGWindowOwnerName, kCGWindowOwnerPID,
};
#[allow(unused_imports)]
use objc::{msg_send, sel, sel_impl};
use objc::runtime::{Class, Object, BOOL};
use std::ffi::c_void;
use std::process;

// --- AppKit リンク (CLIでもNSScreen等を使えるようにする) ---
#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {}

// --- Accessibility FFI ---
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

// --- CoreGraphics FFI ---
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGWindowListCopyWindowInfo(option: u32, relativeToWindow: u32) -> CFArrayRef;
    fn CGRectMakeWithDictionaryRepresentation(dict: CFDictionaryRef, rect: *mut CGRect) -> bool;
}

// --- Display info ---
struct DisplayInfo {
    x: f64,
    w: f64,
}

// --- Window info from CGWindowList ---
struct WindowInfo {
    pid: i32,
    owner_name: String,
    title: String,
    bounds: CGRect,
}

fn get_displays() -> Vec<DisplayInfo> {
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

fn get_frontmost_app() -> (i32, String) {
    unsafe {
        let ws_class = Class::get("NSWorkspace").unwrap();
        let workspace: *mut Object = msg_send![ws_class, sharedWorkspace];
        let front_app: *mut Object = msg_send![workspace, frontmostApplication];
        let pid: i32 = msg_send![front_app, processIdentifier];
        let name_ns: *mut Object = msg_send![front_app, localizedName];
        let name = nsstring_to_string(name_ns);
        (pid, name)
    }
}

unsafe fn nsstring_to_string(nsstr: *mut Object) -> String {
    if nsstr.is_null() {
        return String::new();
    }
    unsafe {
        let cstr: *const i8 = msg_send![nsstr, UTF8String];
        if cstr.is_null() {
            return String::new();
        }
        std::ffi::CStr::from_ptr(cstr)
            .to_string_lossy()
            .into_owned()
    }
}

unsafe fn cfstring_to_string(cfstr: CFTypeRef) -> String {
    if cfstr.is_null() {
        return String::new();
    }
    unsafe {
        let s = CFString::wrap_under_get_rule(cfstr as CFStringRef);
        s.to_string()
    }
}

unsafe fn cfdict_get_i32(dict: CFDictionaryRef, key: CFStringRef) -> Option<i32> {
    unsafe {
        let mut value: CFTypeRef = std::ptr::null();
        if core_foundation::dictionary::CFDictionaryGetValueIfPresent(
            dict,
            key as *const c_void,
            &mut value,
        ) != 0
            && !value.is_null()
        {
            let mut result: i32 = 0;
            if core_foundation::number::CFNumberGetValue(
                value as CFNumberRef,
                core_foundation::number::kCFNumberSInt32Type,
                &mut result as *mut i32 as *mut c_void,
            ) {
                return Some(result);
            }
        }
        None
    }
}

unsafe fn cfdict_get_string(dict: CFDictionaryRef, key: CFStringRef) -> String {
    unsafe {
        let mut value: CFTypeRef = std::ptr::null();
        if core_foundation::dictionary::CFDictionaryGetValueIfPresent(
            dict,
            key as *const c_void,
            &mut value,
        ) != 0
            && !value.is_null()
        {
            cfstring_to_string(value)
        } else {
            String::new()
        }
    }
}

fn get_windows() -> Vec<WindowInfo> {
    unsafe {
        let options = kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;
        let window_list = CGWindowListCopyWindowInfo(options, kCGNullWindowID);
        if window_list.is_null() {
            return Vec::new();
        }

        let count = CFArrayGetCount(window_list);
        let mut windows = Vec::new();

        for i in 0..count {
            let dict = CFArrayGetValueAtIndex(window_list, i) as CFDictionaryRef;

            let layer = cfdict_get_i32(dict, kCGWindowLayer as CFStringRef).unwrap_or(-1);
            if layer != 0 {
                continue;
            }

            let mut bounds = CGRect::default();
            let mut bounds_value: CFTypeRef = std::ptr::null();
            if core_foundation::dictionary::CFDictionaryGetValueIfPresent(
                dict,
                kCGWindowBounds as *const c_void,
                &mut bounds_value,
            ) == 0
                || bounds_value.is_null()
            {
                continue;
            }
            if !CGRectMakeWithDictionaryRepresentation(bounds_value as CFDictionaryRef, &mut bounds)
            {
                continue;
            }

            if bounds.size.width < 50.0 || bounds.size.height < 50.0 {
                continue;
            }

            let pid = cfdict_get_i32(dict, kCGWindowOwnerPID as CFStringRef).unwrap_or(0);
            let owner_name = cfdict_get_string(dict, kCGWindowOwnerName as CFStringRef);
            let title = cfdict_get_string(dict, kCGWindowName as CFStringRef);

            windows.push(WindowInfo {
                pid,
                owner_name,
                title,
                bounds,
            });
        }

        CFRelease(window_list as CFTypeRef);
        windows
    }
}

fn display_index_for_x(displays: &[DisplayInfo], center_x: f64) -> usize {
    for (i, d) in displays.iter().enumerate() {
        if center_x >= d.x && center_x < d.x + d.w {
            return i;
        }
    }
    0
}

fn ax_raise_window(pid: i32, target_bounds: &CGRect) -> bool {
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

fn move_mouse(x: f64, y: f64) {
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

fn main() {
    // 1. ディスプレイ情報
    let displays = get_displays();
    if displays.len() < 2 {
        eprintln!("ERROR: ディスプレイが1枚しか検出されません");
        process::exit(1);
    }

    // 2. フロントアプリ
    let (front_pid, front_app_name) = get_frontmost_app();

    // 3. 全ウィンドウ取得 (Z-order順)
    let windows = get_windows();

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
    move_mouse(center_x, center_y);

    // 8. AXRaise + frontmost
    if !ax_raise_window(target.pid, &target.bounds) {
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
