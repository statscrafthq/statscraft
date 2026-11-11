//! Files Finder asks the app to open on macOS, by a double-click or Open With. AppKit hands them
//! to the application delegate's `application:openURLs:`, which winit 0.30 does not implement and
//! offers no hook for. So when the app is about to finish launching, before AppKit delivers the
//! first files, the method is added to winit's delegate class. The files wait in a queue that
//! the window takes like files dropped on it.

use eframe::egui;
use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSApplication, NSApplicationWillFinishLaunchingNotification};
use objc2_foundation::{NSArray, NSNotification, NSNotificationCenter, NSURL};
use std::path::PathBuf;
use std::ptr::NonNull;
use std::sync::{Mutex, OnceLock};

static QUEUE: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
/// The window's context, to wake it when files arrive while it waits for input.
static CONTEXT: OnceLock<egui::Context> = OnceLock::new();

/// Starts listening; called before the event loop runs.
pub fn listen() {
    let block = block2::RcBlock::new(|_: NonNull<NSNotification>| add_method());
    let center = NSNotificationCenter::defaultCenter();
    // SAFETY: the name is AppKit's constant, and the block is called on the main thread
    let observer = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(NSApplicationWillFinishLaunchingNotification),
            None,
            None,
            &block,
        )
    };
    // the observer stays for the life of the app
    std::mem::forget(observer);
}

/// Wakes the window when files arrive.
pub fn set_context(ctx: &egui::Context) {
    let _ = CONTEXT.set(ctx.clone());
}

/// The files that arrived since the last call.
pub fn take() -> Vec<PathBuf> {
    QUEUE
        .lock()
        .map(|mut q| std::mem::take(&mut *q))
        .unwrap_or_default()
}

fn add_method() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    let Some(delegate) = app.delegate() else {
        return;
    };
    let object: &AnyObject = (*delegate).as_ref();
    let class: &AnyClass = object.class();
    let open: unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject, &NSArray<NSURL>) = open_urls;
    // SAFETY: the function takes the receiver, the selector, the application and an array of
    // URLs, which the type encoding "v@:@@" (void; object, selector, object, object) declares
    unsafe {
        let imp: Imp = std::mem::transmute(open);
        objc2::ffi::class_addMethod(
            class as *const AnyClass as *mut AnyClass,
            sel!(application:openURLs:),
            imp,
            c"v@:@@".as_ptr(),
        );
    }
    // AppKit may note which methods a delegate has when it is set, so set it again
    app.setDelegate(Some(&delegate));
}

unsafe extern "C-unwind" fn open_urls(
    _delegate: &AnyObject,
    _cmd: Sel,
    _app: &AnyObject,
    urls: &NSArray<NSURL>,
) {
    let paths: Vec<PathBuf> = urls.iter().filter_map(|u| u.to_file_path()).collect();
    if let Ok(mut queue) = QUEUE.lock() {
        queue.extend(paths);
    }
    if let Some(ctx) = CONTEXT.get() {
        ctx.request_repaint();
    }
}
