use super::{ApiError, AppState};
use axum::{
    Extension, Json,
    extract::{Multipart, Path, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use mdpdf_core::{PrintDocument, Theme};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime},
};

#[derive(Clone, Debug, Default)]
pub struct SessionId(pub String);
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetInfo {
    pub id: String,
    pub kind: String,
    pub mime: String,
    pub name: String,
    pub url: String,
}
#[derive(Clone)]
pub struct AssetStore {
    root: PathBuf,
}
impl AssetStore {
    pub fn new(root: PathBuf) -> std::io::Result<Self> {
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }
    fn directory(&self, session: &SessionId) -> Result<PathBuf, ApiError> {
        if session.0.len() != 48 || !session.0.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ApiError::new(
                StatusCode::UNAUTHORIZED,
                "Open the workspace to initialize a session",
            ));
        }
        Ok(self.root.join(&session.0))
    }
    pub fn cleanup(&self, ttl: Duration) -> std::io::Result<()> {
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let name = entry.file_name();
            let owned_session = name.to_str().is_some_and(|name| {
                name.len() == 48 && name.bytes().all(|byte| byte.is_ascii_hexdigit())
            });
            if !owned_session || !entry.file_type()?.is_dir() {
                continue;
            }
            let newest = fs::read_dir(entry.path())?
                .filter_map(Result::ok)
                .filter_map(|e| e.metadata().ok()?.modified().ok())
                .max()
                .unwrap_or(SystemTime::UNIX_EPOCH);
            if newest.elapsed().is_ok_and(|age| age > ttl) {
                fs::remove_dir_all(entry.path())?;
            }
        }
        Ok(())
    }
    pub async fn prepare(
        &self,
        markdown: &str,
        theme: &Theme,
        session: &SessionId,
    ) -> Result<PrintDocument, ApiError> {
        let mut context = mdpdf_render::AssetContext::default();
        let mut total_bytes = 0u64;
        let hints = format!(
            "{} {}",
            markdown,
            serde_json::to_string(theme).unwrap_or_default()
        );
        for asset in self.list(session)? {
            if asset.id.len() != 64 || !asset.id.bytes().all(|b| b.is_ascii_hexdigit()) {
                continue;
            }
            let family = format!("assetfont-{}", asset.id);
            if hints.contains(&asset.id) {
                let length = fs::metadata(self.directory(session)?.join(&asset.id))
                    .map_err(io_error)?
                    .len();
                total_bytes += length;
                if total_bytes > 64 * 1024 * 1024 {
                    return Err(ApiError::new(
                        StatusCode::PAYLOAD_TOO_LARGE,
                        "Document assets exceed 64 MB",
                    ));
                }
            }
            if ((asset.kind == "image" && hints.contains(&asset.id))
                || (asset.kind == "font" && hints.contains(&family)))
                && let Ok(bytes) = fs::read(self.directory(session)?.join(&asset.id))
            {
                if asset.kind == "image" {
                    context.images.insert(
                        asset.id,
                        mdpdf_render::ImageAsset {
                            bytes,
                            mime: asset.mime,
                        },
                    );
                } else {
                    context.fonts.push(mdpdf_render::FontAsset {
                        bytes,
                        mime: asset.mime,
                        family,
                    });
                }
            }
        }
        mdpdf_render::prepare_document_with_assets(markdown, theme, &context)
            .await
            .map_err(|error| ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, error.to_string()))
    }
    pub fn list(&self, session: &SessionId) -> Result<Vec<AssetInfo>, ApiError> {
        let directory = self.directory(session)?;
        if !directory.exists() {
            return Ok(Vec::new());
        }
        Ok(fs::read_dir(directory)
            .map_err(io_error)?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|e| e == "json"))
            .filter_map(|entry| serde_json::from_slice(&fs::read(entry.path()).ok()?).ok())
            .collect())
    }
}
fn io_error(error: std::io::Error) -> ApiError {
    eprintln!("Asset I/O: {error}");
    ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Could not access this session's asset storage",
    )
}
fn kind(data: &[u8]) -> Option<(&'static str, &'static str)> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(("image", "image/png"))
    } else if data.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(("image", "image/jpeg"))
    } else if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP") {
        Some(("image", "image/webp"))
    } else if data.starts_with(b"wOF2") {
        Some(("font", "font/woff2"))
    } else if data.starts_with(b"OTTO") {
        Some(("font", "font/otf"))
    } else if data.starts_with(&[0, 1, 0, 0]) {
        Some(("font", "font/ttf"))
    } else {
        None
    }
}
pub async fn upload(
    State(state): State<AppState>,
    Extension(session): Extension<SessionId>,
    mut multipart: Multipart,
) -> Result<Json<AssetInfo>, ApiError> {
    let field = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, e.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "Choose a file to upload"))?;
    let name = field
        .file_name()
        .unwrap_or("upload")
        .chars()
        .filter(|c| !c.is_control())
        .take(100)
        .collect::<String>();
    let bytes = field
        .bytes()
        .await
        .map_err(|e| ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, e.to_string()))?;
    let (kind, mime) = kind(&bytes).ok_or_else(|| {
        ApiError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Use PNG, JPEG, WebP, TTF, OTF or WOFF2",
        )
    })?;
    let existing = state.assets.list(&session)?;
    if kind == "font" && existing.iter().filter(|asset| asset.kind == "font").count() >= 10 {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "This session already contains 10 custom fonts",
        ));
    }
    if existing.len() >= 100 {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "This session already contains 100 assets",
        ));
    }
    let cap = if kind == "font" { 5 } else { 10 } * 1024 * 1024;
    if bytes.len() > cap {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "The file exceeds its upload size limit",
        ));
    }
    let directory = state.assets.directory(&session)?;
    fs::create_dir_all(&directory).map_err(io_error)?;
    let id: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let info = AssetInfo {
        url: format!("/assets/{id}"),
        id: id.clone(),
        kind: kind.into(),
        mime: mime.into(),
        name,
    };
    fs::write(directory.join(&id), &bytes).map_err(io_error)?;
    fs::write(
        directory.join(format!("{id}.json")),
        serde_json::to_vec(&info).unwrap(),
    )
    .map_err(io_error)?;
    Ok(Json(info))
}
pub async fn download(
    State(state): State<AppState>,
    Extension(session): Extension<SessionId>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "Asset not found"));
    }
    let directory = state.assets.directory(&session)?;
    let info: AssetInfo = serde_json::from_slice(
        &fs::read(directory.join(format!("{id}.json")))
            .map_err(|_| ApiError::new(StatusCode::NOT_FOUND, "Asset not found in this session"))?,
    )
    .map_err(|_| ApiError::new(StatusCode::NOT_FOUND, "Asset not found"))?;
    let bytes = fs::read(directory.join(id)).map_err(io_error)?;
    Ok((
        [
            (header::CONTENT_TYPE, info.mime),
            (header::CACHE_CONTROL, "private, no-store".into()),
        ],
        bytes,
    ))
}

pub async fn list(
    State(state): State<AppState>,
    Extension(session): Extension<SessionId>,
) -> Result<Json<Vec<AssetInfo>>, ApiError> {
    state.assets.list(&session).map(Json)
}

#[cfg(test)]
mod tests {
    #[test]
    fn cleanup_removes_expired_sessions_and_preserves_unrelated_directories() {
        let root = tempfile::tempdir().unwrap();
        let store = super::AssetStore::new(root.path().to_owned()).unwrap();
        let session = root.path().join("1".repeat(48));
        let unrelated = root.path().join("keep-my-files");
        for directory in [&session, &unrelated] {
            std::fs::create_dir(directory).unwrap();
            std::fs::write(directory.join("data"), "keep").unwrap();
        }
        store.cleanup(std::time::Duration::ZERO).unwrap();
        assert!(!session.exists());
        assert!(unrelated.join("data").exists());
    }
}
