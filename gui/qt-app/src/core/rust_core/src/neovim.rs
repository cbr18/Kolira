//! A data-only snapshot with fixed, audited Lua glue. Never copy theme-supplied Lua.
use crate::{change_plan::Plan, palette::Palette, paths};
use anyhow::Result;
use std::path::Path;
pub const DATA: &str = ".config/nvim/kolira-palette.json";
pub const SCHEME: &str = ".config/nvim/colors/kolira.lua";
pub const LOADER: &str = ".config/nvim/after/plugin/kolira.lua";
const LUA: &str = r#"-- Kolira: fixed colourscheme; theme input is JSON data, never executable Lua.
local f = io.open(vim.fn.stdpath('config') .. '/kolira-palette.json', 'r')
if not f then return end
local ok, p = pcall(vim.json.decode, f:read('*a')); f:close()
if not ok or type(p.base) ~= 'table' or #p.base ~= 16 then return end
for _, c in ipairs(p.base) do
  if type(c) ~= 'string' or not c:match('^#%x%x%x%x%x%x$') then return end
end
local c = p.base
local terminal = p.terminal or c
if type(terminal) ~= 'table' or #terminal ~= 16 then return end
for _, v in ipairs(terminal) do if type(v) ~= 'string' or not v:match('^#%x%x%x%x%x%x$') then return end end
vim.cmd('highlight clear')
vim.o.background = p.variant == 'light' and 'light' or 'dark'
vim.g.colors_name = 'kolira'
local groups = {
  Normal={fg=c[6],bg=c[1]}, NormalFloat={fg=c[6],bg=c[2]},
  Comment={fg=c[4],italic=true}, String={fg=c[12]}, Character={fg=c[12]},
  Number={fg=c[10]}, Constant={fg=c[10]}, Boolean={fg=c[10]},
  Function={fg=c[14]}, Identifier={fg=c[9]}, Statement={fg=c[15]},
  Keyword={fg=c[15]}, Type={fg=c[11]}, PreProc={fg=c[11]}, Special={fg=c[13]},
  LineNr={fg=c[4]}, CursorLineNr={fg=c[6]}, CursorLine={bg=c[2]},
  Visual={bg=c[3]}, Search={fg=c[1],bg=c[11]},
  StatusLine={fg=c[6],bg=c[3]}, Pmenu={fg=c[6],bg=c[2]},
  PmenuSel={fg=c[1],bg=c[14]}, Error={fg=c[9]}, DiagnosticError={fg=c[9]},
  DiagnosticWarn={fg=c[11]}, DiagnosticInfo={fg=c[14]}, DiagnosticHint={fg=c[13]},
  ['@variable']={fg=c[6]}, ['@function']={fg=c[14]}, ['@keyword']={fg=c[15]},
  ['@string']={fg=c[12]}, ['@comment']={fg=c[4],italic=true}
}
for name, attrs in pairs(groups) do vim.api.nvim_set_hl(0, name, attrs) end
for i = 0, 15 do vim.g['terminal_color_' .. i] = terminal[i+1] end
"#;
pub fn plan(
    source: &Path,
    palette: Option<&Palette>,
    enabled: bool,
    plan: &mut Plan,
) -> Result<()> {
    let stored = source.join(DATA);
    let value: Palette = if stored.is_file() {
        serde_json::from_slice(&std::fs::read(&stored)?)?
    } else if enabled {
        let Some(p) = palette else { return Ok(()) };
        p.clone()
    } else {
        return Ok(());
    };
    value.validate()?;
    let mut data = serde_json::to_value(&value)?;
    data["terminal"] = serde_json::json!(crate::palette::ansi16(&value)
        .iter()
        .map(|c| c.hex())
        .collect::<Vec<_>>());
    if !paths::config_home().join("nvim").is_dir() && which::which("nvim").is_err() {
        return Ok(());
    }
    for (relative, text) in [
        (DATA, serde_json::to_string_pretty(&data)?),
        (SCHEME, LUA.into()),
        (
            LOADER,
            "-- Kolira colours only\npcall(vim.cmd.colorscheme, 'kolira')\n".into(),
        ),
    ] {
        crate::app_sync::write(plan, &paths::live_path(relative), text)?;
    }
    Ok(())
}
