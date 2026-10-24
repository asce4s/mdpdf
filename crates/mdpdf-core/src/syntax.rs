//! Convert uploaded TextMate themes into the same scoped CSS used by presets.
use syntect::{
    highlighting::ThemeSet,
    html::{ClassStyle, css_for_theme_with_class_style},
};

pub fn import_syntax_theme(source: &[u8]) -> std::result::Result<String, String> {
    if source.len() > 2 * 1024 * 1024 {
        return Err("Syntax theme exceeds 2 MB".into());
    }
    let mut theme = ThemeSet::load_from_reader(&mut std::io::Cursor::new(source))
        .map_err(|error| error.to_string())?;
    theme.name = None;
    theme.settings.background = None;
    let raw = css_for_theme_with_class_style(&theme, ClassStyle::Spaced)
        .map_err(|error| error.to_string())?;
    let raw = raw.split_once("*/").map_or(raw.as_str(), |(_, rest)| rest);
    let mut css = String::new();
    for line in raw.lines() {
        if let Some(selectors) = line.strip_suffix(" {") {
            css.push_str(
                &selectors
                    .split(", ")
                    .map(|selector| {
                        if selector == ".code" {
                            ".mdpdf-code code".to_owned()
                        } else {
                            format!(".mdpdf-code {selector}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            css.push_str(" {\n");
        } else {
            css.push_str(line);
            css.push('\n');
        }
    }
    crate::css::sanitize_custom_css(&css)
        .ok_or_else(|| "Syntax theme generates unsupported CSS or exceeds 50 KB".into())
}
#[cfg(test)]
mod tests {
    #[test]
    fn uploaded_themes_are_scoped_and_untrusted_names_are_omitted() {
        let css = super::import_syntax_theme(include_bytes!("../../../assets/syntax/nord.tmTheme"))
            .unwrap();
        assert!(css.contains(".mdpdf-code .comment"));
        assert!(!css.contains("@import"));
        assert!(super::import_syntax_theme(b"not XML").is_err());
    }
}
