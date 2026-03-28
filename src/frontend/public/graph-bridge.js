// Graph Bridge: Sigma.js + Graphology integration for c5t
// Called from Rust/WASM via wasm-bindgen

(function() {
  "use strict";

  // Store active graph instances by container ID
  var instances = {};

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

      // Add nodes with pre-computed positions from backend
      (data.nodes || []).forEach(function(node) {
        graph.addNode(node.id, {
          label: node.label,
          size: node.size || 5,
          color: node.color || "#a6adc8",
          kind: node.kind,
          filePath: node.file_path,
          startLine: node.start_line,
          x: node.x || 0,
          y: node.y || 0,
        });
      });

      // Add edges
      (data.edges || []).forEach(function(edge) {
        try {
          graph.addEdge(edge.source, edge.target, {
            label: edge.label,
            edgeType: edge.type,
            color: "#585b70",
            size: 1,
          });
        } catch (e) {
          // Skip edges with missing nodes
        }
      });

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
          // Draw enlarged node circle
          context.beginPath();
          context.arc(data.x, data.y, data.size + 2, 0, Math.PI * 2);
          context.closePath();
          context.fillStyle = data.color || "#a6adc8";
          context.fill();
          context.strokeStyle = "#89b4fa";
          context.lineWidth = 2;
          context.stroke();

          // Draw hover label
          if (!data.label) return;
          var size = settings.labelSize + 2;
          var font = settings.labelFont;
          context.font = size + "px " + font;
          var textWidth = context.measureText(data.label).width;
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
          context.fillText(data.label, x, y);
        },
      });

      // State
      var hoveredNode = null;
      var hoveredNeighbors = new Set();
      var filteredKinds = new Set(); // empty = show all

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

      // Node reducer: handles both hover dimming and kind filtering
      renderer.setSetting("nodeReducer", function(node, data) {
        var res = Object.assign({}, data);
        var nodeKind = graph.getNodeAttribute(node, "kind");

        // Kind filter: hide nodes not in filteredKinds
        if (filteredKinds.size > 0 && !filteredKinds.has(nodeKind)) {
          res.hidden = true;
          return res;
        }

        // Hover dimming
        if (hoveredNode && hoveredNode !== node && !hoveredNeighbors.has(node)) {
          res.color = "#313244";
          res.label = "";
        }
        if (hoveredNode === node) {
          res.highlighted = true;
        }
        return res;
      });

      // Edge reducer: handles both hover dimming and kind filtering
      renderer.setSetting("edgeReducer", function(edge, data) {
        var res = Object.assign({}, data);
        var source = graph.source(edge);
        var target = graph.target(edge);

        // Kind filter: hide edges where either endpoint is filtered out
        if (filteredKinds.size > 0) {
          var sourceKind = graph.getNodeAttribute(source, "kind");
          var targetKind = graph.getNodeAttribute(target, "kind");
          if (!filteredKinds.has(sourceKind) || !filteredKinds.has(targetKind)) {
            res.hidden = true;
            return res;
          }
        }

        // Hover dimming
        if (hoveredNode) {
          if (source !== hoveredNode && target !== hoveredNode) {
            res.hidden = true;
          } else {
            res.color = "#89b4fa";
            res.size = 2;
          }
        }
        return res;
      });

      instances[containerId] = { graph: graph, renderer: renderer, filteredKinds: filteredKinds };
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
      instances[containerId].renderer.kill();
      delete instances[containerId];
    }
  };

  /**
   * Zoom to fit all nodes in view.
   * @param {string} containerId
   */
  window.graphZoomToFit = function(containerId) {
    if (instances[containerId]) {
      var camera = instances[containerId].renderer.getCamera();
      camera.animatedReset({ duration: 300 });
    }
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
      // Update the filteredKinds set captured in the closure
      // We need to access it via a shared reference on the instance
      inst.filteredKinds.clear();
      kinds.forEach(function(k) { inst.filteredKinds.add(k); });
      inst.renderer.refresh();
    } catch (e) {
      console.error("graphFilterKinds error:", e);
    }
  };
})();
