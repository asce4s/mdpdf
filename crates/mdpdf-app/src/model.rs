use mdpdf_core::{Diagnostic, Theme};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreviewDoc {
    pub html: String,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreviewUpdate {
    pub body: String,
    pub css: String,
    pub diagnostics: Vec<Diagnostic>,
    #[serde(rename = "mathScript")]
    pub math_script: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentRequest {
    pub markdown: String,
    pub theme: Theme,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub strict_diagrams: bool,
}

#[leptos::server(prefix = "/api/rpc", endpoint = "render_preview", input = leptos::server_fn::codec::Json)]
pub async fn render_preview(
    markdown: String,
    theme: Theme,
) -> Result<PreviewDoc, leptos::prelude::ServerFnError> {
    use leptos::prelude::*;
    let state = expect_context::<crate::server::AppState>();
    let session = use_context::<crate::server::assets::SessionId>().unwrap_or_default();
    state
        .preview(markdown, theme, session)
        .await
        .map_err(|error| ServerFnError::new(error.message))
}

/// Subsequent native previews reuse the sandbox's vendor runtimes. Only send
/// content and embedded font/image assets, rather than retransmitting Mermaid.
#[leptos::server(prefix = "/api/rpc", endpoint = "render_preview_update", input = leptos::server_fn::codec::Json)]
pub async fn render_preview_update(
    markdown: String,
    theme: Theme,
    include_math: bool,
) -> Result<PreviewUpdate, leptos::prelude::ServerFnError> {
    let document = render_preview(markdown, theme).await?;
    let body = document
        .html
        .split_once("<main>")
        .and_then(|(_, rest)| rest.split_once("</main>"))
        .map_or("", |(body, _)| body)
        .to_owned();
    let css = document
        .html
        .split_once("id=\"mdpdf-theme\">")
        .and_then(|(_, rest)| rest.split_once("</style>"))
        .map_or("", |(css, _)| css)
        .to_owned();
    let math_script = if include_math {
        document
            .html
            .split("src=\"data:text/javascript;base64,")
            .nth(2)
            .and_then(|script| script.split_once('"'))
            .map(|(source, _)| format!("data:text/javascript;base64,{source}"))
    } else {
        None
    };
    Ok(PreviewUpdate {
        body,
        css,
        diagnostics: document.diagnostics,
        math_script,
    })
}
