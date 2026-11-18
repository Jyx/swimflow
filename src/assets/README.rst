assets
======

``default.ttf`` — VL Gothic Regular
------------------------------------

The embedded default font is **VL Gothic Regular**, taken from the upstream
reference distribution (``tests/VLGothic/VL-Gothic-Regular.ttf``). It is the
same family the reference test suite uses, so text shaping is a close match
while staying self-contained (no system fonts, no ``fontconfig``).

VL Gothic is distributed under a permissive license derived from M+ FONTS and
DejaVu. See the accompanying ``VL-Gothic-LICENSE*`` files:

* ``VL-Gothic-LICENSE`` — family overview (Japanese)
* ``VL-Gothic-LICENSE.en`` — family overview (English)
* ``VL-Gothic-LICENSE_J.mplus`` — M+ FONTS terms
* ``VL-Gothic-LICENSE_E.mplus`` — M+ FONTS terms (English)

The font is loaded by ``src/text.rs`` and used only by the renderer; it does
not affect layout.
