//! Common Types - Shared data structures across the crate

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppStatus {
    Available,
    NotInstalled,
    Partial,
}

/// Component/Application definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub id: String,
    pub name: String,
    pub category: String,
    pub config_paths: Vec<PathBuf>,
    pub status: AppStatus,
    pub supports_live_reload: bool,
    pub description: String,
    pub icon: String,
    pub docs_url: String,
}

/// Profile information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileInfo {
    pub name: String,
    pub is_active: bool,
    pub components: Vec<String>,
    pub created_at: String,
    pub description: String,
    pub wallpaper_path: String,
}

/// A reference to an editor theme carried inside a theme package.
///
/// The theme either downloads the `.vsix` / extension archive on first save
/// and caches it under `assets/editor-themes/`, or the author bundles it
/// directly in the repo/archive. Applying installs it from the local cache
/// (offline) or from the official store if the cache is missing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct EditorTheme {
    /// Editor: `vscode`, `cursor`, `vscodium`, `windsurf`, `zed`.
    pub editor: String,
    /// Store: `vscode-marketplace`, `open-vsx`, `zed`, `url`, `local`.
    pub store: String,
    /// Extension id (`publisher.name`) or Zed extension id.
    pub id: String,
    /// Version hint (may be empty).
    pub version: String,
    /// Download URL (for `url`/`vscode-marketplace`/`open-vsx`/`zed` stores).
    pub url: String,
    /// Path inside the theme package: `assets/editor-themes/<editor>/<id>/`.
    /// Empty when the theme only references a store id (download on demand).
    #[serde(default)]
    pub local_path: String,
    /// When true, create a native extension folder with `"_watch": true` so
    /// the editor reloads the theme without a window restart.
    #[serde(default)]
    pub native_watch: bool,
    /// When false, this editor theme is listed but not installed on apply.
    /// Useful to keep a reference without forcing the editor theme on every
    /// machine. Defaults to true.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}
