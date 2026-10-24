use leptos::prelude::*;
use mdpdf_core::{Metadata, Theme, css};
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

pub fn install_bridge() {
    let markdown = RwSignal::new(String::new());
    let theme = RwSignal::new(Theme::default());
    let styling = Memo::new(move |_| {
        let (effective, metadata, mut diagnostics) =
            mdpdf_core::document_theme(&markdown.get(), &theme.get());
        if effective
            .custom_css
            .as_deref()
            .is_some_and(|value| css::sanitize_custom_css(value).is_none())
        {
            diagnostics.push(mdpdf_core::Diagnostic::error(mdpdf_core::DiagKind::Html,"Custom CSS rejected: remove URLs, at-rules, escapes, comments and HTML; maximum 50 KB",None));
        }
        let mut css = css::compile_css(&effective, &metadata);
        if markdown.get().contains('$') {
            css.insert_str(
                0,
                include_str!("../../../assets/vendor/katex/katex.min.css"),
            );
        }
        // Font faces are retained in a separate, embedded stylesheet by the frame.
        let mut stripped = String::new();
        let mut rest = css.as_str();
        while let Some(start) = rest.find("@font-face") {
            stripped.push_str(&rest[..start]);
            rest = &rest[start..];
            if let Some(end) = rest.find('}') {
                rest = &rest[end + 1..];
            } else {
                break;
            }
        }
        stripped.push_str(rest);
        json!({"css": stripped, "config": css::mermaid_config(&effective), "theme": effective, "metadata":metadata, "diagnostics":diagnostics}).to_string()
    });
    Effect::new(move |_| {
        let callback = Closure::<dyn FnMut(String, String) -> String>::new(
            move |action: String, payload: String| {
                let result: Result<String, String> = (|| match action.as_str() {
                    "compile" => {
                        let input: Value =
                            serde_json::from_str(&payload).map_err(|e| e.to_string())?;
                        markdown.set(input["markdown"].as_str().unwrap_or_default().to_owned());
                        theme.set(
                            serde_json::from_value(input["theme"].clone())
                                .map_err(|e| e.to_string())?,
                        );
                        Ok(styling.get())
                    }
                    "body" => {
                        let input: Value =
                            serde_json::from_str(&payload).map_err(|error| error.to_string())?;
                        let theme: Theme = serde_json::from_value(input["theme"].clone())
                            .map_err(|error| error.to_string())?;
                        let document = mdpdf_core::build_print_document(
                            input["markdown"].as_str().unwrap_or_default(),
                            &theme,
                        );
                        let body = document
                            .html
                            .split_once("<main>")
                            .and_then(|(_, rest)| rest.split_once("</main>"))
                            .map_or("", |(body, _)| body);
                        Ok(json!({"body":body}).to_string())
                    }
                    "preset" => Ok(serde_json::to_string(&Theme::preset(&payload)?).unwrap()),
                    "import" => Ok(serde_json::to_string(&Theme::from_toml(&payload)?).unwrap()),
                    "export" => serde_json::from_str::<Theme>(&payload)
                        .map_err(|e| e.to_string())?
                        .to_toml(),
                    "default" => Ok(serde_json::to_string(&Theme::default()).unwrap()),
                    "metadata" => Ok(serde_json::to_string(&Metadata::default()).unwrap()),
                    _ => Err("Unknown UI operation".into()),
                })();
                result.unwrap_or_else(|error| json!({"error":error}).to_string())
            },
        );
        if let Some(window) = web_sys::window() {
            let _ =
                js_sys::Reflect::set(&window, &JsValue::from_str("mdpdfCore"), callback.as_ref());
            callback.forget();
            if let Ok(event) = web_sys::CustomEvent::new("mdpdf-ready") {
                let _ = window.dispatch_event(&event);
            }
        }
    });
}
