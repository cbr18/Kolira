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
