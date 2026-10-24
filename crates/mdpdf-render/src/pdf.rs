use crate::{RenderError, Result};
use lopdf::{Document, Object, StringFormat};
use mdpdf_core::Metadata;

fn text(value: &str) -> Object {
    let bytes = [0xfe, 0xff]
        .into_iter()
        .chain(value.encode_utf16().flat_map(u16::to_be_bytes))
        .collect();
    Object::String(bytes, StringFormat::Hexadecimal)
}

pub(crate) fn metadata(bytes: &[u8], metadata: &Metadata) -> Result<(Vec<u8>, usize)> {
    let convert = |error: lopdf::Error| RenderError::Pdf(error.to_string());
    let mut pdf = Document::load_mem(bytes).map_err(convert)?;
    let pages = pdf.get_pages().len();
    let mut info = pdf
        .trailer
        .get(b"Info")
        .ok()
        .and_then(|object| pdf.dereference(object).ok())
        .and_then(|(_, object)| object.as_dict().ok())
        .cloned()
        .unwrap_or_default();
    for (key, value) in [
        ("Title", &metadata.title),
        ("Author", &metadata.author),
        ("Subject", &metadata.subject),
        ("Keywords", &metadata.keywords),
    ] {
        if !value.is_empty() {
            info.set(key, text(value));
        }
    }
    let id = pdf.add_object(info);
    pdf.trailer.set("Info", id);
    let mut output = Vec::new();
    pdf.save_to(&mut output)
        .map_err(|error| RenderError::Pdf(error.to_string()))?;
    Ok((output, pages))
}

/// Read actual printed page destinations, avoiding screen-layout estimates for
/// running section labels. Chromium's top-level outlines represent sections.
pub(crate) fn sections(bytes: &[u8]) -> Result<Vec<String>> {
    let pdf = Document::load_mem(bytes).map_err(|error| RenderError::Pdf(error.to_string()))?;
    let pages = pdf.get_pages();
    let mut changes = std::collections::BTreeMap::new();
    let mut next = pdf
        .catalog()
        .ok()
        .and_then(|root| root.get(b"Outlines").ok())
        .and_then(|object| pdf.dereference(object).ok())
        .and_then(|(_, object)| object.as_dict().ok())
        .and_then(|dict| dict.get(b"First").ok())
        .cloned();
    let mut visited = std::collections::BTreeSet::new();
    while let Some(object) = next {
        let Ok((id, object)) = pdf.dereference(&object) else {
            break;
        };
        if let Some(id) = id
            && !visited.insert(id)
        {
            break;
        }
        let Ok(outline) = object.as_dict() else {
            break;
        };
        if let (Ok(title), Ok(destination)) = (
            outline.get(b"Title"),
            outline.get(b"Dest").and_then(Object::as_array),
        ) && let (Ok(title), Some(Ok(page))) = (
            lopdf::decode_text_string(title),
            destination.first().map(Object::as_reference),
        ) && let Some((&number, _)) = pages.iter().find(|(_, id)| **id == page)
        {
            changes.insert(number, title);
        }
        next = outline.get(b"Next").ok().cloned();
    }
    let mut section = String::new();
    Ok(pages
        .keys()
        .map(|number| {
            if let Some(title) = changes.get(number) {
                section = title.clone();
            }
            section.clone()
        })
        .collect())
}

/// Keep the original catalog, bookmarks, annotations and tags while replacing
/// page drawing/resources with an identically paginated labelled print.
pub(crate) fn replace_page_drawings(
    original: &[u8],
    replacements: Vec<Vec<u8>>,
) -> Result<Vec<u8>> {
    let convert = |error: lopdf::Error| RenderError::Pdf(error.to_string());
    let mut pdf = Document::load_mem(original).map_err(convert)?;
    let pages = pdf.get_pages();
    for ((_, id), bytes) in pages.into_iter().zip(replacements) {
        let mut labelled = Document::load_mem(&bytes).map_err(convert)?;
        labelled.renumber_objects_with(pdf.max_id + 1);
        let labelled_id = *labelled
            .get_pages()
            .values()
            .next()
            .ok_or_else(|| RenderError::Pdf("Labelled page is missing".into()))?;
        let page = labelled.get_dictionary(labelled_id).map_err(convert)?;
        let contents = page.get(b"Contents").map_err(convert)?.clone();
        let resources = page.get(b"Resources").map_err(convert)?.clone();
        pdf.max_id = labelled.max_id;
        pdf.objects.append(&mut labelled.objects);
        let page = pdf.get_dictionary_mut(id).map_err(convert)?;
        page.set("Contents", contents);
        page.set("Resources", resources);
    }
    pdf.prune_objects();
    let mut output = Vec::new();
    pdf.save_to(&mut output)
        .map_err(|error| RenderError::Pdf(error.to_string()))?;
    Ok(output)
}
