//! Third priority batch: data generated from the package palette, never personal payloads.
use crate::{
    app_sync,
    change_plan::Plan,
    field,
    palette::{kde_roles, Palette, Variant},
    paths,
};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};
pub const TARGETS: &[&str] = &[
    "micro",
    "lsd",
    "glow",
    "gimp",
    "heroic",
    "prismlauncher",
    "vim",
    "mpv",
    "cliamp",
    "vscodium",
];
pub const HEROIC: &str = ".config/heroic/config.json";
pub const VIM_DATA: &str = ".vim/kolira-palette.json";
pub const VIM_COLORS: &str = ".vim/colors/kolira.vim";
pub const VIM_LOADER: &str = ".vim/after/plugin/kolira.vim";
const BEGIN: &str = "\n/* BEGIN kolira palette */\n";
const END: &str = "\n/* END kolira palette */\n";
const VIM_CODE: &str = r#"" Kolira fixed loader; a theme supplies JSON colour data, never Vimscript.
let s:file = expand('<sfile>:p:h:h') . '/kolira-palette.json'
if !filereadable(s:file) | finish | endif
let s:p = json_decode(join(readfile(s:file), "\n"))
for s:c in s:p.base + [s:p.accent]
  if type(s:c) != v:t_string || s:c !~# '^#[0-9a-fA-F]\{6}$'
    throw 'Invalid Kolira colour data'
  endif
endfor
if len(s:p.base) != 16 | throw 'Invalid Kolira base16 palette' | endif
let s:rows = [['Normal',5,0,7,0],['Comment',3,-1,8,-1],['String',11,-1,2,-1],['Number',9,-1,3,-1],['Statement',14,-1,5,-1],['Function',13,-1,4,-1],['Type',10,-1,3,-1],['Identifier',12,-1,6,-1],['Visual',5,2,7,8],['Search',0,10,0,3],['LineNr',3,0,8,0],['CursorLineNr',13,0,4,0],['StatusLine',5,1,7,0],['VertSplit',2,0,8,0],['Error',8,0,1,0]]
for s:row in s:rows
  let s:cmd = 'highlight ' . s:row[0] . ' guifg=' . s:p.base[s:row[1]] . ' ctermfg=' . s:row[3]
  if s:row[2] >= 0
    let s:cmd .= ' guibg=' . s:p.base[s:row[2]] . ' ctermbg=' . s:row[4]
  else
    let s:cmd .= ' guibg=NONE ctermbg=NONE'
  endif
  execute s:cmd
endfor
let g:colors_name = 'kolira'
"#;
const VIM_START: &str = r#"" Kolira fixed startup glue. Personal vimrc and plugins remain untouched.
if filereadable(expand('<sfile>:p:h:h:h') . '/kolira-palette.json')
  colorscheme kolira
endif
"#;
pub fn fixed_script(rel: &str) -> bool {
    matches!(rel, VIM_COLORS | VIM_LOADER)
}
pub fn paths_for(id: &str) -> Vec<String> {
    let paths: &[&str] = match id {
        "micro" => &[
            ".config/micro/settings.json",
            ".config/micro/colorschemes/kolira.micro",
        ],
        "lsd" => &[".config/lsd/config.yaml", ".config/lsd/themes/kolira.yaml"],
        "glow" => &[".config/glow/glow.yml", ".config/glow/kolira.json"],
        "gimp" => &[
            ".config/GIMP/3.0/gimprc",
            ".config/GIMP/3.0/themes/Kolira/gimp.css",
        ],
        "heroic" => &[HEROIC],
        "prismlauncher" => &[
            ".local/share/PrismLauncher/prismlauncher.cfg",
            ".local/share/PrismLauncher/themes/kolira/theme.json",
        ],
        "vim" => &[VIM_DATA],
        "mpv" => &[".config/mpv/mpv.conf"],
        "cliamp" => &[
            ".config/cliamp/config.toml",
            ".config/cliamp/themes/kolira.toml",
        ],
        "vscodium" => &[".config/VSCodium/User/settings.json"],
        _ => &[],
    };
    paths.iter().map(|s| s.to_string()).collect()
}
fn present(id: &str, root: &str) -> bool {
    paths::live_path(root).exists() || id.split('|').any(|command| which::which(command).is_ok())
}
fn write(plan: &mut Plan, rel: &str, text: String) -> Result<()> {
    if fixed_script(rel) {
        if let Some(old) = plan.text(&paths::live_path(rel))? {
            anyhow::ensure!(
                old == text,
                "Existing fixed Vim loader differs; refusing to replace personal code"
            );
        }
    }
    app_sync::write(plan, &paths::live_path(rel), text)
}
pub fn micro(p: &Palette) -> String {
    let mut out = String::from("# Kolira base16 appearance only\n");
    out.push_str(&format!(
        "color-link default \"{},{}\"\n",
        p.base[5], p.base[0]
    ));
    for (k, s) in [
        ("comment", 3),
        ("constant.string", 11),
        ("constant.number", 9),
        ("constant", 9),
        ("statement", 14),
        ("type", 10),
        ("identifier", 12),
        ("special", 13),
        ("symbol", 12),
        ("error", 8),
        ("line-number", 3),
    ] {
        out.push_str(&format!("color-link {k} \"{}\"\n", p.base[s]));
    }
    out.push_str(&format!(
        "color-link selection \"{},{}\"\ncolor-link statusline \"{},{}\"\n",
        p.base[5], p.base[2], p.base[5], p.base[1]
    ));
    out
}
pub fn lsd(p: &Palette) -> String {
    format!("user: \"{}\"\ngroup: \"{}\"\npermission:\n  read: \"{}\"\n  write: \"{}\"\n  exec: \"{}\"\n  exec-sticky: \"{}\"\n  no-access: \"{}\"\n  octal: \"{}\"\n  acl: \"{}\"\n  context: \"{}\"\ndate:\n  hour-old: \"{}\"\n  day-old: \"{}\"\n  older: \"{}\"\nsize:\n  none: \"{}\"\n  small: \"{}\"\n  medium: \"{}\"\n  large: \"{}\"\ninode:\n  valid: \"{}\"\n  invalid: \"{}\"\nlinks:\n  valid: \"{}\"\n  invalid: \"{}\"\ntree-edge: \"{}\"\n",p.base[13],p.base[12],p.base[11],p.base[10],p.base[8],p.base[14],p.base[3],p.base[12],p.base[12],p.base[12],p.base[11],p.base[12],p.base[13],p.base[3],p.base[11],p.base[10],p.base[8],p.base[14],p.base[3],p.base[14],p.base[3],p.base[3])
}
pub fn glow(p: &Palette) -> String {
    let r = kde_roles(p);
    json!({"document":{"color":r.text.hex(),"block_prefix":"\n","block_suffix":"\n","margin":2},"heading":{"color":p.accent,"bold":true,"block_suffix":"\n"},"h1":{"color":p.accent,"bold":true},"h2":{"prefix":"## "},"h3":{"prefix":"### "},"block_quote":{"indent":1,"indent_token":"│ ","color":r.text_inactive.hex()},"list":{"level_indent":2},"link":{"color":r.link.hex(),"underline":true},"link_text":{"color":r.link.hex(),"bold":true},"emph":{"italic":true},"strong":{"bold":true},"code":{"color":p.base[11],"background_color":r.button.hex()},"code_block":{"color":r.text.hex(),"background_color":r.window_alt.hex()},"horizontal_rule":{"color":r.text_inactive.hex(),"format":"--------"},"table":{"center_separator":"┼","column_separator":"│","row_separator":"─"}}).to_string()+"\n"
}
pub fn gimp(p: &Palette) -> String {
    let r = kde_roles(p);
    let mode = if p.variant == Variant::Dark {
        "dark"
    } else {
        "light"
    };
    let mut out=format!("/* Kolira colour definitions; native GIMP layout is inherited. */\n@import url(\"/usr/share/gimp/3.0/themes/Default/gimp-{mode}.css\");\n");
    for (name, c) in [
        ("fg-color", r.text),
        ("bg-color", r.window),
        ("border-color", p.slot(2)),
        ("dimmed-fg-color", r.text_inactive),
        ("disabled-fg-color", r.text_inactive),
        ("disabled-button-color", r.button),
        ("hover-color", r.button_alt),
        ("widget-bg-color", r.window_alt),
        ("selected-color", p.slot(2)),
        ("extreme-bg-color", r.view),
        ("extreme-selected-color", p.slot(2)),
        ("strong-border-color", p.slot(2)),
        ("stronger-border-color", p.slot(3)),
        ("edge-border-color", p.slot(2)),
        ("scrollbar-slider-color", r.text_inactive),
        ("scrollbar-trough-color", r.window),
        ("accent_color", r.accent),
        ("accent_bg_color", r.accent),
        ("accent_fg_color", r.selection_text),
    ] {
        out.push_str(&format!("@define-color {name} {};\n", c.hex()));
    }
    out
}
pub fn prism(p: &Palette) -> String {
    let r = kde_roles(p);
    json!({"name":"Kolira","author":"Kolira contributors","widgets":"Fusion","fadeAmount":0.35,"fadeColor":r.window.hex(),"colors":{"Window":r.window.hex(),"WindowText":r.text.hex(),"Base":r.view.hex(),"AlternateBase":r.window_alt.hex(),"ToolTipBase":r.window_alt.hex(),"ToolTipText":r.text.hex(),"Text":r.text.hex(),"Button":r.button.hex(),"ButtonText":r.text.hex(),"BrightText":r.text.hex(),"Link":r.link.hex(),"Highlight":r.accent.hex(),"HighlightedText":r.selection_text.hex()}}).to_string()+"\n"
}
pub fn cliamp(p: &Palette) -> String {
    format!("bg = \"{}\"\naccent = \"{}\"\nbright_fg = \"{}\"\nfg = \"{}\"\ngreen = \"{}\"\nyellow = \"{}\"\nred = \"{}\"\n",p.base[0],p.accent,p.base[7],p.base[5],p.base[11],p.base[10],p.base[8])
}
const HEROIC_VARS: &[&str] = &[
    "--background",
    "--background-darker",
    "--background-lighter",
    "--body-background",
    "--text-default",
    "--text-title",
    "--text-secondary",
    "--text-tertiary",
    "--text-quartenary",
    "--accent",
    "--accent-overlay",
    "--accent-02",
    "--primary-button",
    "--primary-button-overlay",
    "--primary-button-text",
    "--secondary-button",
    "--secondary-button-overlay",
    "--secondary-button-text",
    "--tertiary-button",
    "--tertiary-button-overlay",
    "--tertiary-button-text",
    "--cancel-button",
    "--cancel-button-overlay",
    "--navbar-background",
    "--sidebar-background",
    "--status-success",
    "--status-warning",
    "--status-danger",
    "--status-info",
];
fn block(css: &str) -> Result<Option<&str>> {
    let Some(start) = css.find(BEGIN) else {
        return Ok(None);
    };
    let tail = &css[start + BEGIN.len()..];
    let end = tail
        .find(END)
        .context("Unclosed Kolira Heroic colour block")?;
    anyhow::ensure!(
        !tail[end + END.len()..].contains(BEGIN),
        "Duplicate Heroic colour blocks"
    );
    Ok(Some(&css[start..start + BEGIN.len() + end + END.len()]))
}
fn safe_heroic(css: &str) -> Result<String> {
    let own = block(css)?.context("Heroic snapshot must carry only a managed colour block")?;
    let body = own
        .strip_prefix(BEGIN)
        .unwrap()
        .strip_suffix(END)
        .unwrap()
        .trim()
        .strip_prefix("body {")
        .and_then(|s| s.strip_suffix('}'))
        .context("Unsupported Heroic selector")?;
    let mut vars = BTreeMap::new();
    for entry in body.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        let (k, v) = entry.split_once(':').context("Heroic colour declaration")?;
        let v = v.trim().trim_end_matches("!important").trim();
        anyhow::ensure!(!vars.contains_key(k), "Duplicate Heroic colour variable");
        if k == "color-scheme" {
            anyhow::ensure!(
                matches!(v, "dark" | "light"),
                "Invalid Heroic colour scheme"
            );
        } else {
            anyhow::ensure!(HEROIC_VARS.contains(&k), "Unsupported Heroic CSS property");
            crate::palette::Rgb::parse(v)?;
        }
        vars.insert(k, v);
    }
    let mut out = String::from(BEGIN);
    out.push_str("body {\n");
    for (k, v) in vars {
        out.push_str(&format!("  {k}: {v} !important;\n"));
    }
    out.push('}');
    out.push_str(END);
    Ok(out)
}
pub fn heroic(p: &Palette) -> String {
    let r = kde_roles(p);
    // A primary hover must keep the contrast of its paired selection text.
    // An accent-filled tertiary hover would instead use ordinary body text.
    let away_from_text = if r.selection_text.luminance() > 0.5 {
        crate::palette::Rgb(0, 0, 0)
    } else {
        crate::palette::Rgb(255, 255, 255)
    };
    let primary_hover = r.accent.mix(away_from_text, 0.15);
    let colors = [
        r.window,
        r.view,
        r.window_alt,
        r.window,
        r.text,
        r.text,
        r.text_inactive,
        r.text_inactive,
        r.text_inactive,
        r.accent,
        r.hover,
        r.accent.mix(r.window, 0.2),
        r.accent,
        primary_hover,
        r.selection_text,
        r.button,
        r.button_alt,
        r.text,
        r.button_alt,
        r.button_alt,
        r.text,
        r.button,
        r.button_alt,
        r.window_alt,
        r.window_alt,
        r.positive,
        r.neutral,
        r.negative,
        r.link,
    ];
    let mut css = String::from(BEGIN);
    css.push_str("body {\n");
    for (name, c) in HEROIC_VARS.iter().zip(colors) {
        css.push_str(&format!("{name}: {} !important;\n", c.hex()));
    }
    css.push_str(&format!(
        "color-scheme: {};\n}}",
        if p.variant == Variant::Dark {
            "dark"
        } else {
            "light"
        }
    ));
    css.push_str(END);
    json!({"defaultSettings":{"customCSS":safe_heroic(&css).unwrap()}}).to_string() + "\n"
}
fn css_value(text: &str) -> Result<Option<String>> {
    let v: Value = serde_json::from_str(if text.trim().is_empty() { "{}" } else { text })
        .context("Invalid Heroic config JSON")?;
    anyhow::ensure!(v.is_object(), "Heroic config must be an object");
    if let Some(settings) = v.get("defaultSettings") {
        anyhow::ensure!(
            settings.is_object(),
            "Heroic defaultSettings must be an object"
        );
    }
    let value = v.get("defaultSettings").and_then(|s| s.get("customCSS"));
    anyhow::ensure!(
        value.is_none_or(Value::is_string),
        "Heroic customCSS must be a string"
    );
    Ok(value.and_then(Value::as_str).map(str::to_owned))
}
pub fn heroic_merge(theme: &str, live: &str) -> Result<String> {
    let source = css_value(theme)?.unwrap_or_default();
    let Some(_) = block(&source)? else {
        return Ok(live.into());
    };
    let source = safe_heroic(&source)?;
    let old = css_value(live)?.unwrap_or_default();
    let existing = block(&old)?.unwrap_or("");
    let css = if existing.is_empty() {
        old
    } else {
        old.replacen(existing, "", 1)
    } + &source;
    let base = if live.trim().is_empty() { "{}" } else { live };
    let config: Value = serde_json::from_str(base)?;
    let base = if config.get("defaultSettings").is_none() {
        field::json_set(base, &["defaultSettings".into()], &json!({}))?
    } else {
        base.into()
    };
    field::json_set(
        &base,
        &["defaultSettings".into(), "customCSS".into()],
        &json!(css),
    )
}
pub fn heroic_capture(text: &str) -> Result<String> {
    let old = css_value(text)?.unwrap_or_default();
    if let Some(own) = block(&old)? {
        Ok(json!({"defaultSettings":{"customCSS":safe_heroic(own)?}}).to_string())
    } else {
        Ok("{}\n".into())
    }
}
pub fn heroic_restore(before: &str, after: &str, current: &str) -> Result<String> {
    let old = css_value(before)?.unwrap_or_default();
    let written = css_value(after)?.unwrap_or_default();
    let now = css_value(current)?.unwrap_or_default();
    if now == written {
        return Ok(crate::appearance::restore_json(before, after, current));
    }
    let Some(added) = block(&written)? else {
        return Ok(current.into());
    };
    if block(&now)? == Some(added) {
        let mut css = now.replacen(added, "", 1);
        if let Some(previous) = block(&old)? {
            css.push_str(previous);
        }
        return field::json_set(
            current,
            &["defaultSettings".into(), "customCSS".into()],
            &json!(css),
        );
    }
    Ok(current.into())
}
pub fn bundle_resources(p: &Palette) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mode = if p.variant == Variant::Dark {
        "dark"
    } else {
        "light"
    };
    for (path, text) in [
        (
            ".config/micro/settings.json",
            json!({"colorscheme":"kolira"}).to_string(),
        ),
        (".config/micro/colorschemes/kolira.micro", micro(p)),
        (
            ".config/lsd/config.yaml",
            "color:\n  theme: kolira\n".into(),
        ),
        (".config/lsd/themes/kolira.yaml", lsd(p)),
        (".config/glow/glow.yml", "style: \"kolira\"\n".into()),
        (".config/glow/kolira.json", glow(p)),
        (
            ".config/GIMP/3.0/gimprc",
            format!("(theme \"Kolira\")\n(theme-color-scheme {mode})\n"),
        ),
        (".config/GIMP/3.0/themes/Kolira/gimp.css", gimp(p)),
        (HEROIC, heroic(p)),
        (
            ".local/share/PrismLauncher/prismlauncher.cfg",
            "[General]\nApplicationTheme=kolira\n".into(),
        ),
        (
            ".local/share/PrismLauncher/themes/kolira/theme.json",
            prism(p),
        ),
        (VIM_DATA, serde_json::to_string_pretty(p).unwrap() + "\n"),
        (
            ".config/mpv/mpv.conf",
            format!(
                "osd-color=\"{}\"\nosd-border-color=\"{}\"\nosd-shadow-color=\"{}\"\n",
                p.base[5], p.base[0], p.base[0]
            ),
        ),
        (".config/cliamp/config.toml", "theme = \"kolira\"\n".into()),
        (".config/cliamp/themes/kolira.toml", cliamp(p)),
    ] {
        out.insert(path.into(), text);
    }
    out
}

/// Managed resources are colour data. Executable or layout payloads are rejected
/// during preview, before a single live file is changed.
pub fn is_asset(rel: &str) -> bool {
    matches!(
        rel,
        VIM_DATA
            | ".config/micro/colorschemes/kolira.micro"
            | ".config/lsd/themes/kolira.yaml"
            | ".config/glow/kolira.json"
            | ".config/GIMP/3.0/themes/Kolira/gimp.css"
            | ".local/share/PrismLauncher/themes/kolira/theme.json"
            | ".config/cliamp/themes/kolira.toml"
    )
}
fn rgb(value: &Value) -> Result<()> {
    let text = value.as_str().context("Colour must be a string")?;
    anyhow::ensure!(
        text.starts_with('#') && text.len() == 7,
        "Colour must be #rrggbb"
    );
    crate::palette::Rgb::parse(text)?;
    Ok(())
}
pub fn valid_asset(rel: &str, text: &str) -> Result<String> {
    if rel == VIM_DATA {
        let p: Palette = serde_json::from_str(text)?;
        p.validate()?;
        return Ok(serde_json::to_string_pretty(&p)? + "\n");
    }
    if rel.ends_with("/kolira-theme/manifest.json") {
        let v: Value = serde_json::from_str(text)?;
        let object = v.as_object().context("Theme extension must be an object")?;
        anyhow::ensure!(
            object.keys().all(|k| matches!(
                k.as_str(),
                "manifest_version" | "name" | "description" | "version" | "theme"
            )),
            "Only native Chromium theme metadata is allowed"
        );
        anyhow::ensure!(
            v["manifest_version"] == 3 && v["name"].is_string() && v["version"].is_string(),
            "Invalid theme extension metadata"
        );
        let theme = v["theme"].as_object().context("Missing native theme")?;
        anyhow::ensure!(
            theme.len() == 1 && theme.contains_key("colors"),
            "Only native frame colours are allowed"
        );
        let colors = v["theme"]["colors"]
            .as_object()
            .context("Missing frame colours")?;
        anyhow::ensure!(!colors.is_empty(), "Empty frame palette");
        for (k, v) in colors {
            anyhow::ensure!(
                matches!(k.as_str(), "frame" | "frame_inactive"),
                "Browser text/icon colours remain native"
            );
            let channels = v.as_array().context("Frame colour must be RGB")?;
            anyhow::ensure!(
                channels.len() == 3
                    && channels
                        .iter()
                        .all(|c| c.as_u64().is_some_and(|n| n <= 255)),
                "Invalid native RGB colour"
            );
        }
    } else if rel.ends_with("/PrismLauncher/themes/kolira/theme.json") {
        let v: Value = serde_json::from_str(text)?;
        let o = v.as_object().context("Invalid Prism theme")?;
        anyhow::ensure!(
            o.keys().all(|k| matches!(
                k.as_str(),
                "name" | "author" | "widgets" | "colors" | "fadeAmount" | "fadeColor"
            )) && v["name"].is_string()
                && v["widgets"] == "Fusion",
            "Prism theme must contain native colour data only"
        );
        if let Some(c) = v.get("fadeColor") {
            rgb(c)?;
        }
        if let Some(n) = v.get("fadeAmount") {
            anyhow::ensure!(
                n.as_f64().is_some_and(|n| (0.0..=1.0).contains(&n)),
                "Invalid fade amount"
            );
        }
        let colors = v["colors"].as_object().context("Missing Prism palette")?;
        for (k, v) in colors {
            anyhow::ensure!(
                matches!(
                    k.as_str(),
                    "Window"
                        | "WindowText"
                        | "Base"
                        | "AlternateBase"
                        | "ToolTipBase"
                        | "ToolTipText"
                        | "Text"
                        | "Button"
                        | "ButtonText"
                        | "BrightText"
                        | "Link"
                        | "Highlight"
                        | "HighlightedText"
                ),
                "Unsupported Qt palette role"
            );
            rgb(v)?;
        }
    } else if rel.ends_with("/GIMP/3.0/themes/Kolira/gimp.css") {
        let mut imported = false;
        for l in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
            if l == "/* Kolira colour definitions; native GIMP layout is inherited. */" {
                continue;
            }
            if matches!(
                l,
                "@import url(\"/usr/share/gimp/3.0/themes/Default/gimp-dark.css\");"
                    | "@import url(\"/usr/share/gimp/3.0/themes/Default/gimp-light.css\");"
            ) {
                anyhow::ensure!(!imported, "Duplicate native GIMP import");
                imported = true;
                continue;
            }
            let body = l
                .strip_prefix("@define-color ")
                .and_then(|s| s.strip_suffix(';'))
                .context("Only native GIMP colour definitions are allowed")?;
            let (name, color) = body
                .split_once(' ')
                .context("Invalid GIMP colour definition")?;
            anyhow::ensure!(
                name.bytes()
                    .all(|c| c.is_ascii_alphabetic() || c == b'-' || c == b'_'),
                "Invalid colour name"
            );
            rgb(&json!(color.trim()))?;
        }
        anyhow::ensure!(imported, "Missing native GIMP layout import");
    } else if rel.ends_with("/micro/colorschemes/kolira.micro") {
        for l in text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
        {
            let body = l
                .strip_prefix("color-link ")
                .context("Micro theme must contain colour links only")?;
            let (scope, colors) = body.split_once(' ').context("Invalid Micro colour link")?;
            anyhow::ensure!(
                scope
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b".-_".contains(&c)),
                "Invalid Micro colour scope"
            );
            let colors: String = serde_json::from_str(colors.trim())?;
            for c in colors.split(',') {
                rgb(&json!(c))?;
            }
        }
    } else if rel.ends_with("/cliamp/themes/kolira.toml") {
        let v: toml::Value = toml::from_str(text)?;
        let o = v.as_table().context("Invalid Cliamp theme")?;
        anyhow::ensure!(
            o.keys().all(|k| matches!(
                k.as_str(),
                "bg" | "accent" | "bright_fg" | "fg" | "green" | "yellow" | "red"
            )),
            "Only native Cliamp colours are allowed"
        );
        for key in ["accent", "bright_fg", "fg", "green", "yellow", "red"] {
            anyhow::ensure!(o.contains_key(key), "Missing Cliamp colour {key}");
        }
        for v in o.values() {
            rgb(&json!(v.as_str().context("Invalid colour")?))?;
        }
    } else if rel.ends_with("/glow/kolira.json") {
        let v: Value = serde_json::from_str(text)?;
        anyhow::ensure!(v.is_object(), "Invalid Glamour palette");
        fn colors(v: &Value) -> Result<()> {
            if let Some(o) = v.as_object() {
                for (k, v) in o {
                    if matches!(k.as_str(), "color" | "background_color") {
                        rgb(v)?;
                    } else {
                        colors(v)?;
                    }
                }
            } else if let Some(a) = v.as_array() {
                for v in a {
                    colors(v)?;
                }
            }
            Ok(())
        }
        colors(&v)?;
    } else if rel.ends_with("/lsd/themes/kolira.yaml") {
        for l in text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
        {
            let (k, v) = l.split_once(':').context("Invalid lsd palette")?;
            anyhow::ensure!(
                k.bytes().all(|b| b.is_ascii_lowercase() || b == b'-'),
                "Invalid native palette key"
            );
            if !v.trim().is_empty() {
                rgb(&json!(v.trim().trim_matches(['\'', '"'])))?;
            }
        }
    }
    Ok(text.into())
}
pub fn plan(source: &Path, package: &crate::theme::Package, plan: &mut Plan) -> Result<()> {
    if source.join(VIM_DATA).is_file() && present("vim", ".vim") {
        write(plan, VIM_COLORS, VIM_CODE.into())?;
        write(plan, VIM_LOADER, VIM_START.into())?;
    }
    if source.join(".config/glow/kolira.json").is_file() && present("glow", ".config/glow") {
        let rel = ".config/glow/glow.yml";
        let old = plan.text(&paths::live_path(rel))?.unwrap_or_default();
        write(
            plan,
            rel,
            crate::structured_appearance::merge(rel, "style: \"kolira\"\n", &old)?,
        )?;
    }
    let Some(p) = &package.palette else {
        return Ok(());
    };
    p.validate()?;
    let resources = bundle_resources(p);
    for id in TARGETS
        .iter()
        .filter(|id| package.app_targets.iter().any(|t| t == **id))
    {
        let (command, root) = match *id {
            "micro" => ("micro", ".config/micro"),
            "lsd" => ("lsd", ".config/lsd"),
            "glow" => ("glow", ".config/glow"),
            "gimp" => ("gimp", ".config/GIMP/3.0"),
            "heroic" => ("heroic", ".config/heroic"),
            "prismlauncher" => ("prismlauncher", ".local/share/PrismLauncher"),
            "vim" => ("vim", ".vim"),
            "mpv" => ("mpv", ".config/mpv"),
            "cliamp" => ("cliamp", ".config/cliamp"),
            "vscodium" => ("codium|vscodium", ".config/VSCodium"),
            _ => unreachable!(),
        };
        if !present(command, root) {
            continue;
        }
        for variable in match *id {
            "micro" => &["MICRO_CONFIG_HOME"][..],
            "lsd" => &["LSD_CONFIG_DIR"][..],
            "glow" => &["GLOW_CONFIG_HOME"][..],
            "mpv" => &["MPV_HOME"][..],
            _ => &[][..],
        } {
            anyhow::ensure!(
                std::env::var_os(variable).is_none(),
                "Custom {variable} requires native integration"
            );
        }
        if *id == "gimp" {
            anyhow::ensure!(
                Path::new("/usr/share/gimp/3.0/themes/Default").is_dir(),
                "GIMP native Default theme missing; unsupported installation path"
            );
            anyhow::ensure!(
                std::env::var_os("GIMP3_DIRECTORY").is_none(),
                "Custom GIMP3_DIRECTORY requires native integration"
            );
        }
        for rel in paths_for(id) {
            if let Some(text) = resources.get(&rel) {
                if !source.join(&rel).is_file() {
                    let old = plan.text(&paths::live_path(&rel))?;
                    let out = crate::appearance::applied(&rel, text, old.as_deref())?;
                    write(plan, &rel, out)?;
                }
            }
        }
        if *id == "vim" {
            write(plan, VIM_COLORS, VIM_CODE.into())?;
            write(plan, VIM_LOADER, VIM_START.into())?;
        }
        if *id == "vscodium" && !source.join(".config/VSCodium/User/settings.json").is_file() {
            let rel = ".config/VSCodium/User/settings.json";
            let mut old = plan
                .text(&paths::live_path(rel))?
                .unwrap_or_else(|| "{}".into());
            old = field::json_set(
                &old,
                &["workbench.colorTheme".into()],
                &json!(if p.variant == Variant::Dark {
                    "Default Dark Modern"
                } else {
                    "Default Light Modern"
                }),
            )?;
            old = field::json_set(
                &old,
                &["workbench.colorCustomizations".into()],
                &json!({"editor.background":p.base[0],"editor.foreground":p.base[5],"focusBorder":p.accent}),
            )?;
            write(plan, rel, old)?;
        }
    }
    Ok(())
}
