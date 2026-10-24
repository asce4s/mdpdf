use crate::{DiagKind, Diagnostic, PrintDocument, Theme, css, escape, frontmatter, toc, transform};
use comrak::{
    Arena, Options, format_html,
    nodes::{AstNode, NodeValue},
    parse_document,
};
use std::collections::BTreeSet;
use std::sync::OnceLock;
use syntect::parsing::SyntaxSet;

fn syntaxes() -> &'static SyntaxSet {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn plain_text<'a>(node: &'a AstNode<'a>) -> String {
    node.descendants()
        .filter_map(|child| match &child.data.borrow().value {
            NodeValue::Text(text) => Some(text.to_string()),
            NodeValue::Code(code) => Some(code.literal.clone()),
            NodeValue::SoftBreak | NodeValue::LineBreak => Some(" ".into()),
            _ => None,
        })
        .collect()
}
fn raw<'a>(node: &'a AstNode<'a>, html: String) {
    for child in node.children().collect::<Vec<_>>() {
        child.detach();
    }
    node.data.borrow_mut().value = NodeValue::Raw(html);
}
fn clean(html: &str) -> String {
    ammonia::Builder::default()
        .add_tags(&["section", "figure", "figcaption", "nav", "input"])
        .add_generic_attributes(&[
            "class",
            "id",
            "data-display",
            "data-source-line",
            "data-sourcepos",
            "data-width",
            "data-align",
            "data-mermaid-theme",
            "data-diagram-id",
            "aria-label",
            "aria-hidden",
        ])
        .add_url_schemes(&["data"])
        .attribute_filter(|tag, attribute, value| {
            if value.to_ascii_lowercase().starts_with("data:")
                && !(tag == "img"
                    && attribute == "src"
                    && [
                        "data:image/png;base64,",
                        "data:image/jpeg;base64,",
                        "data:image/webp;base64,",
                    ]
                    .iter()
                    .any(|prefix| value.starts_with(prefix)))
            {
                None
            } else {
                Some(value.into())
            }
        })
        .add_tag_attributes("input", &["type", "checked", "disabled"])
        .add_tag_attributes("th", &["align"])
        .add_tag_attributes("td", &["align"])
        .add_tag_attributes("img", &["width", "height"])
        .clean(html)
        .to_string()
}

pub(crate) fn build(md: &str, requested: &Theme) -> PrintDocument {
    let front = frontmatter::extract(if md.len() <= 2 * 1024 * 1024 { md } else { "" }, requested);
    let theme = front.theme;
    let mut diagnostics = front.diagnostics;
    if md.len() > 2 * 1024 * 1024 {
        diagnostics.push(Diagnostic::error(
            DiagKind::Html,
            "Markdown exceeds 2 MB limit",
            None,
        ));
    }
    if theme
        .custom_css
        .as_deref()
        .is_some_and(|css| css::sanitize_custom_css(css).is_none())
    {
        diagnostics.push(Diagnostic::error(DiagKind::Html, "Custom CSS rejected: use at most 50 KB without at-rules, URLs, escapes, comments or HTML", None));
    }
    let mut options = Options::default();
    options.extension.math_dollars = true;
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.strikethrough = true;
    options.extension.footnotes = true;
    options.extension.alerts = true;
    options.render.r#unsafe = true;
    options.render.sourcepos = true;
    let arena = Arena::new();
    let root = parse_document(&arena, front.body, &options);
    let nodes: Vec<_> = root.descendants().collect();
    let mut headings = Vec::new();
    let mut ids = BTreeSet::new();
    let mut counts = [0u32; 6];
    // Collect headings before replacing TOC markers.
    for node in &nodes {
        let value = node.data.borrow().value.clone();
        if let NodeValue::Heading(heading) = value {
            let level = heading.level.clamp(1, 6);
            let text = plain_text(node);
            let id = toc::slug(&text, &mut ids);
            let index = usize::from(level - 1);
            counts[index] += 1;
            counts[index + 1..].fill(0);
            let number = theme.page.heading_numbers.then(|| {
                counts[..=index]
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(".")
            });
            let line = node.data.borrow().sourcepos.start.line + front.line_offset;
            headings.push(toc::Heading {
                level,
                id,
                text,
                number,
                source_line: line as u32,
            });
        }
    }
    let mut diagrams = Vec::new();
    let mut heading_index = 0;
    let mut toc_inserted = false;
    let mut image_count = 0;
    let mut has_math = false;
    let mut caption = None;
    for node in nodes {
        let value = node.data.borrow().value.clone();
        let line = (node.data.borrow().sourcepos.start.line + front.line_offset) as u32;
        match value {
            NodeValue::Math(math) => {
                has_math = true;
                raw(
                    node,
                    format!(
                        "<span class=\"mdpdf-math\" data-display=\"{}\" data-source-line=\"{line}\">{}</span>",
                        math.display_math,
                        escape(&math.literal)
                    ),
                );
            }
            NodeValue::Heading(_) => {
                let heading = &headings[heading_index];
                heading_index += 1;
                let mut inner = String::new();
                for child in node.children() {
                    let _ = format_html(child, &options, &mut inner);
                }
                let number = heading
                    .number
                    .as_ref()
                    .map_or(String::new(), |n| format!("{} ", escape(n)));
                raw(
                    node,
                    format!(
                        "<h{} id=\"{}\" data-source-line=\"{line}\">{number}{inner}</h{}>\n",
                        heading.level,
                        escape(&heading.id),
                        heading.level
                    ),
                );
            }
            NodeValue::CodeBlock(block) => {
                let language = block.info.split_whitespace().next().unwrap_or("");
                if language == "mermaid" {
                    if diagrams.len() >= 200 {
                        diagnostics.push(Diagnostic::error(
                            DiagKind::Mermaid,
                            "Document exceeds 200 diagrams",
                            Some(line),
                        ));
                        raw(
                            node,
                            "<div class=\"mdpdf-error\">Diagram limit exceeded</div>".into(),
                        );
                    } else {
                        let (html, diagram) = transform::diagram(
                            &block.literal,
                            &block.info,
                            line,
                            diagrams.len() + 1,
                            &theme,
                        );
                        diagrams.push(diagram);
                        raw(node, html);
                    }
                } else {
                    raw(
                        node,
                        transform::highlight(
                            &block.literal,
                            language,
                            line,
                            &theme,
                            syntaxes(),
                            &mut diagnostics,
                        ),
                    );
                }
            }
            NodeValue::HtmlBlock(block)
                if block.literal.trim().starts_with("<!-- caption:")
                    && block.literal.trim().ends_with("-->") =>
            {
                caption = Some(
                    block
                        .literal
                        .trim()
                        .trim_start_matches("<!-- caption:")
                        .trim_end_matches("-->")
                        .trim()
                        .to_owned(),
                );
                raw(node, String::new());
            }
            NodeValue::HtmlBlock(block) if block.literal.trim() == "<!-- pagebreak -->" => raw(
                node,
                format!("<div class=\"mdpdf-page-break\" data-source-line=\"{line}\"></div>\n"),
            ),
            NodeValue::Paragraph if plain_text(node).trim() == "[[toc]]" => {
                raw(node, toc::render(&headings, theme.page.toc_depth));
                toc_inserted = true;
            }
            NodeValue::Paragraph
                if front
                    .body
                    .lines()
                    .nth(node.data.borrow().sourcepos.start.line.saturating_sub(1))
                    .is_some_and(|line| line.trim() == "\\newpage") =>
            {
                raw(
                    node,
                    format!("<div class=\"mdpdf-page-break\" data-source-line=\"{line}\"></div>\n"),
                )
            }
            NodeValue::Image(mut link) => {
                image_count += 1;
                if let Some(caption) = caption.take() {
                    link.title = caption;
                }
                let alt = plain_text(node);
                let mut attrs = std::collections::BTreeMap::new();
                if let Some(next) = node.next_sibling() {
                    let mut data = next.data.borrow_mut();
                    if let NodeValue::Text(text) = &mut data.value
                        && text.starts_with('{')
                        && let Some(end) = text.find('}')
                    {
                        attrs = transform::attributes(&text[..=end]);
                        *text = text[end + 1..].to_owned().into();
                    }
                }
                if let Some(id) = link.url.strip_prefix("asset://") {
                    if id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') && !id.is_empty()
                    {
                        link.url = format!("/assets/{id}");
                    } else {
                        link.url.clear();
                    }
                }
                if image_count > 50
                    || link.url.starts_with("http:")
                    || link.url.starts_with("https:")
                    || !(link.url.starts_with("/assets/")
                        || link.url.starts_with("data:image/png;base64,")
                        || link.url.starts_with("data:image/jpeg;base64,")
                        || link.url.starts_with("data:image/webp;base64,"))
                {
                    diagnostics.push(Diagnostic::error(DiagKind::Image, "Image requires resolution into the asset store before rendering (limit: 50 images)", Some(line)));
                    raw(
                        node,
                        format!(
                            "<span class=\"mdpdf-error\">Image unavailable: {}</span>",
                            escape(&alt)
                        ),
                    );
                } else {
                    let caption = if link.title.is_empty() {
                        String::new()
                    } else {
                        format!("<figcaption>{}</figcaption>", escape(&link.title))
                    };
                    let image = format!(
                        "<img src=\"{}\" alt=\"{}\"{}>",
                        escape(&link.url),
                        escape(&alt),
                        transform::width_attribute(&attrs)
                    );
                    let standalone = node.parent().filter(|parent| {
                        matches!(parent.data.borrow().value, NodeValue::Paragraph) && parent.children().all(|child| {
                            std::ptr::eq(child, node) || matches!(&child.data.borrow().value, NodeValue::Text(text) if text.trim().is_empty())
                        })
                    });
                    if let Some(parent) = standalone {
                        raw(
                            parent,
                            format!(
                                "<figure class=\"mdpdf-image\" data-source-line=\"{line}\">{image}{caption}</figure>\n"
                            ),
                        );
                    } else {
                        let caption = caption.replace("figcaption", "span");
                        raw(
                            node,
                            format!(
                                "<span class=\"mdpdf-image\" data-source-line=\"{line}\">{image}{caption}</span>"
                            ),
                        );
                    }
                }
            }
            _ => {}
        }
    }
    let mut body = String::new();
    if front.metadata.cover {
        body.push_str(&format!(
            "<section class=\"mdpdf-cover\"><h1>{}</h1><p>{}</p><p>{}</p></section>",
            escape(&front.metadata.title),
            escape(&front.metadata.author),
            escape(&front.metadata.date)
        ));
    }
    if theme.page.toc && !toc_inserted {
        body.push_str(&toc::render(&headings, theme.page.toc_depth));
    }
    let _ = format_html(root, &options, &mut body);
    body = clean(&source_lines(&body, front.line_offset));
    let mut css = css::compile_css(&theme, &front.metadata);
    let math_script = if has_math {
        css.insert_str(
            0,
            include_str!("../../../assets/vendor/katex/katex.min.css"),
        );
        "<script nonce=\"MDPDF_NONCE_PLACEHOLDER\" src=\"/assets/vendor/katex/katex.min.js\"></script>"
    } else {
        ""
    };
    let config = css::mermaid_config(&theme)
        .to_string()
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026");
    // Replaced with a fresh cryptographic nonce by the native delivery layer.
    let nonce = "MDPDF_NONCE_PLACEHOLDER";
    let bootstrap = include_str!("../../../assets/vendor/mermaid/bootstrap.js");
    let frame_bridge = include_str!("../../../assets/vendor/mermaid/frame-bridge.js");
    let html = format!(
        r#"<!doctype html>
<html lang="{}"><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'nonce-{nonce}'; style-src 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'none'; base-uri 'none'; form-action 'none'; object-src 'none'">
<title>{}</title><style id="mdpdf-theme">{css}</style>
</head><body><main>{body}</main>
<script nonce="{nonce}" type="application/json" id="mdpdf-config">{config}</script>
<script nonce="{nonce}" src="/assets/vendor/mermaid/mermaid.min.js"></script>
{math_script}<script nonce="{nonce}">{bootstrap}</script><script nonce="{nonce}">{frame_bridge}</script></body></html>
"#,
        escape(&front.metadata.lang),
        escape(&front.metadata.title)
    );
    PrintDocument {
        html,
        css,
        metadata: front.metadata,
        diagnostics,
        headings,
        diagrams,
    }
}

fn source_lines(html: &str, offset: usize) -> String {
    let mut parts = html.split("data-sourcepos=\"");
    let mut output = parts.next().unwrap_or_default().to_owned();
    for part in parts {
        if let Some(line) = part
            .split(':')
            .next()
            .and_then(|line| line.parse::<usize>().ok())
        {
            output.push_str(&format!("data-source-line=\"{}\" ", line + offset));
        }
        output.push_str("data-sourcepos=\"");
        output.push_str(part);
    }
    output
}
