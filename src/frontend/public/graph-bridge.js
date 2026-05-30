// Graph Bridge: Sigma.js + Graphology integration for c5t
// Called from Rust/WASM via wasm-bindgen
//
// Performance Optimizations:
// - Dynamic iteration scaling: FA2 iterations scale based on graph size (300/200/100 for small/medium/large)
// - Loading indicators: Visual pulse on nodes during expand operations
// - Auto-tuned settings: Uses ForceAtlas2Layout.inferSettings() for optimal parameters
//
// Web Worker Investigation (2026-05-27):
// The vendored graphology-layout-forceatlas2.min.js (v0.10.1) does NOT include
// Web Worker support. The library exports only ForceAtlas2Layout and ForceAtlas2Layout.inferSettings.
// Worker mode (graphology-layout-forceatlas2/worker) requires separate bundle and is not currently
// available in the vendored file. To enable worker mode in the future:
// 1. Bundle graphology-layout-forceatlas2/worker.js separately
// 2. Use postMessage API to offload layout computation to worker thread
// 3. Update animateLayout() to handle async layout computation
// For now, synchronous layout with scaled iterations provides acceptable performance.

(function() {
  "use strict";

  // Store active graph instances by container ID
  var instances = {};

  /**
   * Calculate optimal ForceAtlas2 iterations based on graph size.
   * Scales down iterations for larger graphs to maintain performance.
   * @param {number} nodeCount - Number of nodes in the graph
   * @returns {number} Recommended number of iterations
   */
  function getLayoutIterations(nodeCount) {
    if (nodeCount < 50) return 300;
    if (nodeCount < 200) return 200;
    return 100;
  }

  /**
   * Build enhanced ForceAtlas2 settings with better node spacing.
   * Overrides inferSettings() defaults to prevent nodes clumping together.
   * @param {object} graph - Graphology graph instance
   * @returns {object} Enhanced FA2 settings
   */
  function getLayoutSettings(graph) {
    var settings = ForceAtlas2Layout.inferSettings
      ? ForceAtlas2Layout.inferSettings(graph)
      : { barnesHutOptimize: true };
    var n = graph.order;
    settings.scalingRatio = Math.max(settings.scalingRatio || 1, 3 + Math.log10(Math.max(n, 1)));
    settings.gravity = 0.8;
    settings.adjustSizes = true;
    if (n > 100) {
      settings.barnesHutOptimize = true;
      settings.barnesHutTheta = 0.5;
    }
    return settings;
  }

  // Animate layout transitions from current positions to target positions
  function animateLayout(inst, targetPositions, duration) {
    duration = duration || 400;
    var startTime = null;
    var startPositions = {};
    inst.graph.forEachNode(function(node) {
      startPositions[node] = {
        x: inst.graph.getNodeAttribute(node, 'x'),
        y: inst.graph.getNodeAttribute(node, 'y')
      };
    });
    function step(timestamp) {
      if (!startTime) startTime = timestamp;
      var progress = Math.min((timestamp - startTime) / duration, 1);
      var t = 1 - Math.pow(1 - progress, 3); // ease-out cubic
      inst.graph.forEachNode(function(node) {
        if (targetPositions[node]) {
          var s = startPositions[node];
          inst.graph.setNodeAttribute(node, 'x', s.x + (targetPositions[node].x - s.x) * t);
          inst.graph.setNodeAttribute(node, 'y', s.y + (targetPositions[node].y - s.y) * t);
        }
      });
      inst.renderer.refresh();
      if (progress < 1) requestAnimationFrame(step);
    }
    requestAnimationFrame(step);
  }

  /**
   * Build ancestor chain for a node (bottom-up: node → parent → grandparent → root).
   * Returns array of {id, label, kind} objects.
   * @param {object} graph - Graphology graph instance
   * @param {string} nodeId - Starting node ID
   * @returns {Array} Array of ancestor objects
   */
  function getAncestorChain(graph, nodeId) {
    var chain = [];
    var current = nodeId;
    var visited = new Set();

    while (current && graph.hasNode(current)) {
      // Prevent infinite loops
      if (visited.has(current)) break;
      visited.add(current);

      var attrs = graph.getNodeAttributes(current);
      chain.push({
        id: current,
        label: attrs.label || current,
        kind: attrs.kind || "unknown"
      });

      current = attrs.parentId;
      if (!current) break;
    }

    return chain;
  }

  /**
   * Layout nodes in a top-down tree structure based on parentId relationships.
   * Roots at top, children below, maintaining hierarchy visibility.
   * @param {object} graph - Graphology graph instance
   */
  function treeLayout(graph) {
    var hSpacing = 180;
    var vSpacing = 120;

    // Build parent→children map from node attributes
    var roots = [];
    var childrenOf = {};

    graph.forEachNode(function(nodeId, attrs) {
      var pid = attrs.parentId;
      if (!pid || !graph.hasNode(pid)) {
        roots.push(nodeId);
      } else {
        if (!childrenOf[pid]) childrenOf[pid] = [];
        childrenOf[pid].push(nodeId);
      }
    });

    // Measure subtree leaf count (determines width)
    function leafCount(id) {
      var kids = childrenOf[id];
      if (!kids || kids.length === 0) return 1;
      var total = 0;
      for (var i = 0; i < kids.length; i++) total += leafCount(kids[i]);
      return total;
    }

    // Position subtree recursively, returns width consumed
    function positionSubtree(id, left, depth) {
      var kids = childrenOf[id];
      if (!kids || kids.length === 0) {
        graph.setNodeAttribute(id, 'x', left);
        graph.setNodeAttribute(id, 'y', depth * vSpacing);
        return hSpacing;
      }
      var cursor = left;
      for (var i = 0; i < kids.length; i++) {
        cursor += positionSubtree(kids[i], cursor, depth + 1);
      }
      var firstX = graph.getNodeAttribute(kids[0], 'x');
      var lastX = graph.getNodeAttribute(kids[kids.length - 1], 'x');
      graph.setNodeAttribute(id, 'x', (firstX + lastX) / 2);
      graph.setNodeAttribute(id, 'y', depth * vSpacing);
      return cursor - left;
    }

    var cursor = 0;
    for (var i = 0; i < roots.length; i++) {
      cursor += positionSubtree(roots[i], cursor, 0);
      cursor += hSpacing; // gap between trees
    }
  }

  // Helper to read CSS variable from :root
  function getCssVar(name, fallback) {
    var value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    return value || fallback;
  }

  // Helper to desaturate a color for a muted effect (works for both light and dark themes)
  // factor: 0 = full color, 1 = fully desaturated (gray)
  function muteColor(hexColor, factor) {
    // Parse hex color
    var r = parseInt(hexColor.slice(1, 3), 16);
    var g = parseInt(hexColor.slice(3, 5), 16);
    var b = parseInt(hexColor.slice(5, 7), 16);

    // Calculate luminance (grayscale value)
    var gray = Math.round(0.299 * r + 0.587 * g + 0.114 * b);

    // Mix toward gray (desaturate) - preserves brightness, reduces vibrance
    r = Math.round(r * (1 - factor) + gray * factor);
    g = Math.round(g * (1 - factor) + gray * factor);
    b = Math.round(b * (1 - factor) + gray * factor);

    return "#" + ((1 << 24) + (r << 16) + (g << 8) + b).toString(16).slice(1);
  }

  // Kind-to-CSS-variable mapping
  var kindColorVars = {
    "function": "--ctp-blue",
    "method":   "--ctp-blue",
    "command":  "--ctp-blue",
    "struct":   "--ctp-green",
    "enum":     "--ctp-yellow",
    "trait":    "--ctp-mauve",
    "interface":"--ctp-mauve",
    "module":   "--ctp-peach",
    "mod":      "--ctp-peach",
    "constant": "--ctp-flamingo",
    "const":    "--ctp-flamingo",
    "static":   "--ctp-red",
    "var":      "--ctp-red",
    "type_alias":"--ctp-teal",
    "type":     "--ctp-teal",
    "macro":    "--ctp-pink",
    "alias":    "--ctp-maroon",
    "extern":   "--ctp-sapphire",
    "class":    "--ctp-green",
    "object":   "--ctp-teal",
    "property": "--ctp-flamingo",
    "extension_function": "--ctp-sapphire",
    "field":    "--ctp-flamingo",
    "interface_method": "--ctp-blue",
    "enum_entry": "--ctp-yellow",
    "test":     "--ctp-yellow",
    "package":  "--ctp-peach",
  };

  // Entry type border color CSS variables
  var entryTypeBorderVars = {
    "main":      "--ctp-green",
    "test":      "--ctp-yellow",
    "export":    "--ctp-sapphire",
    "init":      "--ctp-peach",
    "benchmark": "--ctp-mauve",
    "example":   "--ctp-teal",
  };

  // Edge type-to-CSS-variable mapping
  var edgeColorVars = {
    "Calls":          "--ctp-blue",
    "Uses":           "--ctp-yellow",
    "Returns":        "--ctp-green",
    "Accepts":        "--ctp-teal",
    "FieldType":      "--ctp-pink",
    "TypeAnnotation": "--ctp-peach",
    "Inherits":       "--ctp-mauve",
    "Import":         "--ctp-sapphire",
    "Contains":       "--ctp-surface2",
    "HasField":       "--ctp-overlay0",
    "HasMethod":      "--ctp-overlay0",
    "HasMember":      "--ctp-overlay0",
    "Implements":     "--ctp-mauve",
    "Extends":        "--ctp-mauve",
    "FileImports":    "--ctp-sapphire",
    "aggregate":      "--ctp-overlay1",  // Neutral gray for aggregate edges
  };

  function kindColor(kind) {
    var varName = kindColorVars[kind] || "--ctp-subtext0";
    var color = getCssVar(varName, "#888888");
    // Mute the color by mixing 40% with background for softer appearance
    return muteColor(color, 0.4);
  }

  function entryTypeBorderColor(entryType) {
    var varName = entryTypeBorderVars[entryType];
    return varName ? getCssVar(varName, null) : null;
  }

  function edgeColor(edgeType) {
    var varName = edgeColorVars[edgeType] || "--ctp-surface2";
    var color = getCssVar(varName, "#585b70");
    // Aggregate edges (medium muted for visibility)
    if (edgeType === "aggregate") {
      return muteColor(color, 0.5);
    }
    // Containment edges (lighter)
    if (edgeType === "HasMember" || edgeType === "HasMethod" || edgeType === "HasField") {
      return muteColor(color, 0.3); // Less muted for visibility
    }
    // Other edges (more muted)
    return muteColor(color, 0.6);
  }

  /**
   * Calculate aggregate edge size and label from aggregate_counts.
   * @param {object} aggregateCounts - e.g. {"Calls": 3, "Implements": 1}
   * @returns {object} { size: number, label: string }
   */
  function calcAggregateEdgeProps(aggregateCounts) {
    if (!aggregateCounts || typeof aggregateCounts !== "object") {
      return { size: 1, label: "1" };
    }
    // Sum all counts
    var totalCount = 0;
    var entries = [];
    for (var type in aggregateCounts) {
      var count = aggregateCounts[type];
      totalCount += count;
      entries.push({ type: type, count: count });
    }
    // Scale size logarithmically, capped at 5
    var size = Math.min(1 + Math.log2(totalCount || 1), 5);
    // Build label: show total count
    var label = String(totalCount);
    return { size: size, label: label };
  }



  // Get current theme colors for UI elements
  function getThemeColors() {
    return {
      text: getCssVar("--ctp-text", "#cdd6f4"),
      surface0: getCssVar("--ctp-surface0", "#313244"),
      surface2: getCssVar("--ctp-surface2", "#585b70"),
      overlay0: getCssVar("--ctp-overlay0", "#6c7086"),
    };
  }

  /**
   * Initialize a graph in a container element.
   * @param {string} containerId - DOM element ID for the graph canvas
   * @param {string} graphDataJson - JSON string with { nodes, edges, stats }
   * @param {string} repoId - Repository ID for API calls
   * @param {string} apiBase - API base URL (e.g., "/dev/api/v1" or "/api/v1")
   * @returns {boolean} true if successful
   */
  window.initGraph = function(containerId, graphDataJson, repoId, apiBase) {
    // Clean up existing instance
    if (instances[containerId]) {
      instances[containerId].renderer.kill();
      delete instances[containerId];
    }

    var container = document.getElementById(containerId);
    if (!container) {
      console.error("Graph container not found:", containerId);
      return false;
    }

    try {
      var data = JSON.parse(graphDataJson);
      var graph = new graphology.Graph({ multi: true, type: "directed" });

      // Build entry type lookup (Sigma's draw functions may not receive custom attributes)
      var nodeEntryTypes = {};

      // Add nodes with random initial positions (layout done client-side)
      (data.nodes || []).forEach(function(node) {
        if (node.entry_type) {
          nodeEntryTypes[node.id] = node.entry_type;
        }
        var attrs = {
          label: node.label,
          qualifiedName: node.qualified_name || node.label,
          size: 3,
          color: kindColor(node.kind),
          kind: node.kind,
          language: node.language || "unknown",
          filePath: node.file_path,
          startLine: node.start_line,
          entryType: node.entry_type || null,
          isTest: node.is_test || false,
          childCount: node.child_count || 0,
          parentId: node.parent_id || null,
          x: (Math.random() - 0.5) * 100,
          y: (Math.random() - 0.5) * 100,
        };
        graph.addNode(node.id, attrs);
      });

      // Add edges with type-specific colors
      var edgeStats = { added: 0, skipped: 0, errors: {}, aggregate: 0 };
      (data.edges || []).forEach(function(edge) {
        try {
          var isAggregate = edge.type === "aggregate";
          var edgeType = edge.type;
          var label = (edgeType || "").toLowerCase();
          var size = 1;

          // Handle aggregate edges
          if (isAggregate) {
            var props = calcAggregateEdgeProps(edge.aggregate_counts);
            size = props.size;
            label = props.label;
            edgeStats.aggregate++;
          }

          graph.addEdge(edge.source, edge.target, {
            label: label,
            edgeType: edgeType,
            color: edgeColor(edgeType),
            size: size,
            isAggregate: isAggregate,
            aggregateCounts: edge.aggregate_counts || null,
          });
          edgeStats.added++;
        } catch (e) {
          edgeStats.skipped++;
          var msg = e.message || String(e);
          edgeStats.errors[msg] = (edgeStats.errors[msg] || 0) + 1;
        }
      });
      console.log("[graph-bridge] Edge loading:", edgeStats.added, "added,", edgeStats.skipped, "skipped");
      if (edgeStats.aggregate > 0) {
        console.log("[graph-bridge] Aggregate edges:", edgeStats.aggregate);
      }
      if (edgeStats.skipped > 0) {
        console.warn("[graph-bridge] Skipped edge errors:", edgeStats.errors);
      }

      // Get theme colors at render time
      var themeColors = getThemeColors();

      // Create renderer
      var renderer = new Sigma(graph, container, {
        renderLabels: true,
        labelSize: 11,
        labelColor: { color: themeColors.text },
        labelFont: "ui-monospace, monospace",
        labelRenderedSizeThreshold: 0,
        labelDensity: 0.5,
        defaultEdgeColor: themeColors.surface2,
        defaultEdgeType: "arrow",
        edgeLabelSize: 10,
        minCameraRatio: 0.05,
        maxCameraRatio: 20,
        stagePadding: 40,
        // Hover label styling (theme-aware)
        defaultDrawNodeHover: function(context, data, settings) {
          var label = data.label;
          if (!label) return;

          // Get fresh theme colors (in case theme changed)
          var colors = getThemeColors();

          var size = settings.labelSize || 11;
          var font = settings.labelFont || "ui-monospace, monospace";
          context.font = "bold " + size + "px " + font;

          var x = data.x + data.size + 3;
          var y = data.y + size / 3;

          // Measure text for background
          var textWidth = context.measureText(label).width;
          var padding = 4;

          // Draw background
          context.fillStyle = colors.surface0;
          context.fillRect(x - padding, y - size, textWidth + padding * 2, size + padding);

          // Draw text
          context.fillStyle = colors.text;
          context.fillText(label, x, y);
        },
      });

      // State
      var hoveredNode = null;
      var hoveredNeighbors = new Set();
      var filteredKinds = new Set(); // empty = show all

      // Store instance early so reducers can access shared state
      var inst = {
        graph: graph,
        renderer: renderer,
        repoId: repoId,
        apiBase: apiBase || "/api/v1",  // Fallback for backward compatibility
        expandedNodes: new Set(),
        knownNodes: new Map(),
        filteredKinds: filteredKinds,
        filteredLanguage: null,
        filteredEdgeTypes: new Set(),  // empty = show all
        hideTests: false,
        searchMatches: null,      // Set of matched node IDs from search, null = no active search
        searchResultNodes: null,  // Set of all visible nodes (matches + ancestors) for proper edge visibility
        onSelectCallback: null,   // callback for node selection events
        onLegendChangeCallback: null,  // callback when legend should be refreshed (after expand/collapse)
        entryTypeCanvas: null,    // custom canvas for entry-type shapes
      };

      // Populate knownNodes cache with initial nodes
      graph.forEachNode(function(nodeId, attrs) {
        inst.knownNodes.set(nodeId, attrs);
      });

      // Create custom canvas layer for entry-type shapes (drawn on top of WebGL circles)
      var entryTypeCanvas = document.createElement("canvas");
      entryTypeCanvas.style.position = "absolute";
      entryTypeCanvas.style.top = "0";
      entryTypeCanvas.style.left = "0";
      entryTypeCanvas.style.pointerEvents = "none";
      container.style.position = "relative";
      container.appendChild(entryTypeCanvas);
      inst.entryTypeCanvas = entryTypeCanvas;

      // Draw entry-type shapes after each render
      renderer.on("afterRender", function() {
        var ctx = entryTypeCanvas.getContext("2d");
        var dims = renderer.getDimensions();
        var pixelRatio = window.devicePixelRatio || 1;

        // Resize canvas if needed
        if (entryTypeCanvas.width !== dims.width * pixelRatio || entryTypeCanvas.height !== dims.height * pixelRatio) {
          entryTypeCanvas.width = dims.width * pixelRatio;
          entryTypeCanvas.height = dims.height * pixelRatio;
          entryTypeCanvas.style.width = dims.width + "px";
          entryTypeCanvas.style.height = dims.height + "px";
          ctx.scale(pixelRatio, pixelRatio);
        }

        ctx.clearRect(0, 0, dims.width, dims.height);

        // Draw colored circle outlines for entry-type nodes
        graph.forEachNode(function(nodeId, attrs) {
          var entryType = nodeEntryTypes[nodeId];
          if (!entryType) return;

          // Skip hidden nodes
          if (attrs.hidden) return;
          if (inst.hideTests && attrs.isTest) return;
          if (inst.filteredLanguage && attrs.language !== inst.filteredLanguage) return;
          if (inst.filteredKinds.size > 0 && !inst.filteredKinds.has(attrs.kind)) return;

          // Get viewport coordinates
          var pos = renderer.graphToViewport({ x: attrs.x, y: attrs.y });

          // Use Sigma's scaleSize to get the exact rendered pixel size
          var nodeSize = renderer.scaleSize(attrs.size);

          // Get outline color for entry type
          var outlineColor = entryTypeBorderColor(entryType) || getCssVar("--ctp-blue", "#89b4fa");

          // Draw circle outline matching the node size exactly
          ctx.strokeStyle = outlineColor;
          ctx.lineWidth = 2;
          ctx.beginPath();
          ctx.arc(pos.x, pos.y, nodeSize, 0, Math.PI * 2);
          ctx.stroke();
        });
      });

      renderer.on("enterNode", function(event) {
        hoveredNode = event.node;
        hoveredNeighbors = new Set(graph.neighbors(event.node));
        renderer.refresh();
      });

      renderer.on("leaveNode", function() {
        hoveredNode = null;
        hoveredNeighbors = new Set();
        renderer.refresh();
      });

      // Single click: select node (for info bar)
      renderer.on("clickNode", function(event) {
        if (inst.onSelectCallback) {
          var node = event.node;
          var attrs = graph.getNodeAttributes(node);
          var ancestorChain = getAncestorChain(graph, node);
          var info = JSON.stringify({
            id: node,
            label: attrs.label || "",
            qualifiedName: attrs.qualifiedName || attrs.label || "",
            kind: attrs.kind || "unknown",
            language: attrs.language || "unknown",
            filePath: attrs.filePath || "",
            startLine: attrs.startLine || 0,
            entryType: attrs.entryType || null,
            childCount: attrs.childCount || 0,
            isExpanded: inst.expandedNodes.has(node),
            ancestorChain: ancestorChain,
          });
          inst.onSelectCallback(info);
        }
      });

      // Double-click: expand/collapse container nodes
      renderer.on("doubleClickNode", async function(event) {
        event.preventSigmaDefault(); // Prevent default double-click zoom
        var node = event.node;
        var attrs = graph.getNodeAttributes(node);

        // If already expanded, collapse it (sync)
        if (inst.expandedNodes.has(node)) {
          window.graphCollapseNode(containerId, node);
        }
        // If it's a container (has children), expand it (async — must await)
        else if (attrs.childCount && attrs.childCount > 0) {
          await window.graphExpandNode(containerId, node);
        }
        // Leaf node - no-op
        else {
          return;
        }

        // Re-fire select callback with updated isExpanded state
        // This runs after expand/collapse has completed, so expandedNodes is accurate
        if (inst.onSelectCallback) {
          var updatedAttrs = graph.getNodeAttributes(node);
          var ancestorChain = getAncestorChain(graph, node);
          var info = JSON.stringify({
            id: node,
            label: updatedAttrs.label || "",
            qualifiedName: updatedAttrs.qualifiedName || updatedAttrs.label || "",
            kind: updatedAttrs.kind || "unknown",
            language: updatedAttrs.language || "unknown",
            filePath: updatedAttrs.filePath || "",
            startLine: updatedAttrs.startLine || 0,
            entryType: updatedAttrs.entryType || null,
            childCount: updatedAttrs.childCount || 0,
            isExpanded: inst.expandedNodes.has(node),
            ancestorChain: ancestorChain,
          });
          inst.onSelectCallback(info);
        }
      });

      // Click on empty canvas: clear selection
      renderer.on("clickStage", function() {
        // Clear selection
        if (inst.onSelectCallback) {
          inst.onSelectCallback("");
        }
      });

      // Node dragging support
      var draggedNode = null;
      var isDragging = false;

      renderer.on("downNode", function(event) {
        draggedNode = event.node;
        isDragging = true;
        // Disable camera panning while dragging a node
        renderer.getCamera().disable();
      });

      renderer.getMouseCaptor().on("mousemovebody", function(event) {
        if (!isDragging || !draggedNode) return;
        // Convert viewport coordinates to graph coordinates
        var pos = renderer.viewportToGraph(event);
        graph.setNodeAttribute(draggedNode, "x", pos.x);
        graph.setNodeAttribute(draggedNode, "y", pos.y);
        // Prevent sigma from showing hover while dragging
        event.preventSigmaDefault();
        event.original.preventDefault();
        event.original.stopPropagation();
      });

      renderer.getMouseCaptor().on("mouseup", function() {
        if (isDragging) {
          isDragging = false;
          draggedNode = null;
          renderer.getCamera().enable();
        }
      });

      // Also handle mouse leaving the container
      renderer.getMouseCaptor().on("mouseleave", function() {
        if (isDragging) {
          isDragging = false;
          draggedNode = null;
          renderer.getCamera().enable();
        }
      });

      // Node reducer: handles focus, hover dimming, kind/language/test filtering, entry type styling, container indicators, loading state
      renderer.setSetting("nodeReducer", function(node, data) {
        var res = Object.assign({}, data);
        var nodeKind = graph.getNodeAttribute(node, "kind");
        var nodeLang = graph.getNodeAttribute(node, "language");
        var nodeIsTest = graph.getNodeAttribute(node, "isTest");
        var nodeEntryType = graph.getNodeAttribute(node, "entryType");
        var childCount = graph.getNodeAttribute(node, "childCount") || 0;
        var nodeLoading = graph.getNodeAttribute(node, "loading");

        // Get dimmed color from theme
        var dimColor = getThemeColors().surface0;

        // Loading state: make node pulse
        if (nodeLoading) {
          res.size = res.size * (1 + 0.2 * Math.sin(Date.now() / 200));
        }

        // Container node visual distinction
        if (childCount > 0) {
          var isExpanded = inst.expandedNodes.has(node);
          // Add expand/collapse indicator to label
          var indicator = isExpanded ? "▼ " : "▶ ";
          res.label = indicator + res.label;
        }

        // Test filter: hide test symbols
        if (inst.hideTests && nodeIsTest) {
          res.hidden = true;
          return res;
        }

        // Language filter: hide nodes not matching selected language
        if (inst.filteredLanguage && nodeLang !== inst.filteredLanguage) {
          res.hidden = true;
          return res;
        }

        // Kind filter: hide nodes not in filteredKinds
        if (filteredKinds.size > 0 && !filteredKinds.has(nodeKind)) {
          res.hidden = true;
          return res;
        }

        // Edge type filter: hide nodes that have no visible edges of active types
        if (inst.filteredEdgeTypes.size > 0) {
          var hasVisibleEdge = false;
          graph.forEachEdge(node, function(edge, attrs, src, tgt) {
            if (hasVisibleEdge) return;
            var et = attrs.edgeType;
            if (!inst.filteredEdgeTypes.has(et)) return;
            var peer = (src === node) ? tgt : src;
            if (inst.hideTests && graph.getNodeAttribute(peer, "isTest")) return;
            if (inst.filteredLanguage && graph.getNodeAttribute(peer, "language") !== inst.filteredLanguage) return;
            if (filteredKinds.size > 0 && !filteredKinds.has(graph.getNodeAttribute(peer, "kind"))) return;
            hasVisibleEdge = true;
          });
          if (!hasVisibleEdge) {
            res.hidden = true;
            return res;
          }
        }

        // Entry type styling: pass entryType for label rendering
        if (nodeEntryType) {
          res.entryType = nodeEntryType;
        }

        // Search highlighting: dim nodes not in search result set (matched + ancestors)
        if (inst.searchResultNodes && !inst.searchResultNodes.has(node)) {
          res.color = dimColor;
          res.label = "";
          return res;
        }
        // Highlight matched nodes (not just ancestors)
        if (inst.searchMatches && inst.searchMatches.has(node)) {
          res.highlighted = true;
        }

        // Hover dimming (only when no active search)
        if (hoveredNode && hoveredNode !== node && !hoveredNeighbors.has(node)) {
          res.color = dimColor;
          res.label = "";
        }
        if (hoveredNode === node) {
          res.highlighted = true;
        }
        return res;
      });

      // Edge reducer: handles hover dimming, edge type/kind/language/test filtering
      renderer.setSetting("edgeReducer", function(edge, data) {
        var res = Object.assign({}, data);
        var source = graph.source(edge);
        var target = graph.target(edge);
        var edgeType = graph.getEdgeAttribute(edge, "edgeType");
        var isAggregate = graph.getEdgeAttribute(edge, "isAggregate");
        var aggregateCounts = graph.getEdgeAttribute(edge, "aggregateCounts");

        // Edge type filter: for aggregate edges, check if any constituent type passes
        if (inst.filteredEdgeTypes.size > 0) {
          if (isAggregate && aggregateCounts) {
            // Show aggregate edge if at least one constituent type is in filter
            var hasVisibleType = false;
            for (var type in aggregateCounts) {
              if (inst.filteredEdgeTypes.has(type)) {
                hasVisibleType = true;
                break;
              }
            }
            if (!hasVisibleType) {
              res.hidden = true;
              return res;
            }
          } else {
            // Regular edge: hide if not in filter
            if (!inst.filteredEdgeTypes.has(edgeType)) {
              res.hidden = true;
              return res;
            }
          }
        }

        // Test filter: hide edges where either endpoint is a test symbol
        if (inst.hideTests) {
          var sourceIsTest = graph.getNodeAttribute(source, "isTest");
          var targetIsTest = graph.getNodeAttribute(target, "isTest");
          if (sourceIsTest || targetIsTest) {
            res.hidden = true;
            return res;
          }
        }

        // Language filter: hide edges where either endpoint is filtered out
        if (inst.filteredLanguage) {
          var sourceLang = graph.getNodeAttribute(source, "language");
          var targetLang = graph.getNodeAttribute(target, "language");
          if (sourceLang !== inst.filteredLanguage || targetLang !== inst.filteredLanguage) {
            res.hidden = true;
            return res;
          }
        }

        // Kind filter: hide edges where either endpoint is filtered out
        if (filteredKinds.size > 0) {
          var sourceKind = graph.getNodeAttribute(source, "kind");
          var targetKind = graph.getNodeAttribute(target, "kind");
          if (!filteredKinds.has(sourceKind) || !filteredKinds.has(targetKind)) {
            res.hidden = true;
            return res;
          }
        }

        // Search dimming: hide edges not connecting nodes in search result (matched + ancestors)
        if (inst.searchResultNodes) {
          if (!inst.searchResultNodes.has(source) || !inst.searchResultNodes.has(target)) {
            res.hidden = true;
            return res;
          } else {
            res.size = 2;
          }
        }

        // Hover dimming (only when no active search)
        if (!inst.searchResultNodes && hoveredNode) {
          if (source !== hoveredNode && target !== hoveredNode) {
            res.hidden = true;
          } else {
            res.size = 2;
          }
        }

        // Apply lower opacity to aggregate edges for distinction
        if (isAggregate) {
          // Note: Sigma.js doesn't have built-in opacity control for edges
          // The distinct color from edgeColor() is sufficient
          // If needed, could use custom edge program in future
        }

        return res;
      });

      instances[containerId] = inst;

      // Log initial graph stats
      console.log("[graph-bridge] Graph loaded:", graph.order, "nodes,", graph.size, "edges");

      // Compute node sizes from edge degree (capped to prevent huge nodes)
      graph.forEachNode(function(node) {
        var degree = graph.degree(node);
        var kind = graph.getNodeAttribute(node, "kind");
        var childCount = graph.getNodeAttribute(node, "childCount") || 0;
        // Base size 8, scale by log to compress high-degree nodes
        // Max size ~17 even for nodes with 1000+ edges
        var size = 8 + Math.min(Math.log(degree + 1) * 2, 9);

        // Container nodes (with children) should be larger and more visible
        if (childCount > 0) {
          size = Math.max(size, 12);
        }

        // Boost size for structurally important but low-connectivity nodes
        // Fields, constants, and types often have few edges but are important
        if (kind === "field" || kind === "property" || kind === "constant" || kind === "const" || kind === "type_alias" || kind === "type") {
          size = Math.max(size, 10);  // Ensure well above labelRenderedSizeThreshold
        }

        graph.setNodeAttribute(node, "size", size);
      });

      // Ensure all nodes have initial positions before ForceAtlas2
      var spread = Math.sqrt(graph.order) * 50;
      graph.forEachNode(function(node, attrs) {
        if (attrs.x === undefined || attrs.y === undefined) {
          graph.setNodeAttribute(node, "x", (Math.random() - 0.5) * spread);
          graph.setNodeAttribute(node, "y", (Math.random() - 0.5) * spread);
        }
      });

      // Apply ForceAtlas2 layout on initial load with performance-aware iteration count
      if (typeof ForceAtlas2Layout === "function" && graph.order > 0) {
        var settings = getLayoutSettings(graph);
        var iterations = getLayoutIterations(graph.order);
        ForceAtlas2Layout(graph, {
          iterations: iterations,
          settings: settings
        });
        console.log("[graph-bridge] Initial layout: " + iterations + " iterations for " + graph.order + " nodes");
      }
      renderer.refresh();

      return true;
    } catch (e) {
      console.error("Failed to initialize graph:", e);
      return false;
    }
  };

  /**
   * Destroy a graph instance and free resources.
   * @param {string} containerId
   */
  window.destroyGraph = function(containerId) {
    if (instances[containerId]) {
      instances[containerId].layoutRunning = false;
      instances[containerId].renderer.kill();
      // Remove custom entry-type canvas if it exists
      if (instances[containerId].entryTypeCanvas) {
        instances[containerId].entryTypeCanvas.remove();
      }
      // Remove keyboard event listener to prevent memory leaks
      if (instances[containerId].keyboardHandler) {
        document.removeEventListener("keydown", instances[containerId].keyboardHandler);
      }
      delete instances[containerId];
    }
  };

  /**
   * Expand a container node by fetching and displaying its children.
   * @param {string} containerId
   * @param {string} nodeId - The node to expand
   */
  window.graphExpandNode = async function(containerId, nodeId) {
    var inst = instances[containerId];
    if (!inst) return;

    // Already expanded
    if (inst.expandedNodes.has(nodeId)) return;

    // Get node attributes
    var nodeAttrs = inst.graph.getNodeAttributes(nodeId);

    // Leaf node (no children)
    if (!nodeAttrs.childCount || nodeAttrs.childCount === 0) return;

    try {
      // Set loading state on the expanded node
      inst.graph.setNodeAttribute(nodeId, "loading", true);

      // Start animation loop for loading pulse
      var loadingInterval = setInterval(function() {
        inst.renderer.refresh();
      }, 50);

      // Collect IDs of all currently visible nodes
      var visibleIds = inst.graph.nodes().map(encodeURIComponent).join(',');

      // Fetch children from API using configured API base
      var url = inst.apiBase + "/repos/" + inst.repoId + "/graph?root=" + encodeURIComponent(nodeId) + "&depth=1";
      if (visibleIds) {
        url += "&visible_ids=" + visibleIds;
      }

      var response = await fetch(url);
      if (!response.ok) {
        console.error("Failed to fetch subgraph:", response.statusText);
        // Clear loading state on error
        inst.graph.setNodeAttribute(nodeId, "loading", false);
        clearInterval(loadingInterval);
        return;
      }

      var data = await response.json();
      console.log('[graph-bridge] Expand response:', data); // Debug: check edges in response

      // Get parent position for placing children nearby
      var parentX = nodeAttrs.x;
      var parentY = nodeAttrs.y;
      // Calculate child placement offset based on child count
      var childCount = (data.nodes || []).length;
      var offsetRange = Math.max(150, childCount * 50);

      // Add new nodes with positions near parent (small random offset)
      (data.nodes || []).forEach(function(node) {
        // Skip if already in graph
        if (inst.graph.hasNode(node.id)) return;

        // Place new nodes near parent with small random offset
        var x = parentX + (Math.random() - 0.5) * offsetRange;
        var y = parentY + (Math.random() - 0.5) * offsetRange;

        var attrs = {
          label: node.label,
          qualifiedName: node.qualified_name || node.label,
          size: 8,
          color: kindColor(node.kind),
          kind: node.kind,
          language: node.language || "unknown",
          filePath: node.file_path,
          startLine: node.start_line,
          entryType: node.entry_type || null,
          isTest: node.is_test || false,
          childCount: node.child_count || 0,
          parentId: node.parent_id || null,
          x: x,
          y: y,
        };

        inst.graph.addNode(node.id, attrs);
        inst.knownNodes.set(node.id, attrs);

        // If in search mode, add newly loaded nodes to searchResultNodes
        if (inst.searchResultNodes) {
          inst.searchResultNodes.add(node.id);
        }
      });

      // Add new edges (API provides unique edge IDs, multi-graph handles parallel edges)
      (data.edges || []).forEach(function(edge) {
        // Guard against missing endpoints
        if (!inst.graph.hasNode(edge.source) || !inst.graph.hasNode(edge.target)) {
          return;
        }

        var isAggregate = edge.type === "aggregate";
        var edgeType = edge.type;
        var label = (edgeType || "").toLowerCase();
        var size = 1;

        // Handle aggregate edges
        if (isAggregate) {
          var props = calcAggregateEdgeProps(edge.aggregate_counts);
          size = props.size;
          label = props.label;
        }

        // Use edge.id as key to prevent true duplicates, allow parallel edges of different types
        inst.graph.addEdge(edge.source, edge.target, {
          label: label,
          edgeType: edgeType,
          color: edgeColor(edgeType),
          size: size,
          isAggregate: isAggregate,
          aggregateCounts: edge.aggregate_counts || null,
        });
      });

      // Recompute sizes for new nodes based on their degree
      inst.graph.forEachNode(function(node) {
        var degree = inst.graph.degree(node);
        var kind = inst.graph.getNodeAttribute(node, "kind");
        var childCount = inst.graph.getNodeAttribute(node, "childCount") || 0;
        var size = 8 + Math.min(Math.log(degree + 1) * 2, 9);
        if (childCount > 0) {
          size = Math.max(size, 12);
        }
        if (kind === "field" || kind === "property" || kind === "constant" || kind === "const" || kind === "type_alias" || kind === "type") {
          size = Math.max(size, 10);
        }
        inst.graph.setNodeAttribute(node, "size", size);
      });

      // Mark as expanded
      inst.expandedNodes.add(nodeId);

      // Track in expand history for keyboard shortcuts
      if (!inst.expandHistory) {
        inst.expandHistory = [];
      }
      inst.expandHistory.push(nodeId);

      // Notify legend change callback (graph now has new node kinds/edge types)
      if (inst.onLegendChangeCallback) {
        inst.onLegendChangeCallback();
      }

      // Run ForceAtlas2 burst to redistribute all nodes dynamically with animation
      // Run multiple passes — single pass doesn't converge well after adding nodes
      if (typeof ForceAtlas2Layout === "function" && inst.graph.order > 0) {
        var settings = getLayoutSettings(inst.graph);
        var iterations = getLayoutIterations(inst.graph.order);
        // Save pre-expand positions for animation
        var startPositions = {};
        inst.graph.forEachNode(function(node) {
          startPositions[node] = {
            x: inst.graph.getNodeAttribute(node, 'x'),
            y: inst.graph.getNodeAttribute(node, 'y')
          };
        });
        // Run 3 sequential passes to converge layout
        for (var pass = 0; pass < 5; pass++) {
          ForceAtlas2Layout(inst.graph, { iterations: iterations, settings: settings });
        }
        // Capture converged positions
        var targetPositions = {};
        inst.graph.forEachNode(function(node) {
          targetPositions[node] = {
            x: inst.graph.getNodeAttribute(node, 'x'),
            y: inst.graph.getNodeAttribute(node, 'y')
          };
        });
        // Restore start positions so animateLayout can interpolate
        inst.graph.forEachNode(function(node) {
          if (startPositions[node]) {
            inst.graph.setNodeAttribute(node, 'x', startPositions[node].x);
            inst.graph.setNodeAttribute(node, 'y', startPositions[node].y);
          }
        });
        console.log("[graph-bridge] Expand layout: " + (iterations * 3) + " total iterations for " + inst.graph.order + " nodes");
        animateLayout(inst, targetPositions);
      }

      // Clear loading state after layout completes
      inst.graph.setNodeAttribute(nodeId, "loading", false);
      clearInterval(loadingInterval);
    } catch (e) {
      console.error("Error expanding node:", e);
      // Ensure loading state is cleared on error
      try {
        inst.graph.setNodeAttribute(nodeId, "loading", false);
        if (loadingInterval) clearInterval(loadingInterval);
      } catch (cleanupError) {
        // Ignore cleanup errors
      }
    }
  };

  /**
   * Collapse a container node by removing its descendants.
   * @param {string} containerId
   * @param {string} nodeId - The node to collapse
   */
  window.graphCollapseNode = function(containerId, nodeId) {
    var inst = instances[containerId];
    if (!inst) return;

    // Not expanded
    if (!inst.expandedNodes.has(nodeId)) return;

    // Collect all descendants recursively
    var descendants = [];
    function collectDescendants(parentId) {
      inst.graph.forEachNode(function(id, attrs) {
        if (attrs.parentId === parentId) {
          descendants.push(id);
          collectDescendants(id); // Recurse
        }
      });
    }
    collectDescendants(nodeId);

    // Remove descendants (leaf-first by reversing)
    descendants.reverse().forEach(function(id) {
      try {
        var edges = inst.graph.edges(id);
        edges.forEach(function(e) {
          inst.graph.dropEdge(e);
        });
      } catch (e) {}
      try {
        inst.graph.dropNode(id);
      } catch (e) {}
      inst.expandedNodes.delete(id);
    });

    // If in search mode, remove collapsed nodes from searchResultNodes
    if (inst.searchResultNodes) {
      descendants.forEach(function(id) {
        inst.searchResultNodes.delete(id);
      });
    }

    // Remove this node from expanded set
    inst.expandedNodes.delete(nodeId);

    // Clean up expand history
    if (inst.expandHistory) {
      inst.expandHistory = inst.expandHistory.filter(function(id) {
        return inst.expandedNodes.has(id);
      });
    }

    inst.renderer.refresh();

    // Notify legend change callback (graph kinds/edge types may have changed)
    if (inst.onLegendChangeCallback) {
      inst.onLegendChangeCallback();
    }
  };

  /**
   * Collapse all expanded nodes and reset to root view.
   * @param {string} containerId
   */
  window.graphCollapseAll = async function(containerId) {
    var inst = instances[containerId];
    if (!inst) return;

    try {
      // Fetch root data again using configured API base
      var url = inst.apiBase + "/repos/" + inst.repoId + "/graph";
      var response = await fetch(url);
      if (!response.ok) {
        console.error("Failed to fetch root graph:", response.statusText);
        return;
      }

      var jsonData = await response.text();

      // Clear current graph
      inst.graph.clear();
      inst.expandedNodes.clear();
      inst.knownNodes.clear();

      // Clear expand history
      if (inst.expandHistory) {
        inst.expandHistory = [];
      }

      // Re-initialize with root data
      var data = JSON.parse(jsonData);
      var nodeEntryTypes = {};

      (data.nodes || []).forEach(function(node) {
        if (node.entry_type) {
          nodeEntryTypes[node.id] = node.entry_type;
        }
        var attrs = {
          label: node.label,
          qualifiedName: node.qualified_name || node.label,
          size: 3,
          color: kindColor(node.kind),
          kind: node.kind,
          language: node.language || "unknown",
          filePath: node.file_path,
          startLine: node.start_line,
          entryType: node.entry_type || null,
          isTest: node.is_test || false,
          childCount: node.child_count || 0,
          parentId: node.parent_id || null,
          x: (Math.random() - 0.5) * 100,
          y: (Math.random() - 0.5) * 100,
        };
        inst.graph.addNode(node.id, attrs);
        inst.knownNodes.set(node.id, attrs);
      });

      (data.edges || []).forEach(function(edge) {
        try {
          var isAggregate = edge.type === "aggregate";
          var edgeType = edge.type;
          var label = (edgeType || "").toLowerCase();
          var size = 1;

          // Handle aggregate edges
          if (isAggregate) {
            var props = calcAggregateEdgeProps(edge.aggregate_counts);
            size = props.size;
            label = props.label;
          }

          inst.graph.addEdge(edge.source, edge.target, {
            label: label,
            edgeType: edgeType,
            color: edgeColor(edgeType),
            size: size,
            isAggregate: isAggregate,
            aggregateCounts: edge.aggregate_counts || null,
          });
        } catch (e) {
          // Skip invalid edges
        }
      });

      // Recompute node sizes
      inst.graph.forEachNode(function(node) {
        var degree = inst.graph.degree(node);
        var kind = inst.graph.getNodeAttribute(node, "kind");
        var childCount = inst.graph.getNodeAttribute(node, "childCount") || 0;
        var size = 8 + Math.min(Math.log(degree + 1) * 2, 9);
        if (childCount > 0) {
          size = Math.max(size, 12);
        }
        if (kind === "field" || kind === "property" || kind === "constant" || kind === "const" || kind === "type_alias" || kind === "type") {
          size = Math.max(size, 10);
        }
        inst.graph.setNodeAttribute(node, "size", size);
      });

      // Run layout
      if (typeof ForceAtlas2Layout === "function" && inst.graph.order > 0) {
        var settings = ForceAtlas2Layout.inferSettings(inst.graph);
        settings.strongGravityMode = true;
        settings.linLogMode = true;
        settings.gravity = 1;
        settings.outboundAttractionDistribution = true;
        settings.adjustSizes = true;
        settings.scalingRatio = Math.max(settings.scalingRatio || 1, 1 + Math.log10(inst.graph.order));
        if (inst.graph.order > 100) {
          settings.barnesHutOptimize = true;
          settings.barnesHutTheta = 0.5;
        }
        var totalIterations = Math.min(600, 150 + Math.floor(inst.graph.order / 2));
        ForceAtlas2Layout(inst.graph, { iterations: totalIterations, settings: settings });
      }

      inst.renderer.refresh();

      // Notify legend change callback (back to root view, kinds/edge types reset)
      if (inst.onLegendChangeCallback) {
        inst.onLegendChangeCallback();
      }
    } catch (e) {
      console.error("Error collapsing all:", e);
    }
  };

  /**
   * Zoom to fit visible nodes in view.
   * @param {string} containerId
   */
  window.graphZoomToFit = function(containerId) {
    var inst = instances[containerId];
    if (!inst) return;

    var graph = inst.graph;
    var camera = inst.renderer.getCamera();

    // Collect visible node IDs
    var visibleNodes = [];
    graph.forEachNode(function(node, attrs) {
      if (inst.hideTests && attrs.isTest) return;
      if (inst.filteredLanguage && attrs.language !== inst.filteredLanguage) return;
      if (inst.filteredKinds.size > 0 && !inst.filteredKinds.has(attrs.kind)) return;
      visibleNodes.push(node);
    });

    if (visibleNodes.length === 0) {
      camera.animatedReset({ duration: 300 });
      return;
    }

    // Get full graph bounding box first
    var allMinX = Infinity, allMaxX = -Infinity;
    var allMinY = Infinity, allMaxY = -Infinity;
    graph.forEachNode(function(node, attrs) {
      allMinX = Math.min(allMinX, attrs.x);
      allMaxX = Math.max(allMaxX, attrs.x);
      allMinY = Math.min(allMinY, attrs.y);
      allMaxY = Math.max(allMaxY, attrs.y);
    });
    var rawGraphWidth = allMaxX - allMinX;
    var rawGraphHeight = allMaxY - allMinY;
    var graphWidth = rawGraphWidth || 1;
    var graphHeight = rawGraphHeight || 1;

    // Single node: center on it with tight zoom
    if (visibleNodes.length === 1) {
      var attrs = graph.getNodeAttributes(visibleNodes[0]);
      // Convert to normalized coordinates (0-1)
      // If graph is degenerate (all nodes at same x/y), center = 0.5
      var nx = rawGraphWidth === 0 ? 0.5 : (attrs.x - allMinX) / graphWidth;
      var ny = rawGraphHeight === 0 ? 0.5 : (attrs.y - allMinY) / graphHeight;
      camera.animate({ x: nx, y: ny, ratio: 0.15 }, { duration: 300 });
      return;
    }

    // Multiple nodes: calculate bounding box in graph coordinates
    var minX = Infinity, maxX = -Infinity;
    var minY = Infinity, maxY = -Infinity;

    visibleNodes.forEach(function(node) {
      var attrs = graph.getNodeAttributes(node);
      minX = Math.min(minX, attrs.x);
      maxX = Math.max(maxX, attrs.x);
      minY = Math.min(minY, attrs.y);
      maxY = Math.max(maxY, attrs.y);
    });

    // Center of visible nodes in normalized coordinates (0-1)
    // If graph is degenerate (all nodes collinear), center = 0.5
    var centerX = rawGraphWidth === 0 ? 0.5 : ((minX + maxX) / 2 - allMinX) / graphWidth;
    var centerY = rawGraphHeight === 0 ? 0.5 : ((minY + maxY) / 2 - allMinY) / graphHeight;

    var subsetWidth = maxX - minX;
    var subsetHeight = maxY - minY;

    // Ratio = fraction of graph to show (with padding)
    var ratio = Math.max(
      (subsetWidth / graphWidth) * 1.1,
      (subsetHeight / graphHeight) * 1.1,
      0.05
    );

    camera.animate({ x: centerX, y: centerY, ratio: ratio }, { duration: 300 });
  };

  /**
   * Zoom in by 1.5x.
   * @param {string} containerId
   */
  window.graphZoomIn = function(containerId) {
    var inst = instances[containerId];
    if (!inst) return;
    inst.renderer.getCamera().animatedZoom({ duration: 200, factor: 1.5 });
  };

  /**
   * Zoom out by 1.5x.
   * @param {string} containerId
   */
  window.graphZoomOut = function(containerId) {
    var inst = instances[containerId];
    if (!inst) return;
    inst.renderer.getCamera().animatedUnzoom({ duration: 200, factor: 1.5 });
  };

  /**
   * Redistribute nodes by re-running ForceAtlas2 layout.
   * @param {string} containerId
   */
  window.graphRedistribute = function(containerId) {
    var inst = instances[containerId];
    if (!inst) return;

    // Unfix all nodes
    inst.graph.forEachNode(function(node) {
      inst.graph.setNodeAttribute(node, 'fixed', false);
    });

    // Run ForceAtlas2 with enhanced settings and animate the transition
    // Run multiple passes to converge in a single click
    if (typeof ForceAtlas2Layout === "function" && inst.graph.order > 0) {
      var settings = getLayoutSettings(inst.graph);
      var iterations = getLayoutIterations(inst.graph.order);
      // Save starting positions
      var startPositions = {};
      inst.graph.forEachNode(function(node) {
        startPositions[node] = {
          x: inst.graph.getNodeAttribute(node, 'x'),
          y: inst.graph.getNodeAttribute(node, 'y')
        };
      });
      for (var pass = 0; pass < 5; pass++) {
        ForceAtlas2Layout(inst.graph, { iterations: iterations, settings: settings });
      }
      var targetPositions = {};
      inst.graph.forEachNode(function(node) {
        targetPositions[node] = {
          x: inst.graph.getNodeAttribute(node, 'x'),
          y: inst.graph.getNodeAttribute(node, 'y')
        };
      });
      // Restore for animated transition
      inst.graph.forEachNode(function(node) {
        if (startPositions[node]) {
          inst.graph.setNodeAttribute(node, 'x', startPositions[node].x);
          inst.graph.setNodeAttribute(node, 'y', startPositions[node].y);
        }
      });
      console.log("[graph-bridge] Redistribute: " + (iterations * 3) + " total iterations for " + inst.graph.order + " nodes");
      animateLayout(inst, targetPositions);
    }
  };

  /**
   * Center camera on a specific node.
   * @param {string} containerId
   * @param {string} nodeId - Node ID to center on
   */
  window.graphCenterOnNode = function(containerId, nodeId) {
    var inst = instances[containerId];
    if (!inst || !inst.graph.hasNode(nodeId)) return;

    var graph = inst.graph;
    var camera = inst.renderer.getCamera();

    // Get node position
    var nodeX = graph.getNodeAttribute(nodeId, 'x');
    var nodeY = graph.getNodeAttribute(nodeId, 'y');

    // Get full graph bounding box for normalization
    var allMinX = Infinity, allMaxX = -Infinity;
    var allMinY = Infinity, allMaxY = -Infinity;
    graph.forEachNode(function(node, attrs) {
      allMinX = Math.min(allMinX, attrs.x);
      allMaxX = Math.max(allMaxX, attrs.x);
      allMinY = Math.min(allMinY, attrs.y);
      allMaxY = Math.max(allMaxY, attrs.y);
    });
    var graphWidth = allMaxX - allMinX || 1;
    var graphHeight = allMaxY - allMinY || 1;

    // Convert to normalized coordinates (0-1)
    var nx = (nodeX - allMinX) / graphWidth;
    var ny = (nodeY - allMinY) / graphHeight;

    // Animate camera to node with tight zoom
    camera.animate({ x: nx, y: ny, ratio: 0.2 }, { duration: 300 });
  };

  /**
   * Register a callback for node clicks.
   * @param {string} containerId
   * @param {function} callback - receives node ID as string
   */
  window.onGraphNodeClick = function(containerId, callback) {
    if (instances[containerId]) {
      instances[containerId].renderer.on("clickNode", function(event) {
        callback(event.node);
      });
    }
  };

  /**
   * Filter graph to show only specified kinds.
   * @param {string} containerId
   * @param {string} kindsJson - JSON array of kind strings, e.g. '["function","struct"]'. Empty array = show all.
   */
  window.graphFilterKinds = function(containerId, kindsJson) {
    var inst = instances[containerId];
    if (!inst) return;
    try {
      var kinds = JSON.parse(kindsJson);
      inst.filteredKinds.clear();
      kinds.forEach(function(k) { inst.filteredKinds.add(k); });
      inst.renderer.refresh();
    } catch (e) {
      console.error("graphFilterKinds error:", e);
    }
  };

  /**
   * Get unique kinds and their colors from the current graph.
   * @param {string} containerId
   * @returns {string} JSON array of {kind, color} objects sorted by kind, or "[]"
   */
  window.graphGetKinds = function(containerId) {
    var inst = instances[containerId];
    if (!inst) return "[]";
    var kindMap = {};
    inst.graph.forEachNode(function(node, attrs) {
      var kind = attrs.kind || "unknown";
      if (!kindMap[kind]) {
        kindMap[kind] = kindColor(kind);
      }
    });
    var result = Object.keys(kindMap).sort().map(function(k) {
      return { kind: k, color: kindMap[k] };
    });
    return JSON.stringify(result);
  };

  /**
   * Get unique languages from the current graph.
   * @param {string} containerId
   * @returns {string} JSON array of language strings sorted alphabetically, or "[]"
   */
  window.graphGetLanguages = function(containerId) {
    var inst = instances[containerId];
    if (!inst) return "[]";
    var langs = new Set();
    inst.graph.forEachNode(function(node, attrs) {
      langs.add(attrs.language || "unknown");
    });
    var result = Array.from(langs).sort();
    return JSON.stringify(result);
  };

  /**
   * Filter graph to show only the specified language.
   * @param {string} containerId
   * @param {string} language - language name (e.g. "rust", "nushell"), or empty string / null for all
   */
  window.graphFilterLanguage = function(containerId, language) {
    var inst = instances[containerId];
    if (!inst) return;
    inst.filteredLanguage = language || null;
    inst.renderer.refresh();
  };

  /**
   * Filter graph to show only specified edge types.
   * @param {string} containerId
   * @param {string} typesJson - JSON array of edge type strings, e.g. '["Calls","Uses"]'. Empty array = show all.
   */
  window.graphFilterEdgeTypes = function(containerId, typesJson) {
    var inst = instances[containerId];
    if (!inst) return;
    try {
      var types = JSON.parse(typesJson);
      inst.filteredEdgeTypes.clear();
      types.forEach(function(t) { inst.filteredEdgeTypes.add(t); });
      inst.renderer.refresh();
    } catch (e) {
      console.error("graphFilterEdgeTypes error:", e);
    }
  };

  /**
   * Toggle test symbol visibility.
   * @param {string} containerId
   * @param {boolean} hide - true to hide test symbols, false to show them
   */
  window.graphFilterTests = function(containerId, hide) {
    var inst = instances[containerId];
    if (!inst) return;
    inst.hideTests = !!hide;
    inst.renderer.refresh();
  };

  /**
   * Get unique edge types and their colors from the current graph.
   * @param {string} containerId
   * @returns {string} JSON array of {kind, color} objects sorted by kind, or "[]"
   */
  window.graphGetEdgeTypes = function(containerId) {
    var inst = instances[containerId];
    if (!inst) return "[]";
    var typeMap = {};
    inst.graph.forEachEdge(function(edge, attrs) {
      var et = attrs.edgeType || "unknown";
      if (!typeMap[et]) {
        typeMap[et] = edgeColor(et);
      }
    });
    var result = Object.keys(typeMap).sort().map(function(t) {
      return { kind: t, color: typeMap[t] };
    });
    return JSON.stringify(result);
  };

  /**
   * Search nodes by label substring (case-insensitive).
   * Highlights matching nodes and dims everything else.
   * Empty query clears the search.
   * @param {string} containerId
   * @param {string} query - search string, empty to clear
   * @returns {number} number of matches
   */
  window.graphSearchNodes = function(containerId, query) {
    var inst = instances[containerId];
    if (!inst) return 0;

    if (!query || query.trim() === "") {
      inst.searchMatches = null;
      inst.searchResultNodes = null;
      inst.renderer.refresh();
      return 0;
    }

    var q = query.toLowerCase();
    var matches = new Set();
    inst.graph.forEachNode(function(node, attrs) {
      var label = (attrs.label || "").toLowerCase();
      var qname = (attrs.qualifiedName || "").toLowerCase();
      if (label.indexOf(q) !== -1 || qname.indexOf(q) !== -1) {
        matches.add(node);
      }
    });

    inst.searchMatches = matches.size > 0 ? matches : null;
    inst.renderer.refresh();
    return matches.size;
  };

  /**
   * Server-side search with ancestor chain reveal.
   * Fetches matching symbols from the API and adds them to the graph with their ancestor chains.
   * Highlights matched nodes and marks expanded nodes.
   * @param {string} containerId
   * @param {string} searchTerm - search string
   * @param {function} callback - Called with match count when complete
   */
  window.graphSearchAndReveal = function(containerId, searchTerm, callback) {
    var inst = instances[containerId];
    if (!inst) {
      if (callback) callback(0);
      return;
    }

    if (!searchTerm || searchTerm.trim() === "") {
      graphClearSearch(containerId);
      if (callback) callback(0);
      return;
    }

    // Snapshot graph state before first search (preserve pre-search state)
    if (!inst.preSearchSnapshot) {
      var snapshot = {
        nodes: {},
        edges: {},
        expandedNodes: new Set(inst.expandedNodes),
        knownNodes: new Map(inst.knownNodes),
        filteredKinds: new Set(inst.filteredKinds),
        filteredEdgeTypes: new Set(inst.filteredEdgeTypes)
      };
      inst.graph.forEachNode(function(id, attrs) {
        snapshot.nodes[id] = Object.assign({}, attrs);
      });
      inst.graph.forEachEdge(function(id, attrs, source, target) {
        snapshot.edges[id] = { attrs: Object.assign({}, attrs), source: source, target: target };
      });
      inst.preSearchSnapshot = snapshot;
    }

    // Temporarily clear filters during search to show all results
    var savedFilteredKinds = new Set(inst.filteredKinds);
    var savedFilteredEdgeTypes = new Set(inst.filteredEdgeTypes);
    inst.filteredKinds.clear();
    inst.filteredEdgeTypes.clear();

    // Fetch search results from API
    var url = inst.apiBase + "/repos/" + inst.repoId + "/graph?search=" + encodeURIComponent(searchTerm);

    fetch(url)
      .then(function(response) {
        if (!response.ok) {
          console.error("Search request failed:", response.status);
          inst.searchMatches = null;
          inst.searchResultNodes = null;
          inst.renderer.refresh();
          if (callback) callback(0);
          return null;
        }
        return response.json();
      })
      .then(function(data) {
        if (!data) return;

        var nodes = data.nodes || [];
        var edges = data.edges || [];

        if (nodes.length === 0) {
          inst.searchMatches = null;
          inst.searchResultNodes = null;
          inst.renderer.refresh();
          if (callback) callback(0);
          return;
        }

        // Replace graph with search results (show full hierarchy, not additive)
        inst.graph.clear();
        inst.expandedNodes.clear();
        inst.knownNodes.clear();

        console.log("[search] API returned", nodes.length, "nodes,", edges.length, "edges");

        // Track matched nodes (leaf nodes without children that matched the search)
        var matchedSet = new Set();
        var searchLower = searchTerm.toLowerCase();

        // Add nodes to graph
        for (var i = 0; i < nodes.length; i++) {
          var node = nodes[i];
          var color = kindColor(node.kind);
          var nodeAttrs = {
            label: node.label,
            qualifiedName: node.qualified_name,
            kind: node.kind,
            language: node.language,
            filePath: node.file_path,
            startLine: node.start_line,
            entryType: node.entry_type || null,
            isTest: node.is_test || false,
            childCount: node.child_count || 0,
            parentId: node.parent_id || null,
            color: color,
            size: (node.child_count || 0) > 0 ? 12 : 8,
            x: 0,  // Will be set by tree layout
            y: 0
          };
          inst.graph.addNode(node.id, nodeAttrs);
          inst.knownNodes.set(node.id, nodeAttrs);

          // Mark as expanded if it has children
          if ((node.child_count || 0) > 0) {
            inst.expandedNodes.add(node.id);
          }

          // Check if this node matched the search term
          var label = (node.label || "").toLowerCase();
          var qname = (node.qualified_name || "").toLowerCase();
          if (label.indexOf(searchLower) !== -1 || qname.indexOf(searchLower) !== -1) {
            matchedSet.add(node.id);
          }
        }

        console.log("[search] Added", inst.graph.order, "nodes to graph. Matched:", matchedSet.size);

        // Add edges to graph
        var edgesAdded = 0;
        var edgesFailed = 0;
        for (var i = 0; i < edges.length; i++) {
          var edge = edges[i];
          var edgeId = edge.id;
          var isAggregate = edge.type === "aggregate";
          var edgeType = edge.type;
          var label = (edgeType || "").toLowerCase();
          var size = 1;

          // Handle aggregate edges
          if (isAggregate) {
            var props = calcAggregateEdgeProps(edge.aggregate_counts);
            size = props.size;
            label = props.label;
          }

          try {
            inst.graph.addEdge(edge.source, edge.target, {
              label: label,
              edgeType: edgeType,
              color: edgeColor(edgeType),
              size: size,
              isAggregate: isAggregate,
              aggregateCounts: edge.aggregate_counts || null,
            });
            edgesAdded++;
          } catch (err) {
            edgesFailed++;
            console.warn("[search] Edge failed:", edge.source, "->", edge.target, err.message);
          }
        }

        console.log("[search] Edges added:", edgesAdded, "failed:", edgesFailed);
        console.log("[search] Graph now has", inst.graph.order, "nodes,", inst.graph.size, "edges");

        // Set search matches for highlighting
        inst.searchMatches = matchedSet.size > 0 ? matchedSet : null;
        // Set all nodes currently in graph (matched + ancestors) for visibility checks
        inst.searchResultNodes = matchedSet.size > 0 ? new Set(inst.graph.nodes()) : null;

        // Use tree layout for search results (shows hierarchy clearly)
        treeLayout(inst.graph);

        inst.renderer.refresh();

        // Zoom to fit all search results
        graphZoomToFit(containerId);

        // Notify legend change if callback is registered
        if (inst.onLegendChangeCallback) {
          inst.onLegendChangeCallback();
        }

        if (callback) callback(matchedSet.size);
      })
      .catch(function(error) {
        console.error("Search error:", error);
        inst.searchMatches = null;
        inst.searchResultNodes = null;
        inst.renderer.refresh();
        if (callback) callback(0);
      });
  };

  /**
   * Clear search highlighting.
   * @param {string} containerId
   */
  window.graphClearSearch = function(containerId) {
    var inst = instances[containerId];
    if (!inst) return;
    inst.searchMatches = null;
    inst.searchResultNodes = null;

    // Restore pre-search graph state if we have a snapshot
    if (inst.preSearchSnapshot) {
      var snapshot = inst.preSearchSnapshot;
      inst.preSearchSnapshot = null;

      // Remove all current nodes/edges and restore from snapshot
      inst.graph.clear();

      var nodeIds = Object.keys(snapshot.nodes);
      for (var i = 0; i < nodeIds.length; i++) {
        inst.graph.addNode(nodeIds[i], snapshot.nodes[nodeIds[i]]);
      }

      var edgeIds = Object.keys(snapshot.edges);
      for (var i = 0; i < edgeIds.length; i++) {
        var e = snapshot.edges[edgeIds[i]];
        try {
          inst.graph.addEdge(e.source, e.target, e.attrs);
        } catch (err) { /* skip if source/target missing */ }
      }

      inst.expandedNodes = new Set(snapshot.expandedNodes);
      inst.knownNodes = new Map(snapshot.knownNodes);
      inst.filteredKinds = new Set(snapshot.filteredKinds);
      inst.filteredEdgeTypes = new Set(snapshot.filteredEdgeTypes);

      // Notify legend change
      if (inst.onLegendChangeCallback) {
        inst.onLegendChangeCallback();
      }
    }

    inst.renderer.refresh();
  };

  /**
   * Check if layout is currently running.
   * @param {string} containerId
   * @returns {boolean}
   */
  window.graphIsLayoutRunning = function(containerId) {
    var inst = instances[containerId];
    return inst ? !!inst.layoutRunning : false;
  };

  /**
   * Register a callback for node selection events.
   * Callback receives a JSON string with node info, or empty string on deselect.
   * @param {string} containerId
   * @param {function} callback
   */
  window.graphOnNodeSelect = function(containerId, callback) {
    var inst = instances[containerId];
    if (!inst) return;
    inst.onSelectCallback = callback;
  };

  /**
   * Register a callback to be notified when the legend should be refreshed
   * (after expand/collapse operations that change visible node kinds or edge types).
   * @param {string} containerId
   * @param {function} callback - Will be called with no arguments
   */
  window.graphOnLegendChange = function(containerId, callback) {
    var inst = instances[containerId];
    if (!inst) return;
    inst.onLegendChangeCallback = callback;
  };

  /**
   * Diagnostic: report graph stats and filter state.
   * Call from browser console: graphDiagnostics("graph-container")
   * @param {string} containerId
   */
  window.graphDiagnostics = function(containerId) {
    var inst = instances[containerId];
    if (!inst) { console.log("No graph instance for", containerId); return; }
    var graph = inst.graph;
    var totalNodes = graph.order;
    var totalEdges = graph.size;

    var testNodes = 0, testEdges = 0;
    graph.forEachNode(function(node, attrs) {
      if (attrs.isTest) testNodes++;
    });
    graph.forEachEdge(function(edge, attrs, source, target) {
      var sTest = graph.getNodeAttribute(source, "isTest");
      var tTest = graph.getNodeAttribute(target, "isTest");
      if (sTest || tTest) testEdges++;
    });

    var edgesByType = {};
    graph.forEachEdge(function(edge, attrs) {
      var et = attrs.edgeType || "unknown";
      edgesByType[et] = (edgesByType[et] || 0) + 1;
    });

    console.log("=== Graph Diagnostics ===");
    console.log("Total nodes:", totalNodes, "| Total edges:", totalEdges);
    console.log("Test nodes:", testNodes, "| Test-touching edges:", testEdges);
    console.log("Non-test edges:", totalEdges - testEdges);
    console.log("Edges by type:", edgesByType);
    console.log("Filter state:", {
      hideTests: inst.hideTests,
      filteredLanguage: inst.filteredLanguage,
      filteredKinds: Array.from(inst.filteredKinds),
      filteredEdgeTypes: Array.from(inst.filteredEdgeTypes),
      expandedNodes: Array.from(inst.expandedNodes),
      searchMatches: inst.searchMatches ? inst.searchMatches.size : null,
    });
    console.log("========================");
  };

  /**
   * Initialize keyboard shortcuts for graph navigation.
   * @param {string} containerId - The graph container ID
   */
  window.graphInitKeyboardShortcuts = function(containerId) {
    var inst = instances[containerId];
    if (!inst) {
      console.error("Cannot initialize keyboard shortcuts - graph instance not found:", containerId);
      return;
    }

    // Remove existing keyboard handler if already initialized to prevent double-init
    if (inst.keyboardHandler) {
      document.removeEventListener("keydown", inst.keyboardHandler);
      console.log("[graph-bridge] Removed existing keyboard handler for", containerId);
    }

    // Track expand history (most recent expansions first)
    if (!inst.expandHistory) {
      inst.expandHistory = [];
    }

    // Store selected node ID (updated by existing node selection handler)
    if (!inst.selectedNode) {
      inst.selectedNode = null;
    }

    // Listen for clickNode events to track selection
    inst.renderer.on("clickNode", function(event) {
      inst.selectedNode = event.node;
    });

    // Clear selection on stage click
    inst.renderer.on("clickStage", function() {
      inst.selectedNode = null;
    });

    // Keyboard event handler
    var keydownHandler = function(event) {
      var container = document.getElementById(containerId);
      if (!container) return;

      // Only handle keys when the graph container or its children have focus
      // or when no input/textarea is focused
      var activeEl = document.activeElement;
      var isInputFocused = activeEl && (activeEl.tagName === 'INPUT' || activeEl.tagName === 'TEXTAREA');

      if (isInputFocused) {
        return; // Don't interfere with input fields
      }

      // Escape: Collapse all expanded nodes
      if (event.key === "Escape") {
        event.preventDefault();
        window.graphCollapseAll(containerId);
        console.log("[graph-bridge] Keyboard: Collapse all (Escape)");
      }

      // Enter: Toggle expand/collapse on selected node
      else if (event.key === "Enter") {
        if (inst.selectedNode) {
          event.preventDefault();
          var nodeAttrs = inst.graph.getNodeAttributes(inst.selectedNode);

          // Check if node is expandable (has children)
          if (nodeAttrs.childCount && nodeAttrs.childCount > 0) {
            if (inst.expandedNodes.has(inst.selectedNode)) {
              // Collapse
              window.graphCollapseNode(containerId, inst.selectedNode);
              console.log("[graph-bridge] Keyboard: Collapsed node", inst.selectedNode);
            } else {
              // Expand (graphExpandNode will handle history push)
              window.graphExpandNode(containerId, inst.selectedNode);
              console.log("[graph-bridge] Keyboard: Expanded node", inst.selectedNode);
            }
          }
        }
      }

      // Backspace: Collapse most recently expanded node
      else if (event.key === "Backspace") {
        if (inst.expandHistory.length > 0) {
          event.preventDefault();

          // Pop history entries until we find one that's still expanded, or exhaust the history
          var nodeToCollapse = null;
          while (inst.expandHistory.length > 0) {
            var candidateNode = inst.expandHistory.pop();

            // Only collapse if still expanded (might have been collapsed by other means)
            if (inst.expandedNodes.has(candidateNode)) {
              nodeToCollapse = candidateNode;
              break;
            }
            // Otherwise, continue popping to find a valid entry
          }

          if (nodeToCollapse) {
            window.graphCollapseNode(containerId, nodeToCollapse);
            console.log("[graph-bridge] Keyboard: Collapsed recent expansion", nodeToCollapse);
          }
        }
      }
    };

    // Store handler reference for potential cleanup
    inst.keyboardHandler = keydownHandler;

    // Attach to document for global capture
    document.addEventListener("keydown", keydownHandler);

    console.log("[graph-bridge] Keyboard shortcuts initialized for", containerId);
  };
})();
