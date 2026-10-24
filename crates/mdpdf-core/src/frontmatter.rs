use crate::{DiagKind, Diagnostic, theme::Theme};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Metadata {
    pub title: String,
    pub author: String,
    pub date: String,
    pub subject: String,
    pub keywords: String,
    pub lang: String,
    pub cover: bool,
}

pub(crate) struct FrontMatter<'a> {
    pub body: &'a str,
    pub line_offset: usize,
    pub metadata: Metadata,
    pub theme: Theme,
    pub diagnostics: Vec<Diagnostic>,
}

pub(crate) fn extract<'a>(md: &'a str, user_theme: &Theme) -> FrontMatter<'a> {
    let mut result = FrontMatter {
        body: md,
        line_offset: 0,
        metadata: Metadata {
            lang: "en".into(),
            ..Metadata::default()
        },
        theme: user_theme.clone(),
        diagnostics: Vec::new(),
    };
    let mut lines = md.split_inclusive('\n');
    if lines.next().map(str::trim) != Some("---") {
        return result;
    }
    let mut end = md.split_inclusive('\n').next().map_or(0, str::len);
    let mut yaml = String::new();
    let mut found = false;
    for line in lines {
        end += line.len();
        if matches!(line.trim(), "---" | "...") {
            found = true;
            break;
        }
        yaml.push_str(line);
    }
    if !found {
        result.diagnostics.push(Diagnostic::error(
            DiagKind::FrontMatter,
            "Unclosed YAML front matter",
            Some(1),
        ));
        return result;
    }
    result.body = &md[end..];
    result.line_offset = md[..end].bytes().filter(|byte| *byte == b'\n').count();
    let parsed: Result<Value, _> = serde_saphyr::from_str(&yaml);
    let value = match parsed {
        Ok(Value::Object(object)) => Value::Object(object),
        Ok(_) => {
            result.diagnostics.push(Diagnostic::error(
                DiagKind::FrontMatter,
                "Front matter must be a mapping",
                Some(1),
            ));
            return result;
        }
        Err(error) => {
            result.diagnostics.push(Diagnostic::error(
                DiagKind::FrontMatter,
                error.to_string(),
                Some(1),
            ));
            return result;
        }
    };
    if let Ok(metadata) = serde_json::from_value::<Metadata>(value.clone()) {
        result.metadata = metadata;
        if result.metadata.lang.is_empty() {
            result.metadata.lang = "en".into();
        }
    } else {
        result.diagnostics.push(Diagnostic::error(
            DiagKind::FrontMatter,
            "Metadata values must be strings (cover must be boolean)",
            Some(1),
        ));
    }
    let mut patch = value.as_object().cloned().unwrap_or_default();
    for key in [
        "title", "author", "date", "subject", "keywords", "lang", "cover",
    ] {
        patch.remove(key);
    }
    let selection = patch.remove("theme");
    let mut base = user_theme.clone();
    match selection {
        Some(Value::String(name)) => match Theme::preset(&name) {
            Ok(preset) => base = preset,
            Err(error) => {
                result
                    .diagnostics
                    .push(Diagnostic::error(DiagKind::FrontMatter, error, Some(1)))
            }
        },
        Some(Value::Object(overrides)) => {
            // Nested theme overrides have lower precedence than root overrides.
            let mut nested = Value::Object(overrides);
            crate::theme::merge(&mut nested, Value::Object(patch));
            patch = nested.as_object().cloned().unwrap_or_default();
        }
        Some(_) => result.diagnostics.push(Diagnostic::error(
            DiagKind::FrontMatter,
            "theme must be a preset name or mapping",
            Some(1),
        )),
        None => {}
    }
    if let Some(toc) = patch.remove("toc") {
        let page = patch.entry("page").or_insert_with(|| serde_json::json!({}));
        if let Some(page) = page.as_object_mut() {
            page.insert("toc".into(), toc);
        }
    }
    match base.merged(Value::Object(patch)) {
        Ok(theme) => result.theme = theme,
        Err(error) => {
            result
                .diagnostics
                .push(Diagnostic::error(DiagKind::FrontMatter, error, Some(1)))
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_frontmatter_preserves_offsets_and_nested_edits() {
        let mut theme = Theme::preset("academic").unwrap();
        theme.tables.zebra = false;
        let parsed = extract(
            "---\r\ntitle: Report\r\ntoc: true\r\npage:\r\n  margins:\r\n    top: 24\r\n---\r\n# Body\r\n",
            &theme,
        );
        assert_eq!(parsed.body, "# Body\r\n");
        assert_eq!(parsed.line_offset, 7);
        assert_eq!(parsed.metadata.title, "Report");
        assert_eq!(parsed.theme.page.margins.top, 24.0);
        assert_eq!(parsed.theme.page.margins.left, 18.0);
        assert!(parsed.theme.page.toc);
        assert!(!parsed.theme.tables.zebra);
        assert!(parsed.diagnostics.is_empty());
    }

    #[test]
    fn preset_then_nested_theme_then_document_overrides() {
        let parsed = extract(
            "---\ntheme: academic\ntypography:\n  base_size: 12\n---\n# Body",
            &Theme::default(),
        );
        assert_eq!(parsed.theme.typography.body_font, "Source Serif 4");
        assert_eq!(parsed.theme.typography.base_size, 12.0);
        let parsed = extract(
            "---\ntheme:\n  typography:\n    base_size: 12\ntypography:\n  base_size: 14\n---\nBody",
            &Theme::default(),
        );
        assert_eq!(parsed.theme.typography.base_size, 14.0);
        assert!(parsed.diagnostics.is_empty());
    }

    #[test]
    fn malformed_frontmatter_is_diagnostic_and_not_a_panic() {
        for md in [
            "---\ntitle: [broken\n---\nBody",
            "---\npage: invalid\n---\nBody",
            "---\ntitle: x\nBody",
            "---\n- list\n---\nBody",
        ] {
            let parsed = extract(md, &Theme::default());
            assert!(!parsed.diagnostics.is_empty(), "{md}");
        }
    }
}
