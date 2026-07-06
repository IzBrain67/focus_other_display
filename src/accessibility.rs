use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::display::CGRect;
use core_graphics::geometry::{CGPoint, CGSize};
use std::ffi::c_void;
use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::debug_log;

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

/// CGWindowList と AX の座標のわずかなズレを許容する
const BOUNDS_TOLERANCE: f64 = 5.0;

fn rect_matches(a: &CGRect, b: &CGRect) -> bool {
    (a.origin.x - b.origin.x).abs() < BOUNDS_TOLERANCE
        && (a.origin.y - b.origin.y).abs() < BOUNDS_TOLERANCE
        && (a.size.width - b.size.width).abs() < BOUNDS_TOLERANCE
        && (a.size.height - b.size.height).abs() < BOUNDS_TOLERANCE
}

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

/// アプリ要素の AXFrontmost を読む。
/// NSWorkspace と違い、run loop を回さない CLI プロセスでも常に最新の状態が返る。
unsafe fn ax_app_is_frontmost(app: CFTypeRef) -> bool {
    unsafe {
        let attr = CFString::new("AXFrontmost");
        let mut value: CFTypeRef = std::ptr::null();
        if AXUIElementCopyAttributeValue(app, attr.as_concrete_TypeRef(), &mut value) != 0
            || value.is_null()
        {
            return false;
        }
        // kCFBooleanTrue はシングルトンなのでポインタ比較で判定できる
        let is_true = value == CFBoolean::true_value().as_CFTypeRef();
        CFRelease(value);
        is_true
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

        // 位置・サイズの近似一致でターゲットの AX ウィンドウ要素を特定する
        // (要素は Get ルールなので release 禁止。使用中は windows_ref を保持し続ける)
        let mut target_win: CFTypeRef = std::ptr::null();
        for i in 0..count {
            let win = CFArrayGetValueAtIndex(windows_ref as CFArrayRef, i);
            let Some(rect) = ax_window_rect(win) else {
                continue;
            };
            if rect_matches(&rect, target_bounds) {
                target_win = win;
                break;
            }
        }

        if target_win.is_null() {
            debug_log(&format!(
                "AXWindows {count} 個に bounds が一致するウィンドウなし"
            ));
            CFRelease(windows_ref);
            CFRelease(app);
            return false;
        }

        let cf_true = CFBoolean::true_value();
        let frontmost_attr = CFString::new("AXFrontmost");
        let main_attr = CFString::new("AXMain");
        let raise_action = CFString::new("AXRaise");
        let start = Instant::now();

        // AXMain + AXRaise を設定してからアクティブ化する。
        // アプリがこれを尊重すればターゲットが直接キーウィンドウになり、
        // 前回のキーウィンドウが一瞬フォーカスされるのを避けられる
        AXUIElementSetAttributeValue(
            target_win,
            main_attr.as_concrete_TypeRef(),
            cf_true.as_CFTypeRef(),
        );
        AXUIElementPerformAction(target_win, raise_action.as_concrete_TypeRef());
        AXUIElementSetAttributeValue(
            app,
            frontmost_attr.as_concrete_TypeRef(),
            cf_true.as_CFTypeRef(),
        );

        // アクティベーションは非同期に完了し、完了時にアプリが前回のキーウィンドウを
        // 復元して AXMain を上書きすることがある(Chrome 等)。
        // フォーカスがターゲットに載ったと確認できるまで AXMain + AXRaise を再設定し続ける
        let mut confirmed = false;
        for attempt in 1..=40 {
            if ax_app_is_frontmost(app)
                && get_focused_window_bounds(pid).is_some_and(|b| rect_matches(&b, target_bounds))
            {
                debug_log(&format!(
                    "フォーカス確定 (attempt {attempt}, {:?})",
                    start.elapsed()
                ));
                confirmed = true;
                break;
            }
            AXUIElementSetAttributeValue(
                target_win,
                main_attr.as_concrete_TypeRef(),
                cf_true.as_CFTypeRef(),
            );
            AXUIElementPerformAction(target_win, raise_action.as_concrete_TypeRef());
            sleep(Duration::from_millis(10));
        }
        if !confirmed {
            debug_log(&format!(
                "フォーカスを確認できないままタイムアウト ({:?})",
                start.elapsed()
            ));
        }

        CFRelease(windows_ref);
        CFRelease(app);
        // ウィンドウを特定して raise まで実行できていれば true。
        // ここで false を返すと main 側が activateWithOptions にフォールバックし、
        // かえって別ウィンドウ(前回キーウィンドウ)を前面化してしまう
        true
    }
}
