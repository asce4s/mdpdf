pub mod app;
pub mod model;

#[cfg(all(feature = "hydrate", target_arch = "wasm32"))]
mod client;
#[cfg(feature = "ssr")]
pub mod server;

#[cfg(all(feature = "hydrate", target_arch = "wasm32"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    leptos::mount::hydrate_body(app::App);
}

/// Pure rendering entry point for the preview worker. No DOM or Leptos runtime
/// is needed, so parsing and syntax highlighting stay off the UI thread.
#[cfg(all(feature = "hydrate", target_arch = "wasm32"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn render_preview_local(payload: &str) -> Result<String, wasm_bindgen::JsValue> {
    let request: model::DocumentRequest = serde_json::from_str(payload)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    let document = mdpdf_core::build_print_document(&request.markdown, &request.theme);
    let body = document
        .html
        .split_once("<main>")
        .and_then(|(_, rest)| rest.split_once("</main>"))
        .map_or("", |(body, _)| body);
    let (theme, metadata, _) = mdpdf_core::document_theme(&request.markdown, &request.theme);
    let serialized_theme = serde_json::to_string(&theme).unwrap_or_default();
    let characters: String = request
        .markdown
        .chars()
        .chain(serialized_theme.chars())
        .filter(|character| !character.is_ascii())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    // The persistent frame retains embedded font faces from native delivery.
    let mut css = String::new();
    let mut rest = document.css.as_str();
    while let Some(start) = rest.find("@font-face") {
        css.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest.find('}') else { break };
        rest = &rest[end + 1..];
    }
    css.push_str(rest);
    Ok(serde_json::json!({
        "body": body, "css": css, "theme": theme, "metadata": metadata,
        "config": mdpdf_core::css::mermaid_config(&theme),
        "diagnostics": document.diagnostics,
        "characters": characters,
        "hasMath": document.html.contains("src=\"/assets/vendor/katex/katex.min.js\""),
        // Native delivery resolves uploaded, local and remote image sources.
        "requiresServer": body.contains("src=\"/assets/") || document.diagnostics.iter().any(|item| item.kind == mdpdf_core::DiagKind::Image),
    })
    .to_string())
}
