pub mod app;
pub mod model;

#[cfg(all(any(feature = "hydrate", feature = "browser"), target_arch = "wasm32"))]
mod client;
#[cfg(feature = "ssr")]
pub mod server;

#[cfg(all(feature = "hydrate", target_arch = "wasm32"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    leptos::mount::hydrate_body(app::App);
}

#[cfg(all(feature = "browser", target_arch = "wasm32"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn mount_browser() {
    leptos::mount::mount_to_body(app::App);
}

#[cfg(all(feature = "browser", target_arch = "wasm32"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn render_preview_browser(payload: &str) -> Result<String, wasm_bindgen::JsValue> {
    #[derive(serde::Deserialize)]
    struct Request {
        markdown: String,
        theme: mdpdf_core::Theme,
        #[serde(default)]
        images: std::collections::BTreeMap<String, String>,
        #[serde(default)]
        include_shell: bool,
        #[serde(default)]
        nonce: String,
    }
    let request: Request = serde_json::from_str(payload)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    let images = mdpdf_core::images::prepare_images(&request.markdown, &request.images);
    let mut document = images.embed(mdpdf_core::build_print_document(
        &images.markdown,
        &request.theme,
    ));
    for diagnostic in &mut document.diagnostics {
        if diagnostic.kind == mdpdf_core::DiagKind::Image {
            diagnostic.message = "Image not available locally. Drop the referenced image file here or choose Insert → Image. Remote images need browser CORS permission.".into();
        }
    }
    let body = document
        .html
        .split_once("<main>")
        .and_then(|(_, rest)| rest.split_once("</main>"))
        .map_or("", |(body, _)| body)
        .to_owned();
    let (theme, _, _) = mdpdf_core::document_theme(&request.markdown, &request.theme);
    let shell = if request.include_shell {
        Some(
            document
                .clone()
                .with_nonce(&request.nonce)
                .map_err(|error| wasm_bindgen::JsValue::from_str(&error))?
                .html,
        )
    } else {
        None
    };
    Ok(serde_json::json!({
        "body":body, "css":document.css, "theme":theme,
        "metadata":document.metadata, "config":mdpdf_core::css::mermaid_config(&theme),
        "diagnostics":document.diagnostics, "unresolvedImages":images.unresolved,
        "hasMath":document.html.contains("src=\"/assets/vendor/katex/katex.min.js\""),
        "shell":shell,
    })
    .to_string())
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
