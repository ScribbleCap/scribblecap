//! GTK file drags use button index 1 and retain URI delivery until drag-end.
//! Keep this small adapter separate from the macOS/Windows drag crate: upstream
//! 2.1.1 uses a modifier mask as the button and cleans up at drop-performed.
use gtk::{gdk, glib::SignalHandlerId, prelude::*};
use std::{cell::RefCell, path::Path, rc::Rc};

pub fn start(
    window: &gtk::ApplicationWindow,
    path: &Path,
    finished: impl Fn() + 'static,
) -> Result<(), String> {
    let uri = url::Url::from_file_path(path)
        .map_err(|()| "The shared file path is invalid.")?
        .to_string();
    let target = gtk::TargetEntry::new("text/uri-list", gtk::TargetFlags::OTHER_APP, 0);
    let targets = gtk::TargetList::new(&[target]);
    let handlers: Rc<RefCell<Vec<SignalHandlerId>>> = Rc::default();
    let data = window.connect_drag_data_get(move |_, _, data, _, _| {
        data.set_uris(&[uri.as_str()]);
    });
    handlers.borrow_mut().push(data);
    let cleanup = handlers.clone();
    let end = window.connect_drag_end(move |window, _| {
        finished();
        for handler in cleanup.borrow_mut().drain(..) {
            window.disconnect(handler);
        }
    });
    handlers.borrow_mut().push(end);
    let Some(context) =
        window.drag_begin_with_coordinates(&targets, gdk::DragAction::COPY, 1, None, -1, -1)
    else {
        for handler in handlers.borrow_mut().drain(..) {
            window.disconnect(handler);
        }
        return Err("The desktop could not start a file drag.".into());
    };
    let loader = gtk::gdk_pixbuf::PixbufLoader::new();
    if loader
        .write(include_bytes!("../icons/32x32.png"))
        .and_then(|()| loader.close())
        .is_ok()
        && let Some(icon) = loader.pixbuf()
    {
        context.drag_set_icon_pixbuf(&icon, 0, 0);
    }
    Ok(())
}
