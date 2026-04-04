use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex};
use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::dictionary::CFDictionaryRef;
use core_foundation::number::CFNumberRef;
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::display::CGRect;
use core_graphics::window::{
    kCGNullWindowID, kCGWindowBounds, kCGWindowLayer, kCGWindowListExcludeDesktopElements,
    kCGWindowListOptionOnScreenOnly, kCGWindowName, kCGWindowOwnerName, kCGWindowOwnerPID,
};
use std::ffi::c_void;

use core_foundation::array::CFArrayRef;

use crate::WindowInfo;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGWindowListCopyWindowInfo(option: u32, relativeToWindow: u32) -> CFArrayRef;
    fn CGRectMakeWithDictionaryRepresentation(dict: CFDictionaryRef, rect: *mut CGRect) -> bool;
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

pub fn get_windows() -> Vec<WindowInfo> {
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
