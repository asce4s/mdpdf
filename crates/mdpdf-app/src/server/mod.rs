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
    sync::Arc,
};
use tokio::sync::{Mutex, Semaphore};
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

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = get_configuration(None)?;
    let mut options = config.leptos_options;
    let port: u16 = std::env::var("MDPDF_PORT")
        .unwrap_or_else(|_| "3000".into())
        .parse()?;
    let host = std::env::var("MDPDF_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    options.site_addr = format!("{host}:{port}").parse()?;
    let site_root = std::env::var("LEPTOS_SITE_ROOT").unwrap_or_else(|_| "target/site".into());
    let asset_dir = std::env::var("MDPDF_ASSET_DIR").unwrap_or_else(|_| "target/uploads".into());
    let max_markdown = std::env::var("MDPDF_MAX_MD_BYTES")
        .unwrap_or_else(|_| "2097152".into())
        .parse::<usize>()?
        .min(2 * 1024 * 1024);
    let pool = BrowserPool::new(PoolConfig::from_env()?).await?;
    let assets = Arc::new(assets::AssetStore::new(asset_dir.into())?);
    let cleanup = assets.clone();
    tokio::spawn(async move {
        let hours = std::env::var("MDPDF_ASSET_TTL_HOURS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(24)
            .max(1);
        loop {
            let _ = cleanup.cleanup(std::time::Duration::from_secs(hours * 3600));
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        }
    });
    let state = AppState {
        pool: pool.clone(),
        assets,
        max_markdown,
        rates: Arc::new(Mutex::new(HashMap::new())),
        preview_slots: Arc::new(Semaphore::new(12)),
    };
    let app = router(state, options.clone(), site_root);
    let listener = tokio::net::TcpListener::bind(options.site_addr).await?;
    eprintln!("mdpdf is ready at http://{}", options.site_addr);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        #[cfg(unix)]
        {
            let mut terminate =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("SIGTERM handler");
            tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
        }
        #[cfg(not(unix))]
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
    pool.close().await;
    Ok(())
}
