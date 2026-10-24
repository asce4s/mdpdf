use super::{ApiError, AppState, assets::SessionId};
use crate::model::{DocumentRequest, PreviewDoc};
use axum::{
    Extension, Json,
    extract::{ConnectInfo, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use mdpdf_render::{ExportOptions, RenderError};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::Semaphore;

pub async fn preview(
    State(state): State<AppState>,
    Extension(session): Extension<SessionId>,
    Json(request): Json<DocumentRequest>,
) -> Result<Json<PreviewDoc>, ApiError> {
    state
        .preview(request.markdown, request.theme, session)
        .await
        .map(Json)
}

pub async fn export(
    State(state): State<AppState>,
    Extension(session): Extension<SessionId>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(request): Json<DocumentRequest>,
) -> Result<Response, ApiError> {
    if request.markdown.len() > state.max_markdown {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "Markdown exceeds the document size limit",
        ));
    }
    let ip = peer.ip();
    let semaphore = {
        let mut rates = state.rates.lock().await;
        if rates.len() > 8192 {
            rates.retain(|_, value| value.available_permits() < 2);
        }
        Arc::clone(
            rates
                .entry(ip)
                .or_insert_with(|| Arc::new(Semaphore::new(2))),
        )
    };
    let _permit = semaphore.try_acquire_owned().map_err(|_| {
        ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "Two exports are already running. Wait for one to finish.",
        )
    })?;
    let (result, title) = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        let document = state
            .assets
            .prepare(&request.markdown, &request.theme, &session)
            .await?;
        let title = request
            .filename
            .as_deref()
            .unwrap_or(&document.metadata.title)
            .to_owned();
        let result = state
            .pool
            .render(
                document,
                ExportOptions {
                    strict_diagrams: request.strict_diagrams,
                },
            )
            .await
            .map_err(|error| match error {
                RenderError::DiagramErrors(diagnostics) => ApiError {
                    status: StatusCode::UNPROCESSABLE_ENTITY,
                    message: "A diagram needs attention before export".into(),
                    diagnostics,
                },
                RenderError::Timeout => ApiError::new(
                    StatusCode::GATEWAY_TIMEOUT,
                    "Export exceeded 30 seconds. Try a smaller document.",
                ),
                RenderError::Closed => ApiError::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "The PDF renderer is restarting",
                ),
                error => {
                    eprintln!("Export error: {error}");
                    ApiError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "PDF export failed. Please try again.",
                    )
                }
            })?;
        Ok::<_, ApiError>((result, title))
    })
    .await
    .map_err(|_| ApiError::new(StatusCode::GATEWAY_TIMEOUT, "Export exceeded 30 seconds"))??;
    let stem: String = title
        .trim_end_matches(".md")
        .trim_end_matches(".pdf")
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '/' | '\\'))
        .take(100)
        .collect();
    let stem = if stem.trim().is_empty() {
        "document"
    } else {
        stem.trim()
    };
    let fallback: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let encoded: String = stem
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_') {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{fallback}.pdf\"; filename*=UTF-8''{encoded}.pdf"),
            ),
            (header::CACHE_CONTROL, "no-store".into()),
        ],
        result.bytes,
    )
        .into_response())
}
