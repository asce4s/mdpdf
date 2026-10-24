use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagKind {
    Mermaid,
    Image,
    FrontMatter,
    Font,
    Html,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub kind: DiagKind,
    pub message: String,
    pub source_line: Option<u32>,
}

impl Diagnostic {
    pub fn error(kind: DiagKind, message: impl Into<String>, line: Option<u32>) -> Self {
        Self {
            severity: Severity::Error,
            kind,
            message: message.into(),
            source_line: line,
        }
    }
}
