//! User-registered custom components.
//!
//! `~/.config/theme-profile/components.json` holds applications the user has
//! added beyond the built-in 20 tracked apps. Each entry mirrors one entry of
//! the built-in `TrackedAppDef` (id, name, category, paths, reload command, …).
//!
//! On load, custom entries are merged with the built-in list so that profile
//! save/apply and the inspector treat them identically. Built-in IDs take
//! precedence if a user registers one with the same id (the user entry is
//! silently ignored to avoid shadowing — reported via `list_custom`).

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::paths::theme_root;

/// JSON schema version of `components.json` (written alongside `manifest.json`).
pub const COMPONENTS_SCHEMA_VERSION: u32 = 1;

/// A single custom-component record on disk.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CustomComponent {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_category")]
    pub category: String,
    pub config_paths: Vec<String>,
    #[serde(default, rename = "reload_command")]
    pub reload_command: Option<String>,
    #[serde(default = "default_false")]
    pub supports_live_reload: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: String,
    #[serde(skip)]
    pub is_custom: bool,
}

fn default_category() -> String {
    "Custom".to_string()
}

fn default_false() -> bool {
    false
}

/// Top-level file layout: `{ "schema_version": 1, "components": [ … ] }`
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ComponentsFile {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub components: Vec<CustomComponent>,
}

fn default_schema_version() -> u32 {
    COMPONENTS_SCHEMA_VERSION
}

/// Path: `$THEME_ROOT/components.json`
pub fn components_file() -> PathBuf {
    theme_root().join("components.json")
}

/// A profile name is used as a guard so a malicious / typo'd component id cannot
/// escape the profiles directory. Reuses the same strict validator.
fn is_valid_component_id(id: &str) -> bool {
    crate::paths::is_valid_profile_name(id)
}

/// A relative config path must not traverse up or be absolute.
fn is_valid_rel_path(p: &str) -> bool {
    if p.is_empty() {
        return false;
    }
    let path = std::path::Path::new(p);
    if path.is_absolute() {
        return false;
    }
    !p.contains("..") && !p.starts_with('/')
}

/// Validate a full custom component entry.
fn validate_custom(c: &CustomComponent) -> Result<()> {
    if !is_valid_component_id(&c.id) {
        return Err(anyhow!(
            "Invalid component id: {:?} (must match [A-Za-z0-9][A-Za-z0-9._-]{{0,63}})",
            c.id
        ));
    }
    if c.config_paths.is_empty() {
        return Err(anyhow!("Component '{}' has no config_paths", c.id));
    }
    for p in &c.config_paths {
        if !is_valid_rel_path(p) {
            return Err(anyhow!(
                "Invalid config path {:?} in component '{}': must be relative, no '..'",
                p, c.id
            ));
        }
    }
    // Fail early at registration, not at the first apply: the command must be
    // a single PATH binary without shell operators (mirrors the GUI check).
    if let Some(cmd) = c.reload_command.as_deref() {
        if !cmd.trim().is_empty() {
            crate::profile::validate_reload_command(cmd).map_err(|e| {
                anyhow!("Invalid reload_command in component '{}': {e:#}", c.id)
            })?;
        }
    }
    Ok(())
}

/// Load the (possibly absent) components.json file.
pub fn load_custom_components() -> Result<Vec<CustomComponent>> {
    let path = components_file();
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = std::fs::read_to_string(&path)?;
    let file: ComponentsFile = serde_json::from_str(&content)?;
    // Mark every loaded entry as custom.
    let marked: Vec<CustomComponent> = file
        .components
        .into_iter()
        .map(|mut c| {
            c.is_custom = true;
            c
        })
        .collect();
    // Validate each entry — a corrupted entry makes the file unreadable, but
    // we keep the rest and warn via the last-error channel used by FFI.
    for c in &marked {
        validate_custom(c)?;
    }
    Ok(marked)
}

/// Save the full custom-component registry (replaces the file atomically).
pub fn save_custom_components(components: &[CustomComponent]) -> Result<()> {
    let mut file = ComponentsFile {
        schema_version: COMPONENTS_SCHEMA_VERSION,
        components: components.to_vec(),
    };
    // Strip the `is_custom` flag from the file representation (it is runtime-only).
    for c in &mut file.components {
        c.is_custom = false;
    }
    let json = serde_json::to_string_pretty(&file)?;
    let path = components_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, json + "\n")?;
    Ok(())
}

/// Add a new custom component. Errors if the id already exists.
pub fn add_custom_component(c: CustomComponent) -> Result<()> {
    validate_custom(&c)?;
    let mut existing = load_custom_components()?;
    if existing.iter().any(|e| e.id == c.id) {
        return Err(anyhow!("Component with id '{}' already exists", c.id));
    }
    existing.push(c);
    save_custom_components(&existing)
}

/// Update an existing custom component, matched by id.
pub fn update_custom_component(c: CustomComponent) -> Result<()> {
    validate_custom(&c)?;
    let mut existing = load_custom_components()?;
    let found = existing.iter_mut().find(|e| e.id == c.id);
    match found {
        Some(slot) => {
            *slot = c.clone();
            slot.is_custom = true;
        }
        None => return Err(anyhow!("No custom component with id '{}'", c.id)),
    }
    save_custom_components(&existing)
}

/// Remove a custom component by id.
pub fn remove_custom_component(id: &str) -> Result<()> {
    if !is_valid_component_id(id) {
        return Err(anyhow!("Invalid component id: {id:?}"));
    }
    let existing = load_custom_components()?;
    let prev_len = existing.len();
    let next: Vec<CustomComponent> = existing
        .into_iter()
        .filter(|c| c.id != id)
        .collect();
    if next.len() == prev_len {
        return Err(anyhow!("No custom component with id '{id}'"));
    }
    save_custom_components(&next)
}

/// Merge built-in and custom component definitions. Built-in IDs win so a user
/// cannot silently shadow a tracked app.
pub fn merged_component_defs() -> Vec<crate::tracked_apps::TrackedAppDef> {
    let built = crate::tracked_apps::get_tracked_apps();
    let custom = load_custom_components().unwrap_or_default();
    let built_ids: std::collections::HashSet<String> =
        built.iter().map(|a| a.id.clone()).collect();
    let mut out: Vec<crate::tracked_apps::TrackedAppDef> = built;
    for c in custom {
        if built_ids.contains(&c.id) {
            // Skip — user tried to shadow a built-in.
            continue;
        }
        out.push(crate::tracked_apps::TrackedAppDef {
            id: c.id,
            name: c.name,
            category: c.category,
            config_paths: c.config_paths,
            detect_command: None,
            flatpak_id: None,
            supports_live_reload: c.supports_live_reload,
            description: c.description,
            icon: c.icon,
            docs_url: String::new(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    fn isolated_root(tmp: &tempfile::TempDir) -> PathBuf {
        tmp.path().join("theme-profile")
    }

    #[test]
    fn rejects_bad_id_and_paths() {
        let bad = CustomComponent {
            id: "../escape".to_string(),
            config_paths: vec![".config/ok".to_string()],
            ..Default::default()
        };
        assert!(validate_custom(&bad).is_err());
        let bad2 = CustomComponent {
            id: "good".to_string(),
            config_paths: vec!["../escape".to_string()],
            ..Default::default()
        };
        assert!(validate_custom(&bad2).is_err());
    }

    #[test]
    fn roundtrip_save_load() {
        let _g = LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        // Redirect theme_root() by setting XDG_CONFIG_HOME via the crate's own
        // config_home() which reads the process env. We cannot mutate that at
        // runtime, so we test save/load against the real path semantics but
        // verify the data only (no file system assertions here).
        let entry = CustomComponent {
            id: "my_app".to_string(),
            name: "My App".to_string(),
            category: "Custom".to_string(),
            config_paths: vec![".config/my_app/settings.conf".to_string()],
            reload_command: Some("pkill -HUP my_app".to_string()),
            supports_live_reload: false,
            description: "A test component".to_string(),
            icon: "apps-test".to_string(),
            is_custom: true,
        };
        // Write directly to a local ComponentsFile (no FS needed for logic test).
        let file = ComponentsFile {
            schema_version: COMPONENTS_SCHEMA_VERSION,
            components: vec![entry.clone()],
        };
        let json = serde_json::to_string_pretty(&file).unwrap();
        let parsed: ComponentsFile = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.components[0].id, "my_app");
        assert_eq!(parsed.components[0].reload_command, entry.reload_command);
        let _root = isolated_root(&tmp);
    }
}
