//! Regenerate CSS from the pinned, licensed TextMate themes.
use std::{fs, path::PathBuf};
use syntect::{
    highlighting::ThemeSet,
    html::{ClassStyle, css_for_theme_with_class_style},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/syntax");
    for name in [
        "github-light",
        "one-dark",
        "solarized-light",
        "solarized-dark",
        "nord",
    ] {
        let mut theme = ThemeSet::get_theme(root.join(format!("{name}.tmTheme")))?;
        // The user's code background remains controlled by Theme.code.background.
        theme.settings.background = None;
        let raw = css_for_theme_with_class_style(&theme, ClassStyle::Spaced)?;
        let mut css = String::new();
        for line in raw.lines() {
            if let Some(selectors) = line.strip_suffix(" {") {
                let selectors = selectors
                    .split(", ")
                    .map(|selector| {
                        if selector == ".code" {
                            ".mdpdf-code code".to_owned()
                        } else {
                            format!(".mdpdf-code {selector}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                css.push_str(&selectors);
                css.push_str(" {\n");
            } else {
                css.push_str(line);
                css.push('\n');
            }
        }
        fs::write(root.join(format!("{name}.css")), css)?;
    }
    Ok(())
}
