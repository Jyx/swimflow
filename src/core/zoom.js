// Zoom for the diagram of an interactive page: the buttons of the bar, the
// keys + - 0 1 and Ctrl/Cmd + mouse wheel (a trackpad pinch arrives as one).
// A diagram opens fitted to the window. The size comes from the `viewBox` of
// the SVG, so a tree that is laid out again after a fold is scaled again too.
// The zoom is a view setting of the page: nothing here reaches an export.
(function () {
  'use strict';
  var container = document.getElementById('swimflow');
  var group = container ? container.querySelector('.swimflow-zoom') : null;
  if (!group) return;

  var MIN = 0.05;
  var MAX = 8;
  var STEP = 1.25;
  var PAD = 32; // the padding of the stage on both sides
  var label = group.querySelector('.swimflow-zoom-level');
  var states = {};
  var observer = null;

  function activeSection() {
    return document.querySelector('.swimflow-diagram.is-active');
  }

  function svgOf(section) {
    return section ? section.querySelector('svg') : null;
  }

  function naturalSize(svg) {
    var box = (svg.getAttribute('viewBox') || '').split(/[\s,]+/).map(Number);
    var width = box.length === 4 ? box[2] : parseFloat(svg.getAttribute('width'));
    var height = box.length === 4 ? box[3] : parseFloat(svg.getAttribute('height'));
    return width > 0 && height > 0 ? { w: width, h: height } : null;
  }

  function stateOf(section) {
    var key = section.getAttribute('data-diagram') || '';
    if (!states[key]) states[key] = { fit: true, scale: 1 };
    return states[key];
  }

  // The scale at which the whole diagram is visible, never above 100%.
  function fitScale(section, size) {
    var bar = container.querySelector('.swimflow-bar');
    var width = document.documentElement.clientWidth - PAD;
    // The section controls live in the bar, so its height already covers them.
    var height = window.innerHeight - (bar ? bar.offsetHeight : 0) - PAD;
    return Math.max(MIN, Math.min(1, width / size.w, height / size.h));
  }

  function scaleOf(section, size) {
    var state = stateOf(section);
    return state.fit ? fitScale(section, size) : state.scale;
  }

  function watch(svg) {
    if (!observer || svg.__swimflowZoom) return;
    svg.__swimflowZoom = true;
    observer.observe(svg, { attributes: true, attributeFilter: ['viewBox'] });
  }

  function apply(section) {
    var svg = svgOf(section);
    var size = svg ? naturalSize(svg) : null;
    if (!size) return;
    var scale = scaleOf(section, size);
    svg.style.width = size.w * scale + 'px';
    svg.style.height = size.h * scale + 'px';
    if (label) label.textContent = Math.round(scale * 100) + '%';
    watch(svg);
  }

  function refresh() {
    var section = activeSection();
    if (section) apply(section);
  }

  // Zoom to `scale`; the point (x, y) of the window, when given, stays where
  // it is on the screen.
  function setScale(section, scale, x, y) {
    var svg = svgOf(section);
    if (!svg) return;
    var before = svg.getBoundingClientRect();
    var state = stateOf(section);
    state.fit = false;
    state.scale = Math.min(MAX, Math.max(MIN, scale));
    apply(section);
    if (typeof x === 'number' && before.width > 0 && before.height > 0) {
      var after = svg.getBoundingClientRect();
      window.scrollBy(
        after.left + ((x - before.left) / before.width) * after.width - x,
        after.top + ((y - before.top) / before.height) * after.height - y
      );
    }
  }

  function currentScale(section) {
    var svg = svgOf(section);
    var size = svg ? naturalSize(svg) : null;
    return size ? scaleOf(section, size) : 1;
  }

  function zoomBy(section, factor, x, y) {
    setScale(section, currentScale(section) * factor, x, y);
  }

  function fit(section) {
    stateOf(section).fit = true;
    apply(section);
  }

  function run(action) {
    var section = activeSection();
    if (!section) return;
    if (action === 'in') zoomBy(section, STEP);
    else if (action === 'out') zoomBy(section, 1 / STEP);
    else if (action === 'reset') setScale(section, 1);
    else if (action === 'fit') fit(section);
  }

  if (typeof MutationObserver !== 'undefined') {
    observer = new MutationObserver(refresh);
  }

  group.addEventListener('click', function (event) {
    var button = event.target && event.target.closest ? event.target.closest('[data-zoom]') : null;
    if (!button) return;
    event.preventDefault();
    run(button.getAttribute('data-zoom'));
  });

  document.addEventListener('wheel', function (event) {
    if (!event.ctrlKey && !event.metaKey) return;
    var section = activeSection();
    if (!section) return;
    event.preventDefault();
    var delta = event.deltaMode === 1 ? event.deltaY * 16 : event.deltaY;
    delta = Math.max(-60, Math.min(60, delta));
    zoomBy(section, Math.exp(-delta * 0.005), event.clientX, event.clientY);
  }, { passive: false });

  document.addEventListener('keydown', function (event) {
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    var target = event.target;
    var tag = target && target.tagName ? String(target.tagName).toLowerCase() : '';
    if (tag === 'input' || tag === 'textarea' || tag === 'select' || (target && target.isContentEditable)) return;
    var action = { '+': 'in', '=': 'in', '-': 'out', '0': 'fit', '1': 'reset' }[event.key];
    if (!action) return;
    event.preventDefault();
    run(action);
  });

  // The navigation script announces the diagram it has just made visible.
  document.addEventListener('swimflow:show', refresh);
  window.addEventListener('resize', function () {
    var section = activeSection();
    if (section && stateOf(section).fit) apply(section);
  });

  refresh();
})();
