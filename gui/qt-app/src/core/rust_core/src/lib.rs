//! KDE Theme Profile Manager - Rust Core Library
//!
//! Provides the native Rust implementation of the theme management protocol.
//! The GUI talks to it through the CLI binaries in `src/bin/`
//! (`theme-profile`, `theme-menu`, `wallpaper-*`), which share this library.
//! The former `libkde_theme_core.so` FFI layer was removed together with the
//! second (library) core — the application has a single core now.

pub mod types;
pub mod paths;
pub mod profile;
pub mod wallpaper;
pub mod palette;
pub mod oklab;
pub mod extract;
pub mod kvantum;
pub mod gallery;
pub mod tracked_apps;
pub mod custom_components;
pub mod config_parser;
pub mod status;

#[cfg(test)]
mod tests;

// Re-export main types
pub use types::{AppStatus, Component, ProfileInfo};
