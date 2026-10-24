//! Shared TextMate import implementation, also available in browser WASM.
pub use mdpdf_core::syntax::import_syntax_theme;
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
