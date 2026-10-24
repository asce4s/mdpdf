//! Resolve browser-owned image data without filesystem or network access.
use comrak::{Arena, Options, format_commonmark, nodes::NodeValue, parse_document};
use std::collections::{BTreeMap, BTreeSet};

struct Target {
    start: usize,
    end: usize,
    url: String,
    replacement: String,
}
fn targets(markdown: &str) -> Vec<Target> {
    let mut options = Options::default();
    options.extension.math_dollars = true;
    options.extension.front_matter_delimiter = Some("---".into());
    let arena = Arena::new();
    let root = parse_document(&arena, markdown, &options);
    let mut offsets = vec![0];
    offsets.extend(markdown.match_indices('\n').map(|(index, _)| index + 1));
    let mut images = Vec::new();
    for node in root.descendants() {
        let (url, position) = {
            let mut data = node.data.borrow_mut();
            let position = data.sourcepos;
            let NodeValue::Image(link) = &mut data.value else {
                continue;
            };
            if link.url.starts_with("data:") {
                continue;
            }
            let url = link.url.clone();
            link.url = "MDPDF_IMAGE_DESTINATION".into();
            (url, position)
        };
        let (Some(start_line), Some(end_line)) = (
            offsets.get(position.start.line.saturating_sub(1)),
            offsets.get(position.end.line.saturating_sub(1)),
        ) else {
            continue;
        };
        let start = start_line + position.start.column.saturating_sub(1);
        let end = end_line + position.end.column;
        if start >= end
            || end > markdown.len()
            || !markdown.is_char_boundary(start)
            || !markdown.is_char_boundary(end)
        {
            continue;
        }
        let mut replacement = String::new();
        if format_commonmark(node, &options, &mut replacement).is_err() {
            continue;
        }
        let mut replacement = replacement.trim_end_matches('\n').to_owned();
        let old_lines = markdown[start..end].bytes().filter(|b| *b == b'\n').count();
        let new_lines = replacement.bytes().filter(|b| *b == b'\n').count();
        replacement.push_str(&"\n".repeat(old_lines.saturating_sub(new_lines)));
        images.push(Target {
            start,
            end,
            url,
            replacement,
        });
    }
    images
}

/// Preserve source lines, reference-image syntax and attributes while supplying
/// imported raster data. Unknown sources remain visible as renderer diagnostics.
pub struct ResolvedImages {
    pub markdown: String,
    pub unresolved: Vec<String>,
    embedded: BTreeMap<String, String>,
}

impl ResolvedImages {
    pub fn embed(&self, mut document: crate::PrintDocument) -> crate::PrintDocument {
        for (source, data) in &self.embedded {
            document.html = document
                .html
                .replace(&format!("src=\"{source}\""), &format!("src=\"{data}\""));
        }
        document
    }
}

pub fn prepare_images(markdown: &str, images: &BTreeMap<String, String>) -> ResolvedImages {
    if markdown.len() > 2 * 1024 * 1024 {
        return ResolvedImages {
            markdown: markdown.to_owned(),
            unresolved: Vec::new(),
            embedded: BTreeMap::new(),
        };
    }
    let mut replacements = Vec::new();
    let mut unresolved = BTreeSet::new();
    let mut embedded = BTreeMap::new();
    for target in targets(markdown) {
        let key = target.url.strip_prefix("./").unwrap_or(&target.url);
        let filename = key
            .split(['?', '#'])
            .next()
            .unwrap_or(key)
            .rsplit('/')
            .next()
            .unwrap_or(key);
        let data = images
            .get(&target.url)
            .or_else(|| images.get(key))
            .or_else(|| images.get(filename));
        let Some(data) = data.filter(|data| {
            [
                "data:image/png;base64,",
                "data:image/jpeg;base64,",
                "data:image/webp;base64,",
            ]
            .iter()
            .any(|prefix| {
                data.strip_prefix(prefix).is_some_and(|payload| {
                    !payload.is_empty()
                        && payload.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=')
                        })
                })
            })
        }) else {
            if target.url.starts_with("asset://") {
                replacements.push((
                    target.start,
                    target.end,
                    target
                        .replacement
                        .replace("MDPDF_IMAGE_DESTINATION", "unavailable:image"),
                ));
            }
            unresolved.insert(target.url);
            continue;
        };
        let id = format!("browser-image-{}", embedded.len() + 1);
        embedded.insert(format!("/assets/{id}"), data.clone());
        replacements.push((
            target.start,
            target.end,
            target
                .replacement
                .replace("MDPDF_IMAGE_DESTINATION", &format!("asset://{id}")),
        ));
    }
    let mut resolved = markdown.to_owned();
    replacements.sort_unstable_by_key(|(start, _, _)| *start);
    for (start, end, replacement) in replacements.into_iter().rev() {
        resolved.replace_range(start..end, &replacement);
    }
    ResolvedImages {
        markdown: resolved,
        unresolved: unresolved.into_iter().collect(),
        embedded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_reference_images_preserve_attributes_unicode_and_source_lines() {
        let data = "data:image/png;base64,dGVzdA==";
        let images = BTreeMap::from([("images/test.png".into(), data.into())]);
        let markdown = "---\ntitle: Images\n---\n# 図\n\n![Alt][picture]{width=50%}\n\n[picture]: images/test.png\n\n# After\n";
        let resolved = prepare_images(markdown, &images);
        assert!(resolved.unresolved.is_empty());
        let doc = resolved.embed(crate::build_print_document(
            &resolved.markdown,
            &crate::Theme::default(),
        ));
        assert!(doc.html.contains(data));
        assert!(doc.html.contains("data-width=\"50\""));
        assert_eq!(doc.headings[1].source_line, 10);
        assert!(doc.diagnostics.is_empty());
    }
    #[test]
    fn images_in_code_are_untouched_and_invalid_data_is_not_accepted() {
        let markdown = "```md\n![Example](test.png)\n```\n\n![Missing](missing.png)\n";
        let images = BTreeMap::from([("missing.png".into(), "data:text/html;base64,bm8=".into())]);
        let resolved = prepare_images(markdown, &images);
        assert_eq!(resolved.markdown, markdown);
        assert_eq!(resolved.unresolved, vec!["missing.png"]);
    }

    #[test]
    fn image_bytes_do_not_count_against_the_markdown_size_limit() {
        let data = format!("data:image/png;base64,{}", "A".repeat(3 * 1024 * 1024));
        let images = BTreeMap::from([("large.png".into(), data.clone())]);
        let resolved = prepare_images("# Large image\n\n![Image](large.png)", &images);
        let doc = resolved.embed(crate::build_print_document(
            &resolved.markdown,
            &crate::Theme::default(),
        ));
        assert!(doc.diagnostics.is_empty());
        assert!(doc.html.contains(&data));
    }

    #[test]
    fn missing_assets_produce_a_diagnostic_instead_of_an_api_path() {
        let resolved = prepare_images("![Missing](asset://missing)", &BTreeMap::new());
        let doc = crate::build_print_document(&resolved.markdown, &crate::Theme::default());
        assert!(
            doc.diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == crate::DiagKind::Image)
        );
        assert!(!doc.html.contains("src=\"/assets/missing\""));
    }
}
