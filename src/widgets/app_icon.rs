use gtk::glib;
use gtk::subclass::prelude::*;
use gtk4 as gtk;

glib::wrapper! {
    pub struct AppIcon(ObjectSubclass<imp::AppIconImpl>)
        @extends gtk::FlowBoxChild, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for AppIcon {
    fn default() -> Self {
        glib::Object::builder().build()
    }
}

impl AppIcon {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn destroy(&self) {
        self.imp().destroy();
    }
}

mod imp {
    use std::cell::RefCell;
    use std::cmp;

    use declarative::{block, construct};
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::{gdk, gio, glib};
    use gtk4 as gtk;
    use unicode_width::UnicodeWidthChar;

    use crate::models;
    use crate::prelude::*;
    use crate::services::niri;
    use crate::services::niri::ipc;
    use crate::utils::log;

    type Obj = super::AppIcon;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = Obj)]
    pub struct AppIconImpl {
        action_group: RefCell<Option<gio::SimpleActionGroup>>,
        button: RefCell<Option<gtk::Button>>,
        icon: RefCell<Option<gtk::Image>>,
        right_click: RefCell<Option<gtk::GestureClick>>,
        menu: RefCell<Option<gtk::PopoverMenu>>,
        menu_model: RefCell<Option<gio::Menu>>,

        niri: RefCell<Option<niri::Niri>>,

        #[property(get, set, nullable)]
        app_info: RefCell<Option<models::App>>,
        #[property(get)]
        view: RefCell<Option<gtk::Box>>,
    }

    impl AppIconImpl {
        pub(super) fn destroy(&self) {}

        fn on_constructed(&self) {
            let obj = self.obj();

            block!(obj.clone() {
                add_css_class: "neodock-flowbox-child-app-icon"

                set_child: Some(&_) @gtk::Box view {
                    ~
                    add_css_class: "neodock-app-icon-view"
                    insert_action_group: "menu", Some(&_) @gio::SimpleActionGroup::new() action_group {}

                    append: &_ @gtk::Button button {
                        child: &_ @gtk::Image icon {
                            icon_size: gtk::IconSize::Large
                        }
                        ~
                        add_css_class: "neodock-icon-button"
                        add_css_class: "neodock-icon-button-app"
                    }

                    append: &_ @gtk::PopoverMenu menu {
                        has_arrow: false
                        menu_model: &_ @gio::Menu::new() menu_model {}
                    }
                }
            });

            let right_click = gtk::GestureClick::builder().button(gdk::BUTTON_SECONDARY).build();
            button.add_controller(right_click.clone());

            self.action_group.replace(Some(action_group));
            self.button.replace(Some(button));
            self.icon.replace(Some(icon));
            self.right_click.replace(Some(right_click));
            self.menu.replace(Some(menu));
            self.menu_model.replace(Some(menu_model));
            self.view.replace(Some(view));

            self.bind_application();
            self.bind_root_window();

            self.connect_app_info();
            self.connect_button_clicked();
            self.connect_button_right_clicked();
        }

        fn bind_application(&self) {
            self.obj().with_application(|obj, app| {
                let app = app.downcast::<crate::NeoDockApp>().unwrap();
                obj.imp().niri.replace(Some(app.niri()));
            });
        }

        fn bind_root_window(&self) {
            self.obj().with_root_window(|obj, window| {
                let window = window.downcast::<crate::NeoWindow>().unwrap();
                let revealer = window.view().unwrap().revealer().unwrap();
                obj.imp().menu().connect_visible_notify(glib::clone!(
                    #[weak]
                    window,
                    move |m| {
                        if !m.get_visible() {
                            window.view().unwrap().refresh_prelight();
                        }
                    }
                ));
                // pops down menu on revealer closed.
                revealer.connect_reveal_child_notify(glib::clone!(
                    #[weak]
                    obj,
                    move |r| {
                        if !r.reveals_child() {
                            obj.imp().menu().popdown();
                        }
                    }
                ));
            });
        }

        fn connect_app_info(&self) {
            self.obj().connect_app_info_notify(|obj| {
                obj.imp().on_app_info_changed();
            });
        }

        fn connect_button_clicked(&self) {
            let obj = self.obj().clone();
            self.button().connect_clicked(glib::clone!(
                #[weak]
                obj,
                move |button| {
                    obj.imp().on_button_clicked(button);
                }
            ));
        }

        fn connect_button_right_clicked(&self) {
            let obj = self.obj().clone();
            self.right_click().connect_released(glib::clone!(
                #[weak]
                obj,
                move |_, _, _, _| {
                    obj.imp().on_button_right_clicked();
                }
            ));
        }

        fn on_app_info_changed(&self) {
            let Some(app_info) = self.obj().app_info() else {
                return;
            };

            self.icon().set_icon_name(Some(&app_info.app_id()));
            if let Some(info) = app_info.info() {
                self.button().set_tooltip_text(Some(&info.name()));
            }

            let obj = self.obj().clone();
            app_info.connect_windows_notify(glib::clone!(
                #[weak]
                obj,
                move |app_info| {
                    obj.imp().on_windows_changed(app_info);
                    obj.imp().build_menu_model(app_info);
                }
            ));
            self.on_windows_changed(&app_info);
            self.build_menu_model(&app_info);
        }

        fn on_windows_changed(&self, app_info: &models::App) {
            let windows = app_info.windows().unwrap();
            if let Some(info) = app_info.info() {
                let name = info.name();
                if windows.n_items() == 1 {
                    let window = windows.item(0).and_downcast::<niri::NiriWindow>().unwrap();
                    let title = window.title().unwrap_or_default();
                    self.button().set_tooltip_text(Some(&format!("{name} - {title}")));
                } else {
                    let count = windows.n_items();
                    self.button()
                        .set_tooltip_text(Some(&format!("{name} ({count} windows)")))
                }
            }
        }

        fn on_button_clicked(&self, _: &gtk::Button) {
            let Some(app_info) = self.obj().app_info() else {
                return;
            };
            let windows = app_info.sorted_windows().unwrap();
            let mut window_id = 0;
            let mut focus_ts = ipc::Timestamp { secs: 0, nanos: 0 };
            for i in 0..windows.n_items() {
                let window = windows.item(i).and_downcast::<niri::NiriWindow>().unwrap();
                // focuses the next window if one is already focused.
                if window.is_focused() {
                    let index = (i + 1) % windows.n_items();
                    window_id = windows
                        .item(index)
                        .and_downcast::<niri::NiriWindow>()
                        .map(|w| w.id())
                        .unwrap_or_default();
                    // there's only one window here.
                    if window_id == window.id() {
                        return;
                    }
                    break;
                }
                // finds the last focused window.
                let ts = window.get_focus_timestamp();
                if focus_ts.secs.cmp(&ts.secs).then(focus_ts.nanos.cmp(&ts.nanos)) == cmp::Ordering::Less {
                    focus_ts = ts;
                    window_id = window.id();
                }
            }
            if window_id != 0 {
                self.focus_window(window_id);
            }
        }

        fn on_button_right_clicked(&self) {
            self.menu().popup();
        }

        /// Rebuilds menu for application and its windows.
        fn build_menu_model(&self, app_info: &models::App) {
            // rebuilds menu items.
            let menu = self.menu();
            menu.set_menu_model(None::<&gio::MenuModel>);
            let model = self.menu_model();
            model.remove_all();
            // clears all actions.
            let action_group = self.action_group();
            for action in action_group.list_actions() {
                action_group.remove_action(&action);
            }

            // application actions.
            if let Some(info) = app_info.info() {
                let section = self.build_menu_for_application(&action_group, &info);
                model.append_section(Some(&info.name()), &section);
            }

            // windows actions.
            let windows = app_info.sorted_windows().unwrap();
            if let Some((label, section)) = self.build_menu_for_windows(&action_group, windows.upcast_ref()) {
                model.append_section(Some(&label), &section);
            }

            // applies the new menu model.
            menu.set_menu_model(Some(&model));
        }

        /// Builds menu section for application actions.
        fn build_menu_for_application(
            &self,
            group: &gio::SimpleActionGroup,
            info: &gio_unix::DesktopAppInfo,
        ) -> gio::Menu {
            let section = gio::Menu::new();

            section.append(Some("Launch"), Some("menu.launch"));
            group.add_action(&Self::new_action(
                "launch",
                glib::clone!(
                    #[weak]
                    info,
                    move |_| {
                        _ = info.launch(&[], None::<&gio::AppLaunchContext>);
                    }
                ),
            ));

            // actions.
            for action in info.list_actions() {
                section.append(Some(&info.action_name(&action)), Some(&format!("menu.action-{action}")));
                group.add_action(&Self::new_action(
                    &format!("action-{action}"),
                    glib::clone!(
                        #[weak]
                        info,
                        move |_| {
                            info.launch_action(&action, None::<&gio::AppLaunchContext>);
                        }
                    ),
                ));
            }

            section
        }

        /// Builds menu section for application's open windows, if any.
        fn build_menu_for_windows(
            &self,
            group: &gio::SimpleActionGroup,
            windows: &gio::ListModel,
        ) -> Option<(String, gio::Menu)> {
            if windows.n_items() == 0 {
                return None;
            }

            if windows.n_items() == 1 {
                let window = windows.item(0).and_downcast::<niri::NiriWindow>().unwrap();
                let section = self.build_menu_for_window(group, &window);
                let title = Self::truncate_menu_label(window.title().unwrap_or_default());
                return Some((title, section));
            }

            let section = gio::Menu::new();
            // adds action for windows:
            // focus, fullscreen, toggle floating, close.
            for i in 0..windows.n_items() {
                let window = windows.item(i).and_downcast::<niri::NiriWindow>().unwrap();
                let submenu = self.build_menu_for_window(group, &window);
                let title = Self::truncate_menu_label(window.title().unwrap_or_default());
                section.append_submenu(Some(&format!("{}. {title}", i + 1)), &submenu);
            }

            // closes all windows of application.
            section.append(Some("Close All"), Some("menu.win-close-all"));
            let obj = self.obj();
            group.add_action(&Self::new_action(
                "win-close-all",
                glib::clone!(
                    #[weak]
                    obj,
                    #[weak]
                    windows,
                    move |_| {
                        glib::spawn_future_local(glib::clone!(async move {
                            let niri = obj.imp().niri.borrow().as_ref().unwrap().clone();
                            for i in 0..windows.n_items() {
                                let window = windows.item(i).and_downcast::<niri::NiriWindow>().unwrap();
                                let cmd = ipc::Action::CloseWindow { id: Some(window.id()) };
                                if let Err(err) = niri.send_action(cmd).await {
                                    log::warning!("failed to send action: {err}");
                                }
                            }
                        }));
                    }
                ),
            ));

            Some((format!("{} windows", windows.n_items()), section))
        }

        /// Builds submenu of a single window.
        fn build_menu_for_window(&self, group: &gio::SimpleActionGroup, window: &niri::NiriWindow) -> gio::Menu {
            let obj = self.obj();
            let submenu = gio::Menu::new();
            let id = window.id();

            for (label, cmds) in [
                ("Focus", window.cmd_focus()),
                ("Center", window.cmd_center()),
                ("Fullscreen", window.cmd_toggle_fullscreen()),
                ("Maximize", window.cmd_toggle_maximize()),
                ("Toggle Floating", window.cmd_toggle_floating()),
                ("Close", window.cmd_close()),
            ] {
                let action = label.to_lowercase().replace(" ", "-");
                submenu.append(Some(label), Some(&format!("menu.win-{id}-{action}")));
                let cmds = std::rc::Rc::new(cmds);
                group.add_action(&Self::new_action(
                    &format!("win-{id}-{action}"),
                    glib::clone!(
                        #[weak]
                        obj,
                        move |_| {
                            obj.imp().send_niri_actions(&cmds);
                        }
                    ),
                ));
            }

            submenu
        }

        fn focus_window(&self, id: u64) {
            let niri = self.niri.borrow().as_ref().unwrap().clone();
            glib::spawn_future_local(async move {
                if let Err(err) = niri.send_action(ipc::Action::FocusWindow { id }).await {
                    log::warning!("failed to focus window: {err}");
                }
            });
        }

        fn new_action<F: Fn(&gio::SimpleAction) + 'static>(name: &str, f: F) -> gio::SimpleAction {
            let action = gio::SimpleAction::new(name, None);
            action.connect_activate(move |a, _| f(a));
            action
        }

        fn send_niri_actions(&self, cmds: &std::rc::Rc<Vec<ipc::Action>>) {
            let obj = self.obj();
            let cmds = cmds.clone();
            glib::spawn_future_local(glib::clone!(
                #[weak]
                obj,
                async move {
                    let niri = obj.imp().niri.borrow().as_ref().unwrap().clone();
                    for cmd in cmds.as_ref() {
                        if let Err(err) = niri.send_action(cmd.clone()).await {
                            log::warning!("failed to send action: {err}");
                        }
                    }
                }
            ));
        }

        /// Truncates menu label if it's too long.
        fn truncate_menu_label(mut label: String) -> String {
            const MAX_LEN: usize = 32;
            let (mut bytes, mut width) = (0, 0);
            for c in label.chars() {
                width += c.width().unwrap_or_default();
                if width > MAX_LEN {
                    label.truncate(bytes);
                    label.push_str("...");
                    break;
                }
                bytes += c.len_utf8();
            }
            label
        }

        fn action_group(&self) -> gio::SimpleActionGroup {
            self.action_group.borrow().as_ref().unwrap().clone()
        }

        fn button(&self) -> gtk::Button {
            self.button.borrow().as_ref().unwrap().clone()
        }

        fn icon(&self) -> gtk::Image {
            self.icon.borrow().as_ref().unwrap().clone()
        }

        fn right_click(&self) -> gtk::GestureClick {
            self.right_click.borrow().as_ref().unwrap().clone()
        }

        fn menu(&self) -> gtk::PopoverMenu {
            self.menu.borrow().as_ref().unwrap().clone()
        }

        fn menu_model(&self) -> gio::Menu {
            self.menu_model.borrow().as_ref().unwrap().clone()
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AppIconImpl {
        const NAME: &'static str = "NeoDockAppIcon";
        type Type = Obj;
        type ParentType = gtk::FlowBoxChild;
    }

    #[glib::derived_properties]
    impl ObjectImpl for AppIconImpl {
        fn constructed(&self) {
            self.parent_constructed();
            self.on_constructed();
        }
    }

    impl WidgetImpl for AppIconImpl {}
    impl FlowBoxChildImpl for AppIconImpl {}
}
