//! Neovim the way dotfiles do it (matugen/base16): the palette generates a
//! colour scheme `colors/theme-profile.lua` (base16 → highlight groups, no
//! plugins), the theme file `lua/theme-profile.lua` selects it (and makes the
//! background transparent when the theme's terminal is), and the user's
//! `init.lua` only gains `pcall(require, "theme-profile")` as its last line.

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use anyhow::Result;

use crate::palette::{ansi16, Palette};
use crate::paths::config_home;

pub const HOST: &str = ".config/nvim/init.lua";
pub const THEME: &str = ".config/nvim/lua/theme-profile.lua";
pub const COLORS: &str = ".config/nvim/colors/theme-profile.lua";
pub const REQUIRE: &str = "pcall(require, \"theme-profile\")";
const NOTE: &str = "-- Theme (theme-profile): colours live in lua/theme-profile.lua";

/// The user's init.lua with the theme loaded last (idempotent).
pub fn host(text: &str) -> String {
    let kept: Vec<&str> = text.lines().filter(|l| l.trim() != REQUIRE && l.trim() != NOTE).collect();
    let body = kept.join("\n");
    let body = body.trim_end();
    if body.is_empty() {
        format!("{REQUIRE}\n")
    } else {
        format!("{body}\n\n{NOTE}\n{REQUIRE}\n")
    }
}

/// `lua/theme-profile.lua`: pick the generated scheme, optionally transparent.
pub fn theme_file(transparent: bool) -> String {
    format!(
        "-- theme-profile: generated, edits are overwritten on the next palette apply\n\
         vim.o.termguicolors = true\n\
         pcall(vim.cmd.colorscheme, \"theme-profile\")\n\
         if {transparent} then\n\
         \x20 for _, group in ipairs({{ \"Normal\", \"NormalNC\", \"SignColumn\", \"EndOfBuffer\", \"LineNr\", \"FoldColumn\" }}) do\n\
         \x20   vim.api.nvim_set_hl(0, group, vim.tbl_extend(\"force\", vim.api.nvim_get_hl(0, {{ name = group }}), {{ bg = \"NONE\" }}))\n\
         \x20 end\n\
         end\n"
    )
}

/// A base16 colour scheme for Neovim (the classic base16-vim mapping plus
/// diagnostics, floats and terminal colours; tree-sitter groups link to these).
pub fn colors(p: &Palette) -> String {
    let c = |i: usize| p.slot(i).hex();
    let mut out = String::new();
    let _ = writeln!(out, "-- theme-profile: base16 colour scheme \"{}\" (generated)", p.name.replace('"', "'"));
    out.push_str("vim.cmd(\"highlight clear\")\nif vim.fn.exists(\"syntax_on\") == 1 then vim.cmd(\"syntax reset\") end\n");
    let _ = writeln!(out, "vim.o.background = \"{}\"", if p.slot(0).luminance() < 0.5 { "dark" } else { "light" });
    out.push_str("vim.g.colors_name = \"theme-profile\"\nlocal hl = function(g, o) vim.api.nvim_set_hl(0, g, o) end\n");
    let mut hl = |group: &str, fg: Option<usize>, bg: Option<usize>, extra: &str| {
        let mut parts = Vec::new();
        if let Some(f) = fg {
            parts.push(format!("fg = \"{}\"", c(f)));
        }
        if let Some(b) = bg {
            parts.push(format!("bg = \"{}\"", c(b)));
        }
        if !extra.is_empty() {
            parts.push(extra.to_string());
        }
        let _ = writeln!(out, "hl(\"{group}\", {{ {} }})", parts.join(", "));
    };
    // Editor UI
    hl("Normal", Some(0x05), Some(0x00), "");
    hl("NormalNC", Some(0x05), Some(0x00), "");
    hl("NormalFloat", Some(0x05), Some(0x01), "");
    hl("FloatBorder", Some(0x03), Some(0x01), "");
    hl("Cursor", Some(0x00), Some(0x05), "");
    hl("CursorLine", None, Some(0x01), "");
    hl("CursorColumn", None, Some(0x01), "");
    hl("CursorLineNr", Some(0x0D), Some(0x01), "bold = true");
    hl("LineNr", Some(0x03), Some(0x00), "");
    hl("SignColumn", Some(0x03), Some(0x00), "");
    hl("FoldColumn", Some(0x03), Some(0x00), "");
    hl("Folded", Some(0x03), Some(0x01), "");
    hl("ColorColumn", None, Some(0x01), "");
    hl("Visual", None, Some(0x02), "");
    hl("Search", Some(0x01), Some(0x0A), "");
    hl("IncSearch", Some(0x01), Some(0x09), "");
    hl("CurSearch", Some(0x01), Some(0x09), "");
    hl("MatchParen", None, Some(0x03), "bold = true");
    hl("Pmenu", Some(0x05), Some(0x01), "");
    hl("PmenuSel", Some(0x00), Some(0x0D), "");
    hl("PmenuSbar", None, Some(0x02), "");
    hl("PmenuThumb", None, Some(0x04), "");
    hl("StatusLine", Some(0x05), Some(0x02), "");
    hl("StatusLineNC", Some(0x03), Some(0x01), "");
    hl("TabLine", Some(0x03), Some(0x01), "");
    hl("TabLineSel", Some(0x0B), Some(0x01), "");
    hl("TabLineFill", Some(0x03), Some(0x01), "");
    hl("WinSeparator", Some(0x02), None, "");
    hl("VertSplit", Some(0x02), None, "");
    hl("NonText", Some(0x03), None, "");
    hl("Whitespace", Some(0x02), None, "");
    hl("EndOfBuffer", Some(0x00), None, "");
    hl("Title", Some(0x0D), None, "bold = true");
    hl("Directory", Some(0x0D), None, "");
    hl("Question", Some(0x0D), None, "");
    hl("MoreMsg", Some(0x0B), None, "");
    hl("ModeMsg", Some(0x0B), None, "");
    hl("ErrorMsg", Some(0x08), None, "");
    hl("WarningMsg", Some(0x08), None, "");
    hl("SpecialKey", Some(0x03), None, "");
    hl("Conceal", Some(0x0D), None, "");
    hl("WildMenu", Some(0x08), Some(0x0A), "");
    // Syntax
    hl("Comment", Some(0x03), None, "italic = true");
    hl("Constant", Some(0x09), None, "");
    hl("String", Some(0x0B), None, "");
    hl("Character", Some(0x08), None, "");
    hl("Number", Some(0x09), None, "");
    hl("Boolean", Some(0x09), None, "");
    hl("Float", Some(0x09), None, "");
    hl("Identifier", Some(0x08), None, "");
    hl("Function", Some(0x0D), None, "");
    hl("Statement", Some(0x08), None, "");
    hl("Conditional", Some(0x0E), None, "");
    hl("Repeat", Some(0x0A), None, "");
    hl("Label", Some(0x0A), None, "");
    hl("Operator", Some(0x05), None, "");
    hl("Keyword", Some(0x0E), None, "");
    hl("Exception", Some(0x08), None, "");
    hl("PreProc", Some(0x0A), None, "");
    hl("Include", Some(0x0D), None, "");
    hl("Define", Some(0x0E), None, "");
    hl("Macro", Some(0x08), None, "");
    hl("Type", Some(0x0A), None, "");
    hl("StorageClass", Some(0x0A), None, "");
    hl("Structure", Some(0x0E), None, "");
    hl("Typedef", Some(0x0A), None, "");
    hl("Special", Some(0x0C), None, "");
    hl("SpecialChar", Some(0x0F), None, "");
    hl("Tag", Some(0x0A), None, "");
    hl("Delimiter", Some(0x0F), None, "");
    hl("SpecialComment", Some(0x0C), None, "");
    hl("Debug", Some(0x08), None, "");
    hl("Underlined", Some(0x08), None, "underline = true");
    hl("Error", Some(0x00), Some(0x08), "");
    hl("Todo", Some(0x0A), Some(0x01), "");
    // Tree-sitter: variables in the text colour (base16-nvim does the same).
    hl("@variable", Some(0x05), None, "");
    hl("@property", Some(0x05), None, "");
    hl("@field", Some(0x05), None, "");
    hl("@parameter", Some(0x05), None, "");
    // Diagnostics and diffs
    hl("DiagnosticError", Some(0x08), None, "");
    hl("DiagnosticWarn", Some(0x0A), None, "");
    hl("DiagnosticInfo", Some(0x0D), None, "");
    hl("DiagnosticHint", Some(0x0C), None, "");
    hl("DiagnosticUnderlineError", None, None, &format!("undercurl = true, sp = \"{}\"", c(0x08)));
    hl("DiagnosticUnderlineWarn", None, None, &format!("undercurl = true, sp = \"{}\"", c(0x0A)));
    hl("DiffAdd", Some(0x0B), Some(0x01), "");
    hl("DiffChange", Some(0x03), Some(0x01), "");
    hl("DiffDelete", Some(0x08), Some(0x01), "");
    hl("DiffText", Some(0x0D), Some(0x01), "");
    hl("SpellBad", None, None, &format!("undercurl = true, sp = \"{}\"", c(0x08)));
    for (i, color) in ansi16(p).iter().enumerate() {
        let _ = writeln!(out, "vim.g.terminal_color_{i} = \"{}\"", color.hex());
    }
    out
}

fn nvim_dir() -> PathBuf {
    config_home().join("nvim")
}

/// Is the theme's terminal translucent (kitty / Alacritty opacity < 1)?
fn terminal_translucent() -> bool {
    let kitty = fs::read_to_string(crate::paths::live_path(crate::appearance::KITTY_THEME)).unwrap_or_default();
    kitty
        .lines()
        .find_map(|l| l.trim().strip_prefix("background_opacity").map(|v| v.trim().parse::<f64>().unwrap_or(1.0)))
        .is_some_and(|o| o < 0.999)
}

/// Palette target `nvim`: write the scheme and the theme file; make
/// init.lua load it. Skipped (Ok(None)) when Neovim is not set up.
pub fn apply_palette(p: &Palette) -> Result<Option<PathBuf>> {
    let dir = nvim_dir();
    if which::which("nvim").is_err() && !dir.exists() {
        return Ok(None);
    }
    fs::create_dir_all(dir.join("colors"))?;
    fs::create_dir_all(dir.join("lua"))?;
    let colors = dir.join("colors/theme-profile.lua");
    fs::write(&colors, self::colors(p))?;
    fs::write(dir.join("lua/theme-profile.lua"), theme_file(terminal_translucent()))?;
    let init = dir.join("init.lua");
    if !dir.join("init.vim").exists() {
        let text = fs::read_to_string(&init).unwrap_or_default();
        fs::write(&init, host(&text))?;
    }
    Ok(Some(colors))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_adds_one_require_last() {
        let h = host("vim.opt.number = true\n");
        assert!(h.trim_end().ends_with(REQUIRE) && h.contains("vim.opt.number"));
        assert_eq!(host(&h), h, "idempotent");
        assert_eq!(host(""), format!("{REQUIRE}\n"));
    }

    #[test]
    fn colors_is_a_complete_scheme() {
        let p = crate::palette::parse_base16(
            "scheme: \"T\"\nbase00: \"282a36\"\nbase01: \"21222c\"\nbase02: \"44475a\"\nbase03: \"6272a4\"\nbase04: \"9ea8c7\"\nbase05: \"f8f8f2\"\nbase06: \"f8f8f2\"\nbase07: \"ffffff\"\nbase08: \"ff5555\"\nbase09: \"ffb86c\"\nbase0A: \"f1fa8c\"\nbase0B: \"50fa7b\"\nbase0C: \"8be9fd\"\nbase0D: \"bd93f9\"\nbase0E: \"ff79c6\"\nbase0F: \"993333\"\n",
            "test",
        )
        .unwrap();
        let s = colors(&p);
        assert!(s.contains("vim.g.colors_name = \"theme-profile\""));
        assert!(s.contains("hl(\"Normal\", { fg = \"#f8f8f2\", bg = \"#282a36\" })"));
        assert!(s.contains("vim.o.background = \"dark\""));
        assert!(s.contains("vim.g.terminal_color_15"));
        assert!(theme_file(true).contains("bg = \"NONE\""));
    }
}
