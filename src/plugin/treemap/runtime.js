// Hides and shows the values of a treemap in an HTML bundle.
//
// The SVG contains one `.treemap-value` group per value (a `<text>` or, with
// `--text-as-paths`, one or more paths). The toolbar above the diagram has a
// `treemap-values-toggle` button. Toggling only changes the `display`
// attribute of those groups; the geometry and the zoom are untouched.
(function (global) {
  'use strict';

  // The toggle lives in the page bar, outside the section it acts on.
  function buttonOf(section) {
    if (!section) return null;
    var button = section.querySelector('.treemap-values-toggle');
    if (button) return button;
    if (typeof document === 'undefined' || !document.querySelectorAll) return null;
    var slots = document.querySelectorAll('.swimflow-toolbar-slot');
    var key = section.getAttribute('data-diagram');
    for (var i = 0; i < slots.length; i++) {
      if (slots[i].getAttribute('data-diagram') === key) {
        return slots[i].querySelector('.treemap-values-toggle');
      }
    }
    return null;
  }

  function setValues(section, visible, button) {
    if (!section) return;
    var values = section.querySelectorAll('.treemap-value');
    for (var i = 0; i < values.length; i++) {
      values[i].setAttribute('display', visible ? 'inline' : 'none');
    }
    button = button || buttonOf(section);
    if (button) {
      button.setAttribute('aria-pressed', visible ? 'true' : 'false');
      button.textContent = visible ? 'Hide values' : 'Show values';
    }
  }

  function valuesVisible(section, button) {
    button = button || buttonOf(section);
    return !button || button.getAttribute('aria-pressed') !== 'false';
  }

  function toggle(section, button) {
    var visible = !valuesVisible(section, button);
    setValues(section, visible, button);
    return visible;
  }

  // The toggle lives in the page bar, outside the section it acts on.
  function sectionOf(element) {
    var section = element && element.closest ? element.closest('.swimflow-diagram') : null;
    if (section) return section;
    var slot = element && element.closest ? element.closest('.swimflow-toolbar-slot') : null;
    if (!slot) return null;
    var key = slot.getAttribute('data-diagram');
    return key && document.getElementById ? document.getElementById(key) : null;
  }

  if (typeof document !== 'undefined' && document.addEventListener) {
    document.addEventListener('click', function (event) {
      var target = event.target;
      var button = target && target.closest ? target.closest('.treemap-values-toggle') : null;
      if (!button) return;
      event.preventDefault();
      var section = sectionOf(button);
      if (section) toggle(section, button);
    });
  }

  global.swimflowTreemapToggle = toggle;
  global.swimflowTreemapValues = setValues;
})(typeof window !== 'undefined' ? window : globalThis);
