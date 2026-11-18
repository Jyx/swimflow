// Folds and unfolds the branches of a tree diagram in an HTML bundle.
//
// The SVG carries the geometry of every node in `data-*` attributes, so the
// tree can be laid out again here without the server. `layoutTree` is a port
// of `src/plugin/tree/layout.rs` and must stay in step with it: a test
// (`tests/tree_runtime.js`) compares the two on every example.
(function (global) {
  'use strict';

  var MARGIN = 24;
  var SIBLING_GAP = 20;
  var LEVEL_GAP = 48;

  function half(value) { return Math.trunc(value / 2); }

  // nodes: [{ parent: index or -1, depth, w, h, edgeW, edgeH, collapsed }, ...]
  // with parents before their children. Returns the geometry of the visible
  // part of the tree; a hidden node sits on its parent.
  function layoutTree(nodes, orientation) {
    var portrait = orientation !== 'landscape';
    var count = nodes.length;
    var children = [];
    var i;
    for (i = 0; i < count; i++) children.push([]);
    for (i = 1; i < count; i++) children[nodes[i].parent].push(i);

    var visible = [true];
    for (i = 1; i < count; i++) {
      var parent = nodes[i].parent;
      visible.push(visible[parent] && !nodes[parent].collapsed);
    }
    var shown = nodes.map(function (node, index) {
      return visible[index] && !node.collapsed ? children[index] : [];
    });

    function breadthOf(index) { return portrait ? nodes[index].w : nodes[index].h; }
    function depthOf(index) { return portrait ? nodes[index].h : nodes[index].w; }

    var subtree = [];
    for (i = 0; i < count; i++) subtree.push(0);
    function extent(list) {
      if (!list.length) return 0;
      var sum = 0;
      list.forEach(function (child) { sum += subtree[child]; });
      return sum + SIBLING_GAP * (list.length - 1);
    }
    for (i = count - 1; i >= 0; i--) {
      if (!visible[i]) continue;
      subtree[i] = Math.max(breadthOf(i), extent(shown[i]));
    }

    var center = [];
    var start = [];
    for (i = 0; i < count; i++) { center.push(0); start.push(0); }
    start[0] = MARGIN;
    for (i = 0; i < count; i++) {
      if (!visible[i]) continue;
      var total = extent(shown[i]);
      var cursor = start[i] + half(subtree[i] - total);
      shown[i].forEach(function (child) {
        start[child] = cursor;
        cursor += subtree[child] + SIBLING_GAP;
      });
      center[i] = start[i] + half(subtree[i]);
    }
    for (i = count - 1; i >= 0; i--) {
      if (shown[i].length) {
        center[i] = half(center[shown[i][0]] + center[shown[i][shown[i].length - 1]]);
      }
    }

    var levels = 1;
    for (i = 0; i < count; i++) if (visible[i]) levels = Math.max(levels, nodes[i].depth + 1);
    var thickness = [];
    var gap = [];
    for (i = 0; i < levels; i++) { thickness.push(0); gap.push(LEVEL_GAP); }
    for (i = 0; i < count; i++) {
      if (!visible[i]) continue;
      var node = nodes[i];
      thickness[node.depth] = Math.max(thickness[node.depth], depthOf(i));
      if (node.edgeW > 0 || node.edgeH > 0) {
        var needed = portrait ? node.edgeH : node.edgeW;
        var level = Math.max(node.depth - 1, 0);
        gap[level] = Math.max(gap[level], 2 * (needed + 8));
      }
    }
    var offset = [];
    var run = MARGIN;
    for (i = 0; i < levels; i++) {
      offset.push(run);
      run += thickness[i] + gap[i];
    }
    var depthTotal = offset[levels - 1] + thickness[levels - 1] + MARGIN;
    var breadthTotal = subtree[0] + 2 * MARGIN;

    var boxes = [];
    for (i = 0; i < count; i++) {
      var n = nodes[i];
      var box;
      if (visible[i]) {
        var along = offset[n.depth];
        if (portrait) {
          var x1 = center[i] - half(n.w);
          box = { x1: x1, y1: along, x2: x1 + n.w, y2: along + n.h };
        } else {
          var y1 = center[i] - half(n.h);
          box = { x1: along, y1: y1, x2: along + n.w, y2: y1 + n.h };
        }
      } else {
        var p = boxes[n.parent];
        box = { x1: p.x1, y1: p.y1, x2: p.x1 + n.w, y2: p.y1 + n.h };
      }
      boxes.push(box);
    }

    // Split the bus of a trunk into solid and dashed stretches (the port of
    // `bus_stretches` in layout.rs): a stretch carries every child beyond it,
    // so it is dashed only when all of them are; a solid line wins.
    function busStretches(origin, stops) {
      var points = stops.map(function (stop) { return stop[0]; });
      points.push(origin);
      points.sort(function (a, b) { return a - b; });
      points = points.filter(function (value, k) { return k === 0 || value !== points[k - 1]; });
      var solid = [];
      var dashed = [];
      for (var k = 0; k + 1 < points.length; k++) {
        var a = points[k];
        var b = points[k + 1];
        var dash = stops.every(function (stop) {
          var beyond = a >= origin ? stop[0] >= b : stop[0] <= a;
          return !beyond || stop[1];
        });
        var target = dash ? dashed : solid;
        var last = target[target.length - 1];
        if (last && last[1] === a) last[1] = b; else target.push([a, b]);
      }
      return { solid: solid, dashed: dashed };
    }

    var trunks = [];
    for (i = 0; i < count; i++) {
      if (!children[i].length) { trunks.push(null); continue; }
      var trunk = { stem: [], stemDashed: false, bus: [], busDashed: [] };
      if (shown[i].length) {
        var first = boxes[shown[i][0]];
        var last = boxes[shown[i][shown[i].length - 1]];
        var from = boxes[i];
        var level_gap = gap[nodes[i].depth];
        if (portrait) {
          var busY = first.y1 - half(level_gap);
          var px = from.x1 + half(from.x2 - from.x1);
          var lo = first.x1 + half(first.x2 - first.x1);
          var hi = last.x1 + half(last.x2 - last.x1);
          trunk.stem = [[px, from.y2], [px, busY]];
          if (lo !== hi) {
            var cuts = busStretches(px, shown[i].map(function (c) {
              return [boxes[c].x1 + half(boxes[c].x2 - boxes[c].x1), nodes[c].edgeDashed];
            }));
            trunk.bus = cuts.solid.map(function (r) { return [[r[0], busY], [r[1], busY]]; });
            trunk.busDashed = cuts.dashed.map(function (r) { return [[r[0], busY], [r[1], busY]]; });
          }
        } else {
          var busX = first.x1 - half(level_gap);
          var py = from.y1 + half(from.y2 - from.y1);
          var lo2 = first.y1 + half(first.y2 - first.y1);
          var hi2 = last.y1 + half(last.y2 - last.y1);
          trunk.stem = [[from.x2, py], [busX, py]];
          if (lo2 !== hi2) {
            var cuts2 = busStretches(py, shown[i].map(function (c) {
              return [boxes[c].y1 + half(boxes[c].y2 - boxes[c].y1), nodes[c].edgeDashed];
            }));
            trunk.bus = cuts2.solid.map(function (r) { return [[busX, r[0]], [busX, r[1]]]; });
            trunk.busDashed = cuts2.dashed.map(function (r) { return [[busX, r[0]], [busX, r[1]]]; });
          }
        }
      }
      // Folding does not change the look of a connector: every child counts.
      trunk.stemDashed = children[i].every(function (c) { return nodes[c].edgeDashed; });
      trunks.push(trunk);
    }

    var drops = [];
    var labels = [];
    for (i = 0; i < count; i++) {
      drops.push([]);
      labels.push(null);
      if (i === 0) continue;
      var to = boxes[i];
      var anchor = [to.x1, to.y1];
      if (visible[i]) {
        var g = gap[nodes[i].depth - 1];
        if (portrait) {
          var bus = to.y1 - half(g);
          var cx = to.x1 + half(to.x2 - to.x1);
          drops[i] = [[cx, bus], [cx, to.y1]];
          anchor = [cx, half(bus + to.y1)];
        } else {
          var bus2 = to.x1 - half(g);
          var cy = to.y1 + half(to.y2 - to.y1);
          drops[i] = [[bus2, cy], [to.x1, cy]];
          anchor = [half(bus2 + to.x1), cy];
        }
      }
      if (nodes[i].edgeW > 0 || nodes[i].edgeH > 0) {
        var lx = anchor[0] - half(nodes[i].edgeW);
        var ly = anchor[1] - half(nodes[i].edgeH);
        labels[i] = { x1: lx, y1: ly, x2: lx + nodes[i].edgeW, y2: ly + nodes[i].edgeH };
      }
    }

    // The circle that folds a branch sits where the connector leaves the node.
    var toggles = boxes.map(function (box) {
      return portrait
        ? [box.x1 + half(box.x2 - box.x1), box.y2]
        : [box.x2, box.y1 + half(box.y2 - box.y1)];
    });

    return {
      toggles: toggles,
      width: portrait ? breadthTotal : depthTotal,
      height: portrait ? depthTotal : breadthTotal,
      visible: visible,
      boxes: boxes,
      trunks: trunks,
      drops: drops,
      labels: labels
    };
  }

  global.swimflowTree = { layoutTree: layoutTree };
  if (typeof document === 'undefined') return;

  // ---- the browser part -------------------------------------------------

  function number(element, name) { return parseInt(element.getAttribute('data-' + name), 10) || 0; }

  function pathData(points) {
    if (!points || !points.length) return '';
    return points.map(function (point, index) {
      return (index ? 'L' : 'M') + point[0] + ',' + point[1];
    }).join(' ');
  }

  // The path data of several polylines (the stretches of a bus).
  function pathsData(paths) {
    return (paths || []).map(pathData).join(' ');
  }

  function shift(element, x, y) {
    var dx = x - number(element, 'x');
    var dy = y - number(element, 'y');
    if (dx || dy) element.setAttribute('transform', 'translate(' + dx + ',' + dy + ')');
    else element.removeAttribute('transform');
  }

  function show(element, visible) {
    if (visible) element.removeAttribute('display');
    else element.setAttribute('display', 'none');
  }

  // Lay the tree of one `<g class="swimflow-tree">` out again.
  function relayout(group) {
    var svg = group.ownerSVGElement || group.closest('svg');
    var elements = Array.prototype.slice.call(group.querySelectorAll('.tree-node'));
    var nodes = elements.map(function (element) {
      var parent = element.getAttribute('data-parent');
      return {
        parent: parent ? parseInt(parent.slice(1), 10) : -1,
        depth: number(element, 'depth'),
        w: number(element, 'w'),
        h: number(element, 'h'),
        edgeW: number(element, 'edge-w'),
        edgeH: number(element, 'edge-h'),
        edgeDashed: element.getAttribute('data-edge-dashed') === '1',
        collapsed: element.getAttribute('data-collapsed') === '1'
      };
    });
    var result = layoutTree(nodes, group.getAttribute('data-orientation'));

    function find(selector, index) {
      return group.querySelector(selector + '[data-for="n' + index + '"]');
    }

    elements.forEach(function (element, index) {
      var box = result.boxes[index];
      show(element, result.visible[index]);
      shift(element, box.x1, box.y1);

      var stem = find('.tree-stem', index);
      var bus = find('.tree-bus', index);
      var trunk = result.trunks[index];
      if (stem && trunk) {
        stem.setAttribute('d', pathData(trunk.stem));
        bus.setAttribute('d', pathsData(trunk.bus));
        var busDashed = find('.tree-bus-dashed', index);
        if (busDashed) busDashed.setAttribute('d', pathsData(trunk.busDashed));
      }
      var drop = find('.tree-drop', index);
      if (drop) drop.setAttribute('d', pathData(result.drops[index]));

      var label = find('.tree-label', index);
      if (label && result.labels[index]) {
        shift(label, result.labels[index].x1, result.labels[index].y1);
        show(label, result.visible[index]);
      }

      var toggle = find('.tree-toggle', index);
      if (toggle) {
        shift(toggle, result.toggles[index][0], result.toggles[index][1]);
        show(toggle, result.visible[index]);
        var collapsed = nodes[index].collapsed;
        var plus = toggle.querySelector('.tree-plus');
        if (plus) show(plus, collapsed);
        toggle.setAttribute('aria-label', collapsed ? 'Expand' : 'Collapse');
      }
    });

    if (svg) {
      svg.setAttribute('width', result.width);
      svg.setAttribute('height', result.height);
      if (svg.hasAttribute('viewBox')) svg.setAttribute('viewBox', '0 0 ' + result.width + ' ' + result.height);
      for (var child = svg.firstElementChild; child; child = child.nextElementSibling) {
        if (child.tagName.toLowerCase() === 'rect') {
          child.setAttribute('width', result.width);
          child.setAttribute('height', result.height);
          break;
        }
      }
    }
  }

  function toggleNode(toggle) {
    var group = toggle.closest('.swimflow-tree');
    if (!group) return;
    var node = group.querySelector('.tree-node[data-id="' + toggle.getAttribute('data-for') + '"]');
    if (!node) return;
    node.setAttribute('data-collapsed', node.getAttribute('data-collapsed') === '1' ? '0' : '1');
    relayout(group);
  }

  // Folds (or unfolds) every branch, the ones hidden inside another branch
  // too: a node counts when it has a circle, that is when it has children.
  function setAll(group, collapsed) {
    Array.prototype.slice.call(group.querySelectorAll('.tree-toggle')).forEach(function (toggle) {
      var node = group.querySelector('.tree-node[data-id="' + toggle.getAttribute('data-for') + '"]');
      if (node) node.setAttribute('data-collapsed', collapsed ? '1' : '0');
    });
    relayout(group);
  }

  // The fold-all buttons live in the page bar, outside the section they act on.
  function sectionOf(element) {
    var section = element && element.closest ? element.closest('.swimflow-diagram') : null;
    if (section) return section;
    var slot = element && element.closest ? element.closest('.swimflow-toolbar-slot') : null;
    if (!slot) return null;
    var key = slot.getAttribute('data-diagram');
    return key && document.getElementById ? document.getElementById(key) : null;
  }

  document.addEventListener('click', function (event) {
    var target = event.target;
    var collapseAll = target && target.closest ? target.closest('.tree-collapse-all') : null;
    var expandAll = target && target.closest ? target.closest('.tree-expand-all') : null;
    if (collapseAll || expandAll) {
      event.preventDefault();
      var button = collapseAll || expandAll;
      var section = sectionOf(button);
      var tree = section ? section.querySelector('.swimflow-tree') : null;
      if (tree) setAll(tree, !!collapseAll);
      return;
    }
    var toggle = target && target.closest ? target.closest('.tree-toggle') : null;
    if (!toggle) return;
    event.preventDefault();
    toggleNode(toggle);
  });

  document.addEventListener('keydown', function (event) {
    if (event.key !== 'Enter' && event.key !== ' ') return;
    var target = event.target;
    var toggle = target && target.closest ? target.closest('.tree-toggle') : null;
    if (!toggle) return;
    event.preventDefault();
    toggleNode(toggle);
  });
})(typeof window !== 'undefined' ? window : globalThis);
