use mdpdf_core::{Metadata, Theme, css};
use serde_json::{Value, json};

fn leaves(value: &Value, path: String, out: &mut Vec<(String, Value)>) {
    match value {
        Value::Object(map) if !map.is_empty() => {
            for (key, value) in map {
                leaves(value, format!("{path}/{key}"), out);
            }
        }
        Value::Array(array) => {
            for (index, value) in array.iter().enumerate() {
                leaves(value, format!("{path}/{index}"), out);
            }
        }
        _ => out.push((path, value.clone())),
    }
}

fn replacement(path: &str, value: &Value) -> Value {
    match value {
        Value::Bool(value) => json!(!value),
        Value::Number(_) if path.ends_with("max_width") => json!(80.0),
        Value::Number(value) => value.as_u64().map_or_else(
            || json!(value.as_f64().unwrap() + 1.0),
            |value| json!(value + 1),
        ),
        Value::Null => match path {
            "/custom_css" => json!("p { color: #123456; }"),
            "/diagrams/font_family" => json!("Source Serif 4"),
            _ => json!("#123456"),
        },
        Value::Object(_) => json!({"primaryColor": "#123456"}),
        Value::String(_) => match path {
            "/page/size" => json!("letter"),
            "/page/orientation" => json!("landscape"),
            "/typography/text_align" => json!("justify"),
            "/tables/style" => json!("grid"),
            "/tables/width" => json!("auto"),
            "/tables/wide_tables" => json!("wrap"),
            "/tables/caption_position" => json!("below"),
            "/code/syntax_theme" => json!("nord"),
            "/diagrams/mermaid_theme" => json!("dark"),
            "/diagrams/oversize" => json!("allow-split"),
            "/diagrams/flowchart_curve" => json!("linear"),
            "/diagrams/align" | "/images/align" => json!("left"),
            path if path.starts_with("/header_footer/header/")
                || path.starts_with("/header_footer/footer/") =>
            {
                json!("Sample {page}")
            }
            path if path.ends_with("font") || path == "/name" => json!("Merriweather"),
            _ => json!("#123456"),
        },
        Value::Array(_) => unreachable!(),
    }
}

fn output(theme: &Theme) -> String {
    format!(
        "{}{}",
        css::compile_css(theme, &Metadata::default()),
        css::mermaid_config(theme)
    )
}

#[test]
fn every_theme_leaf_has_a_default_roundtrips_and_changes_compiled_output() {
    let default = Theme::default();
    let value = serde_json::to_value(&default).unwrap();
    let mut fields = Vec::new();
    leaves(&value, String::new(), &mut fields);
    assert!(
        fields.len() > 100,
        "Expected every per-heading field to be covered"
    );
    let baseline = output(&default);
    for (path, old) in fields {
        let mut edited = value.clone();
        *edited.pointer_mut(&path).unwrap() = replacement(&path, &old);
        let changed: Theme =
            serde_json::from_value(edited).unwrap_or_else(|error| panic!("{path}: {error}"));
        assert_ne!(changed, default, "{path}: edit was lost");
        assert!(
            output(&changed) != baseline,
            "{path}: has no CSS/Mermaid mapping"
        );
    }
}

#[test]
fn presets_and_partial_files_roundtrip() {
    for name in ["default", "github", "academic", "corporate", "compact"] {
        let theme = Theme::preset(name).unwrap();
        assert_eq!(Theme::from_toml(&theme.to_toml().unwrap()).unwrap(), theme);
        assert!(output(&theme).contains("@page"));
    }
    assert_eq!(Theme::from_toml("").unwrap(), Theme::default());
    assert!(Theme::preset("unknown").is_err());
    assert!(Theme::from_toml("typo = true").is_err());
}

#[test]
fn nested_patch_preserves_unspecified_user_edits() {
    let mut theme = Theme::preset("academic").unwrap();
    theme.tables.zebra = false;
    let changed = theme
        .merged(json!({"tables": {"style": "minimal"}, "page": {"margins": {"top": 30}}}))
        .unwrap();
    assert!(!changed.tables.zebra);
    assert_eq!(changed.typography, theme.typography);
    assert_eq!(changed.page.margins.left, 18.0);
    assert_eq!(changed.page.margins.top, 30.0);
    assert!(theme.merged(json!({"tables": {"style": "typo"}})).is_err());
}

#[test]
fn custom_page_and_safe_css() {
    let theme = Theme::default()
        .merged(json!({"page": {"size": {"custom": {"width": 180, "height": 240}}}}))
        .unwrap();
    assert_eq!(css::page_dimensions(&theme), (180.0, 240.0));
    for attack in [
        "@import 'https://example.org';",
        "p { background: url(https://evil); }",
        "p { background: u\\72l(https://evil); }",
        "p { background: u/**/rl(https://evil); }",
        "</style><script>alert(1)</script>",
    ] {
        assert!(css::sanitize_custom_css(attack).is_none(), "{attack}");
    }
    assert!(css::sanitize_custom_css(&"x".repeat(50 * 1024 + 1)).is_none());
    assert_eq!(
        css::sanitize_custom_css("p { color: #123456; }"),
        Some("p { color: #123456; }".into())
    );
}

#[test]
fn security_config_and_header_footer_tokens() {
    let mut theme = Theme::default();
    theme.header_footer.footer.center =
        "{title} — {author} — {date} — Page {page} of {pages} — {section}".into();
    theme.header_footer.hide_on_first_page = true;
    let css = css::compile_css(
        &theme,
        &Metadata {
            title: "Report".into(),
            author: "Ada".into(),
            date: "2026".into(),
            ..Metadata::default()
        },
    );
    for token in [
        "counter(page)",
        "counter(pages)",
        "string(section)",
        "Report",
        "Ada",
        "2026",
        "@page :first",
    ] {
        assert!(css.contains(token), "{token}");
    }
    let config = css::mermaid_config(&theme);
    assert_eq!(config["securityLevel"], "strict");
    assert_eq!(config["startOnLoad"], false);
    assert_eq!(config["fontFamily"], theme.typography.body_font);
}
