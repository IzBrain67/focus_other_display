use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::display::CGRect;
use core_graphics::geometry::{CGPoint, CGSize};
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

/// AX ウィンドウ要素の AXPosition/AXSize から CGRect を構成する。
/// 座標は CGWindowList と同じ CG 座標系(グローバル・左上原点)。
/// win は借用として扱い、この関数内では release しない。
unsafe fn ax_window_rect(win: CFTypeRef) -> Option<CGRect> {
    unsafe {
        let pos_attr = CFString::new("AXPosition");
        let mut pos_ref: CFTypeRef = std::ptr::null();
        if AXUIElementCopyAttributeValue(win, pos_attr.as_concrete_TypeRef(), &mut pos_ref) != 0
            || pos_ref.is_null()
        {
            return None;
        }
        let mut pos = CGPoint::new(0.0, 0.0);
        let pos_ok = AXValueGetValue(
            pos_ref,
            AX_VALUE_TYPE_CGPOINT,
            &mut pos as *mut CGPoint as *mut c_void,
        );
        CFRelease(pos_ref);
        if !pos_ok {
            return None;
        }

        let size_attr = CFString::new("AXSize");
        let mut size_ref: CFTypeRef = std::ptr::null();
        if AXUIElementCopyAttributeValue(win, size_attr.as_concrete_TypeRef(), &mut size_ref) != 0
            || size_ref.is_null()
        {
            return None;
        }
        let mut size = CGSize::new(0.0, 0.0);
        let size_ok = AXValueGetValue(
            size_ref,
            AX_VALUE_TYPE_CGSIZE,
            &mut size as *mut CGSize as *mut c_void,
        );
        CFRelease(size_ref);
        if !size_ok {
            return None;
        }

        Some(CGRect::new(&pos, &size))
    }
}

/// フロントアプリの AXFocusedWindow(キーボードフォーカスを持つウィンドウ)の bounds。
/// アクセシビリティ権限がない・アプリが AX 未対応などの場合は None。
pub fn get_focused_window_bounds(pid: i32) -> Option<CGRect> {
    unsafe {
        let app = AXUIElementCreateApplication(pid);
        if app.is_null() {
            return None;
        }
        let attr = CFString::new("AXFocusedWindow");
        let mut win_ref: CFTypeRef = std::ptr::null();
        let err = AXUIElementCopyAttributeValue(app, attr.as_concrete_TypeRef(), &mut win_ref);
        if err != 0 || win_ref.is_null() {
            CFRelease(app);
            return None;
        }
        let rect = ax_window_rect(win_ref);
        // AXFocusedWindow は Copy ルールで返るため release が必要
        // (AXWindows 配列の要素は Get ルールで release 禁止)
        CFRelease(win_ref);
        CFRelease(app);
        rect
    }
}

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
            let Some(rect) = ax_window_rect(win) else {
                continue;
            };

            // Match by position (within tolerance)
            let dx = (rect.origin.x - target_bounds.origin.x).abs();
            let dy = (rect.origin.y - target_bounds.origin.y).abs();
            let dw = (rect.size.width - target_bounds.size.width).abs();
            let dh = (rect.size.height - target_bounds.size.height).abs();

            if dx < 5.0 && dy < 5.0 && dw < 5.0 && dh < 5.0 {
                let cf_true = core_foundation::boolean::CFBoolean::true_value();

                // 先にアプリを frontmost にする。背面のまま AXMain を設定しても、
                // アクティベーション時にアプリが前回のキーウィンドウを復元して上書きされるため
                let frontmost_attr = CFString::new("AXFrontmost");
                AXUIElementSetAttributeValue(
                    app,
                    frontmost_attr.as_concrete_TypeRef(),
                    cf_true.as_CFTypeRef(),
                );

                // AXMain: 最前面のアプリ内でキーウィンドウを切り替える
                let main_attr = CFString::new("AXMain");
                AXUIElementSetAttributeValue(
                    win,
                    main_attr.as_concrete_TypeRef(),
                    cf_true.as_CFTypeRef(),
                );

                // AXRaise
                let raise_action = CFString::new("AXRaise");
                AXUIElementPerformAction(win, raise_action.as_concrete_TypeRef());

                found = true;
                break;
            }
        }

        CFRelease(windows_ref);
        CFRelease(app);
        found
    }
}
