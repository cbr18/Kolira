//! Deterministic follow-up writes shared by preview and profile application.
//! Runs after the desktop hooks, under the same mutation lock and recovery copy.
use crate::{change_plan::Plan, field, palette, paths, stores, theme::Package};
use anyhow::{Context, Result};
use std::{fs, path::Path};

pub(crate) const TARGETS: &[&str] = &[
    "discord",
    "konsole",
    "ghostty",
    "cava",
    "firefox",
    "zen",
    "neovim",
    "obsidian",
    "pywalfox",
    "kitty",
    "fish",
    "pure",
    "oh-my-posh",
    "btop",
    "yazi",
    "bat",
    "wofi",
    "fzf",
    "lazygit",
    "zellij",
    "opencode",
    "claude-code",
    "codex",
    "micro",
    "lsd",
    "glow",
    "chromium",
    "google-chrome",
    "gimp",
    "heroic",
    "prismlauncher",
    "vim",
    "mpv",
    "cliamp",
    "vscodium",
];

/// Reject redirected writes below the user's resource container. Relative links
/// within it are portable (icon aliases); external and dangling links are not.
pub(crate) fn validate_destination(path: &Path, container: &Path) -> Result<()> {
    for current in path.ancestors().take_while(|p| *p != container) {
        if current.is_symlink() {
            let target = fs::read_link(current)?;
            anyhow::ensure!(
                !target.is_absolute()
                    && fs::canonicalize(current)?.starts_with(fs::canonicalize(container)?),
                "symlink in theme asset destination: {}",
                current.display()
            );
        }
    }
    Ok(())
}

fn cache_tree(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    anyhow::ensure!(
        !metadata.file_type().is_symlink(),
        "symlink in editor cache: {}",
        path.display()
    );
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            cache_tree(&entry?.path())?;
        }
    } else {
        anyhow::ensure!(metadata.is_file(), "unsupported editor cache entry");
    }
    Ok(())
}

pub(crate) fn targets(source: &Path, package: &Package) -> Result<Vec<String>> {
    let mut plan = Plan::default();
    plan_apps(source, package, &mut plan)?;
    Ok(plan
        .changed()
        .filter(|(_, e)| matches!(e, crate::change_plan::Entry::File { .. }))
        .map(|(p, _)| p.display().to_string())
        .collect())
}

pub(crate) fn validate_live(path: &Path) -> Result<()> {
    // XDG roots may themselves be relocated; descendants may not escape them.
    let root = [
        paths::config_home(),
        paths::data_home(),
        paths::cache_home(),
        paths::home_dir(),
    ]
    .into_iter()
    .find(|root| path.starts_with(root))
    .context("app path outside HOME/XDG")?;
    for parent in path.ancestors().skip(1).take_while(|p| *p != root) {
        anyhow::ensure!(
            !(parent.is_symlink() && parent.is_dir()),
            "symlink directory in app destination: {}",
            parent.display()
        );
    }
    validate_destination(path, &root)
}
pub(crate) fn write(plan: &mut Plan, path: &Path, text: String) -> Result<()> {
    validate_live(path)?;
    plan.write_text(path, text, "app_theme")
}

/// Only fill missing app resources. A snapshot's explicit app theme always wins
/// over palette generation; applying a custom Discord/Firefox theme preserves it.
pub(crate) fn plan_apps(source: &Path, package: &Package, plan: &mut Plan) -> Result<()> {
    for editor in package.editor_themes.iter().filter(|e| e.enabled) {
        let configured = if editor.editor == "zed" {
            paths::config_home().join("zed").is_dir()
        } else {
            stores::extension_root(&editor.editor)?.is_dir()
        };
        if !configured && !stores::editor_installed(&editor.editor) {
            continue;
        }
        anyhow::ensure!(
            !editor.local_path.is_empty(),
            "editor theme cache missing for {} {}; install/download it before applying",
            editor.editor,
            editor.id
        );
        let cache = source.join(&editor.local_path);
        for ancestor in cache.ancestors().take_while(|p| *p != source) {
            anyhow::ensure!(!ancestor.is_symlink(), "symlink in editor cache path");
        }
        cache_tree(&cache)?;
        stores::plan_from_cache(
            &editor.editor,
            &editor.id,
            &cache,
            editor.native_watch,
            plan,
        )?;
        if editor.editor == "zed" {
            for key in ["zed_dark", "zed_light"] {
                if let Some(value) = package.values.get(key).filter(|v| v.is_string()) {
                    let wanted = value.as_str().unwrap();
                    let available = fs::read_dir(cache.join("themes"))
                        .ok()
                        .into_iter()
                        .flatten()
                        .flatten()
                        .any(|entry| {
                            fs::read(entry.path())
                                .ok()
                                .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                                .is_some_and(|v| {
                                    v["themes"].as_array().is_some_and(|themes| {
                                        themes.iter().any(|t| t["name"] == wanted)
                                    })
                                })
                        });
                    if available {
                        let settings = paths::config_home().join("zed/settings.json");
                        let old = plan.text(&settings)?.unwrap_or_else(|| "{}".into());
                        let side = if key == "zed_dark" { "dark" } else { "light" };
                        let text = field::json_set(&old, &["theme".into(), side.into()], value)?;
                        let text = if let Some(mode) = package.values.get("zed_mode") {
                            field::json_set(&text, &["theme".into(), "mode".into()], mode)?
                        } else {
                            text
                        };
                        write(plan, &settings, text)?;
                    }
                }
            }
        } else {
            let (key, dir) = match editor.editor.as_str() {
                "cursor" => ("cursor_ide_theme", "Cursor"),
                "vscodium" => ("vscode_theme", "VSCodium"),
                "windsurf" => ("windsurf_theme", "Windsurf"),
                _ => ("vscode_theme", "Code"),
            };
            if let Some(value) = package.values.get(key).filter(|v| v.is_string()) {
                let manifest: serde_json::Value =
                    serde_json::from_slice(&fs::read(cache.join("package.json"))?)?;
                if manifest["contributes"]["themes"]
                    .as_array()
                    .is_some_and(|themes| {
                        themes
                            .iter()
                            .any(|t| t["label"] == *value || t["id"] == *value)
                    })
                {
                    let settings = paths::config_home().join(dir).join("User/settings.json");
                    let old = plan.text(&settings)?.unwrap_or_else(|| "{}".into());
                    write(
                        plan,
                        &settings,
                        field::json_set(&old, &["workbench.colorTheme".into()], value)?,
                    )?;
                }
            }
        }
    }
    crate::third_apps::plan(source, package, plan)?;
    crate::terminal_apps::plan(source, package, plan)?;
    crate::neovim::plan(
        source,
        package.palette.as_ref(),
        package.app_targets.iter().any(|t| t == "neovim"),
        plan,
    )?;
    let Some(p) = &package.palette else {
        return Ok(());
    };
    p.validate()?;
    let wants = |id: &str| package.app_targets.iter().any(|t| t == id);
    if wants("obsidian") {
        crate::obsidian::plan(source, p, plan)?;
    }
    if wants("kitty")
        && !source.join(crate::appearance::KITTY_THEME).exists()
        && !source.join(crate::appearance::KITTY_HOST).exists()
    {
        let host = paths::live_path(crate::appearance::KITTY_HOST);
        if let Some(old) = plan.text(&host)? {
            let target = paths::live_path(crate::appearance::KITTY_THEME);
            let look = plan
                .text(&target)?
                .unwrap_or_else(|| crate::appearance::kitty_split(&old).0);
            write(plan, &target, palette::kitty_conf_with_palette(&look, p))?;
            write(plan, &host, crate::appearance::kitty_host(&old))?;
        }
    }
    for (_, rel) in crate::discord::CLIENTS {
        let dir = paths::live_path(rel);
        if !wants("discord") || !dir.is_dir() || source.join(rel).exists() {
            continue;
        }
        write(
            plan,
            &dir.join("themes").join(crate::discord::THEME_FILE),
            crate::discord::css(p, 1.0),
        )?;
        let settings = dir.join("settings/settings.json");
        let old = plan.text(&settings)?.unwrap_or_default();
        write(
            plan,
            &settings,
            crate::discord::enable(&old, crate::discord::THEME_FILE),
        )?;
    }
    let konsole = paths::data_home().join("konsole");
    if wants("konsole")
        && (konsole.is_dir() || which::which("konsole").is_ok() || which::which("yakuake").is_ok())
        && !source.join(".local/share/konsole").exists()
    {
        let rc = paths::config_home().join("konsolerc");
        let rc_text = plan.text(&rc)?.unwrap_or_default();
        let default = field::ini_get(&rc_text, "Desktop Entry", "DefaultProfile")
            .unwrap_or_else(|| crate::konsole::THEME_PROFILE.into());
        anyhow::ensure!(
            Path::new(&default).components().count() == 1
                && default.ends_with(".profile")
                && !default.contains('\\'),
            "invalid Konsole default profile"
        );
        let id = palette::kde_scheme_id(p);
        let scheme = konsole.join(format!("{id}.colorscheme"));
        let mut text = crate::konsole::colorscheme(p);
        if let Some(old) = plan.text(&scheme)? {
            for key in ["Opacity", "Blur"] {
                if let Some(value) = field::ini_get(&old, "General", key) {
                    text = field::set_ini_text(&text, "General", key, &value);
                }
            }
        }
        write(plan, &scheme, text)?;
        let profile = konsole.join(&default);
        let old = plan.text(&profile)?.unwrap_or_default();
        let mut text = field::set_ini_text(&old, "Appearance", "ColorScheme", &id);
        if field::ini_get(&text, "General", "Name").is_none() {
            text = field::set_ini_text(
                &text,
                "General",
                "Name",
                default.trim_end_matches(".profile"),
            );
            text = field::set_ini_text(&text, "General", "Parent", "FALLBACK/");
        }
        write(plan, &profile, text)?;
        write(
            plan,
            &rc,
            field::set_ini_text(&rc_text, "Desktop Entry", "DefaultProfile", &default),
        )?;
    }
    let ghostty = paths::live_path(crate::extras::GHOSTTY_HOST);
    if wants("ghostty") && plan.is_file(&ghostty)? && !source.join(".config/ghostty").exists() {
        let old = plan.text(&ghostty)?.unwrap_or_default();
        let theme = paths::live_path(crate::extras::GHOSTTY_THEME);
        let current = plan
            .text(&theme)?
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| crate::extras::ghostty_split(&old).0);
        write(
            plan,
            &theme,
            crate::extras::ghostty_palette_text(&current, p),
        )?;
        write(plan, &ghostty, crate::extras::ghostty_host(&old))?;
    }
    let cava = paths::live_path(crate::extras::CAVA);
    if wants("cava") && plan.is_file(&cava)? && !source.join(crate::extras::CAVA).exists() {
        let old = plan.text(&cava)?.unwrap_or_default();
        write(plan, &cava, crate::extras::cava_palette_text(&old, p))?;
    }
    if wants("pywalfox") {
        let original = plan
            .text(&paths::cache_home().join("wal/colors.json"))?
            .context(
                "Initialize Pywalfox first: a baseline colors.json is required for reversible sync",
            )?;
        let original: serde_json::Value =
            serde_json::from_str(&original).context("Invalid Pywalfox baseline")?;
        anyhow::ensure!(
            original["special"].is_object() && original["colors"].is_object(),
            "Invalid Pywalfox baseline"
        );
        let bg = p.slot(0).hex();
        let fg = p.slot(5).hex();
        let colors: serde_json::Map<String, serde_json::Value> = palette::ansi16(p)
            .iter()
            .enumerate()
            .map(|(i, c)| (format!("color{i}"), serde_json::json!(c.hex())))
            .collect();
        let value = serde_json::json!({"wallpaper":"","alpha":"100","special":{"background":bg,"foreground":fg,"cursor":fg},"colors":colors});
        write(
            plan,
            &paths::cache_home().join("wal/colors.json"),
            serde_json::to_string_pretty(&value)?,
        )?;
    }
    for zen in [false, true] {
        if !wants(if zen { "zen" } else { "firefox" }) {
            continue;
        }
        for profile in crate::tracked_apps::browser_profiles(zen) {
            let root = paths::live_path(&profile);
            let user = root.join("user.js");
            let old = plan.text(&user)?.unwrap_or_default();
            let dark = p.variant == crate::palette::Variant::Dark;
            let text = crate::browser_appearance::accent(
                &crate::browser_appearance::mode(&old, dark, zen),
                p,
                zen,
            );
            write(plan, &user, text)?;
        }
    }
    Ok(())
}
