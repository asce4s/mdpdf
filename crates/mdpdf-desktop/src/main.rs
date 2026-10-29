use base64::Engine;
use mdpdf_app::server::{RunningServer, ServerOptions};
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
    thread::JoinHandle,
};
use tauri::{DragDropEvent, Url, WebviewUrl, WebviewWindowBuilder, WindowEvent};

struct DesktopServer {
    addr: std::net::SocketAddr,
    shutdown: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl DesktopServer {
    fn start() -> Result<Self, String> {
        let options = ServerOptions::for_desktop().map_err(|error| error.to_string())?;
        ensure_site(Path::new(&options.site_root))?;
        let (result_tx, result_rx) = mpsc::channel();
        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = result_tx.send(Err(error.to_string()));
                    return;
                }
            };
            match runtime.block_on(start_server(options)) {
                Ok(server) => {
                    let addr = server.addr;
                    let _ = result_tx.send(Ok(addr));
                    let _ = shutdown_rx.recv();
                    runtime.block_on(async move {
                        if let Err(error) = server.shutdown().await {
                            eprintln!("mdpdf: {error}");
                        }
                    });
                }
                Err(error) => {
                    let _ = result_tx.send(Err(error));
                }
            }
        });
        let addr = match result_rx.recv() {
            Ok(Ok(addr)) => addr,
            Ok(Err(error)) => {
                let _ = shutdown_tx.send(());
                let _ = thread.join();
                return Err(error);
            }
            Err(error) => {
                let _ = shutdown_tx.send(());
                let _ = thread.join();
                return Err(error.to_string());
            }
        };
        Ok(Self {
            addr,
            shutdown: Some(shutdown_tx),
            thread: Some(thread),
        })
    }
}

impl Drop for DesktopServer {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

async fn start_server(options: ServerOptions) -> Result<RunningServer, String> {
    mdpdf_app::server::start(options)
        .await
        .map_err(|error| error.to_string())
}

fn ensure_site(site_root: &Path) -> Result<(), String> {
    let app = site_root.join("app.js");
    let wasm = site_root.join("pkg").join("mdpdf-app.js");
    if app.is_file() && wasm.is_file() {
        return Ok(());
    }
    Err(format!(
        "Desktop UI assets are missing in {}. Run `cargo leptos build` from the workspace first.",
        site_root.display()
    ))
}

fn allow_navigation(url: &str, port: u16) -> bool {
    // The live preview is a sandboxed srcdoc iframe. WebKit reports that load as
    // about:srcdoc, and its built-in PDF viewer uses a private scheme.
    if url == "about:blank"
        || url.starts_with("about:blank#")
        || url.starts_with("about:srcdoc")
        || url.starts_with("blob:")
        || url.starts_with("webkit-pdfjs-viewer:")
    {
        return true;
    }
    let Some(rest) = url.strip_prefix("http://127.0.0.1:") else {
        return false;
    };
    let port_text = rest.split(['/', '?', '#']).next().unwrap_or("");
    port_text.parse::<u16>().ok() == Some(port)
}

const MARKDOWN_LIMIT: u64 = 2 * 1024 * 1024;
const FONT_LIMIT: u64 = 5 * 1024 * 1024;
const FILE_LIMIT: u64 = 10 * 1024 * 1024;

fn has_extension(name: &str, extensions: &[&str]) -> bool {
    let lower = name.to_ascii_lowercase();
    extensions
        .iter()
        .any(|extension| lower.ends_with(extension))
}

fn markdown_name(name: &str) -> bool {
    has_extension(name, &[".md", ".markdown", ".txt"])
}

fn font_name(name: &str) -> bool {
    has_extension(name, &[".ttf", ".otf", ".woff2"])
}

fn file_mime(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".ttf") {
        "font/ttf"
    } else if lower.ends_with(".otf") {
        "font/otf"
    } else if lower.ends_with(".woff2") {
        "font/woff2"
    } else {
        "application/octet-stream"
    }
}

fn dropped_entry(path: &Path) -> Option<serde_json::Value> {
    if !path.is_file() {
        return None;
    }
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "dropped".to_owned());
    let length = std::fs::metadata(path).ok()?.len();
    if markdown_name(&name) {
        if length > MARKDOWN_LIMIT {
            return Some(serde_json::json!({
                "name": name,
                "error": "Markdown exceeds 2 MB"
            }));
        }
        return Some(match std::fs::read_to_string(path) {
            Ok(text) => serde_json::json!({"name": name, "text": text}),
            Err(error) => serde_json::json!({"name": name, "error": error.to_string()}),
        });
    }
    let limit = if font_name(&name) {
        FONT_LIMIT
    } else {
        FILE_LIMIT
    };
    if length > limit {
        let megabytes = limit / (1024 * 1024);
        return Some(serde_json::json!({
            "name": name,
            "error": format!("File exceeds {megabytes} MB")
        }));
    }
    Some(match std::fs::read(path) {
        Ok(bytes) => serde_json::json!({
            "name": name,
            "base64": base64::engine::general_purpose::STANDARD.encode(bytes),
            "mime": file_mime(&name),
        }),
        Err(error) => serde_json::json!({"name": name, "error": error.to_string()}),
    })
}

/// WebKit gives the editor the dropped path and an empty file list. Read the
/// files here and hand their contents to the page.
fn dropped_files_script(paths: &[PathBuf]) -> Option<String> {
    let entries = paths
        .iter()
        .filter_map(|path| dropped_entry(path))
        .collect::<Vec<_>>();
    if entries.is_empty() {
        return None;
    }
    let payload = serde_json::to_string(&entries).ok()?;
    Some(format!(
        "(()=>{{const files={payload};if(window.mdpdf?.loadDropped)window.mdpdf.loadDropped(files);else window.mdpdfPendingDrop=files;}})()"
    ))
}

fn show_error(error: &str) {
    eprintln!("mdpdf: {error}");
    if std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some() {
        rfd::MessageDialog::new()
            .set_title("mdpdf")
            .set_description(error)
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}

fn main() {
    let server = match DesktopServer::start() {
        Ok(server) => server,
        Err(error) => {
            show_error(&error);
            std::process::exit(1);
        }
    };
    let port = server.addr.port();
    let url = format!("http://{}", server.addr);
    eprintln!("mdpdf is ready at {url}");
    let app = match tauri::Builder::default()
        .setup(move |app| {
            let parsed: Url = url.parse().expect("loopback URL");
            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(parsed))
                .title("mdpdf")
                .inner_size(1400.0, 900.0)
                .on_navigation(move |target| allow_navigation(target.as_str(), port))
                .build()?;
            let drop_target = window.clone();
            window.on_window_event(move |event| {
                let WindowEvent::DragDrop(DragDropEvent::Drop { paths, .. }) = event else {
                    return;
                };
                let Some(script) = dropped_files_script(paths) else {
                    return;
                };
                if let Err(error) = drop_target.eval(script) {
                    eprintln!("mdpdf: {error}");
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
    {
        Ok(app) => app,
        Err(error) => {
            drop(server);
            show_error(&error.to_string());
            std::process::exit(1);
        }
    };
    app.run(|_, _| {});
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_stays_on_the_local_server() {
        assert!(allow_navigation("http://127.0.0.1:43123/", 43123));
        assert!(allow_navigation(
            "http://127.0.0.1:43123/pdfjs/viewer.html",
            43123
        ));
        assert!(allow_navigation("http://127.0.0.1:43123/app.js?x=1", 43123));
        assert!(allow_navigation("blob:http://127.0.0.1:43123/uuid", 43123));
        assert!(allow_navigation(
            "webkit-pdfjs-viewer://pdfjs/web/viewer.html",
            43123
        ));
        assert!(allow_navigation("about:blank", 43123));
        assert!(allow_navigation("about:srcdoc", 43123));
        assert!(!allow_navigation("about:config", 43123));
        assert!(!allow_navigation("http://127.0.0.1:3000/", 43123));
        assert!(!allow_navigation("https://127.0.0.1:43123/", 43123));
        assert!(!allow_navigation("http://localhost:43123/", 43123));
        assert!(!allow_navigation("file:///etc/passwd", 43123));
        assert!(!allow_navigation("http://127.0.0.1:43123.evil/", 43123));
    }

    #[test]
    fn dropped_markdown_is_loaded_by_the_page() {
        let dir = std::env::temp_dir().join(format!("mdpdf-drop-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("note.md");
        std::fs::write(&path, "say \"hi\"\n").unwrap();
        let script = dropped_files_script(std::slice::from_ref(&path)).unwrap();
        assert!(script.contains("loadDropped"));
        assert!(script.contains("note.md"));
        assert!(script.contains("say \\\"hi\\\""));
        assert!(dropped_files_script(&[dir.join("missing.md")]).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dropped_image_is_base64_and_oversized_markdown_is_rejected() {
        let dir = std::env::temp_dir().join(format!("mdpdf-drop-bin-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let image = dir.join("pixel.png");
        std::fs::write(&image, b"abc").unwrap();
        let script = dropped_files_script(std::slice::from_ref(&image)).unwrap();
        assert!(script.contains("YWJj"));
        assert!(script.contains("image/png"));

        let huge = dir.join("huge.md");
        let file = std::fs::File::create(&huge).unwrap();
        file.set_len(MARKDOWN_LIMIT + 1).unwrap();
        let script = dropped_files_script(std::slice::from_ref(&huge)).unwrap();
        assert!(script.contains("Markdown exceeds 2 MB"));
        assert!(script.len() < 500);
        std::fs::remove_dir_all(&dir).ok();
    }
}
