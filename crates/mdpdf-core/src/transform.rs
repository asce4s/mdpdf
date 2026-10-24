use crate::{DiagKind, Diagnostic, Diagram, escape, theme::Theme};
use std::collections::BTreeMap;
use syntect::{
    html::{ClassStyle, ClassedHTMLGenerator},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

/// A small quoted attribute lexer for the documented fence/image attributes.
/// Values are never copied into HTML without validation and escaping.
pub(crate) fn attributes(input: &str) -> BTreeMap<String, String> {
    let mut attrs = BTreeMap::new();
    let mut rest = input
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim();
    while !rest.is_empty() {
        let Some(eq) = rest.find('=') else {
            break;
        };
        let key = rest[..eq].trim();
        if key.is_empty() || !key.bytes().all(|b| b.is_ascii_alphabetic() || b == b'-') {
            break;
        }
        rest = rest[eq + 1..].trim_start();
        let (value, remaining) =
            if let Some(quote) = rest.chars().next().filter(|c| matches!(c, '\'' | '"')) {
                let text = &rest[quote.len_utf8()..];
                let Some(end) = text.find(quote) else {
                    break;
                };
                (&text[..end], &text[end + quote.len_utf8()..])
            } else {
                let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                (&rest[..end], &rest[end..])
            };
        attrs.insert(key.to_owned(), value.to_owned());
        rest = remaining.trim_start();
    }
    attrs
}

pub(crate) fn width_attribute(attrs: &BTreeMap<String, String>) -> String {
    attrs
        .get("width")
        .and_then(|s| s.strip_suffix('%'))
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|n| n.is_finite() && *n > 0.0 && *n <= 100.0)
        .map_or(String::new(), |n| format!(" data-width=\"{n}\""))
}

pub(crate) fn diagram(
    source: &str,
    info: &str,
    line: u32,
    index: usize,
    theme: &Theme,
) -> (String, Diagram) {
    let attrs = attributes(info.trim_start_matches("mermaid").trim());
    let id = format!("mdpdf-diagram-{index}");
    let align = attrs
        .get("align")
        .filter(|s| matches!(s.as_str(), "left" | "center" | "right"))
        .map_or(String::new(), |s| format!(" data-align=\"{s}\""));
    let per_theme = attrs
        .get("theme")
        .filter(|s| {
            matches!(
                s.as_str(),
                "default" | "neutral" | "dark" | "forest" | "base"
            )
        })
        .map_or(String::new(), |s| format!(" data-mermaid-theme=\"{s}\""));
    let caption = if theme.diagrams.captions {
        attrs.get("caption").map_or(String::new(), |caption| {
            let number = if theme.diagrams.numbering {
                format!("Figure {index}: ")
            } else {
                String::new()
            };
            format!("<figcaption>{number}{}</figcaption>", escape(caption))
        })
    } else {
        String::new()
    };
    let html = format!(
        "<figure class=\"mdpdf-diagram\" data-source-line=\"{line}\"{}{}{}><pre class=\"mermaid\" data-diagram-id=\"{id}\">{}</pre>{caption}</figure>\n",
        width_attribute(&attrs),
        align,
        per_theme,
        escape(source)
    );
    (
        html,
        Diagram {
            id,
            source: source.into(),
            source_line: line,
        },
    )
}

pub(crate) fn highlight(
    source: &str,
    language: &str,
    line: u32,
    theme: &Theme,
    syntaxes: &SyntaxSet,
    diagnostics: &mut Vec<Diagnostic>,
) -> String {
    let syntax = syntaxes
        .find_syntax_by_token(language)
        .unwrap_or_else(|| syntaxes.find_syntax_plain_text());
    let mut generator =
        ClassedHTMLGenerator::new_with_class_style(syntax, syntaxes, ClassStyle::Spaced);
    for text in LinesWithEndings::from(source) {
        if let Err(error) = generator.parse_html_for_line_which_includes_newline(text) {
            diagnostics.push(Diagnostic::error(
                DiagKind::Html,
                error.to_string(),
                Some(line),
            ));
            return format!(
                "<pre class=\"mdpdf-code\" data-source-line=\"{line}\"><code>{}</code></pre>\n",
                escape(source)
            );
        }
    }
    let keep = if source.lines().count() < theme.code.avoid_break_under_lines as usize {
        " keep"
    } else {
        ""
    };
    let numbers = if theme.code.line_numbers {
        let numbers = (1..=source.lines().count())
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        format!("<span class=\"line-numbers\" aria-hidden=\"true\">{numbers}</span>")
    } else {
        String::new()
    };
    format!(
        "<pre class=\"mdpdf-code{keep}\" data-source-line=\"{line}\">{numbers}<code class=\"language-{}\">{}</code></pre>\n",
        escape(language),
        generator.finalize()
    )
}
