//! Tracked Apps Core Implementation

use std::path::PathBuf;
use std::process::Command;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use which;

use crate::custom_components::merged_component_defs;
use crate::paths::live_path;

/// Auto-detect the Zen Browser profile directory name inside `~/.zen`.
///
/// The profile path used to be hardcoded (`.zen/gp0y8wrv.Default (release)/`),
/// which broke after a profile rename or on other machines. Resolution rules
/// (shared with the CLI adapter):
///   * scan `~/.zen` for directories containing "Default" with a `user.js`;
///   * pick the first one in byte order (deterministic);
///   * fall back to the historical hardcoded name when nothing matches.
pub fn zen_profile_dir() -> String {
    let home = crate::paths::home_dir();
    let zen_root = home.join(".zen");
    let mut candidates: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&zen_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            let dir_name = entry.file_name().to_string_lossy().to_string();
            if dir_name.contains("Default") && path.join("user.js").is_file() {
                candidates.push(dir_name);
            }
        }
    }
    candidates.sort();
    candidates
        .into_iter()
        .next()
        .unwrap_or_else(|| "gp0y8wrv.Default (release)".to_string())
}

/// Relative config paths for the detected Zen profile.
pub fn zen_config_paths() -> Vec<String> {
    let dir = zen_profile_dir();
    let rel = |suffix: &str| format!(".zen/{dir}/{suffix}");
    vec![
        rel("user.js"),
        rel("chrome/userChrome.css"),
        rel("chrome/userContent.css"),
        rel("chrome/zen-themes.css"),
    ]
}

/// Tracked application definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedAppDef {
    pub id: String,
    pub name: String,
    pub category: String,
    pub config_paths: Vec<String>,
    pub detect_command: Option<String>,
    pub flatpak_id: Option<String>,
    pub supports_live_reload: bool,
    pub description: String,
    pub icon: String,
    pub docs_url: String,
}

/// All tracked applications
/// Component id → config paths, for profile-content detection. Built-in +
/// user-registered custom components are merged so new registrations are picked up
/// the next time a profile is saved.
pub fn component_config_paths() -> Vec<(String, Vec<String>)> {
    let all = merged_component_defs();
    all
        .into_iter()
        .map(|app| (app.id, app.config_paths))
        .collect()
}

pub fn get_tracked_apps() -> Vec<TrackedAppDef> {
    vec![
        TrackedAppDef {
            id: "kwin".to_string(),
            name: "KWin".to_string(),
            category: "KDE / Plasma".to_string(),
            config_paths: vec![".config/kwinrc".to_string(), ".config/kwinrulesrc".to_string()],
            detect_command: None,
            flatpak_id: None,
            supports_live_reload: false,
            description: "KWin window manager: effects, rules, blur, glass".to_string(),
            icon: "preferences-system-windows".to_string(),
            docs_url: "".to_string(),
        },
        TrackedAppDef {
            id: "plasma_shell".to_string(),
            name: "Plasma Shell".to_string(),
            category: "KDE / Plasma".to_string(),
            config_paths: vec![
                ".config/plasmarc".to_string(),
                ".config/plasmashellrc".to_string(),
                ".config/plasma-org.kde.plasma.desktop-appletsrc".to_string(),
            ],
            detect_command: None,
            flatpak_id: None,
            supports_live_reload: false,
            description: "Plasma panels, widgets, desktop applets".to_string(),
            icon: "plasma".to_string(),
            docs_url: "".to_string(),
        },
        TrackedAppDef {
            id: "kdeglobals".to_string(),
            name: "KDE Globals".to_string(),
            category: "KDE / Plasma".to_string(),
            // kcminputrc: only the cursor theme/size (appearance.rs).
            config_paths: vec![".config/kdeglobals".to_string(), ".config/kcminputrc".to_string()],
            detect_command: None,
            flatpak_id: None,
            supports_live_reload: false,
            description: "Colors, accent, Look & Feel, color scheme, widget style".to_string(),
            icon: "preferences-desktop-theme".to_string(),
            docs_url: "".to_string(),
        },
        TrackedAppDef {
            id: "gtk3".to_string(),
            name: "GTK3".to_string(),
            category: "GTK / Qt".to_string(),
            config_paths: vec![
                ".config/gtk-3.0/settings.ini".to_string(),
                ".config/gtk-3.0/gtk.css".to_string(),
                ".config/gtk-3.0/colors.css".to_string(),
            ],
            detect_command: None,
            flatpak_id: None,
            supports_live_reload: false,
            description: "GTK3 theme, custom CSS, colors".to_string(),
            icon: "preferences-desktop-theme-gtk".to_string(),
            docs_url: "".to_string(),
        },
        TrackedAppDef {
            id: "gtk4".to_string(),
            name: "GTK4".to_string(),
            category: "GTK / Qt".to_string(),
            config_paths: vec![
                ".config/gtk-4.0/settings.ini".to_string(),
                ".config/gtk-4.0/gtk.css".to_string(),
                ".config/gtk-4.0/colors.css".to_string(),
            ],
            detect_command: None,
            flatpak_id: None,
            supports_live_reload: false,
            description: "GTK4 theme, custom CSS, colors".to_string(),
            icon: "preferences-desktop-theme-gtk".to_string(),
            docs_url: "".to_string(),
        },
        TrackedAppDef {
            id: "qt5ct".to_string(),
            name: "Qt5/Qt6 Style".to_string(),
            category: "GTK / Qt".to_string(),
            // Only Trolltech.conf (the Qt palette KDE derives from the colour
            // scheme). QtProject.conf is file-dialog state (last folder, view
            // mode) — not appearance; restoring it rolled the dialog back.
            config_paths: vec![".config/Trolltech.conf".to_string()],
            detect_command: None,
            flatpak_id: None,
            supports_live_reload: false,
            description: "Qt5/Qt6 widget style, font, icon theme".to_string(),
            icon: "qtlogo".to_string(),
            docs_url: "".to_string(),
        },
        TrackedAppDef {
            id: "breeze".to_string(),
            name: "Breeze style".to_string(),
            category: "Window Decorations".to_string(),
            config_paths: vec![".config/breezerc".to_string()],
            detect_command: None,
            flatpak_id: None,
            supports_live_reload: false,
            description: "Breeze widget style: menu transparency and other style options".to_string(),
            icon: "preferences-desktop-theme".to_string(),
            docs_url: "".to_string(),
        },
        TrackedAppDef {
            id: "klassy".to_string(),
            name: "Klassy".to_string(),
            category: "Window Decorations".to_string(),
            config_paths: vec![
                ".config/klassy/klassyrc".to_string(),
                ".config/klassy/windecopresetsrc".to_string(),
            ],
            detect_command: Some("klassy".to_string()),
            flatpak_id: None,
            supports_live_reload: false,
            description: "KWin decoration: titlebar opacity, corner radius, buttons".to_string(),
            icon: "preferences-system-windows".to_string(),
            docs_url: "https://github.com/paul-g/klassy".to_string(),
        },
        TrackedAppDef {
            id: "kvantum".to_string(),
            name: "Kvantum".to_string(),
            category: "Window Decorations".to_string(),
            config_paths: vec![
                ".config/Kvantum/kvantum.kvconfig".to_string(),
                ".config/Kvantum/Sandstone".to_string(),
            ],
            detect_command: Some("kvantummanager".to_string()),
            flatpak_id: None,
            supports_live_reload: false,
            description: "SVG-based theme engine for Qt/KDE".to_string(),
            icon: "preferences-system-windows".to_string(),
            docs_url: "https://github.com/tsujan/Kvantum".to_string(),
        },
        TrackedAppDef {
            id: "kitty".to_string(),
            name: "Kitty".to_string(),
            category: "Terminals".to_string(),
            // kitty.conf is only an include host (never stored); the look lives
            // in theme-profile.conf (appearance.rs). Host first: apply derives
            // the include from old themes before the include entry runs.
            config_paths: vec![
                ".config/kitty/kitty.conf".to_string(),
                ".config/kitty/theme-profile.conf".to_string(),
            ],
            detect_command: Some("kitty".to_string()),
            flatpak_id: None,
            supports_live_reload: true,
            description: "GPU-accelerated terminal with live config reload".to_string(),
            icon: "utilities-terminal".to_string(),
            docs_url: "https://sw.kovidgoyal.net/kitty/".to_string(),
        },
        TrackedAppDef {
            id: "alacritty".to_string(),
            name: "Alacritty".to_string(),
            category: "Terminals".to_string(),
            config_paths: vec![
                ".config/alacritty/alacritty.toml".to_string(),
                ".config/alacritty/theme-profile.toml".to_string(),
                ".config/alacritty/themes/noctalia.toml".to_string(),
            ],
            detect_command: Some("alacritty".to_string()),
            flatpak_id: None,
            supports_live_reload: false,
            description: "GPU-accelerated terminal, TOML config".to_string(),
            icon: "utilities-terminal".to_string(),
            docs_url: "https://github.com/alacritty/alacritty".to_string(),
        },
        TrackedAppDef {
            id: "wofi".to_string(),
            name: "Wofi".to_string(),
            category: "Launchers / Menu".to_string(),
            config_paths: vec![
                ".config/wofi/style.css".to_string(),
                ".config/wofi/wallpaper.css".to_string(),
            ],
            detect_command: Some("wofi".to_string()),
            flatpak_id: None,
            supports_live_reload: false,
            description: "Application launcher / menu (GTK)".to_string(),
            icon: "system-run".to_string(),
            docs_url: "https://hg.sr.ht/~scoopta/wofi".to_string(),
        },
        TrackedAppDef {
            id: "btop".to_string(),
            name: "btop".to_string(),
            category: "Monitors".to_string(),
            config_paths: vec![
                ".config/btop/btop.conf".to_string(),
                ".config/btop/themes/noctalia.theme".to_string(),
                ".config/btop/themes/paper.theme".to_string(),
            ],
            detect_command: Some("btop".to_string()),
            flatpak_id: None,
            supports_live_reload: false,
            description: "Resource monitor with themes".to_string(),
            icon: "utilities-system-monitor".to_string(),
            docs_url: "https://github.com/aristocratos/btop".to_string(),
        },
        TrackedAppDef {
            id: "fastfetch".to_string(),
            name: "Fastfetch".to_string(),
            category: "Sysinfo".to_string(),
            config_paths: vec![
                ".config/fastfetch/config.jsonc".to_string(),
                ".config/fastfetch/logo".to_string(),
                ".config/fastfetch/cat-source.jpg".to_string(),
            ],
            detect_command: Some("fastfetch".to_string()),
            flatpak_id: None,
            supports_live_reload: false,
            description: "System info tool with custom logo/config".to_string(),
            icon: "preferences-desktop-theme".to_string(),
            docs_url: "https://github.com/fastfetch-cli/fastfetch".to_string(),
        },
        TrackedAppDef {
            id: "zed".to_string(),
            name: "Zed".to_string(),
            category: "Editors".to_string(),
            config_paths: vec![
                ".config/zed/settings.json".to_string(),
                ".config/zed/themes".to_string(),
            ],
            detect_command: Some("zeditor|zed".to_string()),
            flatpak_id: None,
            supports_live_reload: false,
            description: "Code editor with native themes".to_string(),
            icon: "text-editor".to_string(),
            docs_url: "https://zed.dev/".to_string(),
        },
        TrackedAppDef {
            id: "vscode".to_string(),
            name: "VS Code".to_string(),
            category: "Editors".to_string(),
            config_paths: vec![".config/Code/User/settings.json".to_string()],
            detect_command: Some("code|code-oss|codium|vscodium".to_string()),
            flatpak_id: None,
            supports_live_reload: false,
            description: "VS Code settings (theme, font, etc.)".to_string(),
            icon: "visual-studio-code".to_string(),
            docs_url: "https://code.visualstudio.com/".to_string(),
        },
        TrackedAppDef {
            id: "cursor".to_string(),
            name: "Cursor".to_string(),
            category: "Editors".to_string(),
            config_paths: vec![".config/Cursor/User/settings.json".to_string()],
            detect_command: Some("cursor".to_string()),
            flatpak_id: None,
            supports_live_reload: false,
            description: "Cursor editor settings".to_string(),
            icon: "text-editor".to_string(),
            docs_url: "https://cursor.sh/".to_string(),
        },
        TrackedAppDef {
            id: "zen".to_string(),
            name: "Zen Browser".to_string(),
            category: "Browser".to_string(),
            config_paths: zen_config_paths(),
            detect_command: None,
            flatpak_id: Some("app.zen_browser.zen".to_string()),
            supports_live_reload: false,
            description: "Firefox-based browser with user.js + userChrome.css theming".to_string(),
            icon: "web-browser".to_string(),
            docs_url: "https://zen-browser.app/".to_string(),
        },
        TrackedAppDef {
            id: "obsidian".to_string(),
            name: "Obsidian".to_string(),
            category: "Notes".to_string(),
            // The look lives in the vault's appearance.json; obsidian.json (vault
            // list) and Preferences (spellcheck, zoom) are not appearance.
            config_paths: vec!["obsidian/vault/.obsidian/appearance.json".to_string()],
            detect_command: Some("obsidian".to_string()),
            flatpak_id: Some("md.obsidian.Obsidian".to_string()),
            supports_live_reload: false,
            description: "Markdown notes with theme/appearance settings".to_string(),
            icon: "x-office-note".to_string(),
            docs_url: "https://obsidian.md/".to_string(),
        },
        TrackedAppDef {
            id: "color_schemes".to_string(),
            name: "Color Schemes".to_string(),
            category: "System Profiles".to_string(),
            config_paths: vec![".local/share/color-schemes".to_string()],
            detect_command: None,
            flatpak_id: None,
            supports_live_reload: false,
            description: "KDE color scheme files".to_string(),
            icon: "preferences-desktop-color".to_string(),
            docs_url: "".to_string(),
        },
        TrackedAppDef {
            id: "palette".to_string(),
            name: "Palette".to_string(),
            category: "System Profiles".to_string(),
            config_paths: vec![".config/theme-profile/palette.json".to_string()],
            detect_command: None,
            flatpak_id: None,
            supports_live_reload: false,
            description: "base16 palette the generated theme was built from".to_string(),
            icon: "color-management".to_string(),
            docs_url: "".to_string(),
        },
        TrackedAppDef {
            id: "look_and_feel".to_string(),
            name: "Look & Feel".to_string(),
            category: "System Profiles".to_string(),
            config_paths: vec![
                ".local/share/plasma/look-and-feel/glass".to_string(),
                ".local/share/plasma/look-and-feel/paper".to_string(),
            ],
            detect_command: None,
            flatpak_id: None,
            supports_live_reload: false,
            description: "Plasma Look & Feel packages (glass/paper)".to_string(),
            icon: "preferences-desktop-theme".to_string(),
            docs_url: "".to_string(),
        },
    ]
}

/// Detect status of a single app
///
/// Mirrors Python's `detect_app_status`: the flatpak id is only consulted when
/// a `detect_command` is missing, otherwise apps that are installed outside
/// flatpak (e.g. zen with a local ~/.zen profile) were reported as
/// "not_installed" even though their configs are right there.
fn detect_app_status(app: &TrackedAppDef) -> crate::types::AppStatus {
    if let Some(cmd) = &app.detect_command {
        // `a|b`: any alternative counts (Zed ships as `zeditor` on Arch).
        let found = cmd
            .split('|')
            .filter_map(|alt| alt.split_whitespace().next())
            .any(|bin| which::which(bin).is_ok());
        if !found {
            let via_flatpak = app
                .flatpak_id
                .as_ref()
                .map(|id| is_flatpak_installed(id))
                .unwrap_or(false);
            if !via_flatpak {
                return crate::types::AppStatus::NotInstalled;
            }
        }
    }

    // Check configs through the XDG-aware resolver.
    let has_configs = app.config_paths.iter().any(|p| live_path(p).exists());

    if has_configs {
        crate::types::AppStatus::Available
    } else {
        crate::types::AppStatus::Partial
    }
}

/// Check if flatpak is installed
fn is_flatpak_installed(flatpak_id: &str) -> bool {
    let output = Command::new("flatpak")
        .args(["list", "--app", "--columns=application"])
        .output();
    
    match output {
        Ok(output) => String::from_utf8_lossy(&output.stdout).contains(flatpak_id),
        Err(_) => false,
    }
}

/// Build Component from TrackedAppDef
fn build_component(app: TrackedAppDef) -> crate::types::Component {
    let status = detect_app_status(&app);
    crate::types::Component {
        id: app.id,
        name: app.name,
        category: app.category,
        config_paths: app.config_paths.into_iter().map(PathBuf::from).collect(),
        status,
        supports_live_reload: app.supports_live_reload,
        description: app.description,
        icon: app.icon,
        docs_url: app.docs_url,
    }
}

/// Detect all apps (built-in + user-registered custom components)
pub fn detect_all_apps() -> Result<Vec<crate::types::Component>> {
    let apps = merged_component_defs();
    let components = apps.into_iter().map(build_component).collect();
    Ok(components)
}

/// Get unique categories
pub fn get_categories() -> Result<Vec<String>> {
    let apps = merged_component_defs();
    let mut seen = std::collections::HashSet::new();
    let mut categories = Vec::new();

    for app in apps {
        if seen.insert(app.category.clone()) {
            categories.push(app.category);
        }
    }
    Ok(categories)
}

/// Get components by category
pub fn get_components_by_category(category: &str) -> Result<Vec<crate::types::Component>> {
    let apps = merged_component_defs();
    let components: Vec<crate::types::Component> = apps
        .into_iter()
        .filter(|a| a.category == category)
        .map(build_component)
        .collect();
    Ok(components)
}

/// Get all components
pub fn get_all_components() -> Result<Vec<crate::types::Component>> {
    let apps = merged_component_defs();
    let components = apps.into_iter().map(build_component).collect();
    Ok(components)
}

/// Refresh all apps (re-detect)
pub fn refresh() -> Result<Vec<crate::types::Component>> {
    detect_all_apps()
}