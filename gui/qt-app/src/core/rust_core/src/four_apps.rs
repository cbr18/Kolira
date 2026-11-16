//! Native prompt/terminal palettes. User commands and Lua never travel in themes.
use crate::{
    app_sync,
    change_plan::Plan,
    palette::{self, Palette},
    paths,
};
use anyhow::{Context, Result};
use std::{collections::BTreeMap, path::Path};
pub const TARGETS: &[&str] = &["starship", "tmux", "foot", "wezterm"];
pub const STARSHIP: &str = ".config/starship.toml";
pub const FOOT: &str = ".config/foot/foot.ini";
pub const TMUX_DATA: &str = ".config/tmux/kolira.json";
pub const TMUX_SCRIPT: &str = ".config/tmux/kolira.conf";
pub const TMUX_HOSTS: &[&str] = &[".tmux.conf", ".config/tmux/tmux.conf"];
pub const WEZ_DATA: &str = ".config/wezterm/kolira.toml";
const WEZ_FIELDS: &[&str] = &[
    "background",
    "foreground",
    "cursor_bg",
    "cursor_fg",
    "cursor_border",
    "selection_bg",
    "selection_fg",
    "ansi",
    "brights",
];
pub const WEZ_HOSTS: &[&str] = &[".config/wezterm/wezterm.lua", ".wezterm.lua"];
const WEZ_BEGIN: &str = "-- BEGIN kolira native colours\nlocal _kolira_config = (function(...)\n";
const WEZ_MIDDLE: &str = "\nend)(...)\n-- END kolira personal config\n";
const TMUX_BEGIN: &str = "\n# BEGIN kolira native colours\n";
const TMUX_END: &str = "# END kolira native colours\n";
pub fn is_host(rel: &str) -> bool {
    WEZ_HOSTS.contains(&rel) || TMUX_HOSTS.contains(&rel)
}
pub fn preserved(rel: &str) -> bool {
    is_host(rel) || rel == TMUX_SCRIPT
}
pub fn paths_for(id: &str) -> Vec<String> {
    let paths: &[&str] = match id {
        "starship" => &[STARSHIP],
        "foot" => &[FOOT],
        "tmux" => &[".tmux.conf", ".config/tmux/tmux.conf", TMUX_DATA],
        "wezterm" => &[".wezterm.lua", ".config/wezterm/wezterm.lua", WEZ_DATA],
        _ => &[],
    };
    paths.iter().map(|s| s.to_string()).collect()
}
pub fn starship(p: &Palette) -> String {
    let mut doc = toml_edit::DocumentMut::new();
    doc["palette"] = toml_edit::value("kolira");
    let ansi = palette::ansi16(p);
    for (i, name) in [
        "black", "red", "green", "yellow", "blue", "purple", "cyan", "white",
    ]
    .iter()
    .enumerate()
    {
        doc["palettes"]["kolira"][name] = toml_edit::value(ansi[i].hex());
        doc["palettes"]["kolira"][format!("bright-{name}")] = toml_edit::value(ansi[i + 8].hex());
    }
    for i in 0..16 {
        doc["palettes"]["kolira"][format!("base{i:02X}")] = toml_edit::value(p.base[i].clone());
    }
    doc["palettes"]["kolira"]["accent"] = toml_edit::value(p.accent.clone());
    doc.to_string()
}
/// Preserve custom style aliases, recolouring their RGB definitions rather than formats/commands.
pub fn starship_source(source: &str, live: &str) -> Result<String> {
    let mut doc: toml_edit::DocumentMut = source.parse()?;
    if let Some(value) = doc.get("palette") {
        anyhow::ensure!(
            value.as_str() == Some("kolira"),
            "Only the owned Starship palette selector is portable"
        );
    }
    let old: toml_edit::DocumentMut = live.parse().context("Invalid personal Starship TOML")?;
    let Some(table) = doc
        .get("palettes")
        .and_then(|v| v.get("kolira"))
        .and_then(|v| v.as_table_like())
    else {
        return Ok(source.into());
    };
    for (_, v) in table.iter() {
        native_rgb(
            v.as_str()
                .context("Starship palette must contain RGB colours only")?,
        )?;
    }
    let base = (0..16)
        .map(|i| {
            table
                .get(&format!("base{i:02X}"))
                .and_then(|v| v.as_str())
                .map(str::to_owned)
        })
        .collect::<Option<Vec<_>>>();
    if let Some(base) = base {
        let p = Palette {
            name: "kolira".into(),
            author: String::new(),
            variant: palette::Variant::Dark,
            base,
            accent: table
                .get("accent")
                .and_then(|v| v.as_str())
                .context("Missing accent")?
                .into(),
            source: "theme".into(),
        };
        let recolor = crate::kvantum::Recolor::new(
            palette::Rgb(0, 0, 0),
            palette::Rgb(255, 255, 255),
            palette::Rgb(0, 0, 255),
            &p,
        );
        if let Some(selected) = old
            .get("palette")
            .and_then(|v| v.as_str())
            .and_then(|name| old.get("palettes")?.get(name)?.as_table_like())
        {
            for (key, value) in selected.iter() {
                if doc["palettes"]["kolira"].get(key).is_none() {
                    let c = palette::Rgb::parse(
                        value
                            .as_str()
                            .context("Unsupported custom Starship palette colour")?,
                    )?;
                    doc["palettes"]["kolira"][key] =
                        if old.get("palette").and_then(|v| v.as_str()) == Some("kolira") {
                            value.clone()
                        } else {
                            toml_edit::value(recolor.map(c).hex())
                        };
                }
            }
        }
    }
    Ok(doc.to_string())
}
pub fn foot(p: &Palette) -> String {
    let selected = palette::kde_roles(p).selection_text.hex();
    let ansi = palette::ansi16(p);
    let mut out = String::new();
    // Both native variants use the selected package; a stale system mode must not reveal old colours.
    for section in ["colors-dark", "colors-light"] {
        out.push_str(&format!("[{section}]\nbackground={}\nforeground={}\nselection-background={}\nselection-foreground={}\ncursor={} {}\n", &p.base[0][1..],&p.base[5][1..],&p.accent[1..],&selected[1..],&selected[1..],&p.accent[1..]));
        for i in 0..8 {
            out.push_str(&format!(
                "regular{i}={}\nbright{i}={}\n",
                &ansi[i].hex()[1..],
                &ansi[i + 8].hex()[1..]
            ));
        }
    }
    out
}
pub fn wezterm(p: &Palette) -> String {
    let selected = palette::kde_roles(p).selection_text.hex();
    let ansi = palette::ansi16(p);
    let mut doc = toml_edit::DocumentMut::new();
    for (key, value) in [
        ("background", &p.base[0]),
        ("foreground", &p.base[5]),
        ("cursor_bg", &p.accent),
        ("cursor_fg", &selected),
        ("cursor_border", &p.accent),
        ("selection_bg", &p.accent),
        ("selection_fg", &selected),
    ] {
        doc["colors"][key] = toml_edit::value(value.clone());
    }
    for (key, values) in [("ansi", &ansi[..8]), ("brights", &ansi[8..])] {
        let a: toml_edit::Array = values.iter().map(|c| c.hex()).collect();
        doc["colors"][key] = toml_edit::value(a);
    }
    doc["metadata"]["name"] = toml_edit::value("Kolira");
    doc.to_string()
}
pub fn valid_wez(text: &str) -> Result<()> {
    let doc: toml_edit::DocumentMut = text.parse()?;
    anyhow::ensure!(
        doc.iter().all(|(k, _)| matches!(k, "colors" | "metadata")),
        "Only WezTerm colour data is allowed"
    );
    let meta = doc
        .get("metadata")
        .and_then(|v| v.as_table_like())
        .context("Missing WezTerm metadata")?;
    anyhow::ensure!(
        meta.len() == 1 && meta.get("name").and_then(|v| v.as_str()) == Some("Kolira"),
        "Only owned WezTerm metadata is portable"
    );
    let t = doc
        .get("colors")
        .and_then(|v| v.as_table_like())
        .context("Missing WezTerm colours")?;
    anyhow::ensure!(
        t.len() == WEZ_FIELDS.len() && WEZ_FIELDS.iter().all(|key| t.contains_key(key)),
        "Incomplete WezTerm palette; native defaults must not replace missing colours"
    );
    for (key, v) in t.iter() {
        if matches!(key, "ansi" | "brights") {
            let a = v.as_array().context("Invalid ANSI colours")?;
            anyhow::ensure!(a.len() == 8, "Expected eight ANSI colours");
            for v in a {
                native_rgb(v.as_str().context("Invalid RGB")?)?;
            }
        } else {
            anyhow::ensure!(
                [
                    "background",
                    "foreground",
                    "cursor_bg",
                    "cursor_fg",
                    "cursor_border",
                    "selection_bg",
                    "selection_fg"
                ]
                .contains(&key),
                "Unsupported WezTerm colour"
            );
            native_rgb(v.as_str().context("Invalid RGB")?)?;
        }
    }
    Ok(())
}
fn wez_end() -> Result<String> {
    let path = paths::live_path(WEZ_DATA);
    let s = path.to_str().context("Non UTF-8 WezTerm path")?;
    anyhow::ensure!(
        !s.chars().any(char::is_control),
        "Control characters in WezTerm path"
    );
    let fields = WEZ_FIELDS
        .iter()
        .map(|key| format!("'{key}'"))
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!("{WEZ_MIDDLE}local _kolira_file = {}\nlocal _kolira_stream = io.open(_kolira_file, 'r')\nif _kolira_stream then\n  _kolira_stream:close()\n  local _kolira_colors = require('wezterm').color.load_scheme(_kolira_file)\n  _kolira_config = _kolira_config or {{}}\n  local _kolira_merged = {{}}\n  for _kolira_key, _kolira_value in pairs(_kolira_config.colors or {{}}) do _kolira_merged[_kolira_key] = _kolira_value end\n  for _, _kolira_key in ipairs({{{fields}}}) do _kolira_merged[_kolira_key] = _kolira_colors[_kolira_key] end\n  _kolira_config.colors = _kolira_merged\nend\nreturn _kolira_config\n-- END kolira native colours\n",serde_json::to_string(s)?))
}
fn unwrap_wez(text: &str) -> Result<Option<&str>> {
    if !text.starts_with(WEZ_BEGIN) {
        return Ok(None);
    }
    Ok(text
        .strip_prefix(WEZ_BEGIN)
        .and_then(|s| s.strip_suffix(&wez_end().ok()?)))
}
pub fn wez_host(text: &str) -> Result<String> {
    if text.starts_with(WEZ_BEGIN) {
        anyhow::ensure!(
            unwrap_wez(text)?.is_some(),
            "Modified Kolira WezTerm wrapper; refusing replacement"
        );
        return Ok(text.into());
    }
    anyhow::ensure!(
        !text.starts_with("#!") && !text.starts_with('\u{feff}'),
        "Unsupported WezTerm shebang/BOM; config remains unchanged"
    );
    anyhow::ensure!(
        !text.contains("_kolira_") && !text.contains("BEGIN kolira native colours"),
        "Reserved Kolira wrapper namespace already used in personal Lua"
    );
    let body = if text.trim().is_empty() {
        "return {}\n"
    } else {
        text
    };
    Ok(format!("{WEZ_BEGIN}{body}{}", wez_end()?))
}
fn tmux_line() -> Result<String> {
    let path = paths::live_path(TMUX_SCRIPT);
    let text = path.to_str().context("Non UTF-8 tmux path")?;
    anyhow::ensure!(
        !text.chars().any(char::is_control),
        "Control characters in tmux path"
    );
    let escaped = text
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`");
    Ok(format!(
        "{TMUX_BEGIN}source-file -q \"{escaped}\"\n{TMUX_END}"
    ))
}
pub fn tmux_host(text: &str) -> Result<String> {
    if text.contains(TMUX_BEGIN) {
        anyhow::ensure!(
            text.matches(TMUX_BEGIN).count() == 1 && text.contains(&tmux_line()?),
            "Modified tmux colour include; refusing replacement"
        );
        return Ok(text.into());
    }
    Ok(format!("{text}{}", tmux_line()?))
}
pub fn restore_host(rel: &str, before: &str, after: &str, current: &str) -> Result<String> {
    if WEZ_HOSTS.contains(&rel) {
        if !before.starts_with(WEZ_BEGIN) && after.starts_with(WEZ_BEGIN) {
            if let Some(body) = unwrap_wez(current)? {
                return Ok(body.into());
            }
        }
    } else if !before.contains(TMUX_BEGIN) && after.contains(TMUX_BEGIN) {
        let block = tmux_line()?;
        if let Some(start) = current.find(&block) {
            let (left, right) = (&current[..start], &current[start + block.len()..]);
            let separator = if !left.is_empty()
                && !left.ends_with('\n')
                && !right.is_empty()
                && !right.starts_with('\n')
            {
                "\n"
            } else {
                ""
            };
            return Ok(format!("{left}{separator}{right}"));
        }
    }
    Ok(current.into())
}
pub fn bundle_resources(p: &Palette) -> BTreeMap<String, String> {
    [
        (STARSHIP.into(), starship(p)),
        (FOOT.into(), foot(p)),
        (WEZ_DATA.into(), wezterm(p)),
        (
            TMUX_DATA.into(),
            serde_json::to_string_pretty(&crate::tmux_runtime::styles(p)).unwrap() + "\n",
        ),
    ]
    .into_iter()
    .collect()
}
fn host<'a>(choices: &'a [&str], plan: &mut Plan) -> Result<&'a str> {
    for path in choices {
        if !matches!(
            plan.get(&paths::live_path(path))?,
            crate::change_plan::Entry::Missing
        ) {
            return Ok(path);
        }
    }
    Ok(choices
        .iter()
        .copied()
        .find(|p| p.starts_with(".config/"))
        .unwrap_or(choices[choices.len() - 1]))
}
pub fn plan(source: &Path, package: &crate::theme::Package, plan: &mut Plan) -> Result<()> {
    let wants = |id: &str| package.app_targets.iter().any(|s| s == id);
    let generated = package
        .palette
        .as_ref()
        .map(bundle_resources)
        .unwrap_or_default();
    for id in TARGETS {
        let configured = paths_for(id).iter().any(|r| paths::live_path(r).exists());
        if !configured && which::which(id).is_err() {
            continue;
        }
        check_version(id)?;
        for key in match *id {
            "starship" => &["STARSHIP_CONFIG"][..],
            "wezterm" => &["WEZTERM_CONFIG_FILE", "WEZTERM_CONFIG_DIR"][..],
            _ => &[][..],
        } {
            if let Some(value) = std::env::var_os(key).filter(|v| !v.is_empty()) {
                let allowed = if *key == "WEZTERM_CONFIG_DIR" {
                    Path::new(&value) == paths::config_home().join("wezterm")
                        || Path::new(&value) == paths::home_dir()
                } else {
                    paths_for(id)
                        .iter()
                        .any(|p| paths::live_path(p) == Path::new(&value))
                };
                anyhow::ensure!(
                    allowed,
                    "Custom {key} requires explicit integration; refusing the default path"
                );
            }
        }
        let resource = match *id {
            "starship" => STARSHIP,
            "foot" => FOOT,
            "tmux" => TMUX_DATA,
            _ => WEZ_DATA,
        };
        let text = if source.join(resource).is_file() {
            Some(std::fs::read_to_string(source.join(resource))?)
        } else if wants(id) {
            generated.get(resource).cloned()
        } else {
            None
        };
        let Some(text) = text else { continue };
        if resource == WEZ_DATA {
            valid_wez(&text)?;
        }
        if resource == TMUX_DATA {
            crate::tmux_runtime::capture()?;
            crate::tmux_runtime::parse(&text)?;
        }
        let path = paths::live_path(resource);
        let previous = plan.text(&path)?;
        if resource == TMUX_DATA {
            if let Some(script) = plan.text(&paths::live_path(TMUX_SCRIPT))? {
                anyhow::ensure!(previous.as_deref().is_some_and(|data|crate::tmux_runtime::script(data).is_ok_and(|generated|generated==script)),"Existing tmux kolira.conf is not a matching core-generated script; preserving personal commands");
            }
        }
        let merged = crate::appearance::applied(resource, &text, previous.as_deref())?;
        app_sync::write(plan, &path, merged)?;
        if *id == "wezterm" || *id == "tmux" {
            if *id == "wezterm"
                && paths::config_home() != paths::home_dir().join(".config")
                && paths::home_dir()
                    .join(".config/wezterm/wezterm.lua")
                    .exists()
                && !paths::live_path(".config/wezterm/wezterm.lua").exists()
            {
                anyhow::bail!("WezTerm uses a legacy HOME/.config fallback outside XDG_CONFIG_HOME; explicit integration required");
            }
            let rel = if *id == "wezterm" {
                if let Some(path) =
                    std::env::var_os("WEZTERM_CONFIG_FILE").filter(|v| !v.is_empty())
                {
                    WEZ_HOSTS
                        .iter()
                        .copied()
                        .find(|p| paths::live_path(p) == Path::new(&path))
                        .context("Unsupported WezTerm config path")?
                } else {
                    host(WEZ_HOSTS, plan)?
                }
            } else {
                host(TMUX_HOSTS, plan)?
            };
            let path = paths::live_path(rel);
            anyhow::ensure!(
                !path.is_symlink(),
                "Linked executable config requires explicit integration"
            );
            let old = plan.text(&path)?.unwrap_or_default();
            app_sync::write(
                plan,
                &path,
                if *id == "wezterm" {
                    wez_host(&old)?
                } else {
                    tmux_host(&old)?
                },
            )?;
            if *id == "tmux" {
                app_sync::write(
                    plan,
                    &paths::live_path(TMUX_SCRIPT),
                    crate::tmux_runtime::script(&text)?,
                )?;
            }
        }
    }
    Ok(())
}

pub fn starship_capture(text: &str) -> Result<String> {
    let doc: toml_edit::DocumentMut = text.parse()?;
    let Some(name) = doc.get("palette").and_then(|v| v.as_str()) else {
        return Ok(String::new());
    };
    let table = doc
        .get("palettes")
        .and_then(|v| v.get(name))
        .and_then(|v| v.as_table_like())
        .context("Missing selected Starship palette")?;
    let mut out = toml_edit::DocumentMut::new();
    out["palette"] = toml_edit::value("kolira");
    for (key, value) in table.iter() {
        native_rgb(
            value
                .as_str()
                .context("Starship palettes must contain RGB data")?,
        )?;
        out["palettes"]["kolira"][key] = value.clone();
    }
    Ok(out.to_string())
}
pub fn foot_merge(source: &str, live: &str) -> Result<String> {
    // Only colour keys leave the theme; includes, alpha, fonts and commands stay local.
    let source = crate::appearance::for_theme(FOOT, source).unwrap_or_default();
    let keys = [
        "background",
        "foreground",
        "selection-background",
        "selection-foreground",
        "cursor",
        "regular0",
        "regular1",
        "regular2",
        "regular3",
        "regular4",
        "regular5",
        "regular6",
        "regular7",
        "bright0",
        "bright1",
        "bright2",
        "bright3",
        "bright4",
        "bright5",
        "bright6",
        "bright7",
    ];
    let mut section = String::new();
    for line in source.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            section = t.into();
            continue;
        }
        if let Some((key, value)) = t.split_once('=') {
            if ["[colors]", "[colors-dark]", "[colors-light]"].contains(&section.as_str())
                && keys.contains(&key.trim())
            {
                let values = value.split_whitespace().collect::<Vec<_>>();
                anyhow::ensure!(
                    values.len() == if key.trim() == "cursor" { 2 } else { 1 },
                    "Invalid Foot colour value"
                );
                for c in values {
                    anyhow::ensure!(
                        c.len() == 6 && c.bytes().all(|b| b.is_ascii_hexdigit()),
                        "Invalid Foot RGB colour"
                    );
                }
            }
        }
    }
    const BEGIN: &str = "# BEGIN kolira Foot colours\n";
    const END: &str = "# END kolira Foot colours\n";
    let mut cleaned = live.to_string();
    if let Some(start) = cleaned.find(BEGIN) {
        anyhow::ensure!(
            cleaned.matches(BEGIN).count() == 1,
            "Duplicate managed Foot block"
        );
        let end = cleaned[start..]
            .find(END)
            .context("Unclosed managed Foot colours")?
            + start
            + END.len();
        let body = &cleaned[start + BEGIN.len()..end - END.len()];
        let owned = body.lines().all(|line| {
            let t = line.trim();
            t.is_empty()
                || ["[colors]", "[colors-dark]", "[colors-light]"].contains(&t)
                || t.split_once('=')
                    .is_some_and(|(k, _)| keys.contains(&k.trim()))
        });
        if owned {
            cleaned.replace_range(start..end, "");
        } else {
            cleaned = cleaned.replace(BEGIN, "").replace(END, "");
        }
    }
    let live = cleaned.as_str();
    let mut colour = false;
    let mut out = String::new();
    for line in live.split_inclusive('\n') {
        let t = line.trim();
        if t.starts_with('[') {
            colour = ["[colors]", "[colors-dark]", "[colors-light]"].contains(&t);
        }
        if colour
            && t.split_once('=')
                .is_some_and(|(key, _)| keys.contains(&key.trim()))
        {
            continue;
        }
        out.push_str(line);
    }
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(BEGIN);
    out.push_str(&source);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(END);
    Ok(out)
}

/// Refuse an older parser before changing its config; NO_APPLY never starts binaries.
fn check_version(id: &str) -> Result<()> {
    if crate::field::no_apply() || which::which(id).is_err() {
        return Ok(());
    }
    let flag = if id == "tmux" { "-V" } else { "--version" };
    let (status, bytes) = paths::output_bounded(std::process::Command::new(id).arg(flag))
        .context("Cannot verify native app version")?;
    anyhow::ensure!(status.success(), "Cannot verify {id} version");
    let text = String::from_utf8_lossy(&bytes);
    let numbers = text
        .split(|c: char| !c.is_ascii_digit() && c != '.')
        .find(|s| s.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .context("Cannot parse native app version")?;
    let mut parts = numbers.split('.').filter_map(|p| p.parse::<u32>().ok());
    let version = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
    let min = match id {
        "foot" => (1, 28),
        "tmux" => (3, 2),
        "starship" => (1, 27),
        _ => (20220807, 0),
    };
    anyhow::ensure!(
        version >= min,
        "Unsupported {id} version {numbers}; requires {}.{} or newer",
        min.0,
        min.1
    );
    Ok(())
}

fn native_rgb(value: &str) -> Result<palette::Rgb> {
    anyhow::ensure!(
        value.len() == 7 && value.starts_with('#'),
        "Native colours must be literal #rrggbb"
    );
    palette::Rgb::parse(value)
}
