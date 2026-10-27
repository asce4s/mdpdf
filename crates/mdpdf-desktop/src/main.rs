use mdpdf_app::server::{RunningServer, ServerOptions};
use std::{path::Path, sync::mpsc, thread::JoinHandle};
use tauri::{Url, WebviewUrl, WebviewWindowBuilder};

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
    if url == "about:blank" || url.starts_with("blob:") || url.starts_with("webkit-pdfjs-viewer:") {
        return true;
    }
    let Some(rest) = url.strip_prefix("http://127.0.0.1:") else {
        return false;
    };
    let port_text = rest.split(['/', '?', '#']).next().unwrap_or("");
    port_text.parse::<u16>().ok() == Some(port)
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
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(parsed))
                .title("mdpdf")
                .inner_size(1400.0, 900.0)
                .on_navigation(move |target| allow_navigation(target.as_str(), port))
                .build()?;
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
        assert!(!allow_navigation("http://127.0.0.1:3000/", 43123));
        assert!(!allow_navigation("https://127.0.0.1:43123/", 43123));
        assert!(!allow_navigation("http://localhost:43123/", 43123));
        assert!(!allow_navigation("file:///etc/passwd", 43123));
        assert!(!allow_navigation("http://127.0.0.1:43123.evil/", 43123));
    }
}
