/**
 * Mermaid diagram rendering bridge.
 * Loaded after mermaid.min.js. Provides global functions called from Rust via wasm-bindgen inline_js.
 */

// Track rendered diagrams for re-rendering on theme change
const __MERMAID_DIAGRAMS__ = new Map();

/**
 * Initialize Mermaid with current Catppuccin CSS variable colors.
 * @param {boolean} isLight - true for Latte, false for Mocha/dark themes
 */
window.mermaid_init_for_theme = function(isLight) {
  if (!window.mermaid) return;

  const rootStyles = getComputedStyle(document.documentElement);
  const ctp = (name) => rootStyles.getPropertyValue('--ctp-' + name).trim() || '';

  // Fallbacks if CSS vars not yet applied
  const fallbacks = isLight
    ? { surface0: '#eff1f5', surface1: '#bcc0cc', overlay0: '#9ca0b0', text: '#4c4f69', mantle: '#e6e9ef', crust: '#dce0e8' }
    : { surface0: '#1e1e2e', surface1: '#45475a', overlay0: '#6c7086', text: '#cdd6f4', mantle: '#181825', crust: '#11111b' };

  window.mermaid.initialize({
    startOnLoad: false,
    theme: 'base',
    themeVariables: {
      background: ctp('surface0') || fallbacks.surface0,
      primaryColor: ctp('surface1') || fallbacks.surface1,
      primaryBorderColor: ctp('overlay0') || fallbacks.overlay0,
      primaryTextColor: ctp('text') || fallbacks.text,
      lineColor: ctp('overlay0') || fallbacks.overlay0,
      secondaryColor: ctp('mantle') || fallbacks.mantle,
      tertiaryColor: ctp('crust') || fallbacks.crust,
      edgeLabelBackground: ctp('surface1') || fallbacks.surface1,
      clusterBkg: ctp('surface1') || fallbacks.surface1,
      clusterBorder: ctp('overlay0') || fallbacks.overlay0,
    }
  });
};

/**
 * Render all <pre><code class="language-mermaid"> blocks within a DOM selector.
 * Replaces each with a <div class="mermaid-diagram"> containing SVG.
 * Stores source in __MERMAID_DIAGRAMS__ for future re-rendering.
 *
 * @param {string|null} selector - CSS selector string or null for document-wide
 */
window.mermaid_render_in_selector = async function(selector) {
  if (!window.mermaid) return;

  const root = selector ? document.querySelector(selector) : document;
  if (!root) return;

  const blocks = root.querySelectorAll('pre > code.language-mermaid');

  for (const codeEl of blocks) {
    const preEl = codeEl.parentElement;
    if (!preEl) continue;

    // Skip if already rendered (guard against double-call)
    if (preEl.querySelector(':scope > .mermaid-diagram')) continue;

    const source = codeEl.textContent.trim();
    if (!source) continue;

    try {
      const uniqueId = 'm-' + Date.now() + '-' + Math.random().toString(36).slice(2, 11);
      const { svg } = await window.mermaid.render(uniqueId, source);

      const div = document.createElement('div');
      div.className = 'mermaid-diagram';
      div.innerHTML = svg;

      // Store for theme re-render
      __MERMAID_DIAGRAMS__.set(div, { source, id: uniqueId });

      // Create wrapper with copy button
      const wrapper = document.createElement('div');
      wrapper.className = 'mermaid-wrapper';

      const btn = document.createElement('button');
      btn.className = 'mermaid-copy-btn';
      btn.setAttribute('aria-label', 'Copy diagram source');
      btn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path></svg>';
      btn.title = 'Copy diagram source';

      btn.addEventListener('click', async function() {
        try {
          await navigator.clipboard.writeText(source);
          btn.title = 'Copied!';
          const orig = btn.innerHTML;
          btn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"></polyline></svg>';
          setTimeout(function() {
            btn.innerHTML = orig;
            btn.title = 'Copy diagram source';
          }, 1500);
        } catch(e) {
          console.error('Clipboard copy failed:', e);
        }
      });

      wrapper.appendChild(btn);
      wrapper.appendChild(div);
      preEl.replaceWith(wrapper);
    } catch (e) {
      const errSpan = document.createElement('span');      errSpan.style.cssText = 'font-size:0.75em;font-weight:bold;';
      codeEl.prepend(errSpan);
    }
  }
};

/**
 * Re-render ALL previously rendered mermaid diagrams with current theme colors.
 * Call this when the user toggles light/dark mode.
 */
window.mermaid_rerender_all = async function() {
  if (!window.mermaid || __MERMAID_DIAGRAMS__.size === 0) return;

  // Re-initialize with current theme
  const isLight = document.documentElement.getAttribute('data-theme') === 'latte';
  window.mermaid_init_for_theme(isLight);

  // Small delay to let mermaid.initialize settle
  await new Promise(r => setTimeout(r, 50));

  for (const [div, { source, id }] of __MERMAID_DIAGRAMS__) {
    try {
      const { svg } = await window.mermaid.render(id, source);
      div.innerHTML = svg;
    } catch (e) {
      console.error('Mermaid re-render error:', e);
    }
  }
};

/**
 * Highlight code blocks using highlight.js within a CSS selector scope.
 * Skips mermaid blocks and already-highlighted blocks.
 * @param {string|null} selector - CSS selector string or null for document-wide
 */
/**
 * Highlight code blocks using highlight.js within a CSS selector scope.
 * Skips mermaid blocks and already-highlighted blocks.
 * Wraps each <pre> in a .code-block-wrapper with a copy button.
 * @param {string|null} selector - CSS selector string or null for document-wide
 */
window.highlight_code_blocks = function(selector) {
  if (typeof hljs === 'undefined') return;

  const root = selector ? document.querySelector(selector) : document;
  if (!root) return;

  const pres = root.querySelectorAll('pre > code');
  for (const el of pres) {
    // Skip mermaid blocks — handled by mermaid_render_in_selector
    if (el.classList.contains('language-mermaid')) continue;

    const preEl = el.parentElement;
    if (!preEl) continue;

    // Skip already highlighted
    if (el.dataset.highlighted) {
      // Still add wrapper/button if not present
      if (!preEl.closest('.code-block-wrapper')) {
        _wrapCodeBlock(preEl, el.textContent);
      }
      continue;
    }

    try {
      hljs.highlightElement(el);
    } catch (e) {
      console.error('Highlight.js error:', e);
    }

    // Wrap in container with copy button (idempotent)
    if (!preEl.closest('.code-block-wrapper')) {
      _wrapCodeBlock(preEl, el.textContent);
    }
  }
};

/**
 * Internal: wrap a <pre> element with a copy-button container.
 */
function _wrapCodeBlock(preEl, sourceText) {
  if (preEl.closest('.code-block-wrapper')) return;

  const parent = preEl.parentNode;
  if (!parent) return;

  const wrapper = document.createElement('div');
  wrapper.className = 'code-block-wrapper';

  const btn = document.createElement('button');
  btn.className = 'code-copy-btn';
  btn.setAttribute('aria-label', 'Copy code');
  btn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path></svg>';
  btn.title = 'Copy code';

  btn.addEventListener('click', async function() {
    try {
      await navigator.clipboard.writeText(sourceText);
      btn.title = 'Copied!';
      const orig = btn.innerHTML;
      btn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"></polyline></svg>';
      setTimeout(function() {
        btn.innerHTML = orig;
        btn.title = 'Copy code';
      }, 1500);
    } catch(e) {
      console.error('Clipboard copy failed:', e);
    }
  });

  wrapper.appendChild(btn);
  parent.replaceChild(wrapper, preEl);
  wrapper.appendChild(preEl);
}