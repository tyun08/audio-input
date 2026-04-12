use objc::{class, msg_send, sel, sel_impl};
use std::os::raw::{c_char, c_void};
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter, Manager};
use tracing::{info, warn};

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

// Raw ObjC runtime — always available on macOS
extern "C" {
    fn objc_allocateClassPair(
        superclass: *const c_void,
        name: *const c_char,
        extra_bytes: usize,
    ) -> *mut c_void;
    fn objc_registerClassPair(cls: *mut c_void);
    fn class_addMethod(
        cls: *mut c_void,
        name: *const c_void,
        imp: *const c_void,
        types: *const c_char,
    ) -> bool;
}

/// Register this app as a macOS service provider so "Polish with Audio Input"
/// appears in the right-click Services submenu when text is selected.
pub fn register_service_provider(handle: AppHandle) {
    let _ = APP_HANDLE.set(handle);
    unsafe { do_register() };
}

unsafe fn do_register() {
    use std::ffi::CString;

    let superclass = class!(NSObject) as *const objc::runtime::Class as *const c_void;
    let cls_name = CString::new("AudioInputServiceProvider").unwrap();
    let cls = objc_allocateClassPair(superclass, cls_name.as_ptr(), 0);

    if cls.is_null() {
        warn!("objc_allocateClassPair returned null — class may already be registered");
    } else {
        let the_sel = sel!(polishSelectedText:userData:error:);
        // Encoding: void return, id self, SEL _cmd, id pboard, id userData, id error
        let types = CString::new("v@:@@@").unwrap();
        let sel_ptr: *const c_void = std::mem::transmute(the_sel);
        class_addMethod(cls, sel_ptr, polish_handler as *const c_void, types.as_ptr());
        objc_registerClassPair(cls);
    }

    let Some(cls_ref) = objc::runtime::Class::get("AudioInputServiceProvider") else {
        warn!("Could not find AudioInputServiceProvider class after registration");
        return;
    };

    let instance: *mut objc::runtime::Object = msg_send![cls_ref, new];
    if instance.is_null() {
        warn!("Failed to allocate service provider instance");
        return;
    }

    let ns_app: *mut objc::runtime::Object =
        msg_send![class!(NSApplication), sharedApplication];
    let _: () = msg_send![ns_app, setServicesProvider: instance];

    // Raw pointer — no Rust drop, ObjC retain count stays at +2 (new + setServicesProvider).
    // The NSApplication holds a strong reference; this is intentional.

    info!("macOS Services provider registered — right-click text → Services → Polish with Audio Input");
}

extern "C" fn polish_handler(
    _this: *mut c_void,
    _sel: *const c_void,
    pboard: *mut c_void,
    _user_data: *mut c_void,
    _error: *mut c_void,
) {
    use std::ffi::{CStr, CString};

    let Some(handle) = APP_HANDLE.get() else {
        warn!("Service called but AppHandle not initialized");
        return;
    };

    // --- Read selected text from the pasteboard ---
    let text: String = unsafe {
        let pboard_obj = pboard as *mut objc::runtime::Object;
        let types_to_try: &[&[u8]] = &[b"NSStringPboardType\0", b"public.utf8-plain-text\0"];
        let mut found: Option<String> = None;

        for pb_type_bytes in types_to_try {
            let pb_type: *mut objc::runtime::Object =
                msg_send![class!(NSString), stringWithUTF8String: pb_type_bytes.as_ptr()];
            let text_obj: *mut objc::runtime::Object =
                msg_send![pboard_obj, stringForType: pb_type];
            if !text_obj.is_null() {
                let ptr: *const c_char = msg_send![text_obj, UTF8String];
                if !ptr.is_null() {
                    if let Ok(s) = CStr::from_ptr(ptr).to_str() {
                        if !s.is_empty() {
                            found = Some(s.to_owned());
                            break;
                        }
                    }
                }
            }
        }

        match found {
            Some(s) => s,
            None => {
                warn!("Service: no string found on pasteboard");
                return;
            }
        }
    };

    info!("Service: polishing {} chars", text.chars().count());

    // --- Get provider config ---
    let (provider, pcfg) = {
        let config =
            handle.state::<std::sync::Arc<std::sync::Mutex<crate::config::AppConfig>>>();
        let cfg = config.lock().unwrap();
        (cfg.provider.clone(), cfg.get_pcfg(&cfg.provider))
    };

    // --- Polish via a fresh single-threaded tokio runtime (service handler is sync) ---
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let Ok(rt) = rt else {
        warn!("Service: failed to build tokio runtime");
        return;
    };

    let (polished, failed) =
        rt.block_on(crate::commands::polish_with_provider(&provider, &pcfg, &text, None));

    if failed {
        warn!("Service: polish API failed — text unchanged");
        return;
    }

    // --- Write polished text back to the pasteboard ---
    unsafe {
        let pboard_obj = pboard as *mut objc::runtime::Object;
        let Ok(cstr) = CString::new(polished.as_str()) else {
            warn!("Service: polished text contains null bytes");
            return;
        };
        let ns_str: *mut objc::runtime::Object =
            msg_send![class!(NSString), stringWithUTF8String: cstr.as_ptr()];
        if ns_str.is_null() {
            return;
        }

        let pb_type: *mut objc::runtime::Object = {
            let bytes = b"NSStringPboardType\0";
            msg_send![class!(NSString), stringWithUTF8String: bytes.as_ptr()]
        };

        let _: () = msg_send![pboard_obj, clearContents];
        let _: objc::runtime::BOOL =
            msg_send![pboard_obj, setString: ns_str forType: pb_type];
    }

    info!("Service: polish complete ({} chars)", polished.chars().count());
    let _ = handle.emit("transcription-result", &polished);
}
