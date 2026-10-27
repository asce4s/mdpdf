pub mod assets;
pub mod export;
pub mod themes;

use crate::{app::shell, model::PreviewDoc};
use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, State},
    http::{Request, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use leptos::prelude::*;
use mdpdf_core::Theme;
use mdpdf_render::{BrowserPool, PoolConfig};
use serde::Serialize;
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::{
    sync::{Mutex, Semaphore},
    task::JoinHandle,
};
use tower_http::{compression::CompressionLayer, services::ServeDir};

#[derive(Clone)]
pub struct AppState {
    pub pool: BrowserPool,
    pub assets: Arc<assets::AssetStore>,
    pub max_markdown: usize,
    pub rates: Arc<Mutex<HashMap<IpAddr, Arc<Semaphore>>>>,
    pub preview_slots: Arc<Semaphore>,
}
impl AppState {
    pub async fn preview(
        &self,
        markdown: String,
        theme: Theme,
        session: assets::SessionId,
    ) -> Result<PreviewDoc, ApiError> {
        if markdown.len() > self.max_markdown {
            return Err(ApiError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "Markdown exceeds the document size limit",
            ));
        }
        let _permit = self
            .preview_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                ApiError::new(
                    StatusCode::TOO_MANY_REQUESTS,
                    "The renderer is busy. Please try again shortly.",
                )
            })?;
        let document = self.assets.prepare(&markdown, &theme, &session).await?;
        Ok(PreviewDoc {
            html: document.html,
            diagnostics: document.diagnostics,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    #[serde(skip)]
    pub status: StatusCode,
    pub message: String,
    pub diagnostics: Vec<mdpdf_core::Diagnostic>,
}
impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            diagnostics: Vec::new(),
        }
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self)).into_response()
    }
}

async fn session(mut request: Request<Body>, next: Next) -> Response {
    if let Some(origin) = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
    {
        let host = request
            .headers()
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        if origin != format!("http://{host}") && origin != format!("https://{host}") {
            return ApiError::new(
                StatusCode::FORBIDDEN,
                "This request came from a different site",
            )
            .into_response();
        }
    }
    let current = request
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies
                .split(';')
                .find_map(|cookie| cookie.trim().strip_prefix("mdpdf_session="))
                .filter(|id| id.len() == 48 && id.bytes().all(|b| b.is_ascii_hexdigit()))
        })
        .map(str::to_owned);
    let fresh = current.is_none();
    let id = match current {
        Some(id) => id,
        None => {
            let mut bytes = [0u8; 24];
            if getrandom::fill(&mut bytes).is_err() {
                return ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Could not initialize the session",
                )
                .into_response();
            }
            bytes.iter().map(|b| format!("{b:02x}")).collect()
        }
    };
    request
        .extensions_mut()
        .insert(assets::SessionId(id.clone()));
    let mut response = next.run(request).await;
    if fresh {
        response.headers_mut().insert(
            header::SET_COOKIE,
            format!("mdpdf_session={id}; Path=/; HttpOnly; SameSite=Strict")
                .parse()
                .unwrap(),
        );
    }
    response
        .headers_mut()
        .insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    response
        .headers_mut()
        .insert(header::REFERRER_POLICY, "no-referrer".parse().unwrap());
    response
}

pub fn router(state: AppState, options: LeptosOptions, site_root: String) -> Router {
    // Leptos initializes its global Tokio executor during route discovery.
    let _ = leptos_axum::generate_route_list(crate::app::App);
    let rpc_state = state.clone();
    let home = leptos_axum::render_app_to_stream(move || shell(options.clone()));
    Router::new()
        .route("/", get(home))
        .route(
            "/healthz",
            get(|State(state): State<AppState>| async move {
                if state.pool.is_ready().await {
                    StatusCode::OK
                } else {
                    StatusCode::SERVICE_UNAVAILABLE
                }
            }),
        )
        .route("/api/preview", post(export::preview))
        .route("/api/export", post(export::export))
        .route("/api/themes", get(themes::list))
        .route("/api/theme/import", post(themes::import))
        .route("/api/syntax/import", post(themes::import_syntax))
        .route("/api/theme/export", post(themes::export))
        .route(
            "/api/rpc/{*name}",
            post(move |request: Request<Body>| {
                let state = rpc_state.clone();
                let session = request
                    .extensions()
                    .get::<assets::SessionId>()
                    .cloned()
                    .unwrap_or_default();
                leptos_axum::handle_server_fns_with_context(
                    move || {
                        provide_context(state.clone());
                        provide_context(session.clone());
                    },
                    request,
                )
            }),
        )
        .route(
            "/api/assets",
            get(assets::list)
                .post(assets::upload)
                .layer(DefaultBodyLimit::max(11 * 1024 * 1024)),
        )
        .route("/assets/{id}", get(assets::download))
        .fallback_service(ServeDir::new(site_root))
        .layer(DefaultBodyLimit::max(3 * 1024 * 1024))
        .layer(CompressionLayer::new())
        .layer(middleware::from_fn(session))
        .with_state(state)
}

pub type ServerError = Box<dyn std::error::Error + Send + Sync>;

/// Settings for one Axum process. The web binary reads the environment; the desktop shell overrides the bind address and pool.
pub struct ServerOptions {
    pub bind: SocketAddr,
    pub site_root: String,
    pub asset_dir: PathBuf,
    pub max_markdown: usize,
    pub pool: PoolConfig,
    pub asset_ttl: Duration,
    /// Skip the cargo-leptos reload socket. The desktop window sets this.
    pub production_shell: bool,
}

struct SharedOptions {
    site_root: String,
    asset_dir: PathBuf,
    max_markdown: usize,
    pool: PoolConfig,
    asset_ttl: Duration,
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_owned())
}

fn shared_from_env() -> Result<SharedOptions, ServerError> {
    let max_markdown = env_or("MDPDF_MAX_MD_BYTES", "2097152")
        .parse::<usize>()?
        .min(2 * 1024 * 1024);
    let hours = std::env::var("MDPDF_ASSET_TTL_HOURS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(24)
        .max(1);
    Ok(SharedOptions {
        site_root: env_or("LEPTOS_SITE_ROOT", "target/site"),
        asset_dir: PathBuf::from(env_or("MDPDF_ASSET_DIR", "target/uploads")),
        max_markdown,
        pool: PoolConfig::from_env()?,
        asset_ttl: Duration::from_secs(hours * 3600),
    })
}

impl ServerOptions {
    /// Web-server defaults: `127.0.0.1:3000` unless `MDPDF_HOST` / `MDPDF_PORT` are set, pool size 4.
    pub fn from_env() -> Result<Self, ServerError> {
        let port: u16 = env_or("MDPDF_PORT", "3000").parse()?;
        let host = env_or("MDPDF_HOST", "127.0.0.1");
        let shared = shared_from_env()?;
        Ok(Self {
            bind: format!("{host}:{port}").parse()?,
            site_root: shared.site_root,
            asset_dir: shared.asset_dir,
            max_markdown: shared.max_markdown,
            pool: shared.pool,
            asset_ttl: shared.asset_ttl,
            production_shell: false,
        })
    }

    /// Desktop defaults: ephemeral loopback port, one Chromium tab unless `MDPDF_POOL_SIZE` is set.
    pub fn for_desktop() -> Result<Self, ServerError> {
        let pool_size_configured = std::env::var("MDPDF_POOL_SIZE").is_ok();
        let asset_dir_configured = std::env::var("MDPDF_ASSET_DIR").is_ok();
        let shared = shared_from_env()?;
        let mut pool = shared.pool;
        apply_desktop_pool(&mut pool, pool_size_configured);
        let asset_dir = if asset_dir_configured {
            shared.asset_dir
        } else {
            uploads_dir(
                std::env::var_os("XDG_DATA_HOME").as_deref().map(Path::new),
                std::env::var_os("HOME").as_deref().map(Path::new),
            )
        };
        Ok(Self {
            bind: SocketAddr::from(([127, 0, 0, 1], 0)),
            site_root: shared.site_root,
            asset_dir,
            max_markdown: shared.max_markdown,
            pool,
            asset_ttl: shared.asset_ttl,
            production_shell: true,
        })
    }
}

fn apply_desktop_pool(pool: &mut PoolConfig, pool_size_configured: bool) {
    if !pool_size_configured {
        pool.size = 1;
    }
}

pub fn uploads_dir(xdg_data_home: Option<&Path>, home: Option<&Path>) -> PathBuf {
    let base = xdg_data_home
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .or_else(|| home.map(|path| path.join(".local/share")))
        .unwrap_or_else(|| PathBuf::from(".local/share"));
    base.join("mdpdf").join("uploads")
}

pub struct RunningServer {
    pub addr: SocketAddr,
    shutdown: tokio::sync::oneshot::Sender<()>,
    serve: JoinHandle<Result<(), std::io::Error>>,
    cleanup: JoinHandle<()>,
    pool: BrowserPool,
}

impl RunningServer {
    pub async fn shutdown(self) -> Result<(), ServerError> {
        let _ = self.shutdown.send(());
        let serve_result = self.serve.await;
        self.cleanup.abort();
        self.pool.close().await;
        serve_result??;
        Ok(())
    }
}

pub async fn start(config: ServerOptions) -> Result<RunningServer, ServerError> {
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let mut options = get_configuration(Some(manifest))?.leptos_options;
    if config.production_shell {
        options.env = leptos::config::Env::PROD;
    }
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    let addr = listener.local_addr()?;
    options.site_addr = addr;
    let pool = BrowserPool::new(config.pool).await?;
    let assets = Arc::new(assets::AssetStore::new(config.asset_dir)?);
    let cleanup_store = assets.clone();
    let asset_ttl = config.asset_ttl;
    let cleanup = tokio::spawn(async move {
        loop {
            let _ = cleanup_store.cleanup(asset_ttl);
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    });
    let state = AppState {
        pool: pool.clone(),
        assets,
        max_markdown: config.max_markdown,
        rates: Arc::new(Mutex::new(HashMap::new())),
        preview_slots: Arc::new(Semaphore::new(12)),
    };
    let app = router(state, options, config.site_root);
    let (shutdown, shutdown_rx) = tokio::sync::oneshot::channel();
    let serve = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        })
        .await
    });
    Ok(RunningServer {
        addr,
        shutdown,
        serve,
        cleanup,
        pool,
    })
}

pub async fn run() -> Result<(), ServerError> {
    let server = start(ServerOptions::from_env()?).await?;
    eprintln!("mdpdf is ready at http://{}", server.addr);
    wait_for_shutdown_signal().await;
    server.shutdown().await
}

async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = terminate.recv() => {},
        }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uploads_follow_xdg_then_home() {
        assert_eq!(
            uploads_dir(Some(Path::new("/xdg")), Some(Path::new("/home/a"))),
            PathBuf::from("/xdg/mdpdf/uploads")
        );
        assert_eq!(
            uploads_dir(Some(Path::new("")), Some(Path::new("/home/a"))),
            PathBuf::from("/home/a/.local/share/mdpdf/uploads")
        );
        assert_eq!(
            uploads_dir(None, Some(Path::new("/home/a"))),
            PathBuf::from("/home/a/.local/share/mdpdf/uploads")
        );
        assert_eq!(
            uploads_dir(None, None),
            PathBuf::from(".local/share/mdpdf/uploads")
        );
    }

    #[test]
    fn desktop_pool_defaults_to_one_tab() {
        let mut pool = PoolConfig::default();
        assert_eq!(pool.size, 4);
        apply_desktop_pool(&mut pool, false);
        assert_eq!(pool.size, 1);
        pool.size = 4;
        apply_desktop_pool(&mut pool, true);
        assert_eq!(pool.size, 4);
    }

    #[test]
    fn app_manifest_loads_output_name() {
        let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
        let config = get_configuration(Some(manifest)).unwrap();
        assert_eq!(config.leptos_options.output_name.as_ref(), "mdpdf-app");
    }
}
