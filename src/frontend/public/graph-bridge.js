// Graph Bridge: Sigma.js + Graphology integration for c5t
// Called from Rust/WASM via wasm-bindgen

(function() {
  "use strict";

  // Store active graph instances by container ID
  var instances = {};

  // Kind-to-color mapping (Catppuccin Mocha palette)
  var kindColors = {
    "function": "#89b4fa",    // blue
    "method":   "#89b4fa",    // blue
    "command":  "#89b4fa",    // blue
    "struct":   "#a6e3a1",    // green
    "enum":     "#f9e2af",    // yellow
    "trait":    "#cba6f7",    // mauve
    "interface":"#cba6f7",    // mauve
    "module":   "#fab387",    // peach
    "mod":      "#fab387",    // peach
    "constant": "#f2cdcd",    // flamingo
    "const":    "#f2cdcd",    // flamingo
    "static":   "#f38ba8",    // red
    "var":      "#f38ba8",    // red
    "type_alias":"#94e2d5",   // teal
    "type":     "#94e2d5",    // teal
    "macro":    "#f5c2e7",    // pink
    "alias":    "#eba0ac",    // maroon
    "extern":   "#74c7ec",    // sapphire
  };
  var defaultColor = "#a6adc8"; // subtext0

  // Entry type border colors (Catppuccin Mocha palette)
  var entryTypeBorderColors = {
    "main":      "#a6e3a1",   // green - application entry points
    "test":      "#f9e2af",   // yellow - test functions
    "export":    "#74c7ec",   // sapphire - exported/FFI symbols
    "init":      "#fab387",   // peach - initialization functions
    "benchmark": "#cba6f7",   // mauve - benchmark functions
    "example":   "#94e2d5",   // teal - example functions
  };

  function kindColor(kind) {
    return kindColors[kind] || defaultColor;
  }

  function entryTypeBorderColor(entryType) {
    return entryTypeBorderColors[entryType] || null;
  }

  // Edge type-to-color mapping (Catppuccin Mocha palette)
  var edgeColors = {
    "Calls":          "#89b4fa",  // blue
    "Uses":           "#f9e2af",  // yellow
    "Returns":        "#a6e3a1",  // green
    "Accepts":        "#94e2d5",  // teal
    "FieldType":      "#f5c2e7",  // pink
    "TypeAnnotation": "#fab387",  // peach
    "Inherits":       "#cba6f7",  // mauve
    "Import":         "#74c7ec",  // sapphire
    "Contains":       "#585b70",  // surface2 (subtle)
  };
  var defaultEdgeColor = "#585b70"; // surface2

  function edgeColor(edgeType) {
    return edgeColors[edgeType] || defaultEdgeColor;
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

      // Helper: draw node shape based on entry type
      function drawNodeShape(context, x, y, size, color, entryType) {
        context.fillStyle = color;
        context.beginPath();

        if (entryType === "main") {
          // Square for main entry points
          var half = size;
          context.rect(x - half, y - half, half * 2, half * 2);
        } else if (entryType === "test") {
          // Diamond for test
          context.moveTo(x, y - size * 1.2);
          context.lineTo(x + size * 1.2, y);
          context.lineTo(x, y + size * 1.2);
          context.lineTo(x - size * 1.2, y);
          context.closePath();
        } else if (entryType === "export") {
          // Triangle pointing right for exports
          context.moveTo(x + size * 1.2, y);
          context.lineTo(x - size * 0.8, y - size);
          context.lineTo(x - size * 0.8, y + size);
          context.closePath();
        } else if (entryType === "init") {
          // Triangle pointing up for init
          context.moveTo(x, y - size * 1.2);
          context.lineTo(x + size, y + size * 0.8);
          context.lineTo(x - size, y + size * 0.8);
          context.closePath();
        } else if (entryType === "benchmark" || entryType === "example") {
          // Hexagon for benchmark/example
          for (var i = 0; i < 6; i++) {
            var angle = (Math.PI / 3) * i - Math.PI / 6;
            var px = x + size * Math.cos(angle);
            var py = y + size * Math.sin(angle);
            if (i === 0) context.moveTo(px, py);
            else context.lineTo(px, py);
          }
          context.closePath();
        } else {
          // Circle for regular nodes (default)
          context.arc(x, y, size, 0, Math.PI * 2);
        }

        context.fill();
      }

      // Create renderer
      var renderer = new Sigma(graph, container, {
        renderLabels: true,
        labelSize: 11,
        labelColor: { color: "#cdd6f4" },
        labelFont: "ui-monospace, monospace",
        labelRenderedSizeThreshold: 6,
        labelDensity: 0.5,
        defaultEdgeColor: "#585b70",
        defaultEdgeType: "arrow",
        edgeLabelSize: 10,
        minCameraRatio: 0.05,
        maxCameraRatio: 20,
        stagePadding: 40,
        defaultDrawNode: function(context, data, settings) {
          var entryType = nodeEntryTypes[data.key];
          drawNodeShape(context, data.x, data.y, data.size, data.color || "#a6adc8", entryType);
        },
        defaultDrawNodeLabel: function(context, data, settings) {
          if (!data.label) return;
          var size = settings.labelSize;
          var font = settings.labelFont;
          context.font = size + "px " + font;
          var textWidth = context.measureText(data.label).width;
          var padding = 4;
          var x = data.x + data.size + 3;
          var y = data.y + size / 3;

          // Draw background
          context.fillStyle = "#181825";
          context.fillRect(
            x - padding / 2,
            y - size + 1,
            textWidth + padding,
            size + 2
          );

          // Draw text
          context.fillStyle = "#cdd6f4";
          context.fillText(data.label, x, y);
        },
        defaultDrawNodeHover: function(context, data, settings) {
          // Draw enlarged node shape with highlight
          var entryType = nodeEntryTypes[data.key];
          drawNodeShape(context, data.x, data.y, data.size + 2, data.color || "#a6adc8", entryType);

          // Add highlight border
          context.strokeStyle = "#89b4fa";
          context.lineWidth = 2;
          context.stroke();

          // Draw hover label — show qualified name for disambiguation
          var hoverLabel = data.qualifiedName || data.label;
          if (!hoverLabel) return;
          var size = settings.labelSize + 2;
          var font = settings.labelFont;
          context.font = size + "px " + font;
          var textWidth = context.measureText(hoverLabel).width;
          var padding = 6;
          var x = data.x + data.size + 5;
          var y = data.y + size / 3;
          // Dark background with blue border
          context.fillStyle = "#1e1e2e";
          context.strokeStyle = "#89b4fa";
          context.lineWidth = 1;
          context.beginPath();
          context.roundRect(
            x - padding,
            y - size,
            textWidth + padding * 2,
            size + padding,
            4
          );
          context.fill();
          context.stroke();
          // Light text
          context.fillStyle = "#cdd6f4";
          context.fillText(hoverLabel, x, y);
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
      };

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
          res.color = "#313244";
          res.label = "";
          return res;
        }
        if (inst.searchMatches && inst.searchMatches.has(node)) {
          res.highlighted = true;
        }

        // Hover dimming (only when not in focus mode and no active search)
        if (!inst.focusedNode && hoveredNode && hoveredNode !== node && !hoveredNeighbors.has(node)) {
          res.color = "#313244";
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
        // Base size 3, scale by log to compress high-degree nodes
        // Max size ~12 even for nodes with 1000+ edges
        var size = 3 + Math.min(Math.log(degree + 1) * 2, 9);
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
            // Fit to view after layout completes
            window.graphZoomToFit(containerId);
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
      delete instances[containerId];
    }
  };

  /**
   * Zoom to fit visible nodes in view.
   * Accounts for focus, language, and kind filters.
   * @param {string} containerId
   */
  window.graphZoomToFit = function(containerId) {
    var inst = instances[containerId];
    if (!inst) return;

    var camera = inst.renderer.getCamera();

    // Collect visible node IDs
    var visibleNodes = [];
    inst.graph.forEachNode(function(node, attrs) {
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

    // Get ALL node positions to find the full bounding box (Sigma's normalization base)
    var allXs = [], allYs = [];
    inst.graph.forEachNode(function(n, a) { allXs.push(a.x); allYs.push(a.y); });
    var fullMinX = Math.min.apply(null, allXs), fullMaxX = Math.max.apply(null, allXs);
    var fullMinY = Math.min.apply(null, allYs), fullMaxY = Math.max.apply(null, allYs);
    var fullRangeX = fullMaxX - fullMinX || 1;
    var fullRangeY = fullMaxY - fullMinY || 1;
    var fullRange = Math.max(fullRangeX, fullRangeY);

    // Get visible node positions in graph coordinates
    var visXs = [], visYs = [];
    visibleNodes.forEach(function(node) {
      var attrs = inst.graph.getNodeAttributes(node);
      visXs.push(attrs.x);
      visYs.push(attrs.y);
    });

    // Single node — center on it
    if (visibleNodes.length === 1) {
      // Normalize to Sigma's [0, 1] space
      var nx = (visXs[0] - fullMinX) / fullRange;
      var ny = (visYs[0] - fullMinY) / fullRange;
      camera.animate({ x: nx, y: ny, ratio: 0.1 }, { duration: 300 });
      return;
    }

    // Bounding box of visible nodes in graph coordinates
    var visMinX = Math.min.apply(null, visXs), visMaxX = Math.max.apply(null, visXs);
    var visMinY = Math.min.apply(null, visYs), visMaxY = Math.max.apply(null, visYs);

    // Convert center to normalized coordinates
    var cx = ((visMinX + visMaxX) / 2 - fullMinX) / fullRange;
    var cy = ((visMinY + visMaxY) / 2 - fullMinY) / fullRange;

    // Ratio: proportion of full graph that the subset spans, with padding
    var subRangeX = visMaxX - visMinX;
    var subRangeY = visMaxY - visMinY;
    var subRange = Math.max(subRangeX, subRangeY);
    var padding = 1.4;
    var ratio = Math.max(0.05, (subRange / fullRange) * padding);

    camera.animate({ x: cx, y: cy, ratio: ratio }, { duration: 300 });
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
