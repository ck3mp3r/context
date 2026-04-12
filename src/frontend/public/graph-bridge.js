// Graph Bridge: Sigma.js + Graphology integration for c5t
// Called from Rust/WASM via wasm-bindgen

(function() {
  "use strict";

  // Store active graph instances by container ID
  var instances = {};

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
    // Mute edges more heavily (60% background) for subtlety
    return muteColor(color, 0.6);
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

  // N-hop BFS from a node
  function bfsNeighbors(graph, startNode, depth) {
    var visited = new Set();
    visited.add(startNode);
    var frontier = [startNode];
    for (var d = 0; d < depth; d++) {
      var nextFrontier = [];
      for (var i = 0; i < frontier.length; i++) {
        var neighbors = graph.neighbors(frontier[i]);
        for (var j = 0; j < neighbors.length; j++) {
          if (!visited.has(neighbors[j])) {
            visited.add(neighbors[j]);
            nextFrontier.push(neighbors[j]);
          }
        }
      }
      frontier = nextFrontier;
      if (frontier.length === 0) break;
    }
    return visited;
  }

  /**
   * Initialize a graph in a container element.
   * @param {string} containerId - DOM element ID for the graph canvas
   * @param {string} graphDataJson - JSON string with { nodes, edges, stats }
   * @returns {boolean} true if successful
   */
  window.initGraph = function(containerId, graphDataJson) {
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
        graph.addNode(node.id, {
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
          x: (Math.random() - 0.5) * 100,
          y: (Math.random() - 0.5) * 100,
        });
      });

      // Add edges with type-specific colors
      var edgeStats = { added: 0, skipped: 0, errors: {} };
      (data.edges || []).forEach(function(edge) {
        try {
          var label = (edge.type || "").toLowerCase();
          graph.addEdge(edge.source, edge.target, {
            label: label,
            edgeType: edge.type,
            color: edgeColor(edge.type),
            size: 1,
          });
          edgeStats.added++;
        } catch (e) {
          edgeStats.skipped++;
          var msg = e.message || String(e);
          edgeStats.errors[msg] = (edgeStats.errors[msg] || 0) + 1;
        }
      });
      console.log("[graph-bridge] Edge loading:", edgeStats.added, "added,", edgeStats.skipped, "skipped");
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
        labelRenderedSizeThreshold: 6,
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
        filteredKinds: filteredKinds,
        filteredLanguage: null,
        filteredEdgeTypes: new Set(),  // empty = show all
        hideTests: false,
        focusedNode: null,        // double-click focus
        focusedNeighbors: null,   // Set of neighbor IDs when focused
        focusDepth: 1,            // BFS depth for double-click focus
        savedCameraState: null,   // camera state before focus
        searchMatches: null,      // Set of matched node IDs from search, null = no active search
        onSelectCallback: null,   // callback for node selection events
        onFocusCallback: null,    // callback for focus state changes (double-click)
        entryTypeCanvas: null,    // custom canvas for entry-type shapes
      };

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
          if (inst.focusedNode && inst.focusedNeighbors && !inst.focusedNeighbors.has(nodeId)) return;
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
          var info = JSON.stringify({
            id: node,
            label: attrs.label || "",
            qualifiedName: attrs.qualifiedName || attrs.label || "",
            kind: attrs.kind || "unknown",
            language: attrs.language || "unknown",
            filePath: attrs.filePath || "",
            startLine: attrs.startLine || 0,
            entryType: attrs.entryType || null,
          });
          inst.onSelectCallback(info);
        }
      });

      // Double-click: focus on node and its N-hop neighbors
      renderer.on("doubleClickNode", function(event) {
        var node = event.node;
        var neighbors = bfsNeighbors(graph, node, inst.focusDepth);

        // Save camera state before focusing
        var camera = renderer.getCamera();
        inst.savedCameraState = camera.getState();
        inst.focusedNode = node;
        inst.focusedNeighbors = neighbors;

        // Notify Rust of focus state change
        if (inst.onFocusCallback) {
          inst.onFocusCallback(true);
        }

        renderer.refresh();

        // Defer zoom to let Sigma recalculate after reducer hides nodes
        setTimeout(function() {
          window.graphZoomToFit(containerId);
        }, 50);
      });

      // Click on empty canvas: restore previous zoom if focused
      renderer.on("clickStage", function() {
        if (inst.focusedNode && inst.savedCameraState) {
          inst.focusedNode = null;
          inst.focusedNeighbors = null;
          renderer.refresh();
          var camera = renderer.getCamera();
          camera.animate(inst.savedCameraState, { duration: 300 });
          inst.savedCameraState = null;
          // Notify Rust of focus state change
          if (inst.onFocusCallback) {
            inst.onFocusCallback(false);
          }
        }
        // Clear selection
        if (inst.onSelectCallback) {
          inst.onSelectCallback("");
        }
      });

      // Node reducer: handles focus, hover dimming, kind/language/test filtering, entry type styling
      renderer.setSetting("nodeReducer", function(node, data) {
        var res = Object.assign({}, data);
        var nodeKind = graph.getNodeAttribute(node, "kind");
        var nodeLang = graph.getNodeAttribute(node, "language");
        var nodeIsTest = graph.getNodeAttribute(node, "isTest");
        var nodeEntryType = graph.getNodeAttribute(node, "entryType");

        // Get dimmed color from theme
        var dimColor = getThemeColors().surface0;

        // Focus filter: hide nodes not in focused subgraph
        if (inst.focusedNode && inst.focusedNeighbors && !inst.focusedNeighbors.has(node)) {
          res.hidden = true;
          return res;
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

        // Highlight focused node
        if (inst.focusedNode === node) {
          res.highlighted = true;
        }

        // Search highlighting: dim non-matching nodes
        if (inst.searchMatches && !inst.searchMatches.has(node)) {
          res.color = dimColor;
          res.label = "";
          return res;
        }
        if (inst.searchMatches && inst.searchMatches.has(node)) {
          res.highlighted = true;
        }

        // Hover dimming (only when not in focus mode and no active search)
        if (!inst.focusedNode && hoveredNode && hoveredNode !== node && !hoveredNeighbors.has(node)) {
          res.color = dimColor;
          res.label = "";
        }
        if (hoveredNode === node) {
          res.highlighted = true;
        }
        return res;
      });

      // Edge reducer: handles focus, hover dimming, edge type/kind/language/test filtering
      renderer.setSetting("edgeReducer", function(edge, data) {
        var res = Object.assign({}, data);
        var source = graph.source(edge);
        var target = graph.target(edge);
        var edgeType = graph.getEdgeAttribute(edge, "edgeType");

        // Focus filter: hide edges not in focused subgraph
        if (inst.focusedNode && inst.focusedNeighbors) {
          if (!inst.focusedNeighbors.has(source) || !inst.focusedNeighbors.has(target)) {
            res.hidden = true;
            return res;
          }
        }

        // Edge type filter: hide edges not in filteredEdgeTypes
        if (inst.filteredEdgeTypes.size > 0 && !inst.filteredEdgeTypes.has(edgeType)) {
          res.hidden = true;
          return res;
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

        // Search dimming: hide edges not connecting matched nodes
        if (inst.searchMatches) {
          if (!inst.searchMatches.has(source) || !inst.searchMatches.has(target)) {
            res.hidden = true;
            return res;
          } else {
            res.size = 2;
          }
        }

        // Hover dimming (only when not in focus mode)
        if (!inst.focusedNode && !inst.searchMatches && hoveredNode) {
          if (source !== hoveredNode && target !== hoveredNode) {
            res.hidden = true;
          } else {
            res.size = 2;
          }
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
        // Base size 3, scale by log to compress high-degree nodes
        // Max size ~12 even for nodes with 1000+ edges
        var size = 3 + Math.min(Math.log(degree + 1) * 2, 9);

        // Boost size for structurally important but low-connectivity nodes
        // Fields, constants, and types often have few edges but are important
        if (kind === "field" || kind === "constant" || kind === "const" || kind === "type_alias" || kind === "type") {
          size = Math.max(size, 6);  // Ensure above labelRenderedSizeThreshold
        }

        graph.setNodeAttribute(node, "size", size);
      });

      // Run ForceAtlas2 layout in animated batches
      if (typeof ForceAtlas2Layout === "function" && graph.order > 0) {
        var settings = ForceAtlas2Layout.inferSettings(graph);
        var totalIterations = Math.min(300, 100 + Math.floor(graph.order / 5));
        var batchSize = 5;
        var currentIteration = 0;
        inst.layoutRunning = true;

        function runBatch() {
          if (!inst.layoutRunning || currentIteration >= totalIterations) {
            inst.layoutRunning = false;
            renderer.refresh();
            return;
          }
          var iters = Math.min(batchSize, totalIterations - currentIteration);
          ForceAtlas2Layout(graph, { iterations: iters, settings: settings });
          currentIteration += iters;
          renderer.refresh();
          setTimeout(runBatch, 0);
        }
        runBatch();
      }

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
      delete instances[containerId];
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
      if (inst.focusedNode && inst.focusedNeighbors && !inst.focusedNeighbors.has(node)) return;
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
    var graphWidth = allMaxX - allMinX || 1;
    var graphHeight = allMaxY - allMinY || 1;

    // Single node: center on it with tight zoom
    if (visibleNodes.length === 1) {
      var attrs = graph.getNodeAttributes(visibleNodes[0]);
      // Convert to normalized coordinates (0-1)
      var nx = (attrs.x - allMinX) / graphWidth;
      var ny = (attrs.y - allMinY) / graphHeight;
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
    var centerX = ((minX + maxX) / 2 - allMinX) / graphWidth;
    var centerY = ((minY + maxY) / 2 - allMinY) / graphHeight;

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
   * Set the BFS depth for double-click focus.
   * @param {string} containerId
   * @param {number} depth - Number of hops (1 = direct neighbors, 2 = neighbors of neighbors, etc.)
   */
  window.graphSetFocusDepth = function(containerId, depth) {
    var inst = instances[containerId];
    if (!inst) return;
    inst.focusDepth = Math.max(1, Math.min(depth, 5));
    if (inst.focusedNode) {
      inst.focusedNeighbors = bfsNeighbors(inst.graph, inst.focusedNode, inst.focusDepth);
      inst.renderer.refresh();
      setTimeout(function() {
        window.graphZoomToFit(containerId);
      }, 50);
    }
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
   * Register a callback for node selection (click) events.
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
   * Register a callback for focus state changes (double-click).
   * Callback receives a boolean: true when focused, false when unfocused.
   * @param {string} containerId
   * @param {function} callback
   */
  window.graphOnFocusChange = function(containerId, callback) {
    var inst = instances[containerId];
    if (!inst) return;
    inst.onFocusCallback = callback;
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
      focusedNode: inst.focusedNode,
      searchMatches: inst.searchMatches ? inst.searchMatches.size : null,
    });
    console.log("========================");
  };
})();
