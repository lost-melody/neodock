use gtk4::glib;

pub fn gettext(msgid: &str) -> glib::GString {
    glib::dgettext(None, msgid)
}

pub fn pgettext(context: &str, msgid: &str) -> glib::GString {
    glib::dpgettext2(None, context, msgid)
}
