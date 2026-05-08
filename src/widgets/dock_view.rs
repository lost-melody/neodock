use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk4 as gtk;

glib::wrapper! {
    pub struct DockView(ObjectSubclass<imp::DockViewImpl>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl Default for DockView {
    fn default() -> Self {
        glib::Object::builder().build()
    }
}

impl DockView {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn destroy(&self) {
        self.imp().destroy();
    }

    /// Unsets the prelight state flag if not hovered.
    ///
    /// This is used to fix it when the prelight flag is not properly unset on popover menu closed.
    pub fn refresh_prelight(&self) {
        if !self.motion().unwrap().contains_pointer() {
            self.unset_state_flags(gtk::StateFlags::PRELIGHT);
        }
    }
}

mod imp {
    use std::cell::RefCell;

    use declarative::{block, construct};
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::{gio, glib};
    use gtk4 as gtk;

    use crate::models;
    use crate::prelude::*;
    use crate::services::niri;
    use crate::utils::log;
    use crate::widgets;

    type Obj = super::DockView;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = Obj)]
    pub struct DockViewImpl {
        timer: RefCell<Option<glib::SourceId>>,
        niri: RefCell<Option<niri::Niri>>,
        peek: RefCell<Option<gtk::Revealer>>,

        /// The connector of the window's output monitor, e.g. `DP-1`.
        #[property(get, set)]
        output: RefCell<String>,
        /// Filtered application information list to display.
        #[property(get, set)]
        apps: RefCell<Option<gtk::FilterListModel>>,
        #[property(get)]
        motion: RefCell<Option<gtk::EventControllerMotion>>,
        #[property(get)]
        revealer: RefCell<Option<gtk::Revealer>>,
        #[property(get)]
        view: RefCell<Option<gtk::CenterBox>>,
    }

    impl DockViewImpl {
        pub(super) fn destroy(&self) {}

        fn on_constructed(&self) {
            let obj = self.obj();
            self.apps.replace(Some(gtk::FilterListModel::new(
                None::<gio::ListModel>,
                None::<gtk::CustomFilter>,
            )));

            block!(obj.clone() {
                set_orientation: gtk::Orientation::Vertical
                set_homogeneous: false
                add_css_class: "neodock-view"

                // reveals the main view on hovered.
                append: &_ @gtk::Revealer revealer {
                    reveal_child: false
                    transition_type: gtk::RevealerTransitionType::SlideUp

                    child: &_ @gtk::CenterBox view {
                        ~
                        add_css_class: "neodock-centered-view"

                        set_center_widget: Some(&_) @gtk::Box {
                            orientation: gtk::Orientation::Horizontal
                            homogeneous: false
                            ~

                            append: &_ @gtk::Button {
                                tooltip_text: "App Launcher"

                                child: &_ @gtk::Image {
                                    icon_name: "applications-all-symbolic"
                                    icon_size: gtk::IconSize::Large
                                }
                                ~

                                add_css_class: "neodock-icon-button"
                                add_css_class: "neodock-icon-button-launcher"
                                connect_clicked: |_| {
                                    if let Err(err) = std::process::Command::new("qs").
                                        args(["-c", "noctalia-shell", "ipc", "call", "launcher", "toggle"]).
                                        spawn() {
                                        log::warning!("failed to spawn launcher: {err}");
                                    }
                                }
                            }

                            append: &_ @gtk::FlowBox flow_box {
                                hexpand: false
                                selection_mode: gtk::SelectionMode::None
                                min_children_per_line: 100
                                max_children_per_line: 100
                                ~
                            }
                        }
                    }
                }

                append: &_ @gtk::Revealer peek {
                    reveal_child: true
                    transition_type: gtk::RevealerTransitionType::SlideUp

                    child: &_ @gtk::Box {
                        height_request: 4
                        ~
                        add_css_class: "neodock-peek"
                    }
                }
            });

            let motion = gtk::EventControllerMotion::new();
            self.obj().add_controller(motion.clone());

            self.motion.replace(Some(motion));
            self.revealer.replace(Some(revealer));
            self.peek.replace(Some(peek));
            self.view.replace(Some(view));

            let model = obj.apps().unwrap();
            flow_box.bind_model(Some(&model), |o| {
                let app_info = o.downcast_ref::<models::App>().unwrap();
                let app_icon = widgets::AppIcon::new();
                app_icon.set_app_info(Some(app_info));
                app_icon.upcast()
            });

            self.bind_application();
            self.bind_root_window();
            self.connect_state_flags();
        }

        /// Finds the [crate::NeoDockApp] and retrieves the [niri::Niri] object.
        fn bind_application(&self) {
            self.obj().with_application(|obj, app| {
                let app = app.downcast::<crate::NeoDockApp>().expect("NeoDockApp required");
                obj.apps().unwrap().set_model(app.sorted_apps().as_ref());
                let niri = app.niri().clone();
                obj.imp().connect_niri_overview(&niri);
                obj.imp().niri.replace(Some(niri));
            });
        }

        /// Finds the [crate::NeoWindow] and retrieves the output connector.
        fn bind_root_window(&self) {
            self.obj().with_root_window(|_, win| {
                let win = win.downcast::<crate::NeoWindow>().expect("NeoWindow required");
                win.connect_output_notify(|win| {
                    let output = win.output();
                    log::message!("window output: {output}");
                });
            });
        }

        /// Detects dock hovered events.
        fn connect_state_flags(&self) {
            self.obj().connect_state_flags_changed(move |obj, _| {
                obj.imp().reveal_or_hide_view();
            });
            self.reveal_or_hide_view();
        }

        /// Detects niri overview opened or closed.
        fn connect_niri_overview(&self, niri: &niri::Niri) {
            let obj = self.obj().clone();
            niri.connect_overview_is_open_notify(glib::clone!(
                #[weak]
                obj,
                move |_| {
                    obj.imp().reveal_or_hide_view();
                }
            ));
        }

        /// Reveals the main view on hovered or overview opened;
        /// otherwise unreveals the view after a timeout.
        fn reveal_or_hide_view(&self) {
            let is_revealed = if let Some(revealer) = &*self.revealer.borrow() {
                revealer.reveals_child()
            } else {
                false
            };
            let should_reveal = self.should_reveal_view();

            if should_reveal {
                // removes existing timer.
                if let Some(t) = self.timer.replace(None) {
                    t.remove();
                }
                if !is_revealed {
                    self.show_view(true);
                }
            } else {
                if self.timer.borrow().is_none() {
                    let obj = self.obj().clone();
                    let timer = glib::timeout_add_local(std::time::Duration::from_millis(800), move || {
                        // removes timer on itself.
                        obj.imp().timer.replace(None);
                        obj.imp().show_view(false);
                        glib::ControlFlow::Break
                    });
                    self.timer.replace(Some(timer));
                }
            }
        }

        fn should_reveal_view(&self) -> bool {
            let flags = self.obj().state_flags();
            let prelight = flags & gtk::StateFlags::PRELIGHT != gtk::StateFlags::NORMAL;
            if prelight {
                true
            } else if let Some(niri) = &*self.niri.borrow() {
                niri.overview_is_open()
            } else {
                false
            }
        }

        fn show_view(&self, show: bool) {
            if show {
                self.obj().set_state_flags(gtk::StateFlags::SELECTED, false);
            } else {
                self.obj().unset_state_flags(gtk::StateFlags::SELECTED);
            }
            if let Some(revealer) = &*self.revealer.borrow() {
                revealer.set_reveal_child(show);
            }
            if let Some(peek) = &*self.peek.borrow() {
                peek.set_reveal_child(!show);
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DockViewImpl {
        const NAME: &'static str = "NeoDockView";
        type Type = Obj;
        type ParentType = gtk::Box;
    }

    #[glib::derived_properties]
    impl ObjectImpl for DockViewImpl {
        fn constructed(&self) {
            self.parent_constructed();
            self.on_constructed();
        }
    }

    impl WidgetImpl for DockViewImpl {}
    impl BoxImpl for DockViewImpl {}
}
