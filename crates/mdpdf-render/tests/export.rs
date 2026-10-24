use lopdf::{Document, Object};
use mdpdf_core::{DiagKind, Theme};
use mdpdf_render::{BrowserPool, ExportOptions, PoolConfig, RenderError};
use serde_json::json;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

fn fixture(name: &str) -> String {
    fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../mdpdf-core/tests/fixtures/{name}.md")),
    )
    .unwrap()
}
fn text(pdf: &Document, pages: &[u32]) -> String {
    // Poppler handles Chromium's subset-font CMaps correctly. lopdf remains
    // responsible for structural, page, metadata and vector assertions.
    let mut bytes = Vec::new();
    pdf.clone().save_to(&mut bytes).unwrap();
    let mut result = String::new();
    for page in pages {
        let mut child = Command::new("pdftotext")
            .args(["-f", &page.to_string(), "-l", &page.to_string(), "-", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("install Poppler pdftotext for render integration tests");
        child.stdin.take().unwrap().write_all(&bytes).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        result.push_str(&String::from_utf8(output.stdout).unwrap());
    }
    result
}
fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}
fn page_size(pdf: &Document, page: &Object) -> (f32, f32) {
    let mut page = page.as_dict().unwrap();
    loop {
        if let Ok(size) = page.get(b"MediaBox") {
            let size = size.as_array().unwrap();
            return (size[2].as_float().unwrap(), size[3].as_float().unwrap());
        }
        page = pdf
            .get_dictionary(page.get(b"Parent").unwrap().as_reference().unwrap())
            .unwrap();
    }
}

#[tokio::test]
#[ignore = "requires installed Chromium; run cargo test -p mdpdf-render --test export -- --ignored --nocapture"]
async fn all_fixtures_and_pdf_contracts() {
    let pool = BrowserPool::new(PoolConfig {
        size: 2,
        ..Default::default()
    })
    .await
    .unwrap();
    assert!(pool.is_ready().await);
    let default = Theme::default();
    for name in [
        "basic",
        "gfm-tables",
        "wide-table",
        "code",
        "mermaid-all",
        "mermaid-error",
        "mermaid-init-directive",
        "fonts-cjk",
        "images",
        "frontmatter-overrides",
        "toc-numbering",
        "admonitions",
        "page-breaks",
        "xss",
        "math-cover",
    ] {
        let exported = pool
            .export(&fixture(name), &default, ExportOptions::default())
            .await
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let pdf = Document::load_mem(&exported.bytes).unwrap();
        let pages = pdf.get_pages();
        assert!(!pages.is_empty(), "{name}");
        assert_eq!(exported.page_count, pages.len());
        let all_text = text(&pdf, &pages.keys().copied().collect::<Vec<_>>());
        assert!(!all_text.trim().is_empty(), "{name}: no extractable text");
        let page = pdf.get_object(*pages.values().next().unwrap()).unwrap();
        let (width, height) = page_size(&pdf, page);
        let expected = if name == "frontmatter-overrides" {
            (612.0, 792.0)
        } else {
            (595.28, 841.89)
        };
        assert!(
            (width - expected.0).abs() < 1.0 && (height - expected.1).abs() < 1.0,
            "{name}: {width}x{height}"
        );
        let root = pdf.catalog().unwrap();
        assert!(
            root.get(b"StructTreeRoot").is_ok(),
            "{name}: missing PDF tags"
        );
        assert!(
            root.get(b"Outlines").is_ok(),
            "{name}: missing heading outline"
        );
        if name == "basic" {
            assert!(
                all_text.contains("bold") && all_text.contains("statement"),
                "{all_text:?}"
            );
            assert!(
                pdf.get_toc()
                    .unwrap()
                    .toc
                    .iter()
                    .any(|entry| entry.title.contains("Markdown print"))
            );
        }
        if name == "gfm-tables" {
            assert!(pages.len() > 1);
            let page_text: Vec<_> = pages.keys().map(|number| text(&pdf, &[*number])).collect();
            for body in &page_text {
                assert!(
                    compact(body).contains("LeftCenterRight"),
                    "missing repeating header: {body}"
                );
            }
            for row in 1..=200 {
                let marker = row.to_string();
                let matches: Vec<_> = page_text
                    .iter()
                    .filter(|body| {
                        body.split_whitespace()
                            .collect::<Vec<_>>()
                            .windows(2)
                            .any(|pair| pair[0] == "Row" && pair[1] == marker)
                    })
                    .collect();
                assert_eq!(matches.len(), 1, "row {row} is missing or split");
                assert!(
                    matches[0]
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .windows(2)
                        .any(|pair| pair[0] == "Center" && pair[1] == marker),
                    "row {row} split across pages"
                );
            }
        }
        if name == "math-cover" {
            assert_eq!(pages.len(), 2);
            assert!(text(&pdf, &[1]).contains("Equations and ideas"));
            assert!(text(&pdf, &[2]).contains("Euler"));
        }
        if name == "mermaid-all" {
            assert!(
                exported.diagnostics.is_empty(),
                "{:?}",
                exported.diagnostics
            );
            for label in [
                "Decision",
                "Alice",
                "Customer",
                "Renderer",
                "Shared renderer",
            ] {
                assert!(
                    all_text.to_lowercase().contains(&label.to_lowercase()),
                    "missing vector label {label}"
                );
            }
            assert!(
                !pdf.objects
                    .values()
                    .any(|object| object.as_stream().is_ok_and(|stream| stream
                        .dict
                        .get(b"Subtype")
                        .is_ok_and(|value| value.as_name().ok() == Some(b"Image")))),
                "diagram rasterized into an image"
            );
        }
        if name == "mermaid-error" {
            assert_eq!(
                exported
                    .diagnostics
                    .iter()
                    .filter(|d| d.kind == DiagKind::Mermaid)
                    .count(),
                1
            );
            assert!(all_text.contains("Diagram error"));
            assert!(all_text.contains("Still render"));
        }
        if name == "fonts-cjk" {
            assert!(
                all_text.contains("中文排版测试"),
                "CJK text is not extractable: {all_text}"
            );
            assert!(
                pdf.objects
                    .values()
                    .any(|object| object.as_dict().is_ok_and(|dict| dict
                        .get(b"FontFile2")
                        .is_ok()
                        || dict.get(b"FontFile3").is_ok()
                        || (dict
                            .get(b"Subtype")
                            .is_ok_and(|kind| kind.as_name().ok() == Some(b"Type3"))
                            && dict.get(b"CharProcs").is_ok()))),
                "fonts not embedded"
            );
        }
        if name == "frontmatter-overrides" {
            let info = pdf
                .get_dictionary(pdf.trailer.get(b"Info").unwrap().as_reference().unwrap())
                .unwrap();
            assert_eq!(
                info.get(b"Title").unwrap().as_str().unwrap(),
                [0xfe, 0xff]
                    .into_iter()
                    .chain("Example report".encode_utf16().flat_map(u16::to_be_bytes))
                    .collect::<Vec<_>>()
            );
            assert!(info.get(b"Author").is_ok());
            assert!(info.get(b"Subject").is_ok());
            assert!(info.get(b"Keywords").is_ok());
        }
        if name == "page-breaks" {
            assert_eq!(pages.len(), 3);
        }
        if name == "xss" {
            assert!(all_text.contains("formatting"));
        }
        eprintln!(
            "{name}: {} pages, {} bytes",
            pages.len(),
            exported.bytes.len()
        );
    }
    let strict = pool
        .export(
            &fixture("mermaid-error"),
            &default,
            ExportOptions {
                strict_diagrams: true,
            },
        )
        .await;
    assert!(
        matches!(strict, Err(RenderError::DiagramErrors(ref errors)) if errors.len() == 1 && errors[0].source_line == Some(3))
    );
    // The tab remains usable after strict-mode rejection.
    pool.export("# After strict error", &default, ExportOptions::default())
        .await
        .unwrap();
    let numbered = default
        .merged(json!({"header_footer": {"footer": {"center": "Page {page} of {pages}"}}}))
        .unwrap();
    let result = pool
        .export(
            "# One\n\n<!-- pagebreak -->\n\n# Two\n\n<!-- pagebreak -->\n\n# Three",
            &numbered,
            ExportOptions::default(),
        )
        .await
        .unwrap();
    let pdf = Document::load_mem(&result.bytes).unwrap();
    assert_eq!(result.page_count, 3);
    for number in 1..=3 {
        assert!(compact(&text(&pdf, &[number])).contains(&format!("Page{number}of3")));
    }
    let (one, two, three) = tokio::join!(
        pool.export("# Job one", &default, ExportOptions::default()),
        pool.export("# Job two", &default, ExportOptions::default()),
        pool.export("# Job three", &default, ExportOptions::default())
    );
    for (result, expected) in [(one, "Job one"), (two, "Job two"), (three, "Job three")] {
        assert!(
            text(&Document::load_mem(&result.unwrap().bytes).unwrap(), &[1]).contains(expected)
        );
    }
    let mut section_theme = Theme::default();
    section_theme.header_footer.footer.left = "Section: {section}".into();
    section_theme.header_footer.footer.right = "Page {page} of {pages}".into();
    let sections = pool.export("# Alpha\n\nFirst.\n\n<!-- pagebreak -->\n\nContinuation.\n\n<!-- pagebreak -->\n\n# Beta\n\nLast.", &section_theme, ExportOptions::default()).await.unwrap();
    let pdf = Document::load_mem(&sections.bytes).unwrap();
    assert_eq!(pdf.get_pages().len(), 3);
    for (page, section) in [(1, "Alpha"), (2, "Alpha"), (3, "Beta")] {
        let content = text(&pdf, &[page]);
        assert!(
            content.contains(&format!("Section: {section}")),
            "{content}"
        );
        assert!(content.contains(&format!("Page {page} of 3")), "{content}");
    }
    assert!(pdf.catalog().unwrap().get(b"Outlines").is_ok());
    assert!(pdf.catalog().unwrap().get(b"StructTreeRoot").is_ok());
    pool.close().await;
    assert!(!pool.is_ready().await);
    assert!(matches!(
        pool.export("# Closed", &default, ExportOptions::default())
            .await,
        Err(RenderError::Closed)
    ));
}
