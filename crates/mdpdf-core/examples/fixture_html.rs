//! Emit self-contained fixtures for the dependency-free Chromium check script.
use mdpdf_core::{Theme, build_print_document};
use std::{collections::BTreeMap, env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let assets: BTreeMap<String, String> =
        serde_json::from_slice(&fs::read(args.next().ok_or("expected asset map path")?)?)?;
    let output = PathBuf::from(args.next().ok_or("expected output directory")?);
    fs::create_dir_all(&output)?;
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for entry in fs::read_dir(fixtures)? {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "md") {
            let document = build_print_document(&fs::read_to_string(&path)?, &Theme::default())
                .with_embedded_assets(&assets)?
                .with_nonce("browserCheckNonce0123456789")?;
            fs::write(
                output
                    .join(path.file_stem().unwrap())
                    .with_extension("html"),
                document.html,
            )?;
        }
    }
    Ok(())
}
