/// Passive ⌘V key observer.
///
/// Uses `NSEvent.addGlobalMonitorForEventsMatchingMask:handler:` which observes
/// keyboard events sent to *other* applications without intercepting them.
/// When a Command+V keydown is detected the Tauri event `"paste-detected"` is
/// emitted so the front-end can auto-dismiss the injection-failed HUD.
///
/// The monitor is automatically stopped when the returned `PasteMonitorHandle`
/// is dropped (or `.stop()` is called).
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub struct PasteMonitorHandle {
    stop: Arc<AtomicBool>,
}

impl PasteMonitorHandle {
    #[allow(dead_code)]
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for PasteMonitorHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

// Safety: PasteMonitorHandle only holds an Arc<AtomicBool>, which is Send+Sync.
unsafe impl Send for PasteMonitorHandle {}
unsafe impl Sync for PasteMonitorHandle {}

#[cfg(target_os = "macos")]
pub fn start<R: tauri::Runtime + 'static>(app: tauri::AppHandle<R>) -> PasteMonitorHandle {
    use block::ConcreteBlock;
    use objc::{class, msg_send, runtime::Object, sel, sel_impl};
    use std::ffi::c_void;
    use tauri::Emitter as _;
    use tracing::{info, warn};

    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = Arc::clone(&stop);

    std::thread::Builder::new()
        .name("paste-monitor".into())
        .spawn(move || {
            // NSEventMaskKeyDown = 1 << 10
            let mask: u64 = 1 << 10;

            let app_clone = app.clone();
            let stop_inner = Arc::clone(&stop_thread);

            // The block is called on this thread's run loop for every KeyDown
            // event in any application.
            let block = ConcreteBlock::new(move |event: *mut Object| {
                if stop_inner.load(Ordering::Relaxed) {
                    return;
                }
                unsafe {
                    let modifier_flags: u64 = msg_send![event, modifierFlags];
                    let key_code: u16 = msg_send![event, keyCode];
                    // NSEventModifierFlagCommand = 0x100000, kVK_ANSI_V = 9
                    if key_code == 9 && (modifier_flags & 0x100_000) != 0 {
                        let _ = app_clone.emit("paste-detected", ());
                    }
                }
            });
            let block = block.copy();

            let monitor: *mut Object = unsafe {
                msg_send![
                    class!(NSEvent),
                    addGlobalMonitorForEventsMatchingMask: mask
                    handler: &*block
                ]
            };

            if monitor.is_null() {
                warn!("paste monitor: addGlobalMonitorForEventsMatchingMask returned nil (accessibility permission required)");
                return;
            }

            #[link(name = "CoreFoundation", kind = "framework")]
            extern "C" {
                fn CFRunLoopGetCurrent() -> *mut c_void;
                fn CFRunLoopRunInMode(
                    mode: *const c_void,
                    seconds: f64,
                    return_after_source_handled: bool,
                ) -> i32;
                static kCFRunLoopDefaultMode: *const c_void;
            }

            unsafe {
                let _rl = CFRunLoopGetCurrent(); // ensure this thread has a run loop
            }
            info!("paste monitor started");

            while !stop_thread.load(Ordering::Relaxed) {
                unsafe {
                    CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.1, false);
                }
            }

            unsafe {
                let _: () = msg_send![class!(NSEvent), removeMonitor: monitor];
            }
            info!("paste monitor stopped");
        })
        .ok();

    PasteMonitorHandle { stop }
}

#[cfg(not(target_os = "macos"))]
pub fn start<R: tauri::Runtime>(_app: tauri::AppHandle<R>) -> PasteMonitorHandle {
    PasteMonitorHandle {
        stop: Arc::new(AtomicBool::new(false)),
    }
}
