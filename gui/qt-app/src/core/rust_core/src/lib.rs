//! KDE Theme Profile Manager - Rust Core Library
//!
//! Provides the native Rust implementation of the theme management protocol.
//! The GUI talks to it through the CLI binaries in `src/bin/`
//! (`kolira`, `theme-menu`, `wallpaper-*`), which share this library.
//! The former `libkolira_core.so` FFI layer was removed together with the
//! second (library) core — the application has a single core now.

pub mod actions;
pub mod appearance;
pub mod automation;
pub mod config_parser;
pub mod constructor;
pub mod custom_components;
pub mod dependency_jobs;
pub mod dependency_registry;
pub mod deps;
pub mod discord;
pub mod extract;
pub mod extras;
pub mod field;
pub mod gallery;
pub mod konsole;
pub mod kvantum;
pub mod kwin_effects;
pub mod layout;
pub mod net;
pub mod oklab;
pub mod palette;
pub mod paths;
pub mod profile;
pub mod share;
pub mod spicetify;
pub mod status;
pub mod stores;
pub mod telegram;
pub mod theme;
pub mod tracked_apps;
pub mod types;
pub mod undo;
pub mod wallpaper;

#[cfg(test)]
mod tests;

// Re-export main types
pub use types::{AppStatus, Component, ProfileInfo};

pub mod app_themes;
pub mod bundle_view;
pub mod config_edit;
pub mod gui_api;
pub mod inspector_session;
pub mod inspector_view;
pub mod resource_clone;

pub mod operation;

pub mod change_plan;

pub mod preview;

pub mod schedule;

pub mod agent;
