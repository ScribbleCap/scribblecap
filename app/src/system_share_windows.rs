//! HWND-bound Windows share UI, with an asynchronous StorageFile data request.
#![allow(unsafe_code)]
use super::{Anchor, ShareFile};
use std::{
    cell::RefCell,
    sync::{Arc, Mutex, mpsc::Sender},
};
use tauri::Manager;
use windows::{
    ApplicationModel::DataTransfer::{DataRequestedEventArgs, DataTransferManager},
    Foundation::TypedEventHandler,
    Storage::{IStorageItem, StorageFile},
    Win32::UI::Shell::IDataTransferManagerInterop,
    core::{HSTRING, Interface, factory},
};

struct Pending {
    file: Option<ShareFile>,
    sender: Option<Sender<Result<(), String>>>,
}
impl Pending {
    fn finish(&mut self, result: Result<(), String>) {
        self.file.take();
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(result);
        }
    }
}
struct Registration {
    manager: DataTransferManager,
    token: i64,
    pending: Arc<Mutex<Pending>>,
}
impl Drop for Registration {
    fn drop(&mut self) {
        let _ = self.manager.RemoveDataRequested(self.token);
    }
}
thread_local! { static REQUESTS: RefCell<Vec<Registration>> = const { RefCell::new(Vec::new()) }; }

pub(super) fn show(
    app: &tauri::AppHandle,
    file: ShareFile,
    _anchor: Anchor,
    sender: Sender<Result<(), String>>,
) {
    let pending = Arc::new(Mutex::new(Pending {
        file: Some(file),
        sender: Some(sender),
    }));
    let result = (|| -> Result<(), String> {
        let window = app
            .get_webview_window("main")
            .ok_or("The editor window is unavailable.")?;
        let hwnd = window.hwnd().map_err(|e| e.to_string())?;
        let interop: IDataTransferManagerInterop =
            factory::<DataTransferManager, _>().map_err(|e| e.to_string())?;
        // SAFETY: HWND belongs to the live main editor; this runs on its UI thread.
        let manager: DataTransferManager =
            unsafe { interop.GetForWindow(hwnd) }.map_err(|e| e.to_string())?;
        let state = pending.clone();
        let token = manager
            .DataRequested(&TypedEventHandler::<
                DataTransferManager,
                DataRequestedEventArgs,
            >::new(move |_, args| {
                let setup = (|| -> windows::core::Result<()> {
                    let args = args.as_ref().ok_or_else(windows::core::Error::empty)?;
                    let request = args.Request()?;
                    let deferral = request.GetDeferral()?;
                    let state = state.clone();
                    // The deferral keeps the data request alive while Windows opens the file.
                    std::thread::spawn(move || {
                        let result = (|| -> windows::core::Result<()> {
                            // SAFETY: this is a fresh worker thread; balance initialization below.
                            unsafe {
                                windows::Win32::System::WinRT::RoInitialize(
                                    windows::Win32::System::WinRT::RO_INIT_MULTITHREADED,
                                )?;
                            }
                            struct Apartment;
                            impl Drop for Apartment {
                                fn drop(&mut self) {
                                    unsafe {
                                        windows::Win32::System::WinRT::RoUninitialize();
                                    }
                                }
                            }
                            let _apartment = Apartment;
                            let path = state
                                .lock()
                                .map_err(|_| windows::core::Error::empty())?
                                .file
                                .as_ref()
                                .map(|f| f.path.clone())
                                .ok_or_else(windows::core::Error::empty)?;
                            let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(
                                path.as_os_str(),
                            ))?
                            .join()?;
                            let package = request.Data()?;
                            package
                                .Properties()?
                                .SetTitle(&HSTRING::from("ScribbleCap image"))?;
                            let item: IStorageItem = file.cast()?;
                            let items: windows_collections::IIterable<IStorageItem> =
                                vec![Some(item)].into();
                            package.SetStorageItems(&items, true)?;
                            let completed = state.clone();
                            package.ShareCompleted(&TypedEventHandler::new(move |_, _| {
                                if let Ok(mut pending) = completed.lock() {
                                    pending.finish(Ok(()));
                                }
                                Ok(())
                            }))?;
                            let cancelled = state.clone();
                            package.ShareCanceled(&TypedEventHandler::new(move |_, _| {
                                if let Ok(mut pending) = cancelled.lock() {
                                    pending.finish(Ok(()));
                                }
                                Ok(())
                            }))?;
                            Ok(())
                        })();
                        if let Err(error) = result {
                            let _ = request.FailWithDisplayText(&HSTRING::from(
                                "The image could not be shared.",
                            ));
                            if let Ok(mut pending) = state.lock() {
                                pending
                                    .finish(Err(format!("The image could not be shared: {error}")));
                            }
                        }
                        let _ = deferral.Complete();
                    });
                    Ok(())
                })();
                if let Err(error) = &setup
                    && let Ok(mut pending) = state.lock()
                {
                    pending.finish(Err(format!("The share data request failed: {error}")));
                }
                setup
            }))
            .map_err(|e| e.to_string())?;
        let registration = Registration {
            manager,
            token,
            pending: pending.clone(),
        };
        // SAFETY: live HWND on the window's UI thread as required by this API.
        unsafe { interop.ShowShareUIForWindow(hwnd) }.map_err(|e| e.to_string())?;
        REQUESTS.with_borrow_mut(|requests| {
            requests.retain(|r| r.pending.lock().is_ok_and(|p| p.file.is_some()));
            requests.push(registration);
        });
        Ok(())
    })();
    if let Err(error) = result
        && let Ok(mut pending) = pending.lock()
    {
        pending.finish(Err(error));
    }
}
pub(super) fn cleanup() {
    REQUESTS.with_borrow_mut(|requests| {
        for request in requests.iter() {
            if let Ok(mut pending) = request.pending.lock() {
                pending.finish(Ok(()));
            }
        }
        requests.clear();
    });
}
