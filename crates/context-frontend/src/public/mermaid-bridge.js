/**
 * Mermaid diagram rendering bridge.
 * Loaded after mermaid.min.js. Provides global functions called from Rust via wasm-bindgen inline_js.
 */

// Track rendered diagrams for re-rendering on theme change
const __MERMAID_DIAGRAMS__ = new Map();

// Stack of currently-open expand overlays, oldest first. The last entry is the
// visible (topmost) overlay, so nested overlays close in order.
const __MERMAID_OVERLAYS__ = [];

/**
 * Initialize Mermaid with current Catppuccin CSS variable colors.
 * @param {boolean} isLight - true for Latte, false for Mocha/dark themes
 */
window.mermaid_init_for_theme = function(isLight) {
  if (!window.mermaid) return;

  // When forcing light mode (for printing), use hardcoded Latte values
  // instead of reading from getComputedStyle (which still has dark values
  // because the @media print CSS hasn't taken effect yet)
  if (isLight) {
    window.mermaid.initialize({
      startOnLoad: false,
      theme: 'base',
      themeVariables: {
        background: '#eff1f5',
        primaryColor: '#bcc0cc',
        primaryBorderColor: '#9ca0b0',
        primaryTextColor: '#4c4f69',
        lineColor: '#9ca0b0',
        secondaryColor: '#e6e9ef',
        tertiaryColor: '#dce0e8',
        edgeLabelBackground: '#bcc0cc',
        clusterBkg: '#bcc0cc',
        clusterBorder: '#9ca0b0',
      }
    });
    return;
  }

  // Dark mode — read from computed styles (normal behavior)
  const rootStyles = getComputedStyle(document.documentElement);
  const ctp = (name) => rootStyles.getPropertyValue('--ctp-' + name).trim() || '';

  const fallbacks = { surface0: '#1e1e2e', surface1: '#45475a', overlay0: '#6c7086', text: '#cdd6f4', mantle: '#181825', crust: '#11111b' };

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

      // Add per-diagram zoom controls (Zoom in, Zoom out, Fill)
      mdBuildControls(wrapper);

      preEl.replaceWith(wrapper);
    } catch (e) {
      const errSpan = document.createElement('span');      errSpan.style.cssText = 'font-size:0.75em;font-weight:bold;';
      codeEl.prepend(errSpan);
    }
  }
};

/**
 * Zoom controls for mermaid diagrams.
 *
 * Zoom scales the rendered svg with CSS transform: scale(). The scale is
 * stored per diagram on the .mermaid-diagram div (dataset.scale). The
 * data-zoom attribute gates the transform origin (center) in CSS. Fill
 * (fit) uses fit-to-page semantics: the whole svg viewBox becomes visible
 * (small diagrams may upscale above 1), with the resulting scale clamped to
 * [ZOOM_MIN, ZOOM_MAX].
 */
const ZOOM_STEP = 1.25;
const ZOOM_MIN = 0.1;
const ZOOM_MAX = 8;

/**
 * Read the current zoom scale stored on a .mermaid-diagram div.
 * @param {HTMLElement} diagram
 * @returns {number}
 */
function mdGetScale(diagram) {
  const stored = parseFloat(diagram.dataset.scale);
  return Number.isFinite(stored) && stored > 0 ? stored : 1;
}

/**
 * Apply a scale to a diagram's svg and record it on the diagram div.
 * @param {HTMLElement} diagram - the .mermaid-diagram div
 * @param {number} scale - the scale to apply
 */
function mdApplyScale(diagram, scale) {
  diagram.dataset.scale = String(scale);
  if (scale === 1) {
    delete diagram.dataset.zoom;
  } else {
    diagram.dataset.zoom = 'true';
  }
  const svg = diagram.querySelector('svg');
  if (svg) {
    const offX = parseFloat(diagram.dataset.offX) || 0;
    const offY = parseFloat(diagram.dataset.offY) || 0;
    svg.style.transform = 'translate(' + offX + 'px, ' + offY + 'px) scale(' + scale + ')';
  }
}

/**
 * Zoom a diagram in by the step factor (clamped to the max scale).
 * @param {HTMLElement} diagram
 */
window.mdZoomIn = function(diagram) {
  if (!diagram) return;
  const svg = diagram.querySelector('svg');
  if (!svg) return;
  mdApplyScale(diagram, Math.min(mdGetScale(diagram) * ZOOM_STEP, ZOOM_MAX));
};

/**
 * Zoom a diagram out by the step factor (clamped to the min scale).
 * @param {HTMLElement} diagram
 */
window.mdZoomOut = function(diagram) {
  if (!diagram) return;
  const svg = diagram.querySelector('svg');
  if (!svg) return;
  mdApplyScale(diagram, Math.max(mdGetScale(diagram) / ZOOM_STEP, ZOOM_MIN));
};

/**
 * Fit a diagram to the container client area (fit-to-page: the whole viewBox
 * becomes visible; small diagrams may upscale above 1). The resulting scale is
 * clamped to [ZOOM_MIN, ZOOM_MAX], pan offsets are reset, and scroll returns 0/0.
 * @param {HTMLElement} diagram
 * @returns {boolean} true when a scale was applied
 */
window.mdFit = function(diagram) {
  if (!diagram) return false;
  const svg = diagram.querySelector('svg');
  if (!svg) return false;
  if (!diagram.clientWidth || !diagram.clientHeight) return false;

  let vbW = 0;
  let vbH = 0;
  if (svg.viewBox && svg.viewBox.baseVal) {
    vbW = svg.viewBox.baseVal.width;
    vbH = svg.viewBox.baseVal.height;
  } else {
    const parts = (svg.getAttribute('viewBox') || '').split(' ');
    vbW = parseFloat(parts[2] || 0);
    vbH = parseFloat(parts[3] || 0);
  }
  if (!vbW || !vbH) return false;

  const scale = Math.min(
    Math.max(Math.min(diagram.clientWidth / vbW, diagram.clientHeight / vbH), ZOOM_MIN),
    ZOOM_MAX
  );
  diagram.dataset.offX = '0';
  diagram.dataset.offY = '0';
  mdApplyScale(diagram, scale);
  diagram.scrollLeft = 0;
  diagram.scrollTop = 0;
  return true;
};

function mdEnablePan(diagram) {
  const svg = diagram.querySelector('svg');
  if (!svg) return;

  let dragging = false;
  let startX = 0;
  let startY = 0;
  let startOffX = 0;
  let startOffY = 0;

  const onMove = function(e) {
    if (!dragging) return;
    const s = mdGetScale(diagram);
    const offX = startOffX + (e.clientX - startX) / s;
    const offY = startOffY + (e.clientY - startY) / s;
    diagram.dataset.offX = String(offX);
    diagram.dataset.offY = String(offY);
    mdApplyScale(diagram, s);
  };

  const onUp = function() {
    dragging = false;
    document.removeEventListener('mousemove', onMove);
    window.removeEventListener('mouseup', onUp);
  };

  svg.addEventListener('mousedown', function(e) {
    if (mdGetScale(diagram) <= 1) return;
    dragging = true;
    startX = e.clientX;
    startY = e.clientY;
    startOffX = parseFloat(diagram.dataset.offX) || 0;
    startOffY = parseFloat(diagram.dataset.offY) || 0;
    e.preventDefault();
    document.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  });
}

/**
 * Reset a diagram's zoom to scale 1.0 (used after a theme re-render).
 * @param {HTMLElement} diagram
 */
window.mdResetZoom = function(diagram) {
  if (!diagram) return;
  diagram.dataset.offX = '0';
  diagram.dataset.offY = '0';
  mdApplyScale(diagram, 1);
  diagram.scrollLeft = 0;
  diagram.scrollTop = 0;
};

const MD_SVG =
  '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">';

const MD_ICON_ZOOM_IN =
  MD_SVG +
  '<circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/><line x1="11" y1="8" x2="11" y2="14"/><line x1="8" y1="11" x2="14" y2="11"/></svg>';

const MD_ICON_ZOOM_OUT =
  MD_SVG +
  '<circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/><line x1="8" y1="11" x2="14" y2="11"/></svg>';

const MD_ICON_FIT =
  MD_SVG +
  '<path d="M8 3H5a2 2 0 0 0-2 2v3"/><path d="M21 8V5a2 2 0 0 0-2-2h-3"/><path d="M3 16v3a2 2 0 0 0 2 2h3"/><path d="M16 21h3a2 2 0 0 0 2-2v-3"/></svg>';

/**
 * Build a per-diagram control strip (Zoom in, Zoom out, Fill) and insert it
 * over the top-left corner of the .mermaid-wrapper frame. CSS positions the
 * strip with position:absolute so it overlays the diagram without changing
 * layout. Each button carries a visible inline <svg> glyph.
 * @param {HTMLElement} wrapper - the .mermaid-wrapper div
 */
const MD_ICON_EXPAND =
  MD_SVG +
  '<polyline points="15 3 21 3 21 9"/>' +
  '<polyline points="9 21 3 21 3 15"/>' +
  '<line x1="21" y1="3" x2="14" y2="10"/>' +
  '<line x1="3" y1="21" x2="10" y2="14"/>' +
  '</svg>';

/**
 * Build a per-diagram zoom control strip (Zoom in, Zoom out, Fill) and insert
 * it before a diagram inside a container. Shared by the in-note wrapper and the
 * expand overlay so both use the same controls and zoom functions.
 * @param {HTMLElement} container - element to insert the strip into
 * @param {HTMLElement} diagram - the .mermaid-diagram div the controls act on
 */
function mdAppendZoomStrip(container, diagram) {
  if (!container || !diagram) return;

  const strip = document.createElement('div');
  strip.className = 'mermaid-controls';

  const makeButton = (ariaLabel, title, icon, onClick) => {
    const btn = document.createElement('button');
    btn.className =
      'mermaid-control-btn bg-ctp-surface1 text-ctp-overlay0 border border-ctp-surface2 rounded-md cursor-pointer hover:bg-ctp-overlay0 hover:text-ctp-base active:scale-95';
    btn.setAttribute('aria-label', ariaLabel);
    btn.title = title;
    btn.innerHTML = icon;
    btn.addEventListener('click', onClick);
    return btn;
  };

  strip.appendChild(
    makeButton('Zoom in', 'Zoom in', MD_ICON_ZOOM_IN, () => window.mdZoomIn(diagram))
  );
  strip.appendChild(
    makeButton('Zoom out', 'Zoom out', MD_ICON_ZOOM_OUT, () => window.mdZoomOut(diagram))
  );
  strip.appendChild(
    makeButton('Fill', 'Fill', MD_ICON_FIT, () => window.mdFit(diagram))
  );

  container.insertBefore(strip, diagram);
}

/**
 * Open a mermaid diagram in a near-fullscreen overlay, re-rendered from its
 * stored source so theme and zoom state stay clean. Reuses the in-note zoom
 * functions, so Zoom in / Zoom out / Fill work identically inside the overlay.
 * Nested overlays stack and close in order via __MERMAID_OVERLAYS__.
 * @param {HTMLElement} diagram - the .mermaid-diagram div in the note
 */
window.mdOpenDiagram = function(diagram) {
  if (!diagram) return;
  const entry = __MERMAID_DIAGRAMS__.get(diagram);
  if (!entry) return;
  const { source, id } = entry;
  if (!window.mermaid) return;

  // Hide any overlays already on the stack so this one is the only visible one.
  for (const record of __MERMAID_OVERLAYS__) {
    record.overlay.style.display = 'none';
  }

  // Scrim (fixed, full viewport, dark).
  const overlay = document.createElement('div');
  overlay.className = 'mermaid-overlay';

  // Centered popup panel (Option B: fills most of the window, with a margin).
  const panel = document.createElement('div');
  panel.className = 'mermaid-overlay-panel';

  // Top bar: title on the left, close button on the right.
  const bar = document.createElement('div');
  bar.className = 'mermaid-overlay-bar';

  const title = document.createElement('span');
  title.className = 'mermaid-overlay-title';
  title.textContent = 'Expand diagram';

  const closeBtn = document.createElement('button');
  closeBtn.className =
    'mermaid-overlay-close bg-ctp-surface1 text-ctp-overlay0 border border-ctp-surface2 rounded-md cursor-pointer hover:bg-ctp-overlay0 hover:text-ctp-base active:scale-95';
  closeBtn.setAttribute('aria-label', 'Close overlay');
  closeBtn.title = 'Close overlay';
  closeBtn.innerHTML =
    MD_SVG + '<line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>';
  closeBtn.addEventListener('click', () => mdCloseDiagram(overlay));

  bar.appendChild(title);
  bar.appendChild(closeBtn);

  // Scrollable body hosting a fresh, re-rendered diagram.
  const body = document.createElement('div');
  body.className = 'mermaid-overlay-body';
  const fullDiagram = document.createElement('div');
  fullDiagram.className = 'mermaid-diagram mermaid-diagram--fullscreen';
  body.appendChild(fullDiagram);

  panel.appendChild(bar);
  panel.appendChild(body);
  overlay.appendChild(panel);
  // Close handlers: Escape (only when this overlay is the top one) and clicking
  // the scrim directly (clicks inside the panel are ignored).
  const onKeydown = function(e) {
    const top = __MERMAID_OVERLAYS__[__MERMAID_OVERLAYS__.length - 1];
    if (e.key === 'Escape' && top && top.overlay === overlay) {
      mdCloseDiagram(overlay);
    }
  };
  const onClick = function(e) {
    if (e.target === overlay) {
      mdCloseDiagram(overlay);
    }
  };
  document.addEventListener('keydown', onKeydown);
  overlay.addEventListener('click', onClick);

  // Register on the stack and mount BEFORE the async render, so the overlay can
  // always be closed while the render is pending and even if it fails.
  __MERMAID_OVERLAYS__.push({ overlay, onKeydown, onClick });
  document.body.appendChild(overlay);

  // Reuse the same zoom controls/functions as the in-note diagram.
  mdAppendZoomStrip(body, fullDiagram);

  // Render the diagram from the stored source (kept separate from the live svg).
  // mermaid v11 render() is async: it returns a Promise resolving to { svg }.
  window.mermaid.render(id + '-fs', source)
    .then((result) => {
      fullDiagram.innerHTML = result.svg;
      // Fit once on open. NO auto-refit on window resize: that would clobber a
      // user's zoom state; the Fill button re-fits on demand.
      if (!window.mdFit(fullDiagram)) {
        requestAnimationFrame(function() { window.mdFit(fullDiagram); });
      }
    })
    .catch((e) => {
      console.error('Mermaid overlay render error:', e);
    });
}

/**
 * Close an expand overlay: remove it from the DOM, drop it from the stack, and
 * make the previous overlay visible again if any remain.
 * @param {HTMLElement} overlay
 */
window.mdCloseDiagram = function(overlay) {
  if (!overlay) return;
  const idx = __MERMAID_OVERLAYS__.findIndex((r) => r.overlay === overlay);
  if (idx === -1) return;

  const record = __MERMAID_OVERLAYS__.splice(idx, 1)[0];
  if (record.onKeydown) document.removeEventListener('keydown', record.onKeydown);
  if (record.onClick) overlay.removeEventListener('click', record.onClick);
  if (overlay.parentNode) overlay.parentNode.removeChild(overlay);

  // Reveal the previous overlay, if any remains.
  const prev = __MERMAID_OVERLAYS__[__MERMAID_OVERLAYS__.length - 1];
  if (prev) prev.overlay.style.display = '';
}

function mdBuildControls(wrapper) {
  const diagram = wrapper.querySelector('.mermaid-diagram');
  if (!diagram) return;

  const strip = document.createElement('div');
  strip.className = 'mermaid-controls';

  const makeButton = (ariaLabel, title, icon, onClick) => {
    const btn = document.createElement('button');
    btn.className =
      'mermaid-control-btn bg-ctp-surface1 text-ctp-overlay0 border border-ctp-surface2 rounded-md cursor-pointer hover:bg-ctp-overlay0 hover:text-ctp-base active:scale-95';
    btn.setAttribute('aria-label', ariaLabel);
    btn.title = title;
    btn.innerHTML = icon;
    btn.addEventListener('click', onClick);
    return btn;
  };

  strip.appendChild(
    makeButton('Zoom in', 'Zoom in', MD_ICON_ZOOM_IN, () => window.mdZoomIn(diagram))
  );
  strip.appendChild(
    makeButton('Zoom out', 'Zoom out', MD_ICON_ZOOM_OUT, () => window.mdZoomOut(diagram))
  );
  strip.appendChild(
    makeButton('Fill', 'Fill', MD_ICON_FIT, () => window.mdFit(diagram))
  );
  strip.appendChild(
    makeButton('Open', 'Expand diagram', MD_ICON_EXPAND, () => window.mdOpenDiagram(diagram))
  );

  wrapper.insertBefore(strip, diagram);
  mdEnablePan(diagram);
}

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
      window.mdResetZoom(div);
      mdEnablePan(div);
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

/**
 * Re-render all mermaid diagrams with light theme, then print, then restore.
 * Called from the print button via eval — must be fully self-contained.
 * NOT async — uses .then() chains so eval() doesn't need to await the Promise.
 */
window.printWithLightMermaid = function() {
  if (!window.mermaid || __MERMAID_DIAGRAMS__.size === 0) {
    // No mermaid diagrams — just print directly
    window.print();
    return;
  }

  // Save current theme
  const wasLight = document.documentElement.getAttribute('data-theme') === 'latte';

  // Initialize with light theme (Latte colors)
  window.mermaid_init_for_theme(true);

  // Re-render all diagrams with light colors, THEN print
  var promises = [];
  for (const [div, { source, id }] of __MERMAID_DIAGRAMS__) {
    var p = window.mermaid.render(id + '-print', source).then(function(result) {
      div.innerHTML = result.svg;
    }).catch(function(e) {
      console.error('Mermaid print re-render error:', e);
    });
    promises.push(p);
  }

  // Wait for ALL diagrams to re-render, THEN call window.print()
  Promise.all(promises).then(function() {
    // Small delay to let DOM update with new SVGs
    requestAnimationFrame(function() {
      window.print();
      // Restore original theme after print dialog
      var restorePromises = [];
      window.mermaid_init_for_theme(wasLight);
      for (const [div, { source, id }] of __MERMAID_DIAGRAMS__) {
        var rp = window.mermaid.render(id + '-restore', source).then(function(result) {
          div.innerHTML = result.svg;
        }).catch(function(e) {
          console.error('Mermaid restore error:', e);
        });
        restorePromises.push(rp);
      }
    });
  });
};
