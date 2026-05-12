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
        scroll: RefCell<Option<gtk::EventControllerScroll>>,
        menu: RefCell<Option<gtk::PopoverMenu>>,
        pin_icon: RefCell<Option<gtk::Image>>,

        niri: RefCell<Option<niri::Niri>>,
        indicators: RefCell<Option<gio::ListStore>>,

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

                set_child: Some(&_) @gtk::Overlay {
                    child: &_ @gtk::Box view {
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
                            menu_model: &_ @gio::Menu::new() {}
                        }
                    }
                    ~

                    add_overlay: &_ @gtk::Image pin_icon {
                        visible: false
                        halign: gtk::Align::End
                        valign: gtk::Align::Start
                        margin_end: 4
                        margin_top: 4
                        pixel_size: 8
                        icon_name: "pager-checked-symbolic"
                        ~
                        add_css_class: "neodock-icon-button-pin-icon"
                    }

                    add_overlay: &_ @gtk::FlowBox dots {
                        halign: gtk::Align::Center
                        valign: gtk::Align::End
                        hexpand: false
                        selection_mode: gtk::SelectionMode::None
                        min_children_per_line: 100
                        max_children_per_line: 100
                        ~
                        add_css_class: "neodock-icon-button-indicators-box"
                    }
                }
            });

            let right_click = gtk::GestureClick::builder().button(gdk::BUTTON_SECONDARY).build();
            button.add_controller(right_click.clone());
            let scroll = gtk::EventControllerScroll::builder()
                .flags(gtk::EventControllerScrollFlags::BOTH_AXES | gtk::EventControllerScrollFlags::DISCRETE)
                .build();
            button.add_controller(scroll.clone());

            let indicators = gio::ListStore::new::<gtk::Image>();
            dots.bind_model(Some(&indicators), |icon| icon.downcast_ref().cloned().unwrap());

            self.action_group.replace(Some(action_group));
            self.button.replace(Some(button));
            self.icon.replace(Some(icon));
            self.right_click.replace(Some(right_click));
            self.scroll.replace(Some(scroll));
            self.menu.replace(Some(menu));
            self.view.replace(Some(view));
            self.pin_icon.replace(Some(pin_icon));
            self.indicators.replace(Some(indicators));

            self.bind_application();
            self.bind_root_window();

            self.connect_app_info();
            self.connect_button_clicked();
            self.connect_button_right_clicked();
            self.connect_button_scrolled();
        }

        fn bind_application(&self) {
            self.obj().with_neo_app(|obj, app| {
                obj.imp().niri.replace(Some(app.niri()));
            });
        }

        fn bind_root_window(&self) {
            self.obj().with_neo_window(|obj, window| {
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
            let obj = self.obj();
            self.button().connect_clicked(glib::clone!(
                #[weak]
                obj,
                move |button| {
                    obj.imp().on_button_clicked(button);
                }
            ));
        }

        fn connect_button_right_clicked(&self) {
            let obj = self.obj();
            self.right_click().connect_released(glib::clone!(
                #[weak]
                obj,
                move |_, _, _, _| {
                    obj.imp().on_button_right_clicked();
                }
            ));
        }

        fn connect_button_scrolled(&self) {
            let obj = self.obj();
            self.scroll().connect_scroll(glib::clone!(
                #[weak]
                obj,
                #[upgrade_or]
                glib::Propagation::Proceed,
                move |_, x, y| {
                    obj.imp().on_button_scrolled(x as u8, y as u8);
                    glib::Propagation::Stop
                }
            ));
        }

        fn on_app_info_changed(&self) {
            let Some(app_info) = self.obj().app_info() else {
                return;
            };

            if let Some(icon) = app_info.info().and_then(|i| i.icon()) {
                self.icon().set_from_gicon(&icon);
            } else {
                self.icon().set_icon_name(Some(&app_info.app_id()));
            }

            let obj = self.obj().clone();
            app_info.connect_is_pinned_notify(glib::clone!(
                #[weak]
                obj,
                move |app_info| {
                    obj.imp().on_pinned_changed(app_info);
                }
            ));
            self.on_pinned_changed(&app_info);
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

        fn on_pinned_changed(&self, app_info: &models::App) {
            let is_pinned = app_info.is_pinned();
            self.pin_icon().set_visible(is_pinned);
            if is_pinned {
                self.button().add_css_class("neodock-icon-button-pinned");
            } else {
                self.button().remove_css_class("neodock-icon-button-pinned");
            }
        }

        fn on_windows_changed(&self, app_info: &models::App) {
            let windows = app_info.sorted_windows().unwrap();
            let app_id = app_info.app_id();
            let name = app_info.info().map(|i| i.name()).unwrap_or("Unknown".into());
            let button = self.button();
            let pin_icon = self.pin_icon();
            let windows_count = windows.n_items();

            let app_id_mark = {
                if app_id.is_empty() {
                    "".into()
                } else {
                    format!("\n#{app_id}")
                }
            };

            self.update_indicators(&windows);

            // no windows present.
            if windows_count == 0 {
                button.unset_state_flags(gtk::StateFlags::SELECTED);
                pin_icon.unset_state_flags(gtk::StateFlags::SELECTED);
                button.set_tooltip_text(Some(&format!("{name} (pinned){app_id_mark}")));
                return;
            }

            button.set_state_flags(gtk::StateFlags::SELECTED, false);
            pin_icon.set_state_flags(gtk::StateFlags::SELECTED, false);
            if windows_count == 1 {
                let window = windows.item(0).and_downcast::<niri::NiriWindow>().unwrap();
                let title = window.title().unwrap_or_default();
                self.button()
                    .set_tooltip_text(Some(&format!("{name} - {title}{app_id_mark}")));
            } else {
                let count = windows.n_items();
                self.button()
                    .set_tooltip_text(Some(&format!("{name} ({count} windows){app_id_mark}")))
            }
        }

        /// Launches application if no windows present, or cycles focus among windows.
        fn on_button_clicked(&self, _: &gtk::Button) {
            let Some(app_info) = self.obj().app_info() else {
                return;
            };
            let windows = app_info.sorted_windows().unwrap();
            if windows.n_items() == 0 {
                if let Some(info) = app_info.info() {
                    _ = info.launch(&[], None::<&gio::AppLaunchContext>);
                }
                return;
            }

            let last_idx = self.find_last_focused(&windows).unwrap();
            let window: niri::NiriWindow = windows.item(last_idx).and_downcast().unwrap();
            if !window.is_focused() {
                self.focus_window(window.id());
                return;
            }
            self.focus_window_by_index(last_idx + 1);
        }

        fn on_button_right_clicked(&self) {
            self.menu().popup();
        }

        fn on_button_scrolled(&self, x: u8, y: u8) {
            let Some(app_info) = self.obj().app_info() else {
                return;
            };
            let windows = app_info.sorted_windows().unwrap();
            if windows.n_items() <= 1 {
                return;
            }
            if let Some(mut index) = self.find_last_focused(&windows) {
                // focus next or previous.
                index = if x + y > 0 {
                    index + 1
                } else {
                    index + windows.n_items() - 1
                };
                self.focus_window_by_index(index);
            }
        }

        /// Returns the last focused window index.
        fn find_last_focused(&self, windows: &impl IsA<gio::ListModel>) -> Option<u32> {
            if windows.n_items() == 0 {
                return None;
            }
            let mut index = 0;
            let mut focus_ts = ipc::Timestamp { secs: 0, nanos: 0 };
            for i in 0..windows.n_items() {
                let window = windows.item(i).and_downcast::<niri::NiriWindow>().unwrap();
                // currently focused.
                if window.is_focused() {
                    return Some(i);
                }
                // finds the last focused window.
                let ts = window.get_focus_timestamp();
                if focus_ts.secs.cmp(&ts.secs).then(focus_ts.nanos.cmp(&ts.nanos)) == cmp::Ordering::Less {
                    focus_ts = ts;
                    index = i;
                }
            }
            Some(index)
        }

        /// Updates the windows dots indicators.
        fn update_indicators(&self, windows: &impl IsA<gio::ListModel>) {
            let indicators = self.indicators();
            if windows.n_items() == 0 {
                indicators.remove_all();
                return;
            }

            let last_focused = self.find_last_focused(windows).unwrap();
            let (mut left, mut right) = (0, windows.n_items());
            while right - left > 5 {
                if last_focused - left < right - last_focused {
                    right -= 1;
                } else {
                    left += 1;
                }
            }

            while indicators.n_items() > right - left {
                indicators.remove(indicators.n_items() - 1);
            }
            while indicators.n_items() < right - left {
                let icon = gtk::Image::builder()
                    .icon_name("pager-checked-symbolic")
                    .pixel_size(8)
                    .build();
                icon.add_css_class("neodock-icon-button-indicator");
                indicators.append(&icon);
            }

            for i in left..right {
                let window: niri::NiriWindow = windows.item(i).and_downcast().unwrap();
                let icon: gtk::Image = indicators.item(i - left).and_downcast().unwrap();
                if i == last_focused {
                    icon.set_state_flags(gtk::StateFlags::SELECTED, false);
                } else {
                    icon.unset_state_flags(gtk::StateFlags::SELECTED);
                }
                if window.is_focused() {
                    icon.add_css_class("neodock-icon-button-indicator-focused");
                } else {
                    icon.remove_css_class("neodock-icon-button-indicator-focused");
                }
            }
        }

        /// Rebuilds menu for application and its windows.
        fn build_menu_model(&self, app_info: &models::App) {
            // rebuilds menu items.
            let menu = self.menu();
            let model = menu.menu_model().and_downcast::<gio::Menu>().unwrap();
            menu.set_menu_model(None::<&gio::MenuModel>);
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

        /// Focus a specific window by index.
        fn focus_window_by_index(&self, mut index: u32) {
            let app_info = self.obj().app_info().unwrap();
            let windows = app_info.sorted_windows().unwrap();
            index %= windows.n_items();
            let window = windows.item(index).and_downcast::<niri::NiriWindow>().unwrap();
            self.focus_window(window.id());
        }

        /// Focus a specific window by id.
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

        fn scroll(&self) -> gtk::EventControllerScroll {
            self.scroll.borrow().as_ref().unwrap().clone()
        }

        fn menu(&self) -> gtk::PopoverMenu {
            self.menu.borrow().as_ref().unwrap().clone()
        }

        fn pin_icon(&self) -> gtk::Image {
            self.pin_icon.borrow().as_ref().unwrap().clone()
        }

        fn indicators(&self) -> gio::ListStore {
            self.indicators.borrow().as_ref().unwrap().clone()
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
