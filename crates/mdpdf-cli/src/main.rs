use clap::Parser;
use mdpdf_core::{Severity, Theme};
use mdpdf_render::{
    AssetContext, BrowserPool, ExportOptions, PoolConfig, RenderError, prepare_document_with_assets,
};
use serde_json::Value;
use std::{fs, path::PathBuf, process::ExitCode};

#[derive(Debug, Parser)]
#[command(
    name = "mdpdf",
    version,
    about = "Print Markdown and Mermaid diagrams to PDF"
)]
struct Args {
    /// Markdown file to convert.
    input: PathBuf,
    /// PDF destination (defaults to the input path with a .pdf extension).
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Built-in preset or a .mdpdf-theme.toml file.
    #[arg(long, default_value = "default")]
    theme: String,
    /// Override a theme field, for example --set tables.style=grid. Repeatable.
    #[arg(long = "set", value_name = "KEY=VALUE")]
    settings: Vec<String>,
    /// Fail with exit code 2 if any Mermaid diagram fails.
    #[arg(long)]
    strict_diagrams: bool,
    /// Also save the exact self-contained HTML sent to Chromium.
    #[arg(long)]
    html: Option<PathBuf>,
}

fn set_theme(theme: &Theme, settings: &[String]) -> Result<Theme, String> {
    let mut value = serde_json::to_value(theme).map_err(|error| error.to_string())?;
    for setting in settings {
        let (key, raw) = setting
            .split_once('=')
            .ok_or_else(|| format!("Expected KEY=VALUE: {setting}"))?;
        let mut field = &mut value;
        for part in key.split('.') {
            field = if let Some(object) = field.as_object_mut() {
                object.get_mut(part)
            } else {
                None
            }
            .ok_or_else(|| format!("Unknown theme field: {key}"))?;
        }
        *field = if field.is_string() {
            Value::String(raw.to_owned())
        } else {
            serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_owned()))
        };
    }
    serde_json::from_value(value).map_err(|error| format!("Invalid theme override: {error}"))
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    match run(args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("mdpdf: {error}");
            if let RenderError::DiagramErrors(errors) = &error {
                for diagnostic in errors {
                    eprintln!(
                        "  line {}: {}",
                        diagnostic.source_line.unwrap_or(0),
                        diagnostic.message
                    );
                }
                ExitCode::from(2)
            } else {
                ExitCode::FAILURE
            }
        }
    }
}

async fn run(args: Args) -> Result<(), RenderError> {
    let input_error = |error: std::io::Error| RenderError::Configuration(error.to_string());
    let markdown = fs::read_to_string(&args.input).map_err(input_error)?;
    if markdown.len() > 2 * 1024 * 1024 {
        return Err(RenderError::Configuration(
            "Markdown exceeds 2 MB limit".into(),
        ));
    }
    let theme = match Theme::preset(&args.theme) {
        Ok(theme) => theme,
        Err(_) => Theme::from_toml(&fs::read_to_string(&args.theme).map_err(input_error)?)
            .map_err(RenderError::Configuration)?,
    };
    let theme = set_theme(&theme, &args.settings).map_err(RenderError::Configuration)?;
    let output = args
        .output
        .unwrap_or_else(|| args.input.with_extension("pdf"));
    let input = args.input.canonicalize().map_err(input_error)?;
    let destination = |path: &std::path::Path| -> Result<PathBuf, RenderError> {
        if path.exists() {
            return path.canonicalize().map_err(input_error);
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        Ok(parent.canonicalize().map_err(input_error)?.join(
            path.file_name()
                .ok_or_else(|| RenderError::Configuration("Output must name a file".into()))?,
        ))
    };
    for path in std::iter::once(&output).chain(args.html.iter()) {
        if destination(path)? == input {
            return Err(RenderError::Configuration(
                "Output must not overwrite the input Markdown".into(),
            ));
        }
    }
    if let Some(html) = &args.html
        && destination(html)? == destination(&output)?
    {
        return Err(RenderError::Configuration(
            "PDF and HTML destinations must differ".into(),
        ));
    }
    let context = AssetContext {
        local_root: input.parent().map(std::path::Path::to_path_buf),
        ..AssetContext::default()
    };
    let document = prepare_document_with_assets(&markdown, &theme, &context).await?;
    let html = args.html.as_ref().map(|_| document.html.clone());
    // A one-job CLI process needs one tab; server callers use the default pool of four.
    let pool = BrowserPool::new(PoolConfig {
        size: 1,
        ..PoolConfig::from_env()?
    })
    .await?;
    let result = pool
        .render(
            document,
            ExportOptions {
                strict_diagrams: args.strict_diagrams,
            },
        )
        .await;
    pool.close().await;
    let rendered = result?;
    for diagnostic in &rendered.diagnostics {
        eprintln!(
            "{} (line {}): {}",
            if diagnostic.severity == Severity::Error {
                "error"
            } else {
                "warning"
            },
            diagnostic.source_line.unwrap_or(0),
            diagnostic.message
        );
    }
    fs::write(&output, &rendered.bytes).map_err(input_error)?;
    if let (Some(path), Some(html)) = (args.html, html) {
        fs::write(path, html).map_err(input_error)?;
    }
    eprintln!(
        "Wrote {} ({} page(s))",
        output.display(),
        rendered.page_count
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overrides_keep_types_and_reject_unknown_fields() {
        let theme = set_theme(
            &Theme::default(),
            &[
                "tables.style=grid".into(),
                "tables.zebra=false".into(),
                "typography.base_size=12".into(),
            ],
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(&theme).unwrap()["tables"]["style"],
            "grid"
        );
        assert!(!theme.tables.zebra);
        assert_eq!(theme.typography.base_size, 12.0);
        for value in [
            "tables.typo=grid",
            "tables.zebra=maybe",
            "page.size=invalid",
            "tables",
        ] {
            assert!(set_theme(&theme, &[value.into()]).is_err(), "{value}");
        }
    }
}
