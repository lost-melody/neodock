use gtk::glib;
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
}

mod imp {
    use std::cell::RefCell;

    use declarative::{block, construct};
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk4 as gtk;

    type Obj = super::DockView;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = Obj)]
    pub struct DockViewImpl {
        #[property(get)]
        view: RefCell<gtk::CenterBox>,
    }

    impl DockViewImpl {
        pub(super) fn destroy(&self) {}

        fn on_constructed(&self) {
            let obj = self.obj();

            block!(obj.clone() {
                set_orientation: gtk::Orientation::Vertical
                set_homogeneous: false
                add_css_class: "neodock-container"

                append: &_ @gtk::CenterBox view {
                    start_widget: &_ @gtk::Label {
                        label: "Start"
                    }

                    end_widget: &_ @gtk::Label {
                        label: "End"
                    }

                    center_widget: &_ @gtk::Label {
                        label: "Center"
                    }
                }
            });

            *self.view.borrow_mut() = view;
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
