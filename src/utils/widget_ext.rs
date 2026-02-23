use glib::clone::{Downgrade, Upgrade};
use gtk::glib;
use gtk::prelude::*;
use gtk4 as gtk;

use super::signal;

pub trait WidgetWithRootWindowExt: IsA<gtk::Widget> {
    /// Runs callback `f(self, window)` if already rooted;
    /// otherwise runs on rooted.
    fn with_root_window<F>(&self, f: F)
    where
        F: FnOnce(&Self, gtk::Window) + 'static;
}

pub trait WidgetWithApplicationExt: IsA<gtk::Widget> {
    /// Runs callback `f(self, application)` if already rooted and attached
    /// to an application; otherwise runs on rooted and attached.
    fn with_application<F>(&self, f: F)
    where
        F: FnOnce(&Self, gtk::Application) + 'static;
}

impl<W> WidgetWithRootWindowExt for W
where
    W: IsA<gtk::Widget> + IsA<glib::Object>,
{
    fn with_root_window<F>(&self, f: F)
    where
        F: FnOnce(&Self, gtk::Window) + 'static,
    {
        if let Some(root) = self.root().and_downcast() {
            f(self, root);
            return;
        }

        let once = signal::OnceCallback::default();
        once.store(
            f,
            self.connect_root_notify({
                let once = once.clone();
                move |obj| {
                    if let Some(root) = obj.root().and_downcast()
                        && let Some(f) = once.disconnect(obj)
                    {
                        f(obj, root);
                    }
                }
            }),
        );
    }
}

impl<W, U> WidgetWithApplicationExt for W
where
    W: IsA<gtk::Widget> + IsA<glib::Object> + Downgrade<Weak = U>,
    U: Upgrade<Strong = W> + 'static,
{
    fn with_application<F>(&self, f: F)
    where
        F: FnOnce(&Self, gtk::Application) + 'static,
    {
        self.with_root_window(|obj, window| {
            if let Some(app) = window.application() {
                f(obj, app);
                return;
            }

            let once = signal::OnceCallback::default();
            once.store(
                f,
                window.connect_application_notify(glib::clone!(
                    #[weak]
                    obj,
                    #[strong]
                    once,
                    move |window| {
                        if let Some(app) = window.application()
                            && let Some(f) = once.disconnect(window)
                        {
                            f(&obj, app);
                        }
                    }
                )),
            );
        });
    }
}
