swimflow support for Neovim
===========================

Vimscript syntax, filetype detection and filetype settings for ``.diag``
files. No tree-sitter grammar or external dependency is required.

The filetype is named ``actdiag`` for compatibility with existing editor
setups; it covers the swimflow extensions as well.

What is included
----------------

.. list-table::
   :header-rows: 1

   * - Path
     - Purpose
   * - ``ftdetect/actdiag.vim``
     - maps ``*.diag`` and ``*.actdiag`` to ``actdiag``
   * - ``syntax/actdiag.vim``
     - keywords, attributes, constants, comments, strings, numbers, operators
   * - ``ftplugin/actdiag.vim``
     - ``commentstring=//%s``, comments, suffixes and ``iskeyword``
   * - ``lua/swimflow/init.lua``
     - optional ``require("swimflow").setup()`` filetype registration

Installation
------------

Manual / package directory
~~~~~~~~~~~~~~~~~~~~~~~~~~

Put this directory on Neovim's ``runtimepath``. Using a package is the
simplest option:

.. code-block:: console

   mkdir -p ~/.local/share/nvim/site/pack/swimflow/start
   ln -s /path/to/swimflow/editors/nvim \
     ~/.local/share/nvim/site/pack/swimflow/start/swimflow.nvim

Or append it from your ``init.lua`` before filetype detection runs:

.. code-block:: lua

   vim.opt.runtimepath:append("/path/to/swimflow/editors/nvim")

Then open an example:

.. code-block:: console

   nvim examples/zoom-demo.diag
   :set ft?            " actdiag
   :set commentstring? " //%s

lazy.nvim / NvChad
~~~~~~~~~~~~~~~~~~

Add this to ``lua/plugins/init.lua`` (NvChad) or any lazy.nvim plugin spec.
Keep it non-lazy so ``ftdetect`` and the filetype registration are available
before the first ``.diag`` file is opened:

.. code-block:: lua

   {
     dir = "/path/to/swimflow/editors/nvim",
     name = "swimflow.nvim",
     lazy = false,
     config = function()
       require("swimflow").setup()
     end,
   }

packer.nvim
~~~~~~~~~~~

.. code-block:: lua

   use({
     "/path/to/swimflow/editors/nvim",
     as = "swimflow.nvim",
     config = function()
       require("swimflow").setup()
     end,
   })

Highlight groups
----------------

The syntax file links to standard Vim groups, so it follows your colorscheme
by default:

.. list-table::
   :header-rows: 1

   * - Group
     - Standard link
   * - ``actdiagKeyword``
     - ``Keyword``
   * - ``actdiagAttribute``
     - ``Identifier``
   * - ``actdiagConstant``
     - ``Constant``
   * - ``actdiagOperator``
     - ``Operator``
   * - ``actdiagDelimiter``
     - ``Delimiter``
   * - ``actdiagNumber``
     - ``Number``
   * - ``actdiagString``, ``actdiagTripleString``
     - ``String``
   * - ``actdiagEscape``
     - ``SpecialChar``
   * - ``actdiagComment``
     - ``Comment``

Override any of them in your config, for example:

.. code-block:: lua

   vim.api.nvim_set_hl(0, "actdiagKeyword", { fg = "#c678dd", bold = true })

Verification
------------

The editor files are checked by ``tests/editor.rs``:

.. code-block:: console

   cargo test --test editor

When ``nvim`` is installed, the test opens every file in ``examples/``
headlessly, checks the filetype/commentstring and verifies representative
syntax groups in ``tests/fixtures/syntax_highlight.diag``. Without ``nvim`` it
still checks that the editor files are present and wired up.

Manual review checklist
-----------------------

This is a useful smoke test whenever the syntax file changes. Open the review
files in a real Neovim session (with the ``editors/nvim`` files on your
``runtimepath``, or the plugin installed) and check the colours:

.. code-block:: console

   $ nvim examples/hello.diag examples/deployment.diag examples/zoom-demo.diag

To exercise the local ``editors/nvim`` files without any user config, start
Neovim with ``--clean`` after adding the directory to ``runtimepath`` (see the
installation notes above).

* ``examples/hello.diag`` — keywords, comments, one edge.
* ``examples/deployment.diag`` — classes, legend, edge labels, numbers.
* ``examples/zoom-demo.diag`` — multiple diagrams and ``ref`` values.
* ``examples/unicode.diag`` — unicode ids and a multiline triple-quoted
  string.
* ``examples/cross-file/overview.diag`` — cross-file ``ref``.

Useful commands:

.. code-block:: vim

   :set ft?
   :set commentstring?
   :echo synIDattr(synID(line('.'), col('.'), 1), 'name')
   :hi actdiagKeyword
