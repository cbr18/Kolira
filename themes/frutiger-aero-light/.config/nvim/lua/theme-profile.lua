-- theme-profile: generated, edits are overwritten on the next palette apply
vim.o.termguicolors = true
pcall(vim.cmd.colorscheme, "theme-profile")
if true then
  for _, group in ipairs({ "Normal", "NormalNC", "SignColumn", "EndOfBuffer", "LineNr", "FoldColumn" }) do
    vim.api.nvim_set_hl(0, group, vim.tbl_extend("force", vim.api.nvim_get_hl(0, { name = group }), { bg = "NONE" }))
  end
end
