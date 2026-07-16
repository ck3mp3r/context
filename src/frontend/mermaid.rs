use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
export function mermaid_init_for_theme(isLight) {
  if (typeof window.mermaid_init_for_theme === 'function') {
    window.mermaid_init_for_theme(isLight);
  }
}

export function mermaid_render_in_selector(selector) {
  if (typeof window.mermaid_render_in_selector === 'function') {
    window.mermaid_render_in_selector(selector);
  }
}

export function mermaid_rerender_all() {
  if (typeof window.mermaid_rerender_all === 'function') {
    window.mermaid_rerender_all();
  }
}

export function highlight_code_blocks(selector) {
  if (typeof window.highlight_code_blocks === 'function') {
    window.highlight_code_blocks(selector);
  }
}
"#)]
extern "C" {
    /// Initialize Mermaid with Catppuccin theme colors for light/dark mode
    pub fn mermaid_init_for_theme(is_light: bool);

    /// Render all mermaid code blocks within a CSS selector scope
    pub fn mermaid_render_in_selector(selector: &str);

    /// Re-render ALL previously rendered diagrams with current theme
    pub fn mermaid_rerender_all();

    /// Highlight code blocks using highlight.js
    pub fn highlight_code_blocks(selector: &str);
}
