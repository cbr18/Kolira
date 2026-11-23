//! Explicit local vault selection and portable snapshot slots.
use crate::{change_plan::Plan, paths};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
const SELECTION: &str = "app-selection.json";
pub const SNAPSHOT: &str = ".config/kolira/app-snapshots/obsidian";
pub(crate) const MODE_FIELD: &str = "obsidian_mode";
pub(crate) const THEME_FIELD: &str = "obsidian_theme";
pub(crate) const APPEARANCE: &str = "appearance.json";
pub(crate) const APPEARANCE_KEYS: &[&str] =
    &["baseTheme", "theme", "cssTheme", "enabledCssSnippets"];
const LEGACY_SNAPSHOT: &str = "obsidian/vault/.obsidian";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Override {
    Value(String),
    Exclude,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Overrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mode: Option<Override>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    theme: Option<Override>,
}

impl Overrides {
    pub(crate) fn is_empty(&self) -> bool {
        self.mode.is_none() && self.theme.is_none()
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if let Some(Override::Value(mode)) = &self.mode {
            ensure!(native_mode(mode).is_some(), "Invalid Obsidian color mode");
        }
        if let Some(Override::Value(name)) = &self.theme {
            ensure!(
                name.is_empty() || safe_theme_name(name),
                "Invalid Obsidian theme name"
            );
        }
        Ok(())
    }
}

fn safe_theme_name(name: &str) -> bool {
    !name.trim().is_empty()
        && !name.contains(['/', '\\'])
        && !name.chars().any(char::is_control)
        && name != "."
        && name != ".."
}

fn native_mode(value: &str) -> Option<&'static str> {
    match value {
        "dark" => Some("obsidian"),
        "light" => Some("moonstone"),
        _ => None,
    }
}

fn display_mode(value: &Value) -> Option<&'static str> {
    match value.as_str()? {
        "obsidian" => Some("dark"),
        "moonstone" => Some("light"),
        _ => None,
    }
}

fn saved_mode(value: &serde_json::Map<String, Value>) -> Option<&str> {
    value
        .get("baseTheme")
        .and_then(Value::as_str)
        .or_else(|| value.get("theme").and_then(display_mode))
}

fn palette_mode(palette: &crate::palette::Palette) -> &'static str {
    if palette.slot(0).luminance() < 0.5 {
        "dark"
    } else {
        "light"
    }
}
pub fn vaults() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for index in [
        paths::config_home().join("obsidian/obsidian.json"),
        paths::home_dir().join(".var/app/md.obsidian.Obsidian/config/obsidian/obsidian.json"),
    ] {
        if let Ok(data) = fs::read(&index)
            .and_then(|b| serde_json::from_slice::<Value>(&b).map_err(std::io::Error::other))
        {
            for value in data["vaults"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(_, v)| v)
            {
                if let Some(path) = value["path"].as_str().map(PathBuf::from) {
                    if safe(&path) && !out.contains(&path) {
                        out.push(path);
                    }
                }
            }
        }
    }
    for root in ["obsidian", "Obsidian"] {
        if let Ok(entries) = fs::read_dir(paths::home_dir().join(root)) {
            for item in entries.flatten() {
                let path = item.path();
                if safe(&path) && !out.contains(&path) {
                    out.push(path);
                }
            }
        }
    }
    out.sort();
    out
}
fn safe(path: &Path) -> bool {
    path.strip_prefix(paths::home_dir()).is_ok_and(|p| {
        !p.as_os_str().is_empty()
            && p.components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
    }) && path.join(".obsidian").is_dir()
        && path
            .ancestors()
            .take_while(|p| *p != paths::home_dir())
            .all(|p| !p.is_symlink())
        && !path.join(".obsidian").is_symlink()
}
pub fn selected() -> Vec<PathBuf> {
    let available = vaults();
    match fs::read(paths::theme_root().join(SELECTION)) {
        Ok(bytes) => serde_json::from_slice::<Vec<String>>(&bytes)
            .unwrap_or_default()
            .iter()
            .map(|p| paths::home_dir().join(p))
            .filter(|p| available.contains(p))
            .collect(),
        Err(_) if available.len() == 1 => available,
        _ => Vec::new(),
    }
}
pub fn rpc(method: &str, args: &Value) -> Result<Value> {
    if method == "select_vaults" {
        let _guard = crate::operation::MutationGuard::acquire()?;
        let chosen: Vec<String> = serde_json::from_value(args["vaults"].clone())?;
        let all = vaults();
        let mut unique = Vec::new();
        for value in chosen {
            anyhow::ensure!(
                !value.is_empty()
                    && !value.contains('\\')
                    && Path::new(&value)
                        .components()
                        .all(|c| matches!(c, std::path::Component::Normal(_))),
                "Vault must be relative to HOME: {value}"
            );
            let path = paths::home_dir().join(&value);
            anyhow::ensure!(
                all.contains(&path) && safe(&path),
                "Unknown/unsafe vault: {value}"
            );
            if !unique.contains(&value) {
                unique.push(value);
            }
        }
        crate::gui_api::write_bytes(
            &paths::theme_root().join(SELECTION),
            &serde_json::to_vec_pretty(&unique)?,
            Some(0o600),
        )?;
    }
    Ok(
        json!({"vaults":vaults().iter().filter_map(|p|p.strip_prefix(paths::home_dir()).ok()).collect::<Vec<_>>(),"selected":selected().iter().filter_map(|p|p.strip_prefix(paths::home_dir()).ok()).collect::<Vec<_>>()}),
    )
}
pub fn config_paths() -> Vec<String> {
    let mut out = Vec::new();
    for vault in selected() {
        for suffix in ["appearance.json", "snippets/kolira.css"] {
            if let Ok(path) = vault
                .join(".obsidian")
                .join(suffix)
                .strip_prefix(paths::home_dir())
            {
                out.push(path.to_string_lossy().to_string());
            }
        }
        let config = vault.join(".obsidian/appearance.json");
        if let Some(name) = fs::read(config)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .and_then(|v| v["cssTheme"].as_str().map(str::to_string))
        {
            if !name.is_empty()
                && Path::new(&name)
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_)))
                && !name.contains('/')
                && !name.contains('\\')
            {
                let theme = vault.join(".obsidian/themes").join(name);
                if theme.is_dir() {
                    out.push(
                        theme
                            .strip_prefix(paths::home_dir())
                            .unwrap()
                            .to_string_lossy()
                            .to_string(),
                    );
                }
            }
        }
    }
    out
}
pub fn snapshot_rel(rel: &str) -> Option<String> {
    for (i, vault) in selected().iter().enumerate() {
        let base = vault
            .strip_prefix(paths::home_dir())
            .ok()?
            .join(".obsidian");
        if let Ok(suffix) = Path::new(rel).strip_prefix(base) {
            return Some(format!("{SNAPSHOT}/{i}/{}", suffix.display()));
        }
    }
    None
}

fn appearance_rel(vault: &Path) -> Result<String> {
    Ok(vault
        .strip_prefix(paths::home_dir())?
        .join(".obsidian")
        .join(APPEARANCE)
        .to_str()
        .context("Invalid Obsidian vault path")?
        .to_owned())
}

pub(crate) fn is_appearance(rel: &str) -> bool {
    rel.ends_with("/.obsidian/appearance.json")
}

pub(crate) fn is_theme_resource(rel: &str) -> bool {
    rel.contains("/.obsidian/themes/")
}

fn source_path_safe(source: &Path, path: &Path) -> bool {
    path.starts_with(source)
        && path
            .ancestors()
            .take_while(|p| *p != source)
            .all(|p| !p.is_symlink())
}

fn slot_roots(source: &Path) -> Vec<PathBuf> {
    let base = source.join(SNAPSHOT);
    if !source_path_safe(source, &base) {
        return Vec::new();
    }
    let mut slots: Vec<_> = fs::read_dir(base)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.parse::<usize>().is_ok())
        })
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && !path.is_symlink())
        .collect();
    slots.sort();
    slots
}

fn bundled_roots(source: &Path) -> Vec<PathBuf> {
    let mut roots = slot_roots(source);
    let legacy = source.join(LEGACY_SNAPSHOT);
    if legacy.is_dir() && source_path_safe(source, &legacy) {
        roots.push(legacy);
    }
    roots
}

pub(crate) fn snapshot_appearances(source: &Path) -> Result<Vec<PathBuf>> {
    let mut files: BTreeSet<_> = slot_roots(source)
        .into_iter()
        .map(|slot| slot.join(APPEARANCE))
        .filter(|path| path.is_file())
        .collect();
    for vault in selected() {
        let path = crate::profile::theme_file(source, &appearance_rel(&vault)?);
        if path.is_file() {
            files.insert(path);
        }
    }
    for file in &files {
        ensure!(
            source_path_safe(source, file),
            "Symlink in Obsidian snapshot"
        );
    }
    Ok(files.into_iter().collect())
}

fn parse_appearance(text: &str) -> Result<serde_json::Map<String, Value>> {
    match serde_json::from_str(text).context("Invalid Obsidian appearance.json")? {
        Value::Object(value) => Ok(value),
        _ => bail!("Obsidian appearance must be an object"),
    }
}

fn theme_dir(root: &Path, name: &str) -> Option<PathBuf> {
    if !safe_theme_name(name) {
        return None;
    }
    let themes = root.join("themes");
    let theme = themes.join(name);
    let css = theme.join("theme.css");
    (!themes.is_symlink() && !theme.is_symlink() && !css.is_symlink() && css.is_file())
        .then_some(theme)
}

pub(crate) fn capture_draft_theme(
    value: &str,
    target: &Path,
    snapshot: &Path,
    plan: &mut Plan,
) -> Result<()> {
    if value != "obsidian" {
        let source = theme_dir(target.parent().context("Obsidian config directory")?, value)
            .context("Selected Obsidian theme is unavailable")?;
        let dest = snapshot
            .parent()
            .context("Obsidian snapshot directory")?
            .join("themes")
            .join(value);
        crate::app_sync::validate_theme_tree(&source)?;
        plan.copy(&source, &dest, "capture")?;
    }
    Ok(())
}

fn choice_catalog(source: &Path, current: &Value) -> Value {
    let slots = bundled_roots(source);
    let live: Vec<_> = selected()
        .into_iter()
        .map(|vault| vault.join(".obsidian"))
        .collect();
    let mut names = BTreeMap::new();
    for root in slots.iter().chain(&live) {
        let themes = root.join("themes");
        if themes.is_symlink() {
            continue;
        }
        for entry in fs::read_dir(themes).into_iter().flatten().flatten() {
            let Some(name) = entry.file_name().into_string().ok() else {
                continue;
            };
            let Some(dir) = theme_dir(root, &name) else {
                continue;
            };
            let manifest = dir.join("manifest.json");
            let label = if manifest.is_file() && !manifest.is_symlink() {
                fs::read(manifest)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                    .and_then(|value| value["name"].as_str().map(str::to_owned))
            } else {
                None
            };
            names
                .entry(name.clone())
                .or_insert_with(|| label.unwrap_or(name));
        }
    }
    if let Some(name) = current.as_str().filter(|name| !name.is_empty()) {
        names.entry(name.into()).or_insert_with(|| name.into());
    }
    let mut choices = vec![json!({"value":"","label":"Default theme","available":true})];
    for (name, label) in names {
        let available = slots.iter().any(|root| theme_dir(root, &name).is_some())
            || !live.is_empty() && live.iter().all(|root| theme_dir(root, &name).is_some());
        choices.push(json!({"value":name,"label":label,"available":available}));
    }
    json!(choices)
}

pub(crate) fn editor(
    source: &Path,
    overrides: &Overrides,
    palette: Option<&crate::palette::Palette>,
) -> Result<(Value, Value)> {
    let snapshots = snapshot_appearances(source)?
        .iter()
        .map(|file| parse_appearance(&fs::read_to_string(file)?))
        .collect::<Result<Vec<_>>>()?;
    let common = |unpack: fn(&serde_json::Map<String, Value>) -> Option<&str>| {
        let mut values = snapshots.iter().map(unpack);
        let first = values.next().flatten();
        if values.all(|value| value == first) {
            json!(first)
        } else {
            json!({"mixed":true})
        }
    };
    let value = |edit: &Option<Override>, inherited: Value| match edit {
        Some(Override::Value(value)) => json!(value),
        Some(Override::Exclude) => Value::Null,
        None => inherited,
    };
    let inherited_mode = if snapshots.is_empty() {
        json!(palette.map(palette_mode))
    } else {
        common(saved_mode)
    };
    let mode = value(&overrides.mode, inherited_mode);
    let theme = value(
        &overrides.theme,
        common(|map| map.get("cssTheme").and_then(Value::as_str)),
    );
    let choices = json!({
        (MODE_FIELD): [
            {"value":"light","label":"Light","available":true},
            {"value":"dark","label":"Dark","available":true},
        ],
        (THEME_FIELD): choice_catalog(source, &theme),
    });
    Ok((json!({(MODE_FIELD):mode,(THEME_FIELD):theme}), choices))
}

pub(crate) fn edit(
    current: &Overrides,
    base: &Value,
    changes: &serde_json::Map<String, Value>,
) -> Result<Overrides> {
    let mut result = current.clone();
    for (field, entry) in [
        (MODE_FIELD, &mut result.mode),
        (THEME_FIELD, &mut result.theme),
    ] {
        if let Some(value) = changes.get(field).filter(|value| **value != base[field]) {
            *entry = Some(match value {
                Value::Null => Override::Exclude,
                Value::String(value) => Override::Value(value.to_owned()),
                _ => bail!("Invalid value for {field}"),
            });
        }
    }
    result.validate()?;
    Ok(result)
}

fn read_theme<'a>(
    appearance: &'a serde_json::Map<String, Value>,
    overrides: &'a Overrides,
) -> Option<&'a str> {
    match &overrides.theme {
        Some(Override::Exclude) => None,
        Some(Override::Value(name)) => Some(name),
        None => appearance.get("cssTheme").and_then(Value::as_str),
    }
}

fn bundled_theme(source: &Path, config: &Path, name: &str) -> Option<PathBuf> {
    let same_vault = config.parent().and_then(|root| theme_dir(root, name));
    same_vault
        .filter(|path| source_path_safe(source, path))
        .or_else(|| {
            bundled_roots(source)
                .iter()
                .find_map(|root| theme_dir(root, name))
        })
}

pub(crate) fn missing_themes(source: &Path, overrides: &Overrides) -> Result<BTreeSet<String>> {
    let mut missing = BTreeSet::new();
    for vault in selected() {
        let snapshot = crate::profile::theme_file(source, &appearance_rel(&vault)?);
        let saved = if snapshot.is_file() && source_path_safe(source, &snapshot) {
            parse_appearance(&fs::read_to_string(&snapshot)?)?
        } else {
            Default::default()
        };
        if let Some(name) = read_theme(&saved, overrides).filter(|name| !name.is_empty()) {
            if theme_dir(&vault.join(".obsidian"), name).is_none()
                && bundled_theme(source, &snapshot, name).is_none()
            {
                missing.insert(name.into());
            }
        }
    }
    Ok(missing)
}

pub(crate) fn plan_saved(
    source: &Path,
    rel: &str,
    overrides: &Overrides,
    plan: &mut Plan,
) -> Result<()> {
    let snapshot = crate::profile::theme_file(source, rel);
    let dst = paths::live_path(rel);
    plan.read(&paths::theme_root().join(SELECTION))?;
    ensure!(
        source_path_safe(source, &snapshot),
        "Symlink in Obsidian snapshot"
    );
    let mut saved = plan
        .text(&snapshot)?
        .map(|text| parse_appearance(&text))
        .transpose()?
        .unwrap_or_default();
    let theme = read_theme(&saved, overrides).map(str::to_owned);
    saved.remove("cssTheme");
    if let Some(name) = theme {
        let live_root = dst.parent().context("Obsidian config directory")?;
        let usable = if name.is_empty() {
            true
        } else if let Some(resource) = bundled_theme(source, &snapshot, &name) {
            let target = live_root.join("themes").join(&name);
            crate::app_sync::validate_live(&target)?;
            crate::app_sync::validate_theme_tree(&resource)?;
            if fs::symlink_metadata(&target).is_ok() {
                crate::app_sync::validate_theme_tree(&target)?;
            }
            plan.copy(&resource, &target, "apply")?;
            true
        } else if safe_theme_name(&name) {
            let css = live_root.join("themes").join(&name).join("theme.css");
            crate::app_sync::validate_live(&css).is_ok() && plan.is_file(&css)?
        } else {
            false
        };
        if usable {
            saved.insert("cssTheme".into(), json!(name));
        }
    }
    let mode = match &overrides.mode {
        Some(Override::Value(value)) => Some(value.clone()),
        Some(Override::Exclude) => {
            saved.remove("theme");
            saved.remove("baseTheme");
            None
        }
        None => saved_mode(&saved)
            .filter(|mode| native_mode(mode).is_some())
            .map(str::to_owned),
    };
    if let Some(mode) = mode {
        saved.insert("theme".into(), json!(native_mode(&mode)));
        saved.insert("baseTheme".into(), json!(mode));
    }
    saved.retain(|key, _| APPEARANCE_KEYS.contains(&key.as_str()));
    if saved.is_empty() {
        return Ok(());
    }
    crate::app_sync::validate_live(&dst)?;
    let old = plan.text(&dst)?.unwrap_or_else(|| "{}".into());
    parse_appearance(&old)?;
    let text =
        crate::appearance::jsonc_overlay(&serde_json::to_string(&saved)?, &old, APPEARANCE_KEYS);
    if text != old {
        plan.write_text(&dst, text, "apply")?;
    }
    Ok(())
}

pub fn plan(
    source: &Path,
    palette: &crate::palette::Palette,
    overrides: &Overrides,
    plan: &mut Plan,
) -> Result<()> {
    let chosen = selected();
    anyhow::ensure!(
        !chosen.is_empty() || vaults().is_empty(),
        "Select Obsidian vaults with app.select_vaults before applying"
    );
    for vault in chosen {
        let rel = appearance_rel(&vault)?;
        if crate::profile::theme_file(source, &rel).exists() {
            continue;
        }
        let config = vault.join(".obsidian/appearance.json");
        let old = plan.text(&config)?.unwrap_or_else(|| "{}".into());
        let mut value: Value =
            serde_json::from_str(&old).context("Invalid Obsidian appearance.json")?;
        anyhow::ensure!(value.is_object(), "Obsidian appearance must be an object");
        let mut enabled = value["enabledCssSnippets"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if !enabled.iter().any(|v| v == "kolira") {
            enabled.push(json!("kolira"));
        }
        value["enabledCssSnippets"] = json!(enabled);
        let text = crate::field::json_set(
            &old,
            &["enabledCssSnippets".into()],
            &value["enabledCssSnippets"],
        )?;
        let text = if overrides.mode.is_none() {
            let mode = palette_mode(palette);
            crate::appearance::jsonc_overlay(
                &json!({"theme":native_mode(mode),"baseTheme":mode}).to_string(),
                &text,
                &["baseTheme", "theme"],
            )
        } else {
            text
        };
        crate::app_sync::write(plan, &config, text)?;
        let css=format!("/* Kolira colours only */\nbody {{ --background-primary: {}; --background-secondary: {}; --text-normal: {}; --text-accent: {}; --interactive-accent: {}; }}\n",palette.base[0],palette.base[1],palette.base[5],palette.accent,palette.accent);
        crate::app_sync::write(plan, &vault.join(".obsidian/snippets/kolira.css"), css)?;
    }
    Ok(())
}
