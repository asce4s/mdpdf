//! Native delivery and Chromium PDF export shared by the CLI and web server.
mod assets;
mod browser;
mod images;
mod pdf;
mod syntax;
pub use images::{AssetContext, FontAsset, ImageAsset, image_mime, prepare_document_with_assets};
pub use syntax::import_syntax_theme;

pub use assets::prepare_document;
pub use browser::{BrowserPool, PoolConfig};
use mdpdf_core::Diagnostic;
use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, Default)]
pub struct ExportOptions {
    pub strict_diagrams: bool,
}

#[derive(Debug)]
pub struct RenderedPdf {
    pub bytes: Vec<u8>,
    pub diagnostics: Vec<Diagnostic>,
    pub page_count: usize,
}

#[derive(Debug)]
pub enum RenderError {
    Configuration(String),
    Browser(String),
    Assets(String),
    Pdf(String),
    DiagramErrors(Vec<Diagnostic>),
    Timeout,
    Closed,
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(message) => write!(f, "Invalid renderer configuration: {message}"),
            Self::Browser(message) => write!(f, "Chromium: {message}"),
            Self::Assets(message) => write!(f, "Document assets: {message}"),
            Self::Pdf(message) => write!(f, "PDF: {message}"),
            Self::DiagramErrors(errors) => {
                write!(f, "{} diagram error(s) in strict mode", errors.len())
            }
            Self::Timeout => f.write_str("Export exceeded its time limit"),
            Self::Closed => f.write_str("Chromium pool is closed"),
        }
    }
}
impl Error for RenderError {}
impl From<chromiumoxide::error::CdpError> for RenderError {
    fn from(error: chromiumoxide::error::CdpError) -> Self {
        Self::Browser(error.to_string())
    }
}
pub type Result<T> = std::result::Result<T, RenderError>;
