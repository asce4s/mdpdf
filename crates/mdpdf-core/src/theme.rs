//! Serializable rendering options. Defaults also apply to partial theme files.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

macro_rules! choice {
    ($name:ident, $default:ident, $($other:ident),* $(,)?) => {
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "kebab-case")]
        pub enum $name { #[default] $default, $($other),* }
    };
}
choice!(Orientation, Portrait, Landscape);
choice!(TextAlign, Left, Justify);
choice!(Alignment, Center, Left, Right);
choice!(TableStyle, HorizontalLines, Grid, Minimal, None);
choice!(TableWidth, Full, Auto);
choice!(WideTables, ShrinkToFit, Wrap, RotateLandscape);
choice!(CaptionPosition, Above, Below);
choice!(Oversize, FitPage, AllowSplit, None);
choice!(MermaidTheme, Neutral, Default, Dark, Forest, Base);
choice!(Curve, Basis, Linear, Step, Cardinal);
choice!(
    SyntaxTheme,
    GithubLight,
    OneDark,
    SolarizedLight,
    SolarizedDark,
    Nord
);

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PageSize {
    A3,
    #[default]
    A4,
    A5,
    Letter,
    Legal,
    Custom {
        width: f64,
        height: f64,
    },
}

macro_rules! options {
    ($name:ident { $($field:ident : $ty:ty = $value:expr),* $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(default, deny_unknown_fields)]
        pub struct $name { $(pub $field: $ty),* }
        impl Default for $name {
            fn default() -> Self { Self { $($field: $value),* } }
        }
    };
}
options!(Margins {
    top: f64 = 20.0,
    right: f64 = 18.0,
    bottom: f64 = 20.0,
    left: f64 = 18.0
});
options!(PageOptions {
    size: PageSize = PageSize::default(),
    orientation: Orientation = Orientation::default(),
    margins: Margins = Margins::default(),
    background: String = "#ffffff".into(),
    toc: bool = false,
    toc_depth: u8 = 3,
    heading_numbers: bool = false,
    break_before_h1: bool = false,
});
options!(Typography {
    body_font: String = "Inter".into(),
    heading_font: String = "Inter".into(),
    mono_font: String = "JetBrains Mono".into(),
    base_size: f64 = 11.0,
    line_height: f64 = 1.5,
    paragraph_spacing: f64 = 0.75,
    text_color: String = "#1a1a1a".into(),
    link_color: String = "#0b5fff".into(),
    text_align: TextAlign = TextAlign::default(),
    hyphenation: bool = false,
});
options!(HeadingLevel {
    size: f64 = 18.0,
    weight: u16 = 600,
    color: String = "inherit".into(),
    margin_top: f64 = 1.2,
    margin_bottom: f64 = 0.5,
    bottom_border: Option<String> = None,
    uppercase: bool = false,
});
options!(HeadingOptions {
    levels: [HeadingLevel; 6] = [24.0, 20.0, 16.0, 14.0, 12.0, 11.0].map(|size| HeadingLevel {
        size,
        ..HeadingLevel::default()
    }),
});
options!(TableOptions {
    style: TableStyle = TableStyle::default(),
    border_color: String = "#d0d7de".into(),
    border_width: f64 = 1.0,
    header_bg: String = "#f6f8fa".into(),
    header_color: String = "inherit".into(),
    header_weight: u16 = 600,
    zebra: bool = true,
    zebra_color: String = "#fafbfc".into(),
    cell_padding_vertical: f64 = 6.0,
    cell_padding_horizontal: f64 = 10.0,
    font_size: f64 = 0.95,
    width: TableWidth = TableWidth::default(),
    repeat_header: bool = true,
    avoid_row_split: bool = true,
    wide_tables: WideTables = WideTables::default(),
    caption_position: CaptionPosition = CaptionPosition::default(),
});
options!(CodeOptions {
    syntax_theme: SyntaxTheme = SyntaxTheme::default(),
    custom_syntax_css: Option<String> = None,
    font_size: f64 = 9.0,
    background: String = "#f6f8fa".into(),
    border: String = "#d0d7de".into(),
    line_numbers: bool = false,
    wrap: bool = true,
    avoid_break_under_lines: u32 = 25,
});
options!(DiagramOptions {
    mermaid_theme: MermaidTheme = MermaidTheme::default(),
    theme_variables: BTreeMap<String, String> = BTreeMap::new(),
    font_family: Option<String> = None,
    font_size: f64 = 14.0,
    max_width: f64 = 100.0,
    align: Alignment = Alignment::default(),
    oversize: Oversize = Oversize::default(),
    captions: bool = true,
    numbering: bool = false,
    background: String = "transparent".into(),
    flowchart_curve: Curve = Curve::default(),
    sequence_mirror_actors: bool = true,
});
options!(ImageOptions {
    numbering: bool = false,
    max_width: f64 = 100.0,
    align: Alignment = Alignment::default(),
    border_radius: f64 = 0.0,
    border: Option<String> = None,
    shadow: bool = false,
    caption_size: f64 = 0.9,
    caption_color: String = "#57606a".into(),
});
options!(Slots {
    left: String = String::new(),
    center: String = String::new(),
    right: String = String::new()
});
options!(HeaderFooter {
    header: Slots = Slots::default(),
    footer: Slots = Slots::default(),
    font_size: f64 = 9.0,
    color: String = "#57606a".into(),
    hide_on_first_page: bool = false,
});
options!(Theme {
    name: String = "Default".into(),
    page: PageOptions = PageOptions::default(),
    typography: Typography = Typography::default(),
    headings: HeadingOptions = HeadingOptions::default(),
    tables: TableOptions = TableOptions::default(),
    code: CodeOptions = CodeOptions::default(),
    diagrams: DiagramOptions = DiagramOptions::default(),
    images: ImageOptions = ImageOptions::default(),
    header_footer: HeaderFooter = HeaderFooter::default(),
    custom_css: Option<String> = None,
});

impl Theme {
    pub fn preset(name: &str) -> Result<Self, String> {
        let input = match name.to_ascii_lowercase().as_str() {
            "default" => include_str!("../../../assets/themes/default.toml"),
            "github" => include_str!("../../../assets/themes/github.toml"),
            "academic" => include_str!("../../../assets/themes/academic.toml"),
            "corporate" => include_str!("../../../assets/themes/corporate.toml"),
            "compact" => include_str!("../../../assets/themes/compact.toml"),
            _ => return Err(format!("Unknown theme preset: {name}")),
        };
        Self::from_toml(input)
    }

    pub fn from_toml(input: &str) -> Result<Self, String> {
        toml::from_str(input).map_err(|error| error.to_string())
    }

    pub fn to_toml(&self) -> Result<String, String> {
        toml::to_string_pretty(self).map_err(|error| error.to_string())
    }

    /// Recursively merge an explicit patch, preserving unspecified user edits.
    pub fn merged(&self, patch: serde_json::Value) -> Result<Self, String> {
        let mut value = serde_json::to_value(self).map_err(|error| error.to_string())?;
        merge(&mut value, patch);
        serde_json::from_value(value).map_err(|error| error.to_string())
    }
}

pub(crate) fn merge(base: &mut serde_json::Value, patch: serde_json::Value) {
    if let (Some(base), Some(patch)) = (base.as_object_mut(), patch.as_object()) {
        for (key, value) in patch {
            merge(
                base.entry(key).or_insert(serde_json::Value::Null),
                value.clone(),
            );
        }
    } else {
        *base = patch;
    }
}
