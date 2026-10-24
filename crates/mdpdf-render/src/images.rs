use crate::{RenderError, Result, assets::bind};
use base64::{Engine, prelude::BASE64_STANDARD};
use comrak::{Arena, Options, format_commonmark, nodes::NodeValue, parse_document};
use futures_util::StreamExt;
use mdpdf_core::{DiagKind, Diagnostic, PrintDocument, Severity, Theme, build_print_document};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    time::Duration,
};

#[derive(Clone, Debug)]
pub struct ImageAsset {
    pub bytes: Vec<u8>,
    pub mime: String,
}
#[derive(Clone, Debug)]
pub struct FontAsset {
    pub bytes: Vec<u8>,
    pub mime: String,
    pub family: String,
}
#[derive(Clone, Debug)]
pub struct AssetContext {
    pub images: BTreeMap<String, ImageAsset>,
    pub fonts: Vec<FontAsset>,
    /// Only CLI callers supply a filesystem base directory. Web requests never do.
    pub local_root: Option<PathBuf>,
    pub allow_remote: bool,
}
impl Default for AssetContext {
    fn default() -> Self {
        Self {
            images: BTreeMap::new(),
            fonts: Vec::new(),
            local_root: None,
            allow_remote: true,
        }
    }
}

struct Target {
    start: usize,
    end: usize,
    line: u32,
    url: String,
    replacement: String,
}
fn targets(markdown: &str) -> Vec<Target> {
    let mut options = Options::default();
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
            line: position.start.line as u32,
            url,
            replacement,
        });
    }
    images
}
fn hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn image_mime(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if data.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else {
        None
    }
}
fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, _, _] = ip.octets();
            !ip.is_private()
                && !ip.is_loopback()
                && !ip.is_link_local()
                && !ip.is_unspecified()
                && !ip.is_multicast()
                && !ip.is_broadcast()
                && !ip.is_documentation()
                && a != 0
                && a < 224
                && !(a == 100 && (64..=127).contains(&b))
                && !(a == 198 && (b == 18 || b == 19))
                && !(a == 192 && b == 0)
        }
        IpAddr::V6(ip) => {
            if let Some(ip) = ip.to_ipv4_mapped() {
                return public_ip(IpAddr::V4(ip));
            }
            let first = ip.segments()[0];
            !ip.is_loopback()
                && !ip.is_unspecified()
                && !ip.is_multicast()
                && first & 0xfe00 != 0xfc00
                && first & 0xffc0 != 0xfe80
                && first & 0xffc0 != 0xfec0
                && first & 0xe000 == 0x2000
                && !(first == 0x2001 && ip.segments()[1] < 0x0200)
                && !(first == 0x2001 && ip.segments()[1] == 0x0db8)
                && first != 0x2002
                && first != 0x3fff
        }
    }
}
async fn remote_image(input: &str) -> std::result::Result<ImageAsset, String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    tokio::time::timeout_at(deadline, async {
        let mut url =
            reqwest::Url::parse(input).map_err(|_| "Invalid remote image URL".to_owned())?;
        for _ in 0..6 {
            if !matches!(url.scheme(), "http" | "https")
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return Err("Images must use public HTTP(S) URLs without credentials".into());
            }
            let host = url
                .host_str()
                .ok_or("Remote image has no hostname")?
                .to_owned();
            let port = url
                .port_or_known_default()
                .ok_or("Remote image has no port")?;
            let addresses: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), port))
                .await
                .map_err(|_| "Could not resolve the remote image hostname")?
                .collect();
            if addresses.is_empty() || addresses.iter().any(|address| !public_ip(address.ip())) {
                return Err(
                    "Remote images cannot access local or private network addresses".into(),
                );
            }
            // Pin the validated DNS result for this request; validate every redirect.
            let client = reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .resolve_to_addrs(&host, &addresses)
                .timeout(Duration::from_secs(10))
                .build()
                .map_err(|error| error.to_string())?;
            let response = client
                .get(url.clone())
                .send()
                .await
                .map_err(|_| "Could not fetch the remote image")?;
            if response.status().is_redirection() {
                let location = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|h| h.to_str().ok())
                    .ok_or("Image redirect has no destination")?;
                url = url.join(location).map_err(|_| "Invalid image redirect")?;
                continue;
            }
            if !response.status().is_success() {
                return Err(format!(
                    "Remote image returned HTTP {}",
                    response.status().as_u16()
                ));
            }
            if response
                .content_length()
                .is_some_and(|size| size > 10 * 1024 * 1024)
            {
                return Err("Remote image exceeds 10 MB".into());
            }
            let mut stream = response.bytes_stream();
            let mut bytes = Vec::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|_| "Remote image download failed")?;
                if bytes.len() + chunk.len() > 10 * 1024 * 1024 {
                    return Err("Remote image exceeds 10 MB".into());
                }
                bytes.extend_from_slice(&chunk);
            }
            let mime = image_mime(&bytes).ok_or("Remote image must be PNG, JPEG or WebP")?;
            return Ok(ImageAsset {
                bytes,
                mime: mime.into(),
            });
        }
        Err("Too many remote image redirects".into())
    })
    .await
    .map_err(|_| "Remote image exceeded its 10-second timeout".to_owned())?
}
async fn resolve(url: &str, context: &AssetContext) -> std::result::Result<ImageAsset, String> {
    if let Some(id) = url
        .strip_prefix("asset://")
        .or_else(|| url.strip_prefix("/assets/"))
    {
        return context
            .images
            .get(id)
            .cloned()
            .ok_or_else(|| "Uploaded image is unavailable or its session expired".into());
    }
    if url.starts_with("http:") || url.starts_with("https:") {
        return if context.allow_remote {
            remote_image(url).await
        } else {
            Err("Remote images are disabled".into())
        };
    }
    let Some(root) = &context.local_root else {
        return Err("Upload local images before using them in the web editor".into());
    };
    if url.contains("://") {
        return Err("Unsupported image URL scheme".into());
    }
    let url = url.split('#').next().unwrap_or(url);
    let mut decoded = Vec::new();
    let bytes = url.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let text = std::str::from_utf8(&bytes[index + 1..index + 3])
                .map_err(|_| "Invalid image path")?;
            if let Ok(byte) = u8::from_str_radix(text, 16) {
                decoded.push(byte);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    let path = root.join(String::from_utf8(decoded).map_err(|_| "Invalid image path")?);
    let metadata = tokio::fs::metadata(&path)
        .await
        .map_err(|_| "Could not read the local image")?;
    if !metadata.is_file() || metadata.len() > 10 * 1024 * 1024 {
        return Err("Local image must be a file of at most 10 MB".into());
    }
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| "Could not read the local image")?;
    if bytes.len() > 10 * 1024 * 1024 {
        return Err("Local image exceeds 10 MB".into());
    }
    let mime = image_mime(&bytes).ok_or("Local image must be PNG, JPEG or WebP")?;
    Ok(ImageAsset {
        bytes,
        mime: mime.into(),
    })
}

/// Resolve uploaded/local/public remote images before calling the one core builder.
/// Replacements retain source newlines, so diagnostics keep their original line numbers.
pub async fn prepare_document_with_assets(
    markdown: &str,
    theme: &Theme,
    context: &AssetContext,
) -> Result<PrintDocument> {
    if markdown.len() > 2 * 1024 * 1024 {
        return Err(RenderError::Assets("Markdown exceeds 2 MB".into()));
    }
    let targets = targets(markdown);
    let mut replacements = Vec::new();
    let mut images = BTreeMap::new();
    let mut known = HashMap::new();
    let mut diagnostics = Vec::new();
    let mut total = 0;
    for target in targets.into_iter().take(50) {
        let destination = if let Some(id) = known.get(&target.url) {
            format!("asset://{id}")
        } else {
            match resolve(&target.url, context).await {
                Ok(image) if total + image.bytes.len() <= 64 * 1024 * 1024 => {
                    total += image.bytes.len();
                    let id = hex(&image.bytes);
                    images.insert(id.clone(), image);
                    known.insert(target.url.clone(), id.clone());
                    format!("asset://{id}")
                }
                Ok(_) => {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        kind: DiagKind::Image,
                        message: "Images exceed the document's 64 MB asset budget".into(),
                        source_line: Some(target.line),
                    });
                    "unavailable:image".into()
                }
                Err(message) => {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        kind: DiagKind::Image,
                        message,
                        source_line: Some(target.line),
                    });
                    "unavailable:image".into()
                }
            }
        };
        replacements.push((
            target.start,
            target.end,
            target
                .replacement
                .replace("MDPDF_IMAGE_DESTINATION", &destination),
        ));
    }
    let mut resolved = markdown.to_owned();
    replacements.sort_unstable_by_key(|(start, _, _)| *start);
    for (start, end, replacement) in replacements.into_iter().rev() {
        resolved.replace_range(start..end, &replacement);
    }
    let theme = theme.clone();
    let mut document =
        tokio::task::spawn_blocking(move || bind(build_print_document(&resolved, &theme)))
            .await
            .map_err(|error| RenderError::Assets(error.to_string()))??;
    document.diagnostics.retain(|diagnostic| {
        diagnostic.kind != DiagKind::Image
            || !diagnostics
                .iter()
                .any(|d| d.source_line == diagnostic.source_line)
    });
    document.diagnostics.extend(diagnostics);
    for (id, image) in images {
        let source = format!("src=\"/assets/{id}\"");
        let embedded = format!(
            "src=\"data:{};base64,{}\"",
            image.mime,
            BASE64_STANDARD.encode(&image.bytes)
        );
        document.html = document.html.replace(&source, &embedded);
    }
    let mut fonts = String::new();
    for font in &context.fonts {
        if !document.html.contains(&font.family) {
            continue;
        }
        if font.bytes.len() > 5 * 1024 * 1024
            || !font
                .family
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(RenderError::Assets("Invalid uploaded font".into()));
        }
        let format = match font.mime.as_str() {
            "font/ttf" => "truetype",
            "font/otf" => "opentype",
            "font/woff2" => "woff2",
            _ => return Err(RenderError::Assets("Unsupported font format".into())),
        };
        fonts.push_str(&format!("@font-face {{font-family:\"{}\";src:url(data:{};base64,{}) format(\"{format}\");font-weight:100 900;font-style:normal;font-display:block;}}\n",font.family,font.mime,BASE64_STANDARD.encode(&font.bytes)));
    }
    if !fonts.is_empty() {
        document.html = document.html.replacen(
            "</head>",
            &format!("<style id=\"mdpdf-custom-fonts\">{fonts}</style></head>"),
            1,
        );
    }
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_networks_and_metadata_addresses_are_blocked() {
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "169.254.169.254",
            "100.64.0.1",
            "192.168.1.1",
            "::1",
            "::ffff:127.0.0.1",
            "fd00::1",
            "fe80::1",
        ] {
            assert!(!public_ip(ip.parse().unwrap()), "{ip}");
        }
        assert!(public_ip("8.8.8.8".parse().unwrap()));
    }
    #[tokio::test]
    async fn uploaded_images_preserve_lines_attributes_and_reference_images() {
        let bytes = BASE64_STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/l9sAAAAASUVORK5CYII=").unwrap();
        let mut context = AssetContext::default();
        context.images.insert(
            "sample".into(),
            ImageAsset {
                bytes,
                mime: "image/png".into(),
            },
        );
        let markdown = "# 图像\n\n![图](asset://sample \"Caption\"){width=50%}\n\n![Reference][image]\n\n[image]: asset://sample\n\n# After\n";
        let document = prepare_document_with_assets(markdown, &Theme::default(), &context)
            .await
            .unwrap();
        assert!(
            document.diagnostics.is_empty(),
            "{:?}",
            document.diagnostics
        );
        assert_eq!(
            document
                .html
                .matches("src=\"data:image/png;base64,")
                .count(),
            2
        );
        assert!(document.html.contains("data-width=\"50\""));
        assert_eq!(document.headings[1].source_line, 9);
    }
}
