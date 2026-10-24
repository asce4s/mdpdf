use crate::{RenderError, Result};
use base64::{Engine, prelude::BASE64_STANDARD};
use mdpdf_core::{PrintDocument, Theme, build_print_document};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

/// Shared, build-time vendored assets; production binaries need no source checkout.
fn bundled() -> &'static BTreeMap<String, String> {
    static ASSETS: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    ASSETS.get_or_init(|| {
        BUNDLED_ASSETS
            .iter()
            .map(|(path, mime, data)| {
                (
                    (*path).to_owned(),
                    format!("data:{mime};base64,{}", BASE64_STANDARD.encode(data)),
                )
            })
            .collect()
    })
}

/// The common preview/export delivery pipeline. Randomness and file loading stay
/// outside core. Both outputs use this document unchanged.
pub fn prepare_document(markdown: &str, theme: &Theme) -> Result<PrintDocument> {
    bind(build_print_document(markdown, theme))
}

pub(crate) fn bind(mut document: PrintDocument) -> Result<PrintDocument> {
    // Keep every font family available for local theme switching, but omit
    // Unicode subsets that cannot contribute a glyph to this document.
    let body = document
        .html
        .split("<main>")
        .nth(1)
        .and_then(|body| body.split("</main>").next())
        .unwrap_or(&document.html);
    let points: BTreeSet<u32> = body
        .chars()
        .chain(document.css.chars())
        .chain(document.metadata.title.chars())
        .chain(document.metadata.author.chars())
        .map(u32::from)
        .collect();
    let original = document.css.clone();
    let mut css = String::new();
    let mut rest = original.as_str();
    while let Some(start) = rest.find("@font-face") {
        css.push_str(&rest[..start]);
        let face = &rest[start..];
        let Some(end) = face.find('}') else {
            css.push_str(face);
            rest = "";
            break;
        };
        let face = &face[..=end];
        let used = face
            .split("unicode-range:")
            .nth(1)
            .and_then(|range| range.split(';').next())
            .is_none_or(|ranges| {
                ranges.split(',').any(|range| {
                    let Some(range) = range.trim().strip_prefix("U+") else {
                        return true;
                    };
                    let (low, high) = range.split_once('-').unwrap_or((range, range));
                    let low = low.replace('?', "0");
                    let high = high.replace('?', "F");
                    match (
                        u32::from_str_radix(&low, 16),
                        u32::from_str_radix(&high, 16),
                    ) {
                        (Ok(low), Ok(high)) if low <= high => {
                            points.range(low..=high).next().is_some()
                        }
                        _ => true,
                    }
                })
            });
        if used {
            css.push_str(face);
        }
        rest = &rest[start + end + 1..];
    }
    css.push_str(rest);
    document.html = document.html.replacen(
        &format!("id=\"mdpdf-theme\">{original}</style>"),
        &format!("id=\"mdpdf-theme\">{css}</style>"),
        1,
    );
    document.css = css;
    let mut nonce = [0u8; 24];
    getrandom::fill(&mut nonce).map_err(|error| RenderError::Assets(error.to_string()))?;
    document
        .with_embedded_assets(bundled())
        .and_then(|document| document.with_nonce(&BASE64_STANDARD.encode(nonce)))
        .map_err(RenderError::Assets)
}

#[cfg(test)]
mod tests {
    #[test]
    fn font_subsets_include_glyphs_used_only_in_headers_and_footers() {
        let mut theme = mdpdf_core::Theme::default();
        let plain = super::prepare_document("# Hello", &theme).unwrap();
        theme.header_footer.footer.center = "劉".into();
        let labelled = super::prepare_document("# Hello", &theme).unwrap();
        assert!(
            labelled.css.len() > plain.css.len() + 1000,
            "Footer glyph subset was omitted"
        );
    }
}
