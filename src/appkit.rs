#[allow(unused_imports)]
use objc::{msg_send, sel, sel_impl};
use objc::runtime::{Class, Object};

// CLIでもNSScreen等を使えるようにする
#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {}

pub fn get_frontmost_app() -> (i32, String) {
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

pub unsafe fn nsstring_to_string(nsstr: *mut Object) -> String {
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
