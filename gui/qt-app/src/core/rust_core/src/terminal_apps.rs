//! Second installed-app batch: native colour resources and minimal selectors.
//! No sessions are killed, no prompt/plugins enabled, no AI credentials copied.
use crate::{
    app_sync,
    change_plan::Plan,
    field, native_appearance,
    palette::{Palette, Rgb, Variant},
    paths,
    theme::Package,
};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path};
pub const TARGETS: &[&str] = &[
    "fish",
    "pure",
    "oh-my-posh",
    "btop",
    "yazi",
    "bat",
    "wofi",
    "fzf",
    "lazygit",
    "zellij",
    "opencode",
    "claude-code",
    "codex",
];
pub const FZF_DATA: &str = ".config/fzf/kolira.opts";
pub const FZF_LOADER: &str = ".config/fish/conf.d/kolira-fzf.fish";
pub const WOFI_HOST: &str = ".config/wofi/style.css";
pub const WOFI_COLORS: &str = ".config/wofi/kolira.css";
const FZF_GLUE: &str = r#"# Kolira colour data only; personal options and commands are not evaluated.
if status is-interactive
    set -l kolira_colors_file "$__fish_config_dir/../fzf/kolira.opts"
    if test -f "$kolira_colors_file"
        set -l kolira_colors (string collect < "$kolira_colors_file")
        if not set -q _kolira_fzf_before
            set -g _kolira_fzf_before "$FZF_DEFAULT_OPTS"
        end
        if not set -q _kolira_fzf_after; or test "$FZF_DEFAULT_OPTS" = "$_kolira_fzf_after"
            set -gx FZF_DEFAULT_OPTS "$_kolira_fzf_before $kolira_colors"
            set -g _kolira_fzf_after "$FZF_DEFAULT_OPTS"
        end
    end
end
"#;
pub fn paths_for(id: &str) -> Vec<String> {
    let list: &[&str] = match id {
        "pure" => &[crate::extras::FISH_VARS],
        "fzf" => &[FZF_DATA],
        "lazygit" => &[".config/lazygit/config.yml"],
        "zellij" => &[
            ".config/zellij/config.kdl",
            ".config/zellij/themes/kolira.kdl",
        ],
        "opencode" => &[".config/opencode/cli.json"],
        "claude-code" => &[".claude/settings.json"],
        "codex" => &[".codex/config.toml", ".codex/themes/kolira.tmTheme"],
        _ => &[],
    };
    list.iter().map(|s| s.to_string()).collect()
}
fn present(app: &str, dir: &str) -> bool {
    paths::live_path(dir).exists() || which::which(app).is_ok()
}
fn write(plan: &mut Plan, rel: &str, text: String) -> Result<()> {
    if rel == FZF_LOADER {
        if let Some(old) = plan.text(&paths::live_path(rel))? {
            anyhow::ensure!(old==FZF_GLUE,"Existing kolira-fzf.fish differs from the fixed loader; refusing to replace personal shell code");
        }
    }
    app_sync::write(plan, &paths::live_path(rel), text)
}
fn kv(text: &str, key: &str, value: &str) -> String {
    let mut lines = Vec::new();
    let mut done = false;
    for line in text.lines() {
        if line
            .trim()
            .split_once('=')
            .is_some_and(|(k, _)| k.trim() == key)
        {
            if !done {
                lines.push(format!("{key} = {value}"));
                done = true;
            }
        } else {
            lines.push(line.to_string());
        }
    }
    if !done {
        lines.push(format!("{key} = {value}"));
    }
    lines.join("\n") + "\n"
}
pub fn tmtheme(p: &Palette) -> String {
    let mut entries=format!("<dict><key>settings</key><dict><key>foreground</key><string>{}</string><key>background</key><string>{}</string><key>caret</key><string>{}</string><key>selection</key><string>{}</string></dict></dict>",p.base[5],p.base[0],p.base[5],p.base[2]);
    for (scope, color) in [
        ("comment", 3),
        ("string", 11),
        ("keyword, storage", 14),
        ("constant.numeric", 9),
        ("entity.name.function", 13),
        ("entity.name.type", 10),
    ] {
        entries.push_str(&format!("<dict><key>scope</key><string>{scope}</string><key>settings</key><dict><key>foreground</key><string>{}</string></dict></dict>",p.base[color]));
    }
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\"><dict><key>name</key><string>Kolira</string><key>settings</key><array>{entries}</array></dict></plist>\n")
}
pub fn btop(p: &Palette) -> String {
    let mut out = String::from("# Kolira native btop colours\n");
    for (key, slot) in [
        ("main_bg", 0),
        ("main_fg", 5),
        ("title", 5),
        ("hi_fg", 13),
        ("selected_bg", 2),
        ("selected_fg", 5),
        ("inactive_fg", 3),
        ("graph_text", 5),
        ("meter_bg", 2),
        ("proc_misc", 14),
        ("cpu_box", 13),
        ("mem_box", 11),
        ("net_box", 12),
        ("proc_box", 14),
        ("div_line", 3),
    ] {
        out.push_str(&format!("theme[{key}]=\"{}\"\n", p.base[slot]));
    }
    for (key, a, b) in [
        ("temp", 11, 8),
        ("cpu", 13, 14),
        ("free", 11, 12),
        ("cached", 13, 12),
        ("available", 11, 10),
        ("used", 10, 8),
        ("download", 13, 12),
        ("upload", 14, 8),
        ("process", 13, 14),
    ] {
        for (suffix, color) in [
            ("start", p.slot(a)),
            ("mid", p.slot(a).mix(p.slot(b), 0.5)),
            ("end", p.slot(b)),
        ] {
            out.push_str(&format!("theme[{key}_{suffix}]=\"{}\"\n", color.hex()));
        }
    }
    out
}
pub fn fish(p: &Palette, pure: bool) -> String {
    let mut out = String::new();
    let rows = if pure {
        vec![
            ("pure_color_primary", 13),
            ("pure_color_info", 12),
            ("pure_color_mute", 3),
            ("pure_color_success", 11),
            ("pure_color_normal", 5),
            ("pure_color_danger", 8),
            ("pure_color_light", 7),
            ("pure_color_warning", 10),
            ("pure_color_dark", 0),
        ]
    } else {
        vec![
            ("fish_color_normal", 5),
            ("fish_color_command", 13),
            ("fish_color_quote", 11),
            ("fish_color_redirection", 12),
            ("fish_color_end", 14),
            ("fish_color_error", 8),
            ("fish_color_param", 5),
            ("fish_color_comment", 3),
            ("fish_color_match", 10),
            ("fish_color_search_match", 10),
            ("fish_color_operator", 14),
            ("fish_color_escape", 12),
            ("fish_color_autosuggestion", 3),
            ("fish_pager_color_prefix", 13),
            ("fish_pager_color_completion", 5),
            ("fish_pager_color_description", 3),
            ("fish_pager_color_progress", 12),
        ]
    };
    for (key, slot) in rows {
        out.push_str(&format!(
            "SETUVAR {key}:{}\n",
            p.base[slot].trim_start_matches('#')
        ));
    }
    out
}
pub fn yazi(p: &Palette) -> String {
    format!("[mgr]\ncwd = {{ fg = \"{}\" }}\nhovered = {{ fg = \"{}\", bg = \"{}\" }}\npreview_hovered = {{ underline = true }}\nfind_keyword = {{ fg = \"{}\", bold = true }}\nmarker_copied = {{ fg = \"{}\" }}\nmarker_cut = {{ fg = \"{}\" }}\nmarker_selected = {{ fg = \"{}\" }}\n[status]\noverall = {{ fg = \"{}\", bg = \"{}\" }}\n[mode]\nnormal_main = {{ fg = \"{}\", bg = \"{}\", bold = true }}\nnormal_alt = {{ fg = \"{}\", bg = \"{}\" }}\nselect_main = {{ fg = \"{}\", bg = \"{}\" }}\n[input]\nborder = {{ fg = \"{}\" }}\nvalue = {{ fg = \"{}\" }}\n",p.base[13],p.base[5],p.base[2],p.base[10],p.base[11],p.base[8],p.base[14],p.base[5],p.base[1],p.base[0],p.base[13],p.base[5],p.base[2],p.base[0],p.base[14],p.base[13],p.base[5])
}
pub fn lazygit(p: &Palette) -> String {
    let mut out = String::from("gui:\n  theme:\n");
    for (key, color) in [
        ("activeBorderColor", p.accent.as_str()),
        ("inactiveBorderColor", p.base[3].as_str()),
        ("searchingActiveBorderColor", p.base[10].as_str()),
        ("optionsTextColor", p.base[13].as_str()),
        ("selectedLineFgColor", p.base[5].as_str()),
        ("selectedLineBgColor", p.base[2].as_str()),
        ("unstagedChangesColor", p.base[8].as_str()),
        ("defaultFgColor", p.base[5].as_str()),
    ] {
        out.push_str(&format!("    {key}: [\"{color}\"]\n"));
    }
    out
}
pub fn zellij(p: &Palette) -> String {
    let mut out = String::from("themes {\n    kolira {\n");
    for key in [
        "text_unselected",
        "text_selected",
        "ribbon_unselected",
        "ribbon_selected",
        "ribbon_hover",
        "table_title",
        "table_cell_unselected",
        "table_cell_selected",
        "list_unselected",
        "list_selected",
        "frame_unselected",
        "frame_selected",
        "frame_highlight",
        "exit_code_success",
        "exit_code_error",
    ] {
        let bg = if key.contains("selected") && !key.contains("unselected") {
            &p.base[2]
        } else {
            &p.base[0]
        };
        out.push_str(&format!("        {key} {{\n            base \"{}\"\n            background \"{bg}\"\n            emphasis_0 \"{}\"\n            emphasis_1 \"{}\"\n            emphasis_2 \"{}\"\n            emphasis_3 \"{}\"\n        }}\n",p.base[5],p.base[13],p.base[11],p.base[10],p.base[14]));
    }
    out.push_str("    }\n}\n");
    out
}
pub fn wofi(p: &Palette) -> String {
    format!("/* Kolira colours only */\nwindow {{ background-color: {}; color: {}; }}\n#input {{ background-color: {}; color: {}; border-color: {}; }}\n#entry:selected {{ background-color: {}; }}\n#text:selected {{ color: {}; }}\n",p.base[0],p.base[5],p.base[1],p.base[5],p.accent,p.base[2],p.base[5])
}
pub fn valid_wofi(text: &str) -> Result<String> {
    let bytes = text.as_bytes();
    let mut colours = Vec::new();
    for (i, c) in bytes.iter().enumerate() {
        if *c == b'#'
            && i + 7 <= bytes.len()
            && bytes[i + 1..i + 7].iter().all(u8::is_ascii_hexdigit)
        {
            anyhow::ensure!(
                !bytes.get(i + 7).is_some_and(u8::is_ascii_hexdigit),
                "Invalid Wofi hex colour"
            );
            colours.push(Rgb::parse(&text[i..i + 7])?.hex());
        }
    }
    anyhow::ensure!(
        colours.len() == 7,
        "Owned Wofi CSS must contain seven controlled colour values"
    );
    let mut base = vec![colours[0].clone(); 16];
    base[5] = colours[1].clone();
    base[1] = colours[2].clone();
    base[2] = colours[5].clone();
    let p = Palette {
        name: "wofi".into(),
        author: String::new(),
        variant: Variant::Dark,
        base,
        accent: colours[4].clone(),
        source: String::new(),
    };
    Ok(wofi(&p))
}

pub fn fzf(p: &Palette) -> String {
    format!("--color=fg:{},bg:{},hl:{},fg+:{},bg+:{},hl+:{},info:{},prompt:{},pointer:{},marker:{},spinner:{},header:{}\n",p.base[5],p.base[0],p.base[13],p.base[5],p.base[2],p.base[13],p.base[12],p.accent,p.accent,p.base[11],p.base[12],p.base[3])
}
pub fn valid_fzf(text: &str) -> Result<String> {
    let value = text
        .trim()
        .strip_prefix("--color=")
        .context("Fzf theme must contain only a --color option")?;
    anyhow::ensure!(!value.is_empty(), "Empty fzf colours");
    for part in value.split(',') {
        let (name, color) = part.split_once(':').context("Invalid fzf colour")?;
        anyhow::ensure!(
            [
                "fg",
                "bg",
                "hl",
                "fg+",
                "bg+",
                "hl+",
                "info",
                "prompt",
                "pointer",
                "marker",
                "spinner",
                "header",
                "border",
                "label",
                "gutter",
                "separator"
            ]
            .contains(&name),
            "Unsupported fzf colour role"
        );
        Rgb::parse(color)?;
    }
    Ok(format!("--color={value}\n"))
}
pub fn omp(p: &Palette, live: &str) -> Result<String> {
    let mut value: Value = serde_json::from_str(live).context("Invalid Oh My Posh theme")?;
    let map = crate::kvantum::Recolor::new(Rgb(0, 0, 0), Rgb(255, 255, 255), Rgb(0, 122, 204), p);
    fn recolor(value: &mut Value, map: &crate::kvantum::Recolor, p: &Palette) {
        match value {
            Value::Object(o) => {
                for (key, v) in o.iter_mut() {
                    if key == "foreground"
                        || key == "background"
                        || key == "palette"
                        || key.ends_with("_templates")
                        || key == "template"
                    {
                        colours(v, map, p);
                    } else {
                        recolor(v, map, p);
                    }
                }
                if let (Some(bg), Some(fg)) = (
                    o.get("background")
                        .and_then(Value::as_str)
                        .and_then(|s| Rgb::parse(s).ok()),
                    o.get("foreground")
                        .and_then(Value::as_str)
                        .and_then(|s| Rgb::parse(s).ok()),
                ) {
                    if crate::palette::contrast(bg, fg) < 3.0 {
                        let white = Rgb(255, 255, 255);
                        let black = Rgb(0, 0, 0);
                        o.insert(
                            "foreground".into(),
                            json!(if crate::palette::contrast(bg, white)
                                > crate::palette::contrast(bg, black)
                            {
                                white.hex()
                            } else {
                                black.hex()
                            }),
                        );
                    }
                }
            }
            Value::Array(a) => {
                for v in a {
                    recolor(v, map, p)
                }
            }
            _ => {}
        }
    }
    fn project(old: Rgb, map: &crate::kvantum::Recolor, p: &Palette) -> Rgb {
        let candidates: Vec<_> = p
            .base
            .iter()
            .chain([&p.accent])
            .map(|s| Rgb::parse(s).unwrap())
            .chain([Rgb(0, 0, 0), Rgb(255, 255, 255)])
            .collect();
        if candidates.contains(&old) {
            return old;
        }
        let mapped = map.map(old);
        *candidates
            .iter()
            .min_by_key(|c| {
                let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2);
                d(c.0, mapped.0) + d(c.1, mapped.1) + d(c.2, mapped.2)
            })
            .unwrap()
    }
    fn colours(value: &mut Value, map: &crate::kvantum::Recolor, p: &Palette) {
        match value {
            Value::String(s) => {
                if let Ok(old) = Rgb::parse(s) {
                    *s = project(old, map, p).hex();
                } else if s.contains('<') {
                    let mut out = String::new();
                    let mut inside = false;
                    let mut i = 0;
                    while i < s.len() {
                        let c = s[i..].chars().next().unwrap();
                        if c == '<' {
                            inside = true;
                        }
                        if c == '>' {
                            inside = false;
                        }
                        if inside
                            && c == '#'
                            && i + 7 <= s.len()
                            && s.as_bytes()[i + 1..i + 7].iter().all(u8::is_ascii_hexdigit)
                        {
                            let old = Rgb::parse(&s[i..i + 7]).unwrap();
                            out.push_str(&project(old, map, p).hex());
                            i += 7;
                        } else {
                            out.push(c);
                            i += c.len_utf8();
                        }
                    }
                    *s = out;
                }
            }
            Value::Object(o) => {
                for v in o.values_mut() {
                    colours(v, map, p)
                }
            }
            Value::Array(a) => {
                for v in a {
                    colours(v, map, p)
                }
            }
            _ => {}
        }
    }
    recolor(&mut value, &map, p);
    Ok(serde_json::to_string_pretty(&value)? + "\n")
}

fn selectors(plan: &mut Plan, source: &Path, rel: &str, data: String) -> Result<()> {
    if source.join(rel).is_file() {
        return Ok(());
    }
    let old = plan.text(&paths::live_path(rel))?.unwrap_or_default();
    write(plan, rel, native_appearance::merge(rel, &data, &old)?)
}
pub fn attach(source: &Path, plan: &mut Plan) -> Result<()> {
    if source.join(FZF_DATA).is_file() {
        let text = fs::read_to_string(source.join(FZF_DATA))?;
        valid_fzf(&text)?;
        if present("fish", ".config/fish") {
            write(plan, FZF_LOADER, FZF_GLUE.into())?;
        }
    }
    if source.join(WOFI_COLORS).is_file() {
        let old = plan.text(&paths::live_path(WOFI_HOST))?.unwrap_or_default();
        write(plan, WOFI_HOST, wofi_host(&old))?;
    }
    Ok(())
}
pub fn wofi_host(old: &str) -> String {
    const IMPORT: &str = "@import url(\"kolira.css\");";
    if old.lines().any(|l| l.trim() == IMPORT) {
        old.into()
    } else {
        format!("{IMPORT}\n{old}")
    }
}
pub fn plan(source: &Path, package: &Package, plan: &mut Plan) -> Result<()> {
    attach(source, plan)?;
    let Some(p) = &package.palette else {
        return Ok(());
    };
    p.validate()?;
    let wants = |id: &str| package.app_targets.iter().any(|s| s == id);
    for (target, pure) in [("fish", false), ("pure", true)] {
        if wants(target)
            && present("fish", ".config/fish")
            && !source.join(crate::extras::FISH_VARS).exists()
        {
            let path = paths::live_path(crate::extras::FISH_VARS);
            let old = plan.text(&path)?.unwrap_or_default();
            let text = crate::extras::lines_merge(
                &fish(p, pure),
                &old,
                if pure { pure_line } else { fish_line },
            );
            write(
                plan,
                crate::extras::FISH_VARS,
                crate::extras::fish_header(&text)?,
            )?;
        }
    }
    if wants("oh-my-posh") && !source.join(crate::extras::OMP_THEME).exists() {
        if let Some(current) = crate::extras::omp_current() {
            app_sync::validate_live(&current)?;
            let old = plan.text(&current)?.context("Prompt missing")?;
            write(plan, crate::extras::OMP_THEME, omp(p, &old)?)?;
            for host in crate::extras::omp_hosts() {
                let old = plan.text(&paths::live_path(&host))?.unwrap_or_default();
                write(plan, &host, crate::extras::omp_host(&old))?;
            }
        }
    }
    if wants("btop")
        && present("btop", ".config/btop")
        && !source.join(".config/btop/themes/kolira.theme").exists()
    {
        write(plan, ".config/btop/themes/kolira.theme", btop(p))?;
        let rel = ".config/btop/btop.conf";
        let old = plan.text(&paths::live_path(rel))?.unwrap_or_default();
        write(plan, rel, kv(&old, "color_theme", "\"kolira\""))?;
    }
    if wants("yazi") && present("yazi", ".config/yazi") {
        selectors(plan, source, ".config/yazi/theme.toml", yazi(p))?;
    }
    if wants("bat")
        && present("bat", ".config/bat")
        && !source.join(".config/bat/themes/kolira.tmTheme").exists()
    {
        write(plan, ".config/bat/themes/kolira.tmTheme", tmtheme(p))?;
        let rel = crate::extras::BAT_CONFIG;
        let old = plan.text(&paths::live_path(rel))?.unwrap_or_default();
        write(
            plan,
            rel,
            crate::extras::lines_merge("--theme=kolira\n", &old, crate::extras::bat_theme_line),
        )?;
    }
    if wants("wofi") && present("wofi", ".config/wofi") && !source.join(WOFI_COLORS).exists() {
        write(plan, WOFI_COLORS, wofi(p))?;
        let old = plan.text(&paths::live_path(WOFI_HOST))?.unwrap_or_default();
        write(plan, WOFI_HOST, wofi_host(&old))?;
    }
    if wants("fzf") && present("fzf", ".config/fzf") && !source.join(FZF_DATA).exists() {
        write(plan, FZF_DATA, fzf(p))?;
        if present("fish", ".config/fish") {
            write(plan, FZF_LOADER, FZF_GLUE.into())?;
        }
    }
    if wants("lazygit") && present("lazygit", ".config/lazygit") {
        anyhow::ensure!(std::env::var("LG_CONFIG_FILE").is_err(),"LG_CONFIG_FILE overrides require explicit native config setup; refusing an ineffective write");
        selectors(plan, source, ".config/lazygit/config.yml", lazygit(p))?;
    }
    if wants("zellij") && present("zellij", ".config/zellij") {
        if !source.join(".config/zellij/themes/kolira.kdl").exists() {
            write(plan, ".config/zellij/themes/kolira.kdl", zellij(p))?;
        }
        selectors(
            plan,
            source,
            ".config/zellij/config.kdl",
            "theme \"kolira\"\ntheme_dark \"kolira\"\ntheme_light \"kolira\"\n".into(),
        )?;
    }
    if wants("opencode")
        && present("opencode", ".config/opencode")
        && !source.join(".config/opencode/cli.json").exists()
    {
        let rel = ".config/opencode/cli.json";
        let old = plan
            .text(&paths::live_path(rel))?
            .unwrap_or_else(|| "{}".into());
        let _: Value = serde_json::from_str(&old).context("Invalid OpenCode cli.json")?;
        write(
            plan,
            rel,
            field::json_set(
                &old,
                &["theme".into()],
                &json!({"name":"system","mode":if p.variant==Variant::Dark{"dark"}else{"light"}}),
            )?,
        )?;
    }
    if wants("claude-code")
        && present("claude", ".claude")
        && !source.join(".claude/settings.json").exists()
    {
        let rel = ".claude/settings.json";
        let old = plan
            .text(&paths::live_path(rel))?
            .unwrap_or_else(|| "{}".into());
        let value: Value =
            serde_json::from_str(&old).context("Invalid Claude Code settings JSON")?;
        let daltonized = value["theme"]
            .as_str()
            .is_some_and(|s| s.ends_with("-daltonized"));
        let mode = if p.variant == Variant::Dark {
            "dark"
        } else {
            "light"
        };
        let choice = format!("{mode}-{}", if daltonized { "daltonized" } else { "ansi" });
        write(
            plan,
            rel,
            field::json_set(&old, &["theme".into()], &json!(choice))?,
        )?;
    }
    if wants("codex") && present("codex", ".codex") {
        if !source.join(".codex/themes/kolira.tmTheme").exists() {
            write(plan, ".codex/themes/kolira.tmTheme", tmtheme(p))?;
        }
        selectors(
            plan,
            source,
            ".codex/config.toml",
            "[tui]\ntheme = \"kolira\"\n".into(),
        )?;
    }
    Ok(())
}
pub fn fish_line(line: &str) -> bool {
    crate::extras::fish_colour_line(line) && !pure_line(line)
}
pub fn pure_line(line: &str) -> bool {
    line.trim_start()
        .strip_prefix("SETUVAR --export ")
        .or_else(|| line.trim_start().strip_prefix("SETUVAR "))
        .is_some_and(|s| s.starts_with("pure_color_"))
}

pub fn bundle_resources(p: &Palette) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (path,text) in [
        (crate::extras::CAVA,crate::extras::cava_palette_text("",p)),
        (crate::extras::FISH_VARS,fish(p,false)+&fish(p,true)),(".config/btop/themes/kolira.theme",btop(p)),
        (".config/btop/btop.conf","color_theme = \"kolira\"\n".into()),(".config/yazi/theme.toml",yazi(p)),
        (".config/bat/themes/kolira.tmTheme",tmtheme(p)),(crate::extras::BAT_CONFIG,"--theme=kolira\n".into()),
        (WOFI_COLORS,wofi(p)),(FZF_DATA,fzf(p)),(".config/lazygit/config.yml",lazygit(p)),
        (".config/zellij/themes/kolira.kdl",zellij(p)),(".config/zellij/config.kdl","theme \"kolira\"\ntheme_dark \"kolira\"\ntheme_light \"kolira\"\n".into()),
        (".config/opencode/cli.json",json!({"theme":{"name":"system","mode":if p.variant==Variant::Dark{"dark"}else{"light"}}}).to_string()),
        (".claude/settings.json",json!({"theme":if p.variant==Variant::Dark{"dark-ansi"}else{"light-ansi"}}).to_string()),
        (".codex/themes/kolira.tmTheme",tmtheme(p)),(".codex/config.toml","[tui]\ntheme = \"kolira\"\n".into()),
    ] {out.insert(path.into(),text);}
    out
}
