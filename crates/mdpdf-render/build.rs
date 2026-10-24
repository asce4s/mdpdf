use std::{env, fs, path::PathBuf};

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../assets");
    let css = fs::read_to_string(root.join("fonts/fonts.css")).unwrap();
    println!(
        "cargo:rerun-if-changed={}",
        root.join("fonts/fonts.css").display()
    );
    let mut files = vec![(
        "vendor/mermaid/mermaid.min.js".to_owned(),
        "text/javascript",
    )];
    for part in css.split("url(/assets/").skip(1) {
        let path = part.split(')').next().unwrap();
        assert!(path.starts_with("fonts/") && !path.contains(".."));
        files.push((path.to_owned(), "font/woff2"));
    }
    files.push(("vendor/katex/katex.min.js".to_owned(), "text/javascript"));
    for entry in fs::read_dir(root.join("vendor/katex/fonts")).unwrap() {
        let path = entry.unwrap().path();
        if path
            .extension()
            .is_some_and(|extension| extension == "woff2")
        {
            files.push((
                format!(
                    "vendor/katex/fonts/{}",
                    path.file_name().unwrap().to_str().unwrap()
                ),
                "font/woff2",
            ));
        }
    }
    files.sort_unstable();
    files.dedup();
    let mut source = String::from("static BUNDLED_ASSETS: &[(&str, &str, &[u8])] = &[\n");
    for (path, mime) in files {
        let full = root.join(&path).canonicalize().unwrap();
        println!("cargo:rerun-if-changed={}", full.display());
        source.push_str(&format!(
            "({:?}, {mime:?}, include_bytes!({:?})),\n",
            format!("/assets/{path}"),
            full
        ));
    }
    source.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("assets.rs"),
        source,
    )
    .unwrap();
}
