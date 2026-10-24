//! Pure Markdown → print-document pipeline shared by preview and PDF export.
//! No runtime file, network, clock, randomness, or asynchronous dependencies.
pub mod css;
pub mod diagnostics;
pub mod frontmatter;
mod parse;
pub mod theme;
pub mod toc;
mod transform;

pub use diagnostics::{DiagKind, Diagnostic, Severity};
pub use frontmatter::Metadata;
pub use theme::Theme;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintDocument {
    pub html: String,
    pub css: String,
    pub metadata: Metadata,
    pub diagnostics: Vec<Diagnostic>,
    pub headings: Vec<toc::Heading>,
    pub diagrams: Vec<Diagram>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagram {
    pub id: String,
    pub source: String,
    pub source_line: u32,
}

pub fn build_print_document(md: &str, theme: &Theme) -> PrintDocument {
    parse::build(md, theme)
}

/// Apply document front matter for lightweight client-side theme compilation.
pub fn document_theme(md: &str, theme: &Theme) -> (Theme, Metadata, Vec<Diagnostic>) {
    let front = frontmatter::extract(md, theme);
    (front.theme, front.metadata, front.diagnostics)
}

pub(crate) fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

impl PrintDocument {
    /// Embed delivery-supplied, base64-encoded vendor assets without I/O in core.
    /// This makes fonts and Mermaid work in an opaque sandboxed srcdoc iframe.
    pub fn with_embedded_assets(
        mut self,
        assets: &std::collections::BTreeMap<String, String>,
    ) -> Result<Self, String> {
        fn data_url<'a>(
            assets: &'a std::collections::BTreeMap<String, String>,
            path: &str,
            mime: &str,
        ) -> Result<&'a str, String> {
            let value = assets
                .get(path)
                .ok_or_else(|| format!("Missing asset: {path}"))?;
            let prefix = format!("data:{mime};base64,");
            let payload = value
                .strip_prefix(&prefix)
                .ok_or_else(|| format!("Invalid asset MIME type: {path}"))?;
            if payload.is_empty()
                || !payload
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
            {
                return Err(format!("Invalid asset encoding: {path}"));
            }
            Ok(value)
        }
        let mermaid = data_url(
            assets,
            "/assets/vendor/mermaid/mermaid.min.js",
            "text/javascript",
        )?;
        let mut css = String::with_capacity(self.css.len());
        let mut parts = self.css.split("url(");
        css.push_str(parts.next().unwrap_or_default());
        for part in parts {
            css.push_str("url(");
            let Some((argument, rest)) = part.split_once(')') else {
                css.push_str(part);
                continue;
            };
            let path = argument.trim_matches(['\'', '"']);
            if path.starts_with("/assets/fonts/") || path.starts_with("/assets/vendor/katex/fonts/")
            {
                css.push_str(data_url(assets, path, "font/woff2")?);
            } else {
                css.push_str(argument);
            }
            css.push(')');
            css.push_str(rest);
        }
        self.html = self
            .html
            .replacen(
                &format!("id=\"mdpdf-theme\">{}</style>", self.css),
                &format!("id=\"mdpdf-theme\">{css}</style>"),
                1,
            )
            .replacen(
                "src=\"/assets/vendor/mermaid/mermaid.min.js\"",
                &format!("src=\"{mermaid}\""),
                1,
            )
            .replacen("font-src 'self'", "font-src data:", 1);
        if self
            .html
            .contains("src=\"/assets/vendor/katex/katex.min.js\"")
        {
            let katex = data_url(
                assets,
                "/assets/vendor/katex/katex.min.js",
                "text/javascript",
            )?;
            self.html = self.html.replacen(
                "src=\"/assets/vendor/katex/katex.min.js\"",
                &format!("src=\"{katex}\""),
                1,
            );
        }
        self.css = css;
        Ok(self)
    }

    /// Bind a fresh, unpredictable nonce supplied by the native delivery layer.
    /// The pure core deliberately does not access randomness. Preview and PDF
    /// delivery must both call this before sending HTML to a browser.
    pub fn with_nonce(mut self, nonce: &str) -> Result<Self, String> {
        if nonce.len() < 22
            || nonce.len() > 128
            || !nonce.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=' | b'-' | b'_')
            })
        {
            return Err("CSP nonce must contain 22–128 base64 characters".into());
        }
        self.html = self
            .html
            .replace("nonce-MDPDF_NONCE_PLACEHOLDER", &format!("nonce-{nonce}"))
            .replace(
                "nonce=\"MDPDF_NONCE_PLACEHOLDER\"",
                &format!("nonce=\"{nonce}\""),
            );
        Ok(self)
    }
}
