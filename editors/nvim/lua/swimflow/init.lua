-- Minimal Neovim plugin wrapper for the swimflow activity filetype.
--
-- lazy.nvim and packer can use editors/nvim as the plugin directory directly;
-- the ftplugin/syntax/ftdetect files then work without calling setup().
-- This module is for users who prefer an explicit Lua call or want to register
-- the filetype with Neovim's newer vim.filetype API.

local M = {}

function M.setup()
  vim.filetype.add({
    extension = {
      diag = "actdiag",
      actdiag = "actdiag",
    },
  })
end

return M
