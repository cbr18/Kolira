//! Explicit local vault selection and portable snapshot slots.
use crate::{change_plan::Plan, paths};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};
const SELECTION: &str = "app-selection.json";
pub const SNAPSHOT: &str = ".config/kolira/app-snapshots/obsidian";
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
pub fn plan(source: &Path, palette: &crate::palette::Palette, plan: &mut Plan) -> Result<()> {
    let chosen = selected();
    anyhow::ensure!(
        !chosen.is_empty() || vaults().is_empty(),
        "Select Obsidian vaults with app.select_vaults before applying"
    );
    for vault in chosen {
        let rel = vault
            .strip_prefix(paths::home_dir())
            .context("Vault outside HOME")?
            .join(".obsidian/appearance.json");
        if crate::profile::theme_file(source, rel.to_str().context("Vault path")?).exists() {
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
        let text = crate::field::json_set(
            &text,
            &["theme".into()],
            &json!(if palette.slot(0).luminance() < 0.5 {
                "obsidian"
            } else {
                "moonstone"
            }),
        )?;
        crate::app_sync::write(plan, &config, text)?;
        let css=format!("/* Kolira colours only */\nbody {{ --background-primary: {}; --background-secondary: {}; --text-normal: {}; --text-accent: {}; --interactive-accent: {}; }}\n",palette.base[0],palette.base[1],palette.base[5],palette.accent,palette.accent);
        crate::app_sync::write(plan, &vault.join(".obsidian/snippets/kolira.css"), css)?;
    }
    Ok(())
}
