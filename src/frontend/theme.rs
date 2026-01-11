use serde::{Deserialize, Serialize};
use thaw::Theme as ThawTheme;

/// Catppuccin theme variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CatppuccinTheme {
    Latte,     // Light theme
    Frappe,    // Dark pastel
    Macchiato, // Dark warm
    Mocha,     // Dark (original)
}

impl CatppuccinTheme {
    /// Get all available themes
    pub fn all() -> &'static [CatppuccinTheme] {
        &[
            CatppuccinTheme::Latte,
            CatppuccinTheme::Frappe,
            CatppuccinTheme::Macchiato,
            CatppuccinTheme::Mocha,
        ]
    }

    /// Get the theme name as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            CatppuccinTheme::Latte => "latte",
            CatppuccinTheme::Frappe => "frappe",
            CatppuccinTheme::Macchiato => "macchiato",
            CatppuccinTheme::Mocha => "mocha",
        }
    }

    /// Get the display name for the theme
    pub fn display_name(&self) -> &'static str {
        match self {
            CatppuccinTheme::Latte => "Latte (Light)",
            CatppuccinTheme::Frappe => "Frappé (Dark)",
            CatppuccinTheme::Macchiato => "Macchiato (Dark)",
            CatppuccinTheme::Mocha => "Mocha (Dark)",
        }
    }

    /// Check if this is a light theme (for Thaw UI)
    pub fn is_light(&self) -> bool {
        matches!(self, CatppuccinTheme::Latte)
    }

    /// Get the corresponding Thaw UI theme
    pub fn to_thaw_theme(&self) -> ThawTheme {
        if self.is_light() {
            ThawTheme::light()
        } else {
            ThawTheme::dark()
        }
    }

    /// Get CSS color values for this theme
    pub fn colors(&self) -> ThemeColors {
        match self {
            CatppuccinTheme::Latte => ThemeColors::LATTE,
            CatppuccinTheme::Frappe => ThemeColors::FRAPPE,
            CatppuccinTheme::Macchiato => ThemeColors::MACCHIATO,
            CatppuccinTheme::Mocha => ThemeColors::MOCHA,
        }
    }
}

impl Default for CatppuccinTheme {
    fn default() -> Self {
        CatppuccinTheme::Mocha
    }
}

/// Color palette for a Catppuccin theme
#[derive(Debug, Clone, Copy)]
pub struct ThemeColors {
    pub base: &'static str,
    pub mantle: &'static str,
    pub crust: &'static str,
    pub text: &'static str,
    pub subtext1: &'static str,
    pub subtext0: &'static str,
    pub overlay0: &'static str,
    pub overlay1: &'static str,
    pub overlay2: &'static str,
    pub surface0: &'static str,
    pub surface1: &'static str,
    pub surface2: &'static str,
    pub rosewater: &'static str,
    pub flamingo: &'static str,
    pub pink: &'static str,
    pub mauve: &'static str,
    pub red: &'static str,
    pub maroon: &'static str,
    pub peach: &'static str,
    pub yellow: &'static str,
    pub green: &'static str,
    pub teal: &'static str,
    pub sky: &'static str,
    pub sapphire: &'static str,
    pub blue: &'static str,
    pub lavender: &'static str,
}

impl ThemeColors {
    /// Latte (Light) color palette
    pub const LATTE: ThemeColors = ThemeColors {
        base: "#eff1f5",
        mantle: "#e6e9ef",
        crust: "#dce0e8",
        text: "#4c4f69",
        subtext1: "#5c5f77",
        subtext0: "#6c6f85",
        overlay0: "#9ca0b0",
        overlay1: "#8c8fa1",
        overlay2: "#7c7f93",
        surface0: "#ccd0da",
        surface1: "#bcc0cc",
        surface2: "#acb0be",
        rosewater: "#dc8a78",
        flamingo: "#dd7878",
        pink: "#ea76cb",
        mauve: "#8839ef",
        red: "#d20f39",
        maroon: "#e64553",
        peach: "#fe640b",
        yellow: "#df8e1d",
        green: "#40a02b",
        teal: "#179299",
        sky: "#04a5e5",
        sapphire: "#209fb5",
        blue: "#1e66f5",
        lavender: "#7287fd",
    };

    /// Frappé (Dark Pastel) color palette
    pub const FRAPPE: ThemeColors = ThemeColors {
        base: "#303446",
        mantle: "#292c3c",
        crust: "#232634",
        text: "#c6d0f5",
        subtext1: "#b5bfe2",
        subtext0: "#a5adce",
        overlay0: "#737994",
        overlay1: "#838ba7",
        overlay2: "#949cbb",
        surface0: "#414559",
        surface1: "#51576d",
        surface2: "#626880",
        rosewater: "#f2d5cf",
        flamingo: "#eebebe",
        pink: "#f4b8e4",
        mauve: "#ca9ee6",
        red: "#e78284",
        maroon: "#ea999c",
        peach: "#ef9f76",
        yellow: "#e5c890",
        green: "#a6d189",
        teal: "#81c8be",
        sky: "#99d1db",
        sapphire: "#85c1dc",
        blue: "#8caaee",
        lavender: "#babbf1",
    };

    /// Macchiato (Dark Warm) color palette
    pub const MACCHIATO: ThemeColors = ThemeColors {
        base: "#24273a",
        mantle: "#1e2030",
        crust: "#181926",
        text: "#cad3f5",
        subtext1: "#b8c0e0",
        subtext0: "#a5adcb",
        overlay0: "#6e738d",
        overlay1: "#8087a2",
        overlay2: "#939ab7",
        surface0: "#363a4f",
        surface1: "#494d64",
        surface2: "#5b6078",
        rosewater: "#f4dbd6",
        flamingo: "#f0c6c6",
        pink: "#f5bde6",
        mauve: "#c6a0f6",
        red: "#ed8796",
        maroon: "#ee99a0",
        peach: "#f5a97f",
        yellow: "#eed49f",
        green: "#a6da95",
        teal: "#8bd5ca",
        sky: "#91d7e3",
        sapphire: "#7dc4e4",
        blue: "#8aadf4",
        lavender: "#b7bdf8",
    };

    /// Mocha (Dark) color palette - original
    pub const MOCHA: ThemeColors = ThemeColors {
        base: "#1e1e2e",
        mantle: "#181825",
        crust: "#11111b",
        text: "#cdd6f4",
        subtext1: "#bac2de",
        subtext0: "#a6adc8",
        overlay0: "#6c7086",
        overlay1: "#7f849c",
        overlay2: "#9399b2",
        surface0: "#313244",
        surface1: "#45475a",
        surface2: "#585b70",
        rosewater: "#f5e0dc",
        flamingo: "#f2cdcd",
        pink: "#f5c2e7",
        mauve: "#cba6f7",
        red: "#f38ba8",
        maroon: "#eba0ac",
        peach: "#fab387",
        yellow: "#f9e2af",
        green: "#a6e3a1",
        teal: "#94e2d5",
        sky: "#89dceb",
        sapphire: "#74c7ec",
        blue: "#89b4fa",
        lavender: "#b4befe",
    };

    /// Apply this theme to the document root element
    pub fn apply_to_document(&self) {
        use wasm_bindgen::JsCast;

        let document = web_sys::window()
            .and_then(|w| w.document())
            .expect("should have document");

        let root = document
            .document_element()
            .expect("should have root element");

        let style = root
            .dyn_ref::<web_sys::HtmlElement>()
            .expect("root should be HtmlElement")
            .style();

        // Apply all CSS variables
        let _ = style.set_property("--ctp-base", self.base);
        let _ = style.set_property("--ctp-mantle", self.mantle);
        let _ = style.set_property("--ctp-crust", self.crust);
        let _ = style.set_property("--ctp-text", self.text);
        let _ = style.set_property("--ctp-subtext1", self.subtext1);
        let _ = style.set_property("--ctp-subtext0", self.subtext0);
        let _ = style.set_property("--ctp-overlay0", self.overlay0);
        let _ = style.set_property("--ctp-overlay1", self.overlay1);
        let _ = style.set_property("--ctp-overlay2", self.overlay2);
        let _ = style.set_property("--ctp-surface0", self.surface0);
        let _ = style.set_property("--ctp-surface1", self.surface1);
        let _ = style.set_property("--ctp-surface2", self.surface2);
        let _ = style.set_property("--ctp-rosewater", self.rosewater);
        let _ = style.set_property("--ctp-flamingo", self.flamingo);
        let _ = style.set_property("--ctp-pink", self.pink);
        let _ = style.set_property("--ctp-mauve", self.mauve);
        let _ = style.set_property("--ctp-red", self.red);
        let _ = style.set_property("--ctp-maroon", self.maroon);
        let _ = style.set_property("--ctp-peach", self.peach);
        let _ = style.set_property("--ctp-yellow", self.yellow);
        let _ = style.set_property("--ctp-green", self.green);
        let _ = style.set_property("--ctp-teal", self.teal);
        let _ = style.set_property("--ctp-sky", self.sky);
        let _ = style.set_property("--ctp-sapphire", self.sapphire);
        let _ = style.set_property("--ctp-blue", self.blue);
        let _ = style.set_property("--ctp-lavender", self.lavender);
    }
}

const THEME_STORAGE_KEY: &str = "catppuccin-theme";

/// Load theme from localStorage
pub fn load_theme_from_storage() -> CatppuccinTheme {
    use gloo_storage::{LocalStorage, Storage};

    LocalStorage::get(THEME_STORAGE_KEY).unwrap_or_default()
}

/// Save theme to localStorage
pub fn save_theme_to_storage(theme: CatppuccinTheme) {
    use gloo_storage::{LocalStorage, Storage};

    let _ = LocalStorage::set(THEME_STORAGE_KEY, theme);
}

/// Apply theme to document and update data-theme attribute
pub fn apply_theme(theme: CatppuccinTheme) {
    // Apply colors to CSS variables
    theme.colors().apply_to_document();

    // Update data-theme attribute on root element
    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
        if let Some(root) = document.document_element() {
            let _ = root.set_attribute("data-theme", theme.as_str());
        }
    }

    // Save to localStorage
    save_theme_to_storage(theme);
}
