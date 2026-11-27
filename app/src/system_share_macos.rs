//! AppKit owns the picker UI; this delegate owns the PNG until completion.
#![allow(unsafe_code)]
use super::{Anchor, ShareFile};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send,
    rc::Retained, runtime::ProtocolObject,
};
use objc2_app_kit::{
    NSSharingService, NSSharingServiceDelegate, NSSharingServicePicker,
    NSSharingServicePickerDelegate, NSView,
};
use objc2_foundation::{
    NSArray, NSError, NSObject, NSObjectProtocol, NSPoint, NSRect, NSRectEdge, NSSize, NSString,
    NSURL,
};
use std::{cell::RefCell, sync::mpsc::Sender};
use tauri::Manager;

struct State {
    file: RefCell<Option<ShareFile>>,
    result: RefCell<Option<Sender<Result<(), String>>>>,
    picker: RefCell<Option<Retained<NSSharingServicePicker>>>,
}
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "ScribbleCapShareDelegate"]
    #[ivars = State]
    struct Delegate;
    unsafe impl NSObjectProtocol for Delegate {}
    unsafe impl NSSharingServicePickerDelegate for Delegate {
        #[unsafe(method(sharingServicePicker:didChooseSharingService:))]
        fn chosen(&self, _picker: &NSSharingServicePicker, service: Option<&NSSharingService>) {
            if service.is_none() {
                self.finish(Ok(()));
            }
        }
        #[unsafe(method_id(sharingServicePicker:delegateForSharingService:))]
        fn service_delegate(
            &self,
            _picker: &NSSharingServicePicker,
            _service: &NSSharingService,
        ) -> Option<Retained<ProtocolObject<dyn NSSharingServiceDelegate>>> {
            Some(ProtocolObject::from_retained(self.retain()))
        }
    }
    unsafe impl NSSharingServiceDelegate for Delegate {
        #[unsafe(method(sharingService:didShareItems:))]
        unsafe fn shared(&self, _service: &NSSharingService, _items: &NSArray) {
            self.finish(Ok(()));
        }
        #[unsafe(method(sharingService:didFailToShareItems:error:))]
        unsafe fn failed(&self, _service: &NSSharingService, _items: &NSArray, error: &NSError) {
            // Cocoa's user-cancelled error is a silent cancellation.
            if error.domain().to_string() == "NSCocoaErrorDomain" && error.code() == 3072 {
                self.finish(Ok(()));
            } else {
                self.finish(Err(format!(
                    "The image could not be shared: {}",
                    error.localizedDescription()
                )));
            }
        }
    }
);
impl Delegate {
    fn finish(&self, result: Result<(), String>) {
        self.ivars().file.borrow_mut().take();
        if let Some(sender) = self.ivars().result.borrow_mut().take() {
            let _ = sender.send(result);
        }
    }
}
// Keep weak AppKit delegates alive. Completed delegates are pruned on the next
// share, never during their own callback. Normal process exit clears the pool.
thread_local! { static DELEGATES: RefCell<Vec<Retained<Delegate>>> = const { RefCell::new(Vec::new()) }; }

pub(super) fn show(
    app: &tauri::AppHandle,
    file: ShareFile,
    anchor: Anchor,
    sender: Sender<Result<(), String>>,
) {
    let result = (|| {
        let mtm = MainThreadMarker::new().ok_or("The share picker requires the main thread.")?;
        let window = app
            .get_webview_window("main")
            .ok_or("The editor window is unavailable.")?;
        let pointer = window.ns_view().map_err(|e| e.to_string())?;
        // SAFETY: Tauri supplies its live NSView on the AppKit main thread.
        let view = unsafe { (pointer as *const NSView).as_ref() }
            .ok_or("The editor view is unavailable.")?;
        let url = NSURL::fileURLWithPath(&NSString::from_str(
            file.path
                .to_str()
                .ok_or("The temporary file path is invalid.")?,
        ));
        let items = NSArray::from_slice(&[url.as_ref() as &objc2::runtime::AnyObject]);
        // SAFETY: NSURL implements NSPasteboardWriting, required by the picker.
        let picker = unsafe {
            NSSharingServicePicker::initWithItems(NSSharingServicePicker::alloc(), &items)
        };
        let state = State {
            file: RefCell::new(Some(file)),
            result: RefCell::new(Some(sender.clone())),
            picker: RefCell::new(Some(picker.clone())),
        };
        // SAFETY: NSObject initialization on its declared main thread.
        let delegate: Retained<Delegate> =
            unsafe { msg_send![super(Delegate::alloc(mtm).set_ivars(state)), init] };
        picker.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        let bounds = view.bounds();
        let y = if view.isFlipped() {
            anchor.y
        } else {
            bounds.size.height - anchor.y - anchor.height
        };
        let rect = NSRect::new(
            NSPoint::new(anchor.x, y),
            NSSize::new(anchor.width, anchor.height),
        );
        DELEGATES.with_borrow_mut(|delegates| {
            delegates.retain(|d| d.ivars().file.borrow().is_some());
            delegates.push(delegate);
        });
        picker.showRelativeToRect_ofView_preferredEdge(rect, view, NSRectEdge::MinY);
        Ok(())
    })();
    if let Err(error) = result {
        let _ = sender.send(Err(error));
    }
}
pub(super) fn cleanup() {
    DELEGATES.with_borrow_mut(|delegates| {
        for delegate in delegates.iter() {
            if let Some(picker) = delegate.ivars().picker.borrow().as_ref() {
                picker.setDelegate(None);
                picker.close();
            }
            delegate.finish(Ok(()));
        }
        delegates.clear();
    });
}
