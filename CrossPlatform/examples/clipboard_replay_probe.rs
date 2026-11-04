#[cfg(target_os = "linux")]
mod linux {
    use gtk4::{gdk, gio, glib, prelude::*};
    use std::{
        cell::{Cell, RefCell},
        io::Write,
        path::PathBuf,
    };

    pub fn record(path: &std::path::Path, event: serde_json::Value) {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        writeln!(file, "{event}").unwrap();
    }

    mod imp {
        use super::*;
        use gdk::subclass::prelude::*;
        use std::{future::Future, pin::Pin};

        #[derive(Default)]
        pub struct Provider {
            pub png: RefCell<Vec<u8>>,
            pub text: RefCell<Vec<u8>>,
            pub log: RefCell<PathBuf>,
            pub html: Cell<bool>,
        }

        #[glib::object_subclass]
        impl ObjectSubclass for Provider {
            const NAME: &'static str = "KoettReplayClipboardProbe";
            type Type = super::Provider;
            type ParentType = gdk::ContentProvider;
        }

        impl ObjectImpl for Provider {}

        impl ContentProviderImpl for Provider {
            fn formats(&self) -> gdk::ContentFormats {
                let rich = if self.html.get() {
                    "text/html"
                } else {
                    "image/png"
                };
                gdk::ContentFormats::new(&[rich, "text/plain;charset=utf-8", "text/plain"])
            }

            fn write_mime_type_future(
                &self,
                mime: &str,
                stream: &gio::OutputStream,
                _priority: glib::Priority,
            ) -> Pin<Box<dyn Future<Output = Result<(), glib::Error>> + 'static>> {
                use base64::Engine as _;
                let bytes = if mime == "image/png" {
                    self.png.borrow().clone()
                } else if mime == "text/html" {
                    format!("<p><img alt=\"Koett sketch\" src=\"data:image/png;base64,{}\"></p><p>Koett repeat clipboard probe</p>",
                        base64::engine::general_purpose::STANDARD.encode(&*self.png.borrow())).into_bytes()
                } else {
                    self.text.borrow().clone()
                };
                record(
                    &self.log.borrow(),
                    serde_json::json!({
                        "event":"read", "mime":mime, "bytes":bytes.len(),
                        "time_us":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_micros()
                    }),
                );
                let stream = stream.clone();
                Box::pin(async move {
                    stream.write_all(&bytes, gio::Cancellable::NONE)?;
                    Ok(())
                })
            }

            fn detach_clipboard(&self, _clipboard: &gdk::Clipboard) {
                record(&self.log.borrow(), serde_json::json!({"event":"detached"}));
            }
        }
    }

    glib::wrapper! {
        pub struct Provider(ObjectSubclass<imp::Provider>) @extends gdk::ContentProvider;
    }

    fn private_lab(
        runtime: Option<&std::path::Path>,
        xdg: Option<&std::path::Path>,
        display: Option<&std::ffi::OsStr>,
    ) -> bool {
        runtime.is_some_and(|path| {
            path.starts_with("/run/user")
                && path.parent().and_then(std::path::Path::file_name)
                    == Some(std::ffi::OsStr::new("agent-lab"))
                && xdg == Some(path)
                && display == Some(std::ffi::OsStr::new("wayland-agent-lab"))
        })
    }

    #[cfg(test)]
    #[test]
    fn probe_requires_a_matching_private_lab_before_opening_a_window() {
        use std::{ffi::OsStr, path::Path};
        let lab = Path::new("/run/user/1000/agent-lab/replay");
        let host = Path::new("/run/user/1000");
        let display = OsStr::new("wayland-agent-lab");
        assert!(private_lab(Some(lab), Some(lab), Some(display)));
        assert!(!private_lab(None, None, Some(display)));
        assert!(!private_lab(Some(lab), Some(host), Some(display)));
        assert!(!private_lab(Some(host), Some(host), Some(display)));
        assert!(!private_lab(
            Some(lab),
            Some(lab),
            Some(OsStr::new("wayland-1"))
        ));
    }

    pub fn run() {
        use glib::subclass::types::ObjectSubclassIsExt;
        let runtime = std::env::var_os("AGENT_LAB_RUNTIME").map(PathBuf::from);
        let xdg = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
        let display = std::env::var_os("WAYLAND_DISPLAY");
        assert!(private_lab(
            runtime.as_deref(),
            xdg.as_deref(),
            display.as_deref()
        ));
        gtk4::init().unwrap();
        let args: Vec<String> = std::env::args().collect();
        let log = PathBuf::from(&args[2]);
        let provider: Provider = glib::Object::new();
        *provider.imp().png.borrow_mut() = std::fs::read(&args[1]).unwrap();
        *provider.imp().text.borrow_mut() = b"Koett repeat clipboard probe".to_vec();
        *provider.imp().log.borrow_mut() = log.clone();
        provider
            .imp()
            .html
            .set(args.get(3).is_some_and(|arg| arg == "html"));
        let button = gtk4::Button::with_label("Offer clipboard");
        let window = gtk4::Window::builder()
            .title("Koett clipboard probe")
            .default_width(320)
            .default_height(100)
            .child(&button)
            .build();
        button.connect_clicked(move |button| {
            button.clipboard().set_content(Some(&provider)).unwrap();
            record(&log, serde_json::json!({"event":"offered"}));
        });
        window.present();
        button.grab_focus();
        glib::MainLoop::new(None, false).run();
    }
}

#[cfg(target_os = "linux")]
fn main() {
    linux::run();
}

#[cfg(not(target_os = "linux"))]
fn main() {}
