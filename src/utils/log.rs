/// Wraps [gtk4::glib::g_critical].
#[allow(unused)]
macro_rules! critical {
    ($format:literal $(,$arg:expr)* $(,)?) => {
        gtk4::glib::g_critical!(crate::constants::LOG_DOMAIN, $format $(, $arg)*);
    };
}

/// Wraps [gtk4::glib::g_debug].
#[allow(unused)]
macro_rules! debug {
    ($format:literal $(,$arg:expr)* $(,)?) => {
        gtk4::glib::g_debug!(crate::constants::LOG_DOMAIN, $format $(, $arg)*);
    };
}

/// Wraps [gtk4::glib::g_info].
#[allow(unused)]
macro_rules! info {
    ($format:literal $(,$arg:expr)* $(,)?) => {
        gtk4::glib::g_info!(crate::constants::LOG_DOMAIN, $format $(, $arg)*);
    };
}

/// Wraps [gtk4::glib::g_message].
#[allow(unused)]
macro_rules! message {
    ($format:literal $(,$arg:expr)* $(,)?) => {
        gtk4::glib::g_message!(crate::constants::LOG_DOMAIN, $format $(, $arg)*);
    };
}

/// Wraps [gtk4::glib::g_warning].
#[allow(unused)]
macro_rules! warning {
    ($format:literal $(,$arg:expr)* $(,)?) => {
        gtk4::glib::g_warning!(crate::constants::LOG_DOMAIN, $format $(, $arg)*);
    };
}

#[allow(unused)]
pub(crate) use critical;
#[allow(unused)]
pub(crate) use debug;
#[allow(unused)]
pub(crate) use info;
#[allow(unused)]
pub(crate) use message;
#[allow(unused)]
pub(crate) use warning;
