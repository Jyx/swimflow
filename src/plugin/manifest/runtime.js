// Highlights, hides and filters a manifest in an HTML bundle.
//
// Every row (`.manifest-row`), cell (`.manifest-cell`) and column header
// (`.manifest-column`) carries `data-row` / `data-col`.
//
// * Hovering a cell, a row label or a column header marks the row and the
//   column with `data-active` (the page CSS shades them).
// * The small cross next to a row label or on a column header
//   (`.manifest-toggle`) folds that row or column into a thin strip with a `+`
//   (`.manifest-expand`) that unfolds it again. The rest of the manifest moves up
//   or left to close the gap: the SVG records the original geometry
//   (`data-x`, `data-y`, `data-edge`, …) and `fold()` only moves, resizes and
//   hides elements, it does not lay the manifest out again.
// * A legend entry (`.manifest-legend-entry`) hides or shows every mark of its
//   kind; the entry is greyed out while its marks are hidden.
// * Clicking a column header (outside its cross) dims the rows that are not
//   members of the column; clicking a row label dims the columns the row is
//   not a member of; clicking a cell does both for its row and column.
//   Clicking it again shows everything.
// * **Show all** (`.manifest-show-all`) undoes every hide and the filter.
//
// The state lives on the section: `data-manifest-hidden` holds the tokens
// (`r3`, `c1`, `m:removed`) and `data-manifest-filter` the filtered column or row (`c2`, `r4`, `r4 c2`).
// The live preview sends both with a download, and the export leaves the
// hidden things out (`Diagram::apply_view`).
(function (global) {
  'use strict';

  function setActive(section, row, col) {
    var kinds = [['.manifest-row', 'data-row', row], ['.manifest-cell', 'data-col', col],
      ['.manifest-column', 'data-col', col]];
    for (var k = 0; k < kinds.length; k++) {
      var marked = section.querySelectorAll(kinds[k][0]);
      for (var i = 0; i < marked.length; i++) {
        if (kinds[k][2] && marked[i].getAttribute(kinds[k][1]) === kinds[k][2]) {
          marked[i].setAttribute('data-active', '1');
        } else {
          marked[i].removeAttribute('data-active');
        }
      }
    }
  }

  function mark(element, attribute, on) {
    if (on) element.setAttribute(attribute, '1');
    else element.removeAttribute(attribute);
  }

  // The hidden tokens. Before the first change they are the defaults the
  // diagram starts with (`data-hidden-default`, e.g. `m:removed`).
  function hiddenTokens(section) {
    var value = section.getAttribute('data-manifest-hidden');
    if (value === null) {
      var root = section.querySelector('.swimflow-manifest');
      value = root ? root.getAttribute('data-hidden-default') || '' : '';
    }
    return value.split(' ').filter(Boolean);
  }

  // `querySelectorAll` as a real array (a browser returns a NodeList).
  function all(scope, selector) {
    return Array.prototype.slice.call(scope.querySelectorAll(selector));
  }

  function num(element, name) {
    return parseFloat(element.getAttribute(name)) || 0;
  }

  function show(element, visible) {
    element.setAttribute('display', visible ? 'inline' : 'none');
  }

  // Fold the hidden rows and columns and move everything else to close the
  // gaps. Every position is computed from the recorded original geometry,
  // so folding is idempotent and unfolding restores the original exactly.
  function fold(section, has) {
    var root = section.querySelector('.swimflow-manifest');
    if (!root) return;
    var foldSize = num(root, 'data-fold');

    // Columns, left to right: the new x and width of each.
    var headers = all(section, '.manifest-column');
    headers.sort(function (a, b) { return num(a, 'data-x') - num(b, 'data-x'); });
    var x = {}, w = {}, dx = {};
    var cursor = num(root, 'data-cells-x');
    var original = cursor;
    for (var i = 0; i < headers.length; i++) {
      var col = headers[i].getAttribute('data-col');
      var cellW = headers[i].querySelector('rect') ? num(headers[i].querySelector('rect'), 'width') : 0;
      x[col] = cursor;
      w[col] = has(col) ? foldSize : cellW;
      dx[col] = cursor - num(headers[i], 'data-x');
      cursor += w[col];
      original += cellW;
    }
    var dxAll = cursor - original;
    var shiftX = function (el, d) { el.setAttribute('transform', d ? 'translate(' + d + ' 0)' : ''); };

    for (var h = 0; h < headers.length; h++) {
      var c = headers[h].getAttribute('data-col');
      shiftX(headers[h], dx[c]);
      show(headers[h], !has(c));
    }
    var folds = section.querySelectorAll('.manifest-column-fold');
    var cells = section.querySelectorAll('.manifest-cell');
    for (var k = 0; k < cells.length; k++) {
      var cc = cells[k].getAttribute('data-col');
      shiftX(cells[k], dx[cc] || 0);
      show(cells[k], !has(cc));
    }
    var counts = all(section, '.manifest-count').concat(all(section, '.manifest-count-header'));
    for (var n = 0; n < counts.length; n++) shiftX(counts[n], dxAll);

    // Rows, top to bottom: how far each band moves up.
    var rows = section.querySelectorAll('.manifest-row');
    var lost = [];
    for (var r = 0; r < rows.length; r++) {
      var hidden = has(rows[r].getAttribute('data-row'));
      show(rows[r].querySelector('.manifest-row-body'), !hidden);
      var stripe = rows[r].querySelector('.manifest-stripe');
      if (stripe) stripe.setAttribute('width', num(stripe, 'data-w') + dxAll);
      var rowFold = rows[r].querySelector('.manifest-row-fold');
      if (rowFold) {
        show(rowFold, hidden);
        var strip = rowFold.querySelector('rect');
        if (strip) strip.setAttribute('width', num(strip, 'data-w') + dxAll);
      }
      if (hidden) lost.push([num(rows[r], 'data-y'), num(rows[r], 'data-h') - foldSize]);
    }
    var upBy = function (y) {
      var d = 0;
      for (var q = 0; q < lost.length; q++) if (lost[q][0] < y) d += lost[q][1];
      return d;
    };
    var dyAll = upBy(Infinity);
    var bands = all(section, '.manifest-row')
      .concat(all(section, '.manifest-rowgroup'))
      .concat(all(section, '.manifest-legend'));
    for (var b = 0; b < bands.length; b++) {
      var dy = upBy(num(bands[b], 'data-y'));
      bands[b].setAttribute('transform', dy ? 'translate(0 ' + (-dy) + ')' : '');
    }

    // The folded columns' strips run down to the new bottom of the table.
    for (var f = 0; f < folds.length; f++) {
      var fc = folds[f].getAttribute('data-col');
      shiftX(folds[f], dx[fc] || 0);
      show(folds[f], has(fc));
      var column = folds[f].querySelector('rect');
      if (column) column.setAttribute('height', num(column, 'data-h') - dyAll);
    }

    // The outline and the vertical lines.
    var outline = section.querySelector('.manifest-outline');
    if (outline) {
      outline.setAttribute('width', num(outline, 'data-w') + dxAll);
      outline.setAttribute('height', num(outline, 'data-h') - dyAll);
    }
    var lines = section.querySelectorAll('line');
    for (var l = 0; l < lines.length; l++) {
      var edge = lines[l].getAttribute('data-edge');
      if (!edge) continue;
      var at = null;
      if (edge === 'end') at = cursor;
      else if (edge.indexOf('l:') === 0) at = x[edge.slice(2)];
      else if (edge.indexOf('r:') === 0) at = x[edge.slice(2)] + w[edge.slice(2)];
      if (at !== null && at !== undefined) {
        lines[l].setAttribute('x1', at);
        lines[l].setAttribute('x2', at);
      }
      lines[l].setAttribute('y2', num(lines[l], 'data-y2') - dyAll);
    }

    // Column groups stretch over their (possibly folded) columns.
    var groups = section.querySelectorAll('.manifest-group');
    for (var g = 0; g < groups.length; g++) {
      var members = (groups[g].getAttribute('data-cols') || '').split(' ').filter(function (m) { return m in x; });
      if (!members.length) continue;
      var x1 = x[members[0]];
      var last = members[members.length - 1];
      var x2 = x[last] + w[last];
      var band = groups[g].querySelector('rect');
      if (band) { band.setAttribute('x', x1); band.setAttribute('width', x2 - x1); }
      var label = groups[g].querySelector('.manifest-group-label');
      if (label) {
        var moved = (x1 + x2) / 2 - (num(groups[g], 'data-x') + num(groups[g], 'data-w') / 2);
        shiftX(label, moved);
        show(label, x2 - x1 >= num(groups[g], 'data-w') / 2);
      }
    }

    // The extends brackets follow their columns; a folded end hides them.
    var links = section.querySelectorAll('.manifest-link');
    for (var j = 0; j < links.length; j++) {
      var child = links[j].getAttribute('data-col');
      var parent = links[j].getAttribute('data-parent');
      var visible = !has(child) && !has(parent) && child in x && parent in x;
      show(links[j], visible);
      if (!visible) continue;
      var cx = x[child] + w[child] / 2, px = x[parent] + w[parent] / 2;
      var track = num(links[j], 'data-track'), bottom = num(links[j], 'data-bottom');
      var tip = bottom - 1, base = num(links[j], 'data-base');
      var path = links[j].querySelector('path');
      if (path) path.setAttribute('d', 'M' + cx + ' ' + bottom + ' L' + cx + ' ' + track +
        ' L' + px + ' ' + track + ' L' + px + ' ' + base);
      var arrow = links[j].querySelector('polygon');
      if (arrow) arrow.setAttribute('points', (px - 4) + ',' + base + ' ' + (px + 4) + ',' + base + ' ' + px + ',' + tip);
    }

    // The canvas shrinks with the folds (the page zoom watches the viewBox).
    var svg = root.closest ? root.closest('svg') : null;
    if (svg) {
      svg.setAttribute('viewBox', '0 0 ' + (num(root, 'data-width') + dxAll) + ' ' +
        (num(root, 'data-height') - dyAll));
    }
  }

  // Fold the hidden rows and columns and hide the hidden kinds of mark.
  function apply(section) {
    var tokens = hiddenTokens(section);
    var has = function (token) { return tokens.indexOf(token) >= 0; };
    fold(section, has);
    var cells = section.querySelectorAll('.manifest-cell');
    for (var k = 0; k < cells.length; k++) {
      mark(cells[k], 'data-mark-hidden', has('m:' + cells[k].getAttribute('data-mark')));
    }
    var entries = section.querySelectorAll('.manifest-legend-entry');
    for (var m = 0; m < entries.length; m++) mark(entries[m], 'data-hidden', has(entries[m].getAttribute('data-toggle')));
  }

  // Hide `token` if it is shown, show it if it is hidden.
  function toggleHidden(section, token) {
    var tokens = hiddenTokens(section);
    var at = tokens.indexOf(token);
    if (at >= 0) tokens.splice(at, 1);
    else tokens.push(token);
    section.setAttribute('data-manifest-hidden', tokens.join(' '));
    apply(section);
    return at < 0;
  }

  // Filter by a column (`cN`: dim the rows that are not its members), by a
  // row (`rN`: dim the columns the row is not a member of) or by both (a
  // cell, `rN cM`); `null` shows everything.
  function filter(section, token) {
    var parts = (token || '').split(' ');
    var col = null, row = null;
    for (var p = 0; p < parts.length; p++) {
      if (parts[p].charAt(0) === 'c') col = parts[p];
      else if (parts[p].charAt(0) === 'r') row = parts[p];
    }
    var rowCols = null;
    var rows = section.querySelectorAll('.manifest-row');
    for (var i = 0; i < rows.length; i++) {
      var cols = (rows[i].getAttribute('data-cols') || '').split(' ');
      if (row && rows[i].getAttribute('data-row') === row) rowCols = cols;
      var keep = !col || cols.indexOf(col) >= 0;
      if (keep) rows[i].removeAttribute('opacity');
      else rows[i].setAttribute('opacity', '0.2');
    }
    var dimmed = all(section, '.manifest-column').concat(all(section, '.manifest-cell'));
    for (var d = 0; d < dimmed.length; d++) {
      var inRow = !rowCols || rowCols.indexOf(dimmed[d].getAttribute('data-col')) >= 0;
      if (inRow) dimmed[d].removeAttribute('opacity');
      else dimmed[d].setAttribute('opacity', '0.2');
    }
    var headers = section.querySelectorAll('.manifest-column');
    for (var j = 0; j < headers.length; j++) {
      mark(headers[j], 'data-filter', col && headers[j].getAttribute('data-col') === col);
    }
    var labels = section.querySelectorAll('.manifest-row-label');
    for (var k = 0; k < labels.length; k++) {
      mark(labels[k], 'data-filter', row && labels[k].getAttribute('data-row') === row);
    }
    var cells = section.querySelectorAll('.manifest-cell');
    for (var m = 0; m < cells.length; m++) {
      mark(cells[m], 'data-filter', row && col && cells[m].getAttribute('data-row') === row &&
        cells[m].getAttribute('data-col') === col);
    }
    section.setAttribute('data-manifest-filter', token || '');
  }

  function toggleFilter(section, token) {
    var current = section.getAttribute('data-manifest-filter') || '';
    var next = current === token ? null : token;
    filter(section, next);
    return next;
  }

  function showAll(section) {
    section.setAttribute('data-manifest-hidden', '');
    apply(section);
    filter(section, null);
  }

  function hit(target) {
    if (!target || !target.closest) return null;
    return target.closest('.manifest-cell') || target.closest('.manifest-row-label') ||
      target.closest('.manifest-column');
  }

  // The controls live in the page bar, outside the section they act on.
  function sectionOf(element) {
    var section = element && element.closest ? element.closest('.swimflow-diagram') : null;
    if (section) return section;
    var slot = element && element.closest ? element.closest('.swimflow-toolbar-slot') : null;
    if (!slot) return null;
    var key = slot.getAttribute('data-diagram');
    return key && document.getElementById ? document.getElementById(key) : null;
  }

  if (typeof document !== 'undefined' && document.addEventListener) {
    document.addEventListener('mouseover', function (event) {
      var target = event.target;
      var section = target && target.closest ? target.closest('.swimflow-diagram') : null;
      if (!section || !section.querySelector('.swimflow-manifest')) return;
      var el = hit(target);
      setActive(section, el ? el.getAttribute('data-row') : null, el ? el.getAttribute('data-col') : null);
    });
    document.addEventListener('click', function (event) {
      var target = event.target;
      if (!target || !target.closest) return;
      var section = target.closest('.swimflow-diagram') || sectionOf(target);
      if (!section) return;
      if (target.closest('.manifest-show-all')) {
        event.preventDefault();
        showAll(section);
        return;
      }
      var control = target.closest('.manifest-toggle') || target.closest('.manifest-expand') ||
        target.closest('.manifest-legend-entry');
      if (control) {
        event.preventDefault();
        toggleHidden(section, control.getAttribute('data-toggle'));
        return;
      }
      var header = target.closest('.manifest-column');
      var label = target.closest('.manifest-row-label');
      var cell = target.closest('.manifest-cell');
      // A column or row with a link follows it; the others filter.
      if ((!header && !label && !cell) || target.closest('a')) return;
      event.preventDefault();
      toggleFilter(section, header ? header.getAttribute('data-col')
        : label ? label.getAttribute('data-row')
          : cell.getAttribute('data-row') + ' ' + cell.getAttribute('data-col'));
    });
  }

  global.swimflowManifestActive = setActive;
  global.swimflowManifestFilter = filter;
  global.swimflowManifestToggle = toggleFilter;
  global.swimflowManifestHide = toggleHidden;
  global.swimflowManifestShowAll = showAll;
})(typeof window !== 'undefined' ? window : globalThis);
