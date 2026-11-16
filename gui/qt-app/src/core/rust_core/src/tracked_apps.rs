//! Tracked Apps Core Implementation

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use which;

use crate::custom_components::merged_component_defs;
use crate::paths::live_path;

/// Read a declared default independent of INI key order. Reject traversal,
/// absolute paths and redirected profiles instead of borrowing another user's files.
fn default_browser_profile(root: &Path) -> Option<String> {
    let safe = |value: &str| {
        !value.is_empty()
            && !value.contains('\\')
            && Path::new(value)
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
            && root.join(value).is_dir()
            && !root.join(value).is_symlink()
    };
    if let Ok(text) = std::fs::read_to_string(root.join("installs.ini")) {
        for line in text.lines() {
            if let Some(value) = line.trim().strip_prefix("Default=") {
                return safe(value).then(|| value.to_string());
            }
        }
    }
    if let Ok(text) = std::fs::read_to_string(root.join("profiles.ini")) {
        for section in crate::appearance::parse_ini(&text) {
            let get = |key: &str| {
                section
                    .lines
                    .iter()
                    .find_map(|l| l.trim().strip_prefix(key))
            };
            if get("Default=") == Some("1") {
                if get("IsRelative=") == Some("0") {
                    return None;
                }
                return get("Path=").filter(|v| safe(v)).map(str::to_string);
            }
        }
    }
    let mut dirs: Vec<_> = std::fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.to_lowercase().contains("default") && safe(n))
        .collect();
    dirs.sort();
    dirs.into_iter().next()
}
pub fn browser_profiles(zen: bool) -> Vec<String> {
    let roots = crate::browser_appearance::ROOTS;
    let roots = if zen { &roots[..3] } else { &roots[3..] };
    roots
        .iter()
        .filter_map(|root| default_browser_profile(&live_path(root)).map(|p| format!("{root}/{p}")))
        .collect()
}
pub fn zen_profile_dir() -> Option<String> {
    default_browser_profile(&crate::paths::home_dir().join(".zen"))
}
pub fn firefox_profile_dir() -> Option<String> {
    default_browser_profile(&crate::paths::home_dir().join(".mozilla/firefox"))
}
fn browser_config_paths(zen: bool) -> Vec<String> {
    browser_profiles(zen)
        .into_iter()
        .flat_map(|root| {
            [
                "user.js",
                "chrome/userChrome.css",
                "chrome/kolira-colors.css",
            ]
            .into_iter()
            .map(move |file| format!("{root}/{file}"))
        })
        .collect()
}
pub fn zen_config_paths() -> Vec<String> {
    browser_config_paths(true)
}
pub fn firefox_config_paths() -> Vec<String> {
    browser_config_paths(false)
}

/// Asset paths of what is *active* on this machine (2026-11-04: the registry
/// used to hard-code one user's assets — Sandstone, glass/paper Look & Feel,
/// noctalia/paper btop themes, every Zed theme and colour
/// scheme — so every theme carried every other theme's assets). A theme
/// carries only the assets it uses; applying restores them from its
/// `theme.json` asset list (theme.rs).
pub mod active_assets {
    use crate::field::{field, read, value_str, Base};
    use crate::paths::{config_home, data_home, home_dir};

    fn rel(path: &std::path::Path) -> Option<String> {
        path.strip_prefix(home_dir())
            .ok()
            .map(|p| p.to_string_lossy().to_string())
    }

    fn live(id: &str) -> Option<String> {
        field(id)
            .and_then(|f| read(f, &Base::Live))
            .map(|v| value_str(&v))
            .filter(|v| !v.is_empty() && !v.contains('/'))
    }

    pub fn kvantum() -> Vec<String> {
        let mut out = vec![".config/Kvantum/kvantum.kvconfig".to_string()];
        if let Some(theme) = live("kvantum_theme") {
            let root = config_home().join("Kvantum");
            let dir = if root
                .join(&theme)
                .join(format!("{theme}.kvconfig"))
                .is_file()
            {
                Some(root.join(&theme))
            } else {
                std::fs::read_dir(&root).ok().and_then(|it| {
                    let mut dirs: Vec<_> = it
                        .flatten()
                        .map(|e| e.path())
                        .filter(|d| d.join(format!("{theme}.kvconfig")).is_file())
                        .collect();
                    dirs.sort();
                    dirs.into_iter().next()
                })
            };
            out.extend(dir.as_deref().and_then(rel));
        }
        out
    }

    pub fn btop() -> Vec<String> {
        let mut out = vec![".config/btop/btop.conf".to_string()];
        if let Some(theme) = live("btop_theme") {
            let file = config_home()
                .join("btop/themes")
                .join(format!("{theme}.theme"));
            if file.is_file() {
                out.extend(rel(&file));
            }
        }
        out
    }

    pub fn zed() -> Vec<String> {
        let mut out = vec![".config/zed/settings.json".to_string()];
        for id in ["zed_dark", "zed_light"] {
            if let Some(theme) = live(id) {
                if let Some(file) = crate::theme::zed_theme_file_pub(&theme) {
                    if let Some(r) = rel(&file)
                        .filter(|r| r.starts_with(".config/zed/themes/") && !out.contains(r))
                    {
                        out.push(r);
                    }
                }
            }
        }
        out
    }

    pub fn color_scheme() -> Vec<String> {
        live("color_scheme")
            .map(|name| {
                data_home()
                    .join("color-schemes")
                    .join(format!("{name}.colors"))
            })
            .filter(|p| p.is_file())
            .and_then(|p| rel(&p))
            .into_iter()
            .collect()
    }

    pub fn look_and_feel() -> Vec<String> {
        live("look_and_feel")
            .map(|name| data_home().join("plasma/look-and-feel").join(name))
            .filter(|p| p.is_dir())
            .and_then(|p| rel(&p))
            .into_iter()
            .collect()
    }
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
    all.into_iter()
        .filter(|app| app.id != "fastfetch")
        .map(|app| {
            (
                app.id,
                app.config_paths
                    .into_iter()
                    .filter(|path| !crate::appearance::is_preserved(path))
                    .collect(),
            )
        })
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
                ".config/gtk-3.0/kolira.css".to_string(),
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
                ".config/gtk-4.0/kolira.css".to_string(),
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
            // The active theme's family dir only (active_assets).
            config_paths: active_assets::kvantum(),
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
            // in kolira.conf (appearance.rs). Host first: apply derives
            // the include from old themes before the include entry runs.
            config_paths: vec![
                ".config/kitty/kitty.conf".to_string(),
                ".config/kitty/kolira.conf".to_string(),
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
                ".config/alacritty/kolira.toml".to_string(),
            ],
            detect_command: Some("alacritty".to_string()),
            flatpak_id: None,
            supports_live_reload: false,
            description: "GPU-accelerated terminal, TOML config".to_string(),
            icon: "utilities-terminal".to_string(),
            docs_url: "https://github.com/alacritty/alacritty".to_string(),
        },
        TrackedAppDef {
            id: "neovim".into(), name: "Neovim".into(), category: "Editors".into(),
            config_paths: vec![crate::neovim::DATA.into()],
            detect_command: Some("nvim".into()), flatpak_id: None, supports_live_reload: false,
            description: "Data-only palette and fixed standalone colourscheme; reload with :colorscheme kolira".into(),
            icon: "text-editor".into(), docs_url: "https://neovim.io".into(),
        },
        TrackedAppDef {
            id: "wofi".to_string(),
            name: "Wofi".to_string(),
            category: "Launchers / Menu".to_string(),
            config_paths: vec![
                ".config/wofi/style.css".to_string(),
                ".config/wofi/kolira.css".to_string(),
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
            config_paths: active_assets::btop(),
            detect_command: Some("btop".to_string()),
            flatpak_id: None,
            supports_live_reload: false,
            description: "Resource monitor with themes".to_string(),
            icon: "utilities-system-monitor".to_string(),
            docs_url: "https://github.com/aristocratos/btop".to_string(),
        },
        TrackedAppDef {
            id: "zed".to_string(),
            name: "Zed".to_string(),
            category: "Editors".to_string(),
            config_paths: active_assets::zed(),
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
            description: "Colours and day/night only; personal browser UI and privacy remain unchanged".to_string(),
            icon: "web-browser".to_string(),
            docs_url: "https://zen-browser.app/".to_string(),
        },
        TrackedAppDef {
            id: "discord".to_string(),
            name: "Discord (Vencord / Vesktop)".to_string(),
            category: "Chat".to_string(),
            // themes/ + only enabledThemes/themeLinks of settings.json (discord.rs).
            config_paths: crate::discord::config_paths(),
            detect_command: Some("vesktop|discord".to_string()),
            flatpak_id: Some("dev.vencord.Vesktop".to_string()),
            supports_live_reload: true,
            description: "CSS themes of Vencord / Vesktop (palette → midnight-based theme)".to_string(),
            icon: "discord".to_string(),
            docs_url: "https://vencord.dev/".to_string(),
        },
        TrackedAppDef {
            id: "spicetify".to_string(),
            name: "Spotify (Spicetify)".to_string(),
            category: "Media".to_string(),
            // Themes/ + only the theme keys of config-xpui.ini (spicetify.rs).
            config_paths: crate::spicetify::config_paths(),
            detect_command: Some("spicetify".to_string()),
            flatpak_id: None,
            supports_live_reload: false,
            description: "Spicetify theme and colour scheme (palette → color.ini); runs spicetify apply".to_string(),
            icon: "spotify".to_string(),
            docs_url: "https://spicetify.app/docs/customization/themes".to_string(),
        },
        TrackedAppDef {
            id: "oh-my-posh".to_string(),
            name: "oh-my-posh (prompt)".to_string(),
            category: "Shell".to_string(),
            // Shell configs are hosts: only the --config path changes (extras.rs).
            config_paths: crate::extras::omp_config_paths(),
            detect_command: Some("oh-my-posh".to_string()),
            flatpak_id: None,
            supports_live_reload: false,
            description: "Prompt theme (.omp.json) used by the shell's oh-my-posh init line".to_string(),
            icon: "utilities-terminal".to_string(),
            docs_url: "https://ohmyposh.dev/docs/themes".to_string(),
        },
        TrackedAppDef {
            id: "fish".to_string(),
            name: "fish colours".to_string(),
            category: "Shell".to_string(),
            config_paths: vec![crate::extras::FISH_VARS.to_string()],
            detect_command: Some("fish".to_string()),
            flatpak_id: None,
            supports_live_reload: true,
            description: "fish_color_* / fish_pager_color_* universal variables only".to_string(),
            icon: "utilities-terminal".to_string(),
            docs_url: "https://fishshell.com/docs/current/interactive.html#syntax-highlighting".to_string(),
        },
        TrackedAppDef {id:"starship".into(),name:"Starship".into(),category:"Terminals".into(),config_paths:crate::four_apps::paths_for("starship"),detect_command:Some("starship".into()),flatpak_id:None,supports_live_reload:true,description:"Palette colours only; prompt modules, formats and commands remain personal".into(),icon:"utilities-terminal".into(),docs_url:String::new()},
        TrackedAppDef {id:"tmux".into(),name:"tmux".into(),category:"Terminals".into(),config_paths:crate::four_apps::paths_for("tmux"),detect_command:Some("tmux".into()),flatpak_id:None,supports_live_reload:true,description:"Native styles and live global colours; sessions and commands remain personal".into(),icon:"utilities-terminal".into(),docs_url:String::new()},
        TrackedAppDef {id:"foot".into(),name:"Foot".into(),category:"Terminals".into(),config_paths:crate::four_apps::paths_for("foot"),detect_command:Some("foot".into()),flatpak_id:None,supports_live_reload:false,description:"Native ANSI and day/night colours; restart to reload, fonts and commands stay personal".into(),icon:"utilities-terminal".into(),docs_url:String::new()},
        TrackedAppDef {id:"wezterm".into(),name:"WezTerm".into(),category:"Terminals".into(),config_paths:crate::four_apps::paths_for("wezterm"),detect_command:Some("wezterm".into()),flatpak_id:None,supports_live_reload:false,description:"Native colour data and fixed local glue; personal Lua never travels in themes".into(),icon:"utilities-terminal".into(),docs_url:String::new()},
        TrackedAppDef { id:"micro".into(), name:"Micro".into(),category:"Editors".into(),config_paths:crate::third_apps::paths_for("micro"),detect_command:Some("micro".into()),flatpak_id:None,supports_live_reload:false,description:"Palette colorscheme only; settings, bindings and plugins remain personal".into(),icon:"accessories-text-editor".into(),docs_url:String::new()},
        TrackedAppDef { id:"lsd".into(), name:"lsd".into(),category:"Tools".into(),config_paths:crate::third_apps::paths_for("lsd"),detect_command:Some("lsd".into()),flatpak_id:None,supports_live_reload:false,description:"Native colour data; preserves sorting, icons, classic mode and LS_COLORS".into(),icon:"utilities-terminal".into(),docs_url:String::new()},
        TrackedAppDef { id:"glow".into(), name:"Glow".into(),category:"Tools".into(),config_paths:crate::third_apps::paths_for("glow"),detect_command:Some("glow".into()),flatpak_id:None,supports_live_reload:false,description:"Native Glamour style and selector; Markdown files remain untouched".into(),icon:"text-markdown".into(),docs_url:String::new()},
        TrackedAppDef { id:"gimp".into(), name:"GIMP".into(),category:"Graphics".into(),config_paths:crate::third_apps::paths_for("gimp"),detect_command:Some("gimp".into()),flatpak_id:None,supports_live_reload:false,description:"GIMP 3 native theme colours and scheme; tool presets, images and plugins remain personal".into(),icon:"gimp".into(),docs_url:String::new()},
        TrackedAppDef { id:"heroic".into(), name:"Heroic Games Launcher".into(),category:"Games".into(),config_paths:crate::third_apps::paths_for("heroic"),detect_command:Some("heroic".into()),flatpak_id:None,supports_live_reload:false,description:"Managed colour variables in native customCSS; accounts, games and personal CSS remain intact".into(),icon:"heroic".into(),docs_url:String::new()},
        TrackedAppDef { id:"prismlauncher".into(), name:"Prism Launcher".into(),category:"Games".into(),config_paths:crate::third_apps::paths_for("prismlauncher"),detect_command:Some("prismlauncher".into()),flatpak_id:None,supports_live_reload:false,description:"Native Qt palette and ApplicationTheme only; accounts, instances and Java settings remain personal".into(),icon:"org.prismlauncher.PrismLauncher".into(),docs_url:String::new()},
        TrackedAppDef { id:"vim".into(), name:"Vim".into(),category:"Editors".into(),config_paths:crate::third_apps::paths_for("vim"),detect_command:Some("vim".into()),flatpak_id:None,supports_live_reload:false,description:"JSON palette plus fixed colourscheme/startup glue; never imports theme-supplied Vimscript or replaces vimrc".into(),icon:"vim".into(),docs_url:String::new()},
        TrackedAppDef { id:"mpv".into(), name:"mpv".into(),category:"Media".into(),config_paths:crate::third_apps::paths_for("mpv"),detect_command:Some("mpv".into()),flatpak_id:None,supports_live_reload:false,description:"OSD colours only; playback, audio, video, profiles and scripts remain personal".into(),icon:"mpv".into(),docs_url:String::new()},
        TrackedAppDef { id:"cliamp".into(), name:"Cliamp".into(),category:"Media".into(),config_paths:crate::third_apps::paths_for("cliamp"),detect_command:Some("cliamp".into()),flatpak_id:None,supports_live_reload:false,description:"Native TOML theme and selector; playback, playlists and visualizer settings stay personal".into(),icon:"multimedia-audio-player".into(),docs_url:String::new()},
        TrackedAppDef { id:"vscodium".into(), name:"VSCodium".into(),category:"Editors".into(),config_paths:crate::third_apps::paths_for("vscodium"),detect_command:Some("codium|vscodium".into()),flatpak_id:None,supports_live_reload:false,description:"Native JSONC palette overrides and shared editor cache; fonts, extensions and behaviour stay personal".into(),icon:"vscodium".into(),docs_url:String::new()},
        TrackedAppDef { id: "pure".into(),name: "Pure colours".into(),category:"Shell".into(),config_paths:crate::terminal_apps::paths_for("pure"), detect_command:Some("fish".into()),flatpak_id:None,supports_live_reload:false,description:"Pure colour variables only; does not enable or replace the prompt".into(),icon:"utilities-terminal".into(),docs_url:String::new()},
        TrackedAppDef { id: "fzf".into(),name: "fzf".into(),category:"Shell".into(),config_paths:crate::terminal_apps::paths_for("fzf"), detect_command:Some("fzf".into()),flatpak_id:None,supports_live_reload:false,description:"Colour-only options with fixed fish startup glue; other shells require native setup".into(),icon:"utilities-terminal".into(),docs_url:String::new()},
        TrackedAppDef { id: "lazygit".into(),name: "lazygit".into(),category:"Tools".into(),config_paths:crate::terminal_apps::paths_for("lazygit"), detect_command:Some("lazygit".into()),flatpak_id:None,supports_live_reload:false,description:"Only gui.theme colours; personal YAML and commands remain unchanged".into(),icon:"utilities-terminal".into(),docs_url:String::new()},
        TrackedAppDef { id: "zellij".into(),name: "Zellij".into(),category:"Terminals".into(),config_paths:crate::terminal_apps::paths_for("zellij"), detect_command:Some("zellij".into()),flatpak_id:None,supports_live_reload:false,description:"Native theme resource and root theme selectors; keybindings and plugins stay personal".into(),icon:"utilities-terminal".into(),docs_url:String::new()},
        TrackedAppDef { id: "opencode".into(),name: "OpenCode".into(),category:"Editors".into(),config_paths:crate::terminal_apps::paths_for("opencode"), detect_command:Some("opencode".into()),flatpak_id:None,supports_live_reload:false,description:"OpenCode V2 native system palette and day/night in cli.json only".into(),icon:"utilities-terminal".into(),docs_url:String::new()},
        TrackedAppDef { id: "claude-code".into(),name: "Claude Code".into(),category:"Editors".into(),config_paths:crate::terminal_apps::paths_for("claude-code"), detect_command:Some("claude".into()),flatpak_id:None,supports_live_reload:false,description:"Native ANSI day/night; preserves accessibility choices, credentials and permissions".into(),icon:"utilities-terminal".into(),docs_url:String::new()},
        TrackedAppDef { id: "codex".into(),name: "Codex".into(),category:"Editors".into(),config_paths:crate::terminal_apps::paths_for("codex"), detect_command:Some("codex".into()),flatpak_id:None,supports_live_reload:false,description:"Native tmTheme syntax highlighting only; models, approvals and tools stay personal".into(),icon:"utilities-terminal".into(),docs_url:String::new()},
        TrackedAppDef {
            id: "ghostty".to_string(),
            name: "Ghostty".to_string(),
            category: "Terminals".to_string(),
            config_paths: vec![crate::extras::GHOSTTY_HOST.to_string(), crate::extras::GHOSTTY_THEME.to_string()],
            detect_command: Some("ghostty".to_string()),
            flatpak_id: None,
            supports_live_reload: false,
            description: "Look in ghostty/kolira, included by the config".to_string(),
            icon: "utilities-terminal".to_string(),
            docs_url: "https://ghostty.org/docs/config".to_string(),
        },
        TrackedAppDef {
            id: "cava".to_string(),
            name: "cava".to_string(),
            category: "Media".to_string(),
            config_paths: vec![crate::extras::CAVA.to_string()],
            detect_command: Some("cava".to_string()),
            flatpak_id: None,
            supports_live_reload: true,
            description: "[color] section (palette → gradient)".to_string(),
            icon: "audio-x-generic".to_string(),
            docs_url: "https://github.com/karlstav/cava".to_string(),
        },
        TrackedAppDef {
            id: "yazi".to_string(),
            name: "yazi".to_string(),
            category: "Shell".to_string(),
            config_paths: vec![".config/yazi/theme.toml".to_string(), ".config/yazi/flavors".to_string()],
            detect_command: Some("yazi".to_string()),
            flatpak_id: None,
            supports_live_reload: false,
            description: "theme.toml and flavors".to_string(),
            icon: "system-file-manager".to_string(),
            docs_url: "https://yazi-rs.github.io/docs/flavors/overview".to_string(),
        },
        TrackedAppDef {
            id: "bat".to_string(),
            name: "bat".to_string(),
            category: "Shell".to_string(),
            config_paths: vec![crate::extras::BAT_CONFIG.to_string(), ".config/bat/themes".to_string()],
            detect_command: Some("bat".to_string()),
            flatpak_id: None,
            supports_live_reload: false,
            description: "--theme line and custom themes (bat cache --build after apply)".to_string(),
            icon: "text-x-generic".to_string(),
            docs_url: "https://github.com/sharkdp/bat#adding-new-themes".to_string(),
        },
        TrackedAppDef {
            id: "konsole".to_string(),
            name: "Konsole / Yakuake".to_string(),
            category: "Terminals".to_string(),
            // The default profile's [Appearance] + its colour scheme (konsole.rs).
            config_paths: crate::konsole::config_paths(),
            detect_command: Some("konsole|yakuake".to_string()),
            flatpak_id: Some("org.kde.konsole".to_string()),
            supports_live_reload: false,
            description: "Colour scheme (opacity, blur) and the profile's scheme/font family".to_string(),
            icon: "utilities-terminal".to_string(),
            docs_url: "https://docs.kde.org/stable5/en/konsole/konsole/".to_string(),
        },
        TrackedAppDef {
            id: "firefox".to_string(),
            name: "Firefox".to_string(),
            category: "Browser".to_string(),
            config_paths: firefox_config_paths(),
            detect_command: Some("firefox".to_string()),
            flatpak_id: Some("org.mozilla.firefox".to_string()),
            supports_live_reload: false,
            description: "userChrome.css / userContent.css and the appearance prefs of user.js".to_string(),
            icon: "firefox".to_string(),
            docs_url: "https://www.userchrome.org/".to_string(),
        },
        TrackedAppDef {
            id: "obsidian".to_string(),
            name: "Obsidian".to_string(),
            category: "Notes".to_string(),
            // The look lives in the vault's appearance.json; obsidian.json (vault
            // list) and Preferences (spellcheck, zoom) are not appearance.
            config_paths: crate::obsidian::config_paths(),
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
            config_paths: active_assets::color_scheme(),
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
            config_paths: vec![".config/kolira/palette.json".to_string()],
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
            config_paths: active_assets::look_and_feel(),
            detect_command: None,
            flatpak_id: None,
            supports_live_reload: false,
            description: "The active Plasma Look & Feel package (user-installed)".to_string(),
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
pub(crate) fn detect_app_status(app: &TrackedAppDef) -> crate::types::AppStatus {
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

/// Should a theme's file for `rel` be skipped because its app is not here?
/// Applying must not make config folders for programs the receiver does not
/// have (`~/.config/Cursor`, `~/obsidian/vault`). The app counts as here when
/// its command exists or its folder (`dst`'s parent) already does.
pub fn app_absent_for(rel: &str, dst: &std::path::Path) -> bool {
    app_absent_without_parent(rel, dst.parent().is_some_and(|p| p.exists()))
}
pub(crate) fn app_absent_without_parent(rel: &str, parent_exists: bool) -> bool {
    if parent_exists {
        return false;
    }
    get_tracked_apps()
        .iter()
        .find(|a| a.config_paths.iter().any(|p| p == rel))
        .is_some_and(|a| {
            a.detect_command.is_some()
                && matches!(detect_app_status(a), crate::types::AppStatus::NotInstalled)
        })
}

/// Check if flatpak is installed
pub(crate) fn is_flatpak_installed(flatpak_id: &str) -> bool {
    [
        crate::paths::data_home().join("flatpak/app"),
        PathBuf::from("/var/lib/flatpak/app"),
    ]
    .iter()
    .any(|root| root.join(flatpak_id).join("current").exists())
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
