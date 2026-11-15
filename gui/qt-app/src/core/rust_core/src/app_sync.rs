//! Deterministic follow-up writes shared by preview and profile application.
//! Runs after the desktop hooks, under the same mutation lock and recovery copy.
use crate::{change_plan::Plan, field, palette, paths, stores, theme::Package};
use anyhow::{Context, Result};
use std::{fs, path::Path};

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

fn write(plan: &mut Plan, path: &Path, text: String) -> Result<()> {
    // XDG roots may themselves be relocated; descendants may not escape them.
    let root = [paths::config_home(), paths::data_home(), paths::home_dir()]
        .into_iter()
        .find(|root| path.starts_with(root))
        .context("app path outside HOME/XDG")?;
    validate_destination(path, &root)?;
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
    }
    let Some(p) = &package.palette else {
        return Ok(());
    };
    p.validate()?;
    let wants = |id: &str| package.app_targets.iter().any(|t| t == id);
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
        && (konsole.is_dir() || which::which("konsole").is_ok())
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
    // Telegram generation is offline: use only its previously downloaded base.
    let base = paths::cache_home().join("kolira/telegram-colors.palette");
    if wants("telegram")
        && paths::data_home().join("TelegramDesktop").is_dir()
        && !source.join(crate::telegram::PALETTE_FILE).exists()
        && plan.is_file(&base)?
    {
        let base = plan.text(&base)?.context("Telegram base palette")?;
        write(
            plan,
            &paths::live_path(crate::telegram::PALETTE_FILE),
            crate::telegram::generate(p, &base),
        )?;
    }
    // Firefox: preserve custom CSS and only manage our own imported stylesheet.
    if let Some(profile) = crate::tracked_apps::firefox_config_paths()
        .first()
        .and_then(|p| Path::new(p).parent().map(|p| p.to_path_buf()))
    {
        if wants("firefox") && !source.join(&profile).exists() {
            let root = paths::live_path(profile.to_str().context("Firefox profile path")?);
            let chrome = root.join("chrome/userChrome.css");
            let old = plan.text(&chrome)?.unwrap_or_default();
            let import = "@import url(\"kolira.css\");";
            let text = if old.contains(import) {
                old
            } else {
                format!("{import}\n{old}")
            };
            write(plan, &chrome, text)?;
            write(plan, &root.join("chrome/kolira.css"), format!(
                "/* Kolira palette */\n:root {{ --toolbar-bgcolor: {}; --toolbar-color: {}; --lwt-accent-color: {}; --lwt-text-color: {}; }}\n",
                p.base[0], p.base[5], p.base[0], p.base[5]))?;
            let user = root.join("user.js");
            let old = plan.text(&user)?.unwrap_or_default();
            write(
                plan,
                &user,
                crate::appearance::applied(
                    profile
                        .join("user.js")
                        .to_str()
                        .context("Firefox user.js")?,
                    "user_pref(\"toolkit.legacyUserProfileCustomizations.stylesheets\", true);\n",
                    Some(&old),
                )?,
            )?;
        }
    }
    // Chromium Preferences contains personal state and is managed by a running
    // browser. Do not edit it without a supported theme-extension contract.
    Ok(())
}
