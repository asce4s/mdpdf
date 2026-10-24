use mdpdf_core::Theme;
use mdpdf_render::{BrowserPool, ExportOptions, PoolConfig};
use std::{fs, process::Command};

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mdpdf"))
}
fn extracted(path: &std::path::Path) -> Vec<u8> {
    let output = Command::new("pdftotext")
        .arg(path)
        .arg("-")
        .output()
        .unwrap();
    assert!(output.status.success());
    output.stdout
}

#[tokio::test]
#[ignore = "requires Chromium and Poppler; validates actual CLI exports and strict exit status"]
async fn cli_and_library_share_layout_and_strict_errors_preserve_files() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input with spaces.md");
    let output = directory.path().join("out.pdf");
    let html = directory.path().join("out.html");
    let theme_file = directory.path().join("theme.toml");
    let markdown = "# Command line\n\nShared **renderer**.\n\n```mermaid\nflowchart LR\n A[Input] --> B[PDF]\n```";
    fs::write(&input, markdown).unwrap();
    let mut theme = Theme::preset("academic").unwrap();
    fs::write(&theme_file, theme.to_toml().unwrap()).unwrap();
    let result = command()
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--html")
        .arg(&html)
        .arg("--theme")
        .arg(&theme_file)
        .args([
            "--set",
            "typography.base_size=12.5",
            "--set",
            "tables.style=grid",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let source = fs::read_to_string(&html).unwrap();
    assert!(source.contains("12.5pt"));
    assert!(source.contains("src=\"data:text/javascript;base64,"));
    assert!(!source.contains("MDPDF_NONCE_PLACEHOLDER"));
    theme.typography.base_size = 12.5;
    theme = theme
        .merged(serde_json::json!({"tables":{"style":"grid"}}))
        .unwrap();
    let pool = BrowserPool::new(PoolConfig {
        size: 1,
        ..Default::default()
    })
    .await
    .unwrap();
    let reference = pool
        .export(markdown, &theme, ExportOptions::default())
        .await
        .unwrap();
    pool.close().await;
    let expected = directory.path().join("reference.pdf");
    fs::write(&expected, &reference.bytes).unwrap();
    assert_eq!(extracted(&output), extracted(&expected));
    assert_eq!(
        lopdf::Document::load(&output).unwrap().get_pages().len(),
        reference.page_count
    );
    fs::write(&input, "```mermaid\nflowchart TD\n A[Unclosed\n```").unwrap();
    fs::write(&output, b"preserve existing file").unwrap();
    let failed = command()
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--strict-diagrams")
        .output()
        .unwrap();
    assert_eq!(failed.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&failed.stderr).contains("line 1"));
    assert_eq!(fs::read(&output).unwrap(), b"preserve existing file");
    let same = command()
        .arg(&input)
        .arg("-o")
        .arg(directory.path().join("same.pdf"))
        .arg("--html")
        .arg(directory.path().join("./same.pdf"))
        .output()
        .unwrap();
    assert!(!same.status.success());
    assert!(String::from_utf8_lossy(&same.stderr).contains("destinations must differ"));
}
