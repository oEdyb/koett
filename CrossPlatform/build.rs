use std::{fs, path::Path};
fn main() {
    let source = Path::new("../Sources/Koett/Resources/AssistantRenderer");
    println!("cargo:rerun-if-changed={}", source.display());
    let mut html = fs::read_to_string(source.join("index.html"))
        .unwrap()
        .replace("script-src 'self'", "script-src 'unsafe-inline'")
        .replace(
            "style-src 'self' 'unsafe-inline'",
            "style-src 'unsafe-inline'",
        )
        .replace("font-src 'self'", "font-src data:");
    for name in ["katex.css", "highlight.css", "renderer.css"] {
        let mut css = fs::read_to_string(source.join(name)).unwrap();
        for entry in fs::read_dir(source.join("fonts")).unwrap() {
            let path = entry.unwrap().path();
            let bytes = fs::read(&path).unwrap();
            let encoded = encode(&bytes);
            css = css.replace(
                &format!("fonts/{}", path.file_name().unwrap().to_str().unwrap()),
                &format!("data:font/woff2;base64,{encoded}"),
            );
        }
        html = html.replace(
            &format!("<link rel=\"stylesheet\" href=\"{name}\">"),
            &format!("<style>{css}</style>"),
        );
    }
    for name in [
        "marked.js",
        "dompurify.js",
        "katex.js",
        "katex-auto-render.js",
        "highlight.js",
        "renderer.js",
    ] {
        let js = fs::read_to_string(source.join(name)).unwrap();
        html = html.replace(
            &format!("<script src=\"{name}\"></script>"),
            &format!("<script>{js}</script>"),
        );
    }
    html.push_str("<template id=\"third-party-notices\">");
    for entry in fs::read_dir(source).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "txt")
            || path.file_name().unwrap() == "THIRD-PARTY-NOTICES.md"
        {
            let notice = fs::read_to_string(path)
                .unwrap()
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            html.push_str(&format!("<pre>{notice}</pre>"));
        }
    }
    html.push_str("</template>");
    fs::write(
        Path::new(&std::env::var("OUT_DIR").unwrap()).join("ask-renderer.html"),
        html,
    )
    .unwrap();
}
fn encode(bytes: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            result.push(if i > chunk.len() {
                '='
            } else {
                CHARS[((n >> (18 - i * 6)) & 63) as usize] as char
            });
        }
    }
    result
}
