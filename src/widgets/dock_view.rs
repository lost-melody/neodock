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
    use std::cell::{Cell, RefCell};

    use declarative::{block, construct};
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::{gdk, gio, glib};
    use gtk4 as gtk;

    use crate::config;
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
        config: RefCell<Option<config::NeoDockConfig>>,
        niri: RefCell<Option<niri::Niri>>,
        peek: RefCell<Option<gtk::Revealer>>,
        launcher_button: RefCell<Option<gtk::Button>>,

        /// The connector of the window's output monitor, e.g. `DP-1`.
        #[property(get, set)]
        output: RefCell<String>,
        /// The active workspace index of the current output.
        #[property(get, set)]
        workspace_idx: Cell<u8>,
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

                            append: &_ @gtk::Button launcher_button {
                                tooltip_text: "App Launcher"

                                child: &_ @gtk::Image {
                                    icon_name: "applications-all-symbolic"
                                    icon_size: gtk::IconSize::Large
                                }
                                ~

                                add_css_class: "neodock-icon-button"
                                add_css_class: "neodock-icon-button-launcher"
                            }

                            append: &_ @gtk::FlowBox flow_box {
                                hexpand: false
                                selection_mode: gtk::SelectionMode::None
                                min_children_per_line: 100
                                max_children_per_line: 100
                                ~
                                add_css_class: "neodock-icon-buttons-box"
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
            obj.add_controller(motion.clone());

            let drop_target = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
            obj.add_controller(drop_target);

            self.motion.replace(Some(motion));
            self.revealer.replace(Some(revealer));
            self.peek.replace(Some(peek));
            self.launcher_button.replace(Some(launcher_button));
            self.view.replace(Some(view));

            let model = obj.apps().unwrap();
            flow_box.bind_model(
                Some(&model),
                glib::clone!(
                    #[weak]
                    obj,
                    #[upgrade_or_else]
                    || unreachable!(),
                    move |o| {
                        let app_info = o.downcast_ref::<models::App>().unwrap();
                        let app_icon = widgets::AppIcon::new();
                        app_icon.set_app_info(Some(app_info));
                        app_icon.set_dock_view(obj);
                        app_icon.upcast()
                    }
                ),
            );

            self.bind_application();
            self.connect_state_flags();
            self.connect_launcher_button();
            self.connect_workspace_idx();
        }

        /// Finds the [crate::NeoDockApp] and retrieves the [niri::Niri] object.
        fn bind_application(&self) {
            self.obj().with_neo_app(|obj, app| {
                obj.apps().unwrap().set_model(app.sorted_apps().as_ref());
                let config = app.config().unwrap();
                obj.imp().connect_config(&config);
                obj.imp().config.replace(Some(config));
                obj.imp().set_apps_filter();
                let niri = app.niri().clone();
                obj.imp().connect_niri_overview(&niri);
                obj.imp().connect_niri_workspaces_changed(&niri);
                obj.imp().connect_niri_workspace_focus(&niri);
                obj.imp().niri.replace(Some(niri));
            });
        }

        fn set_apps_filter(&self) {
            let obj = self.obj();
            let filter = gtk::CustomFilter::new(glib::clone!(
                #[weak]
                obj,
                #[upgrade_or]
                true,
                move |app_info| {
                    let app_info: models::App = app_info.downcast_ref().cloned().unwrap();
                    // always displays pinned apps.
                    if app_info.is_pinned() {
                        return true;
                    }

                    let config = obj.imp().config.borrow().clone().unwrap();
                    // filters apps according to configured windows filter.
                    match config.get_filter_windows() {
                        config::WindowsFilter::All => true,
                        config::WindowsFilter::SameOutput => app_info.in_output(&obj.output()),
                        config::WindowsFilter::SameWorkspace => {
                            app_info.in_output(&obj.output()) && app_info.in_workspace(&obj.workspace_idx())
                        }
                    }
                }
            ));
            self.obj().apps().unwrap().set_filter(Some(&filter));
        }

        /// Detects dock hovered events.
        fn connect_state_flags(&self) {
            self.obj().connect_state_flags_changed(move |obj, _| {
                obj.imp().reveal_or_hide_view();
            });
            self.reveal_or_hide_view();
        }

        fn connect_launcher_button(&self) {
            let button = self.launcher_button.borrow().clone().unwrap();
            let obj = self.obj();
            button.connect_clicked(glib::clone!(
                #[weak]
                obj,
                move |_| {
                    let config = obj.imp().config.borrow().clone().unwrap();
                    let cmd = config.launcher_command();
                    if cmd.is_empty() {
                        log::warning!("configured launcher command is empty");
                        return;
                    }
                    if let Err(err) = std::process::Command::new(&cmd[0]).args(&cmd[1..]).spawn() {
                        log::warning!("failed to spawn launcher: {err}");
                    }
                }
            ));
        }

        fn connect_config(&self, config: &config::NeoDockConfig) {
            let obj = self.obj();
            config.connect_auto_hide_notify(glib::clone!(
                #[weak]
                obj,
                move |_| {
                    obj.imp().reveal_or_hide_view();
                }
            ));
            config.connect_filter_windows_notify(glib::clone!(
                #[weak]
                obj,
                move |_| {
                    // re-filters apps on filter config changed.
                    let apps = obj.apps().unwrap().model().unwrap();
                    apps.items_changed(0, apps.n_items(), apps.n_items());
                }
            ));
        }

        /// Detects niri overview opened or closed.
        fn connect_niri_overview(&self, niri: &niri::Niri) {
            let obj = self.obj();
            niri.connect_overview_is_open_notify(glib::clone!(
                #[weak]
                obj,
                move |_| {
                    obj.imp().reveal_or_hide_view();
                }
            ));
        }

        fn connect_niri_workspaces_changed(&self, niri: &niri::Niri) {
            let obj = self.obj();
            niri.connect_workspaces_notify(glib::clone!(
                #[weak]
                obj,
                move |niri| {
                    // finds the active workspace of output on changed.
                    for workspace in niri.get_workspaces().values() {
                        if workspace.is_active() && workspace.output().unwrap_or_default() == obj.output() {
                            obj.set_workspace_idx(workspace.idx());
                            break;
                        }
                    }
                }
            ));
        }

        fn connect_niri_workspace_focus(&self, niri: &niri::Niri) {
            let obj = self.obj();
            niri.connect_focused_workspace_notify(glib::clone!(
                #[weak]
                obj,
                move |niri| {
                    if let Some(workspace) = niri.focused_workspace()
                        && workspace.output().unwrap_or_default() == obj.output()
                    {
                        obj.set_workspace_idx(workspace.idx());
                    }
                }
            ));
        }

        fn connect_workspace_idx(&self) {
            self.obj().connect_workspace_idx_notify(glib::clone!(move |obj| {
                // re-filters apps when active workspace changed and filter set as
                // "same_workspace".
                if obj.imp().config().get_filter_windows() == config::WindowsFilter::SameWorkspace {
                    let apps = obj.apps().unwrap().model().unwrap();
                    apps.items_changed(0, apps.n_items(), apps.n_items());
                }
            }));
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
                if is_revealed && self.timer.borrow().is_none() {
                    let obj = self.obj().clone();
                    let config = self.config();
                    let delay = std::time::Duration::from_millis(config.auto_hide_delay());
                    let timer = glib::timeout_add_local(delay, move || {
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
            let Some(config) = self.config.borrow().clone() else {
                return true;
            };
            if !config.auto_hide() {
                return true;
            }

            let niri = self.niri.borrow().clone().unwrap();
            if niri.overview_is_open() {
                return config.show_in_overview();
            }

            let flags = self.obj().state_flags();
            let prelight = flags & gtk::StateFlags::PRELIGHT != gtk::StateFlags::NORMAL;
            let drop_active = flags & gtk::StateFlags::DROP_ACTIVE != gtk::StateFlags::NORMAL;
            if prelight || drop_active {
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

        fn config(&self) -> config::NeoDockConfig {
            self.config.borrow().clone().unwrap()
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
