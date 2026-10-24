use super::ApiError;
use axum::{Json, http::StatusCode};
use mdpdf_core::Theme;
use serde_json::{Value, json};

pub async fn list() -> Json<Value> {
    Json(json!(
        ["default", "github", "academic", "corporate", "compact"]
            .map(|name| json!({"id":name,"theme":Theme::preset(name).unwrap()}))
    ))
}
pub async fn import(body: String) -> Result<Json<Theme>, ApiError> {
    if body.len() > 50 * 1024 {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "Theme exceeds 50 KB",
        ));
    }
    Theme::from_toml(&body)
        .map(Json)
        .map_err(|error| ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, error))
}
pub async fn export(Json(theme): Json<Theme>) -> Result<String, ApiError> {
    theme
        .to_toml()
        .map_err(|error| ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, error))
}

pub async fn import_syntax(body: axum::body::Bytes) -> Result<Json<Value>, ApiError> {
    mdpdf_render::import_syntax_theme(&body)
        .map(|css| Json(json!({"css":css})))
        .map_err(|error| ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, error))
}
