//! CSS and Mermaid configuration compilation; all interpolated text is escaped.
use crate::{Metadata, theme::*};
use serde_json::{Value, json};
use std::fmt::Write;

fn word(value: &impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}
fn number(value: f64, min: f64, max: f64) -> f64 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        min
    }
}
fn color(value: &str) -> &str {
    if matches!(value, "transparent" | "inherit" | "currentColor")
        || (value.starts_with('#')
            && matches!(value.len(), 4 | 5 | 7 | 9)
            && value[1..].bytes().all(|b| b.is_ascii_hexdigit()))
    {
        value
    } else {
        "inherit"
    }
}
fn quoted(value: &str) -> String {
    let safe: String = value
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '/' | ':' | '%'))
        .collect();
    format!("\"{safe}\"")
}
fn margins(align: Alignment) -> (&'static str, &'static str) {
    match align {
        Alignment::Left => ("0", "auto"),
        Alignment::Center => ("auto", "auto"),
        Alignment::Right => ("auto", "0"),
    }
}

pub fn page_dimensions(theme: &Theme) -> (f64, f64) {
    let (width, height) = match theme.page.size {
        PageSize::A3 => (297.0, 420.0),
        PageSize::A4 => (210.0, 297.0),
        PageSize::A5 => (148.0, 210.0),
        PageSize::Letter => (215.9, 279.4),
        PageSize::Legal => (215.9, 355.6),
        PageSize::Custom { width, height } => {
            (number(width, 50.0, 2000.0), number(height, 50.0, 2000.0))
        }
    };
    if theme.page.orientation == Orientation::Landscape {
        (height, width)
    } else {
        (width, height)
    }
}

/// Reject CSS constructs that can hide network references or escape the style tag.
/// A deliberately small safe subset avoids a second CSS parser in the WASM core.
pub fn sanitize_custom_css(input: &str) -> Option<String> {
    let inspected = input.replace("\\2b ", "+");
    let lower: String = inspected
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    if input.len() > 50 * 1024
        || inspected.contains(['<', '>', '\\', '@'])
        || input.contains("/*")
        || lower.contains("url(")
        || lower.contains("expression(")
        || input
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        None
    } else {
        Some(input.to_owned())
    }
}

fn content(template: &str, metadata: &Metadata) -> String {
    let mut rest = template;
    let mut output = Vec::new();
    while let Some(start) = rest.find('{') {
        output.push(css_string(&rest[..start]));
        let tail = &rest[start..];
        let Some(end) = tail.find('}') else {
            rest = tail;
            break;
        };
        output.push(match &tail[1..end] {
            "page" => "counter(page)".into(),
            "pages" => "counter(pages)".into(),
            "section" => "string(section)".into(),
            "title" => css_string(&metadata.title),
            "author" => css_string(&metadata.author),
            "date" => css_string(&metadata.date),
            _ => css_string(&tail[..=end]),
        });
        rest = &tail[end + 1..];
    }
    output.push(css_string(rest));
    output.join(" ")
}
fn css_string(input: &str) -> String {
    // Escape every potentially structural character with a CSS hexadecimal escape.
    let mut result = String::from("\"");
    for c in input.chars() {
        if c.is_alphanumeric() || matches!(c, ' ' | '.' | ',' | '-' | '_') {
            result.push(c);
        } else {
            let _ = write!(result, "\\{:x} ", c as u32);
        }
    }
    result.push('"');
    result
}

pub fn compile_css(theme: &Theme, metadata: &Metadata) -> String {
    let (width, height) = page_dimensions(theme);
    let page = &theme.page;
    let t = &theme.typography;
    let table = &theme.tables;
    let code = &theme.code;
    let diagrams = &theme.diagrams;
    let images = &theme.images;
    let m = &page.margins;
    let (mt, mr, mb, ml) = (
        number(m.top, 0.0, 100.0),
        number(m.right, 0.0, 100.0),
        number(m.bottom, 0.0, 100.0),
        number(m.left, 0.0, 100.0),
    );
    let mut css =
        format!("@page {{ size: {width}mm {height}mm; margin: {mt}mm {mr}mm {mb}mm {ml}mm; }}\n");
    css.push_str(include_str!("../../../assets/fonts/fonts.css"));
    let _ = writeln!(
        css,
        ":root {{ --theme-name: {}; --page-width: {width}mm; --page-height: {height}mm; --content-height: {}mm; --toc-depth: {}; --heading-numbers: {}; }}",
        quoted(&theme.name),
        (height - mt - mb).max(1.0),
        page.toc_depth.clamp(1, 4),
        u8::from(page.heading_numbers)
    );
    let _ = writeln!(
        css,
        "html {{ background: {}; color: {}; font-family: {}, \"Noto Sans\", \"Noto Sans CJK SC\", sans-serif; font-size: {}pt; line-height: {}; text-align: {}; hyphens: {}; }}",
        color(&page.background),
        color(&t.text_color),
        quoted(&t.body_font),
        number(t.base_size, 8.0, 16.0),
        number(t.line_height, 1.0, 2.0),
        word(&t.text_align),
        if t.hyphenation { "auto" } else { "none" }
    );
    let _ = writeln!(
        css,
        ":root {{ --toc-enabled: {}; --code-avoid-break-under-lines: {}; --diagram-numbering: {}; }}",
        u8::from(page.toc),
        code.avoid_break_under_lines,
        u8::from(diagrams.numbering)
    );
    css.push_str("* { box-sizing: border-box; } body { margin: 0; } p { orphans: 3; widows: 3; } a { overflow-wrap: anywhere; } h1 { string-set: section content(text); } h1,h2,h3,h4,h5,h6 { break-after: avoid; } .mdpdf-page-break { break-after: page; } blockquote { border-left: 3px solid #d0d7de; padding-left: 1em; } .markdown-alert { padding: .5em 1em; border-left: 3px solid #0b5fff; } .mdpdf-error { border: 1px solid #b42318; padding: 1em; color: #b42318; }\n");
    // Chromium rasterizes SVG groups with CSS filters (including Mermaid's
    // timeline brightness effect), losing vector shapes and selectable labels.
    css.push_str(".mdpdf-diagram svg, .mdpdf-diagram svg * { filter: none !important; }\n");
    let _ = writeln!(
        css,
        "@media screen {{ html {{ width: {width}mm; min-height: {height}mm; }} body {{ padding: {mt}mm {mr}mm {mb}mm {ml}mm; min-height: {height}mm; }} }}"
    );
    let _ = writeln!(
        css,
        "p {{ margin: 0 0 {}em; }} a {{ color: {}; }} code,pre {{ font-family: {}, \"Noto Sans\", \"Noto Sans CJK SC\", monospace; }} .mdpdf-toc {{ display: block; }}",
        number(t.paragraph_spacing, 0.0, 5.0),
        color(&t.link_color),
        quoted(&t.mono_font)
    );
    for (index, level) in theme.headings.levels.iter().enumerate() {
        let _ = writeln!(
            css,
            "h{} {{ font-family: {}, \"Noto Sans\", \"Noto Sans CJK SC\", sans-serif; font-size: {}pt; font-weight: {}; color: {}; margin: {}em 0 {}em; border-bottom: {}; text-transform: {}; }}",
            index + 1,
            quoted(&t.heading_font),
            number(level.size, 6.0, 72.0),
            level.weight.clamp(100, 900),
            color(&level.color),
            number(level.margin_top, 0.0, 10.0),
            number(level.margin_bottom, 0.0, 10.0),
            level
                .bottom_border
                .as_ref()
                .map_or_else(|| "none".into(), |v| format!("1px solid {}", color(v))),
            if level.uppercase { "uppercase" } else { "none" }
        );
    }
    if page.break_before_h1 {
        css.push_str("h1:not(:first-child) { break-before: page; }\n");
    }
    let border = format!(
        "{}px solid {}",
        number(table.border_width, 0.0, 3.0),
        color(&table.border_color)
    );
    let (cell_border, bottom_border) = match table.style {
        TableStyle::Grid => (border.as_str(), "none"),
        TableStyle::HorizontalLines => ("none", border.as_str()),
        TableStyle::Minimal | TableStyle::None => ("none", "none"),
    };
    let _ = writeln!(
        css,
        "table {{ border-collapse: collapse; width: {}; font-size: {}em; caption-side: {}; {} }} th,td {{ padding: {}px {}px; border: {cell_border}; border-bottom: {bottom_border}; }} th {{ background: {}; color: {}; font-weight: {}; }}",
        if table.width == TableWidth::Full {
            "100%"
        } else {
            "auto"
        },
        number(table.font_size, 0.7, 1.0),
        if table.caption_position == CaptionPosition::Above {
            "top"
        } else {
            "bottom"
        },
        match table.wide_tables {
            WideTables::Wrap => "table-layout: fixed; overflow-wrap: anywhere;",
            WideTables::ShrinkToFit => "max-width: 100%; overflow-wrap: anywhere;",
            WideTables::RotateLandscape => "page: wide;",
        },
        number(table.cell_padding_vertical, 0.0, 50.0),
        number(table.cell_padding_horizontal, 0.0, 50.0),
        color(&table.header_bg),
        color(&table.header_color),
        table.header_weight.clamp(400, 800)
    );
    if table.style == TableStyle::Minimal {
        let _ = writeln!(css, "thead {{ border-bottom: {border}; }}");
    }
    if table.zebra {
        let _ = writeln!(
            css,
            "tbody tr:nth-child(even) {{ background: {}; }}",
            color(&table.zebra_color)
        );
    }
    let _ = writeln!(
        css,
        "thead {{ display: {}; }} tr {{ break-inside: {}; }} @page wide {{ size: {}mm {}mm; }}",
        if table.repeat_header {
            "table-header-group"
        } else {
            "table-row-group"
        },
        if table.avoid_row_split {
            "avoid"
        } else {
            "auto"
        },
        height.max(width),
        height.min(width)
    );
    let _ = writeln!(
        css,
        "pre.mdpdf-code {{ font-size: {}pt; background: {}; border: 1px solid {}; padding: 10px; white-space: {}; overflow-wrap: {}; overflow: hidden; }} .mdpdf-code.keep {{ break-inside: avoid; }} .mdpdf-code .line-numbers {{ display: {}; }}",
        number(code.font_size, 6.0, 24.0),
        color(&code.background),
        color(&code.border),
        if code.wrap { "pre-wrap" } else { "pre" },
        if code.wrap { "anywhere" } else { "normal" },
        if code.line_numbers { "block" } else { "none" }
    );
    let (left, right) = margins(diagrams.align);
    let _ = writeln!(
        css,
        ".mdpdf-diagram {{ max-width: {}%; margin-left: {left}; margin-right: {right}; text-align: {}; background: {}; break-inside: {}; }} .mdpdf-diagram svg {{ max-width: 100%; height: auto; {} }} .mdpdf-diagram figcaption {{ display: {}; }}",
        number(diagrams.max_width, 1.0, 100.0),
        word(&diagrams.align),
        color(&diagrams.background),
        if diagrams.oversize == Oversize::AllowSplit {
            "auto"
        } else {
            "avoid"
        },
        if diagrams.oversize == Oversize::FitPage {
            "max-height: var(--content-height);"
        } else {
            ""
        },
        if diagrams.captions { "block" } else { "none" }
    );
    let (left, right) = margins(images.align);
    let _ = writeln!(
        css,
        ".mdpdf-image {{ margin-left: {left}; margin-right: {right}; text-align: {}; }} img {{ max-width: {}%; height: auto; border-radius: {}px; border: {}; box-shadow: {}; }} .mdpdf-image figcaption {{ font-size: {}em; color: {}; }}",
        word(&images.align),
        number(images.max_width, 1.0, 100.0),
        number(images.border_radius, 0.0, 100.0),
        images
            .border
            .as_ref()
            .map_or_else(|| "none".into(), |v| format!("1px solid {}", color(v))),
        if images.shadow {
            "0 2px 6px #00000026"
        } else {
            "none"
        },
        number(images.caption_size, 0.5, 2.0),
        color(&images.caption_color)
    );
    css.push_str(".mdpdf-cover { min-height: calc(var(--page-height) - 60mm); display: flex; flex-direction: column; justify-content: center; text-align: center; break-after: page; } .mdpdf-cover h1 { font-size: 36pt; } .mdpdf-math { overflow-x: auto; }\n");
    if images.numbering {
        css.push_str("main { counter-reset: mdpdf-image; } .mdpdf-image { counter-increment: mdpdf-image; } .mdpdf-image figcaption::before { content: \"Figure \" counter(mdpdf-image) \": \"; }\n");
    }
    let hf = &theme.header_footer;
    css.push_str("@page {\n");
    for (position, template) in [
        ("top-left", &hf.header.left),
        ("top-center", &hf.header.center),
        ("top-right", &hf.header.right),
        ("bottom-left", &hf.footer.left),
        ("bottom-center", &hf.footer.center),
        ("bottom-right", &hf.footer.right),
    ] {
        let _ = writeln!(
            css,
            "@{position} {{ content: {}; font-family: {}, \"Noto Sans\", \"Noto Sans CJK SC\", sans-serif; font-size: {}pt; color: {}; }}",
            content(template, metadata),
            quoted(&t.body_font),
            number(hf.font_size, 6.0, 16.0),
            color(&hf.color)
        );
    }
    css.push_str("}\n");
    if hf.hide_on_first_page {
        css.push_str("@page :first { @top-left { content: none; } @top-center { content: none; } @top-right { content: none; } @bottom-left { content: none; } @bottom-center { content: none; } @bottom-right { content: none; } }\n");
    }
    css.push_str(".mdpdf-code .line-numbers { float: left; padding-right: 1.5em; opacity: .5; user-select: none; white-space: pre; }\n");
    css.push_str(match code.syntax_theme {
        SyntaxTheme::GithubLight => include_str!("../../../assets/syntax/github-light.css"),
        SyntaxTheme::OneDark => include_str!("../../../assets/syntax/one-dark.css"),
        SyntaxTheme::SolarizedLight => include_str!("../../../assets/syntax/solarized-light.css"),
        SyntaxTheme::SolarizedDark => include_str!("../../../assets/syntax/solarized-dark.css"),
        SyntaxTheme::Nord => include_str!("../../../assets/syntax/nord.css"),
    });
    if let Some(custom) = code
        .custom_syntax_css
        .as_deref()
        .and_then(sanitize_custom_css)
    {
        css.push_str(&custom);
    }
    if let Some(custom) = theme.custom_css.as_deref().and_then(sanitize_custom_css) {
        css.push_str(&custom);
    }
    css
}

pub fn mermaid_config(theme: &Theme) -> Value {
    let d = &theme.diagrams;
    let font = d
        .font_family
        .as_deref()
        .unwrap_or(&theme.typography.body_font);
    let mut variables = d.theme_variables.clone();
    variables.insert("fontFamily".into(), font.into());
    variables.insert(
        "fontSize".into(),
        format!("{}px", number(d.font_size, 6.0, 72.0)),
    );
    json!({
        "startOnLoad": false, "securityLevel": "strict", "suppressErrorRendering": true,
        "secure": ["secure", "securityLevel", "startOnLoad", "maxTextSize", "suppressErrorRendering"],
        "theme": word(&d.mermaid_theme), "themeVariables": variables, "fontFamily": font,
        "fontSize": number(d.font_size,6.0,72.0),
        "flowchart": {"curve": word(&d.flowchart_curve)},
        "sequence": {"mirrorActors": d.sequence_mirror_actors}
    })
}
