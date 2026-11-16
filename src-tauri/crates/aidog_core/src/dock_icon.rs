//! macOS Dock 图标随系统明暗切换（运行期）。
//!
//! bundle 里的 icon.icns 是 dark 版（Finder / 未运行时的 Dock 恒为它）；进程运行期间按
//! `NSApp.effectiveAppearance` 设 `applicationIconImage`，并对该属性做 KVO，系统切换明暗即换图。
//! 应用本身不强制 `NSApp.appearance`，所以 effectiveAppearance 就是系统明暗。

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{AnyThread, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication, NSImage};
use objc2_foundation::{
    MainThreadMarker, NSArray, NSData, NSKeyValueObservingOptions,
    NSObjectNSKeyValueObserverRegistration, NSString,
};
use std::cell::RefCell;
use std::ffi::c_void;

const LIGHT_PNG: &[u8] = include_bytes!("../../../icons/dock-light.png");
const DARK_PNG: &[u8] = include_bytes!("../../../icons/dock-dark.png");

thread_local! {
    static OBSERVER: RefCell<Option<Retained<Observer>>> = const { RefCell::new(None) };
}

define_class!(
    // SAFETY: 超类 NSObject 无子类化要求；本类型不实现 Drop，无 ivars。
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "AidogDockIconObserver"]
    struct Observer;

    impl Observer {
        #[unsafe(method(observeValueForKeyPath:ofObject:change:context:))]
        fn observe(
            &self,
            _key_path: *mut NSString,
            _object: *mut AnyObject,
            _change: *mut AnyObject,
            _context: *mut c_void,
        ) {
            apply(self.mtm());
        }
    }
);

/// 解码内嵌 PNG。`pub` 供测试校验两张图可被 AppKit 解码。
pub fn image(dark: bool) -> Option<Retained<NSImage>> {
    let data = NSData::with_bytes(if dark { DARK_PNG } else { LIGHT_PNG });
    NSImage::initWithData(NSImage::alloc(), &data)
}

fn apply(mtm: MainThreadMarker) {
    let app = NSApplication::sharedApplication(mtm);
    // SAFETY: 两个常量是 AppKit 导出的静态 NSString。
    let names = unsafe { NSArray::from_slice(&[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]) };
    let best = app
        .effectiveAppearance()
        .bestMatchFromAppearancesWithNames(&names);
    let dark = best.is_some_and(|n| unsafe { n.isEqualToString(NSAppearanceNameDarkAqua) });
    // SAFETY: 主线程（mtm 保证），参数为有效 NSImage 或 None。
    unsafe { app.setApplicationIconImage(image(dark).as_deref()) };
}

/// 按当前明暗设一次 Dock 图标并开始监听切换。必须在主线程调用；重复调用只重设图标。
pub fn install() {
    let Some(mtm) = MainThreadMarker::new() else {
        tracing::error!("dock_icon::install off main thread, skipped");
        return;
    };
    apply(mtm);
    if OBSERVER.with(|o| o.borrow().is_some()) {
        return;
    }
    let observer: Retained<Observer> = unsafe { msg_send![Observer::alloc(mtm), init] };
    // SAFETY: observer 存进 thread_local 常驻进程生命周期，不会在被观察期间释放。
    unsafe {
        NSApplication::sharedApplication(mtm).addObserver_forKeyPath_options_context(
            &observer,
            &NSString::from_str("effectiveAppearance"),
            NSKeyValueObservingOptions::empty(),
            std::ptr::null_mut(),
        );
    }
    OBSERVER.with(|o| *o.borrow_mut() = Some(observer));
}

#[cfg(test)]
mod tests {
    #[test]
    fn embedded_pngs_decode() {
        for dark in [false, true] {
            let img = super::image(dark).expect("png decodes");
            let size = img.size();
            assert!(
                size.width > 0.0 && size.height > 0.0,
                "dark={dark} size={size:?}"
            );
        }
    }
}
