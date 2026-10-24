use mdpdf_core::{DiagKind, Theme, build_print_document};
use serde_json::json;

macro_rules! fixture {
    ($name:ident, $file:literal) => {
        #[test]
        fn $name() {
            let document = build_print_document(
                include_str!(concat!("fixtures/", $file, ".md")),
                &Theme::default(),
            );
            insta::assert_snapshot!($file, document.html);
        }
    };
}
fixture!(basic, "basic");
fixture!(gfm_tables, "gfm-tables");
fixture!(wide_table, "wide-table");
fixture!(code, "code");
fixture!(mermaid_all, "mermaid-all");
fixture!(mermaid_error, "mermaid-error");
fixture!(mermaid_init_directive, "mermaid-init-directive");
fixture!(fonts_cjk, "fonts-cjk");
fixture!(images, "images");
fixture!(frontmatter_overrides, "frontmatter-overrides");
fixture!(toc_numbering, "toc-numbering");
fixture!(admonitions, "admonitions");
fixture!(page_breaks, "page-breaks");
fixture!(xss, "xss");
fixture!(math_cover, "math-cover");

#[test]
fn raw_html_cannot_inject_scripts_attributes_or_urls() {
    let document = build_print_document(include_str!("fixtures/xss.md"), &Theme::default());
    let body = document
        .html
        .split("<main>")
        .nth(1)
        .unwrap()
        .split("</main>")
        .next()
        .unwrap();
    for forbidden in [
        "<script",
        "onerror",
        "onload",
        "onfocus",
        "javascript:",
        "<iframe",
        "<style",
        "<svg",
        "<form",
    ] {
        assert!(!body.contains(forbidden), "{forbidden}: {body}");
    }
    assert!(body.contains("<strong>formatting</strong>"));
}

#[test]
fn frontmatter_precedence_and_source_lines() {
    let mut theme = Theme::default();
    theme.typography.base_size = 13.0;
    theme.tables.zebra = false;
    let document = build_print_document(
        "---\ntitle: Report\ntoc: true\ntables:\n  style: grid\n---\n# Heading\n",
        &theme,
    );
    assert_eq!(document.metadata.title, "Report");
    assert!(document.css.contains("13pt"));
    assert!(!document.css.contains("nth-child(even)"));
    assert!(document.html.contains("Table of contents"));
    assert_eq!(document.headings[0].source_line, 7);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let invalid = build_print_document("---\npage: [invalid\n---\n# Safe\n", &theme);
    assert!(
        invalid
            .diagnostics
            .iter()
            .any(|d| d.kind == DiagKind::FrontMatter)
    );
    assert!(invalid.html.contains("Safe"));
}

#[test]
fn mermaid_source_is_preserved_with_attributes_and_safe_captions() {
    let md = "```mermaid {width=80% align=right caption=\"Login flow\" theme=dark}\n%%{init: {\"theme\": \"forest\"}}%%\nflowchart LR\n  A --> B\n```";
    let document = build_print_document(md, &Theme::default());
    assert_eq!(document.diagrams.len(), 1);
    assert!(document.diagrams[0].source.starts_with("%%{init:"));
    assert_eq!(document.diagrams[0].source_line, 1);
    for value in [
        "data-width=\"80\"",
        "data-align=\"right\"",
        "data-mermaid-theme=\"dark\"",
        "Login flow",
        "class=\"mermaid\"",
    ] {
        assert!(document.html.contains(value), "{value}");
    }
    let fonts = document.html.find("await document.fonts.ready").unwrap();
    let run = document.html.find("await timeout(mermaid.render").unwrap();
    let ready = document.html.find("window.__MDPDF_READY__ = true").unwrap();
    assert!(fonts < run && run < ready);
}

#[test]
fn page_markers_only_transform_outside_code_and_toc_ids_are_unique() {
    let document = build_print_document(include_str!("fixtures/page-breaks.md"), &Theme::default());
    let body = document
        .html
        .split("<main>")
        .nth(1)
        .unwrap()
        .split("</main>")
        .next()
        .unwrap();
    assert_eq!(body.matches("class=\"mdpdf-page-break\"").count(), 2);
    assert!(body.contains("&lt;!-- pagebreak --&gt;"));
    let theme = Theme::default()
        .merged(json!({"page": {"heading_numbers": true}}))
        .unwrap();
    let document = build_print_document("[[toc]]\n\n# Same\n\n## Child\n\n# Same\n", &theme);
    assert_eq!(document.headings[0].id, "same");
    assert_eq!(document.headings[2].id, "same-1");
    assert_eq!(document.headings[1].number.as_deref(), Some("1.1"));
    assert_eq!(document.headings[2].number.as_deref(), Some("2"));
    assert!(document.html.contains("href=\"#same-1\""));
}

#[test]
fn resource_limits_do_not_truncate_unicode_or_execute_excess_diagrams() {
    let large = "中".repeat(700_000);
    let document = build_print_document(&large, &Theme::default());
    assert!(
        document
            .diagnostics
            .iter()
            .any(|d| d.message.contains("2 MB"))
    );
    let many = "```mermaid\nflowchart LR\n A-->B\n```\n\n".repeat(201);
    let document = build_print_document(&many, &Theme::default());
    assert_eq!(document.diagrams.len(), 200);
    assert!(
        document
            .diagnostics
            .iter()
            .any(|d| d.kind == DiagKind::Mermaid)
    );
}

#[test]
fn image_attributes_caption_and_asset_resolution() {
    let document = build_print_document(
        "<!-- caption: A caption -->\n\n![Alt](asset://abc123){width=50%}\n",
        &Theme::default(),
    );
    assert!(document.html.contains("/assets/abc123"));
    assert!(document.html.contains("data-width=\"50\""));
    assert!(document.html.contains("A caption"));
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
}

#[test]
fn delivery_binds_a_nonce_without_rewriting_markdown_text() {
    let document = build_print_document("MDPDF_NONCE_PLACEHOLDER", &Theme::default());
    assert!(document.clone().with_nonce("bad\" nonce").is_err());
    let nonce = "0123456789abcdef0123456789abcdef";
    let delivered = document.with_nonce(nonce).unwrap();
    assert!(
        delivered
            .html
            .contains(&format!("script-src 'nonce-{nonce}'"))
    );
    assert!(delivered.html.contains(&format!("nonce=\"{nonce}\"")));
    assert!(delivered.html.contains(">MDPDF_NONCE_PLACEHOLDER</p>"));
    assert!(!delivered.html.contains("nonce=\"MDPDF_NONCE_PLACEHOLDER\""));
}

#[test]
fn embedded_assets_are_complete_typed_and_do_not_rewrite_markdown() {
    use std::collections::BTreeMap;
    let document =
        build_print_document("`/assets/vendor/mermaid/mermaid.min.js`", &Theme::default());
    assert!(
        document
            .clone()
            .with_embedded_assets(&BTreeMap::new())
            .is_err()
    );
    let mut assets = BTreeMap::from([(
        "/assets/vendor/mermaid/mermaid.min.js".to_owned(),
        "data:text/javascript;base64,dGVzdA==".to_owned(),
    )]);
    for part in document.css.split("url(").skip(1) {
        let path = part.split(')').next().unwrap().trim_matches(['\'', '"']);
        if path.starts_with("/assets/fonts/") {
            assets.insert(path.to_owned(), "data:font/woff2;base64,d09GMg==".into());
        }
    }
    let embedded = document.clone().with_embedded_assets(&assets).unwrap();
    assert!(!embedded.css.contains("url(/assets/"));
    assert!(embedded.html.contains("font-src data:"));
    assert!(
        embedded
            .html
            .contains("src=\"data:text/javascript;base64,dGVzdA==\"")
    );
    assert!(
        embedded
            .html
            .contains("/assets/vendor/mermaid/mermaid.min.js</code>")
    );
    assets.insert(
        "/assets/vendor/mermaid/mermaid.min.js".into(),
        "data:text/javascript;base64,\" onload=\"bad".into(),
    );
    assert!(document.with_embedded_assets(&assets).is_err());
}

#[test]
fn math_and_cover_use_the_common_document_without_trusting_tex() {
    let doc = mdpdf_core::build_print_document(
        "---\ntitle: Equations\nauthor: Ada\ncover: true\n---\n# Math\n\nInline $e^{i\\pi}+1=0$ and block:\n\n$$\\sum_{n=1}^{10} n$$\n",
        &mdpdf_core::Theme::default(),
    );
    assert!(doc.metadata.cover);
    assert!(doc.html.contains("class=\"mdpdf-cover\""));
    assert!(doc.html.contains("data-display=\"false\""));
    assert!(doc.html.contains("data-display=\"true\""));
    assert!(doc.html.contains("/assets/vendor/katex/katex.min.js"));
    assert!(
        !mdpdf_core::build_print_document("# Plain", &mdpdf_core::Theme::default())
            .html
            .contains("src=\"/assets/vendor/katex/")
    );
}
