use gtk4::{
    self as gtk,
    glib::{self, translate::*},
    prelude::*,
};
use libloading::Library;
use std::{
    ffi::{CString, c_void},
    sync::OnceLock,
};
type Evaluate = unsafe extern "C" fn(
    *mut c_void,
    *const i8,
    isize,
    *const i8,
    *const i8,
    *mut c_void,
    Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void)>,
    *mut c_void,
);
static LIBRARY: OnceLock<Result<Library, String>> = OnceLock::new();
pub struct Renderer {
    pub widget: gtk::Widget,
    evaluate: Evaluate,
    pending: std::rc::Rc<std::cell::RefCell<String>>,
}
impl Renderer {
    pub fn new() -> Result<Self, String> {
        let library = LIBRARY
            .get_or_init(|| unsafe {
                libloading::os::unix::Library::open(
                    Some("libwebkitgtk-6.0.so.4"),
                    libloading::os::unix::RTLD_NOW | libloading::os::unix::RTLD_GLOBAL,
                )
                .map(Library::from)
                .map_err(|_| "Rich answers require WebKitGTK 6.0 (2.40 or later).".into())
            })
            .as_ref()
            .map_err(Clone::clone)?;
        // Stable 6.0 C ABI; keep the library loaded for registered GTypes.
        unsafe {
            let ephemeral = library
                .get::<unsafe extern "C" fn() -> *mut glib::gobject_ffi::GObject>(
                    b"webkit_network_session_new_ephemeral\0",
                )
                .map_err(|e| e.to_string())?;
            let get_type = library
                .get::<unsafe extern "C" fn() -> glib::ffi::GType>(b"webkit_web_view_get_type\0")
                .map_err(|e| e.to_string())?;
            let load = library
                .get::<unsafe extern "C" fn(*mut c_void, *const i8, *const i8)>(
                    b"webkit_web_view_load_html\0",
                )
                .map_err(|e| e.to_string())?;
            let evaluate = *library
                .get::<Evaluate>(b"webkit_web_view_evaluate_javascript\0")
                .map_err(|e| e.to_string())?;
            let background = library
                .get::<unsafe extern "C" fn(*mut c_void, *const gtk::gdk::ffi::GdkRGBA)>(
                    b"webkit_web_view_set_background_color\0",
                )
                .map_err(|e| e.to_string())?;
            let ignore = *library
                .get::<unsafe extern "C" fn(*mut glib::gobject_ffi::GObject)>(
                    b"webkit_policy_decision_ignore\0",
                )
                .map_err(|e| e.to_string())?;
            let session: glib::Object = from_glib_full(ephemeral());
            let object = glib::Object::builder_with_type(from_glib(get_type()))
                .property("network-session", session)
                .build();
            let widget = object
                .downcast::<gtk::Widget>()
                .map_err(|_| "WebKit widget type mismatch")?;
            let get_settings = library
                .get::<unsafe extern "C" fn(*mut c_void) -> *mut glib::gobject_ffi::GObject>(
                    b"webkit_web_view_get_settings\0",
                )
                .map_err(|e| e.to_string())?;
            let settings: glib::Object = from_glib_none(get_settings(widget.as_ptr().cast()));
            settings.set_property("enable-html5-local-storage", false);
            settings.set_property("javascript-can-open-windows-automatically", false);
            let action = *library
                .get::<unsafe extern "C" fn(*mut glib::gobject_ffi::GObject) -> *mut c_void>(
                    b"webkit_navigation_policy_decision_get_navigation_action\0",
                )
                .map_err(|e| e.to_string())?;
            let request = *library
                .get::<unsafe extern "C" fn(*mut c_void) -> *mut c_void>(
                    b"webkit_navigation_action_get_request\0",
                )
                .map_err(|e| e.to_string())?;
            let uri = *library
                .get::<unsafe extern "C" fn(*mut c_void) -> *const i8>(
                    b"webkit_uri_request_get_uri\0",
                )
                .map_err(|e| e.to_string())?;
            widget.connect_local("decide-policy", false, move |values| {
                let decision = values[1].get::<glib::Object>().unwrap();
                if decision.type_().name() == "WebKitNavigationPolicyDecision"
                    && std::ffi::CStr::from_ptr(uri(request(action(decision.as_ptr())))).to_bytes()
                        == b"about:blank"
                {
                    return Some(false.to_value());
                }
                ignore(decision.as_ptr());
                Some(true.to_value())
            });
            background(
                widget.as_ptr().cast(),
                gtk::gdk::RGBA::TRANSPARENT.to_glib_none().0,
            );
            let html =
                CString::new(include_str!(concat!(env!("OUT_DIR"), "/ask-renderer.html"))).unwrap();
            let pending = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
            let source = pending.clone();
            let weak = widget.downgrade();
            widget.connect_local("load-changed", false, move |_| {
                if let Some(widget) = weak.upgrade() {
                    Self::render(&widget, evaluate, &source.borrow());
                }
                None
            });
            load(
                widget.as_ptr().cast(),
                html.as_ptr(),
                c"about:blank".as_ptr(),
            );
            Ok(Self {
                widget,
                evaluate,
                pending,
            })
        }
    }
    pub fn update(&self, source: &str) {
        self.pending.replace(source.to_owned());
        Self::render(&self.widget, self.evaluate, source);
    }
    fn render(widget: &gtk::Widget, evaluate: Evaluate, source: &str) {
        let source = serde_json::to_string(source).unwrap();
        let settings = gtk::Settings::default();
        let dark = std::env::var("GTK_THEME")
            .is_ok_and(|s| s.to_ascii_lowercase().contains("dark"))
            || settings.as_ref().is_some_and(|s| {
                s.is_gtk_application_prefer_dark_theme()
                    || s.gtk_theme_name()
                        .is_some_and(|n| n.to_ascii_lowercase().contains("dark"))
            });
        let color = if dark { "#eeeeec" } else { "#2e3436" };
        let scheme = if dark { "dark" } else { "light" };
        let js = CString::new(format!("if(window.renderMarkdown) {{ let s=document.getElementById('linux-theme'); if(!s) {{s=document.createElement('style');s.id='linux-theme';document.head.appendChild(s);}} s.textContent=':root{{color-scheme:{scheme};color:{color}}} h1,h2,h3,h4,h5,h6,strong,blockquote,th{{color:inherit}} pre,pre code{{background:#202124;color:#eee}}'; window.renderMarkdown({source}); }}")).unwrap();
        unsafe {
            evaluate(
                widget.as_ptr().cast(),
                js.as_ptr(),
                -1,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                None,
                std::ptr::null_mut(),
            );
        }
    }
}
