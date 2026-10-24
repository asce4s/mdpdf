use crate::escape;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Heading {
    pub level: u8,
    pub id: String,
    pub text: String,
    pub number: Option<String>,
    pub source_line: u32,
}

pub(crate) fn slug(text: &str, used: &mut BTreeSet<String>) -> String {
    let base: String = text
        .to_lowercase()
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                Some(c)
            } else if c.is_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .collect();
    let base = if base.is_empty() {
        "heading".into()
    } else {
        base
    };
    let mut id = base.clone();
    let mut suffix = 1;
    while !used.insert(id.clone()) {
        id = format!("{base}-{suffix}");
        suffix += 1;
    }
    id
}

pub fn render(headings: &[Heading], depth: u8) -> String {
    let mut html = String::from("<nav class=\"mdpdf-toc\" aria-label=\"Table of contents\"><ol>\n");
    for heading in headings.iter().filter(|h| h.level <= depth.clamp(1, 4)) {
        let number = heading
            .number
            .as_ref()
            .map_or(String::new(), |n| format!("{} ", escape(n)));
        html.push_str(&format!(
            "<li class=\"toc-level-{}\"><a href=\"#{}\">{}{}</a></li>\n",
            heading.level,
            escape(&heading.id),
            number,
            escape(&heading.text)
        ));
    }
    html.push_str("</ol></nav>\n");
    html
}
