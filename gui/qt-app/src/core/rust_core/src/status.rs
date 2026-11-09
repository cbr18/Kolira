//! Native `theme-profile status` — fast, data-driven.
//!
//! One pass per file, exact-key readers only (see `config_parser`): no shelling
//! out to `kreadconfig6` (the bash version took ~300 ms over 21 subprocess
//! calls), no whole-file JSON dumps. `kwin_glass` derives from the actual
//! better-blur-dx key instead of the non-existent `glassEnabled` one.

use crate::config_parser::{
    ini_group_key, json_key_value, json_nested_value, line_key_value, user_js_pref,
};
use crate::paths::{active_file, config_home, home_dir, live_path};

fn kwin(group: &str, key: &str) -> Option<String> {
    ini_group_key(&config_home().join("kwinrc"), group, key)
}

fn kdeglobals(group: &str, key: &str) -> Option<String> {
    ini_group_key(&config_home().join("kdeglobals"), group, key)
}

/// `qdbus6 org.kde.KWin /Effects loadedEffects` (best effort, 1 s cap).
/// The reply is one effect per line; joined with commas for a single value.
fn kwin_loaded_effects() -> String {
    use std::process::Command;
    let raw = Command::new("timeout")
        .args(["1", "qdbus6", "org.kde.KWin", "/Effects", "loadedEffects"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    raw.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(",")
}

/// Contents of `theme-profile status`.
pub fn status() -> String {
    let cfg = config_home();
    let mut out = String::new();
    let mut emit = |key: &str, value: Option<String>| {
        out.push_str(key);
        out.push('=');
        out.push_str(&value.unwrap_or_default());
        out.push('\n');
    };

    let active = std::fs::read_to_string(active_file())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    emit("active_profile", Some(active));

    emit("kde_color_scheme", kdeglobals("General", "ColorScheme"));
    emit("kde_accent", kdeglobals("General", "AccentColor"));
    emit(
        "kde_accent_source",
        if kdeglobals("General", "accentColorFromWallpaper").as_deref() == Some("true") {
            Some("wallpaper".to_string())
        } else {
            Some("explicit".to_string())
        },
    );
    emit("kde_look_and_feel", kdeglobals("KDE", "LookAndFeelPackage"));
    emit("kde_widget_style", kdeglobals("KDE", "widgetStyle"));

    // The "glass" look is provided by the better-blur-dx effect; there is no
    // `glassEnabled` key in Plasma 6 (it printed empty forever in bash).
    emit(
        "kwin_glass",
        Some(
            if kwin("Plugins", "better_blur_dxEnabled").as_deref() == Some("true") {
                "true".to_string()
            } else {
                "false".to_string()
            },
        ),
    );
    emit("kwin_blur", kwin("Plugins", "blurEnabled"));
    emit("kwin_contrast", kwin("Plugins", "contrastEnabled"));
    emit(
        "kwin_better_blur_dx",
        kwin("Plugins", "better_blur_dxEnabled"),
    );
    // Ground truth from the running wm: which KWin effects are actually loaded.
    // One best-effort D-Bus call, bounded by `timeout` (absent on CI).
    emit("kwin_loaded_effects", Some(kwin_loaded_effects()));

    for (key, k) in [
        ("bbdx_blur_strength", "BlurStrength"),
        ("bbdx_contrast", "Contrast"),
        ("bbdx_blur_decorations", "BlurDecorations"),
        ("bbdx_blur_docks", "BlurDocks"),
        ("bbdx_blur_menus", "BlurMenus"),
        ("bbdx_refraction_strength", "RefractionStrength"),
        ("bbdx_refraction_edge_size", "RefractionEdgeSize"),
        ("bbdx_refraction_normal_pow", "RefractionNormalPow"),
        ("bbdx_window_classes", "WindowClasses"),
    ] {
        emit(key, kwin("Effect-better-blur-dx", k));
    }

    emit(
        "kwin_translucency_inactive",
        kwin("Effect-translucency", "Inactive"),
    );

    emit(
        "kvantum_theme",
        line_key_value(&cfg.join("Kvantum/kvantum.kvconfig"), "theme="),
    );
    emit(
        "gtk3_theme",
        line_key_value(&cfg.join("gtk-3.0/settings.ini"), "gtk-theme-name="),
    );
    emit(
        "gtk3_prefer_dark",
        line_key_value(
            &cfg.join("gtk-3.0/settings.ini"),
            "gtk-application-prefer-dark-theme=",
        ),
    );
    emit(
        "klassy_titlebar_active_opacity",
        ini_group_key(
            &cfg.join("klassy/klassyrc"),
            "TitleBarOpacity",
            "ActiveTitleBarOpacity",
        ),
    );
    emit(
        "klassy_titlebar_inactive_opacity",
        ini_group_key(
            &cfg.join("klassy/klassyrc"),
            "TitleBarOpacity",
            "InactiveTitleBarOpacity",
        ),
    );
    emit(
        "klassy_corner_radius",
        ini_group_key(
            &cfg.join("klassy/klassyrc"),
            "Windeco",
            "WindowCornerRadius",
        ),
    );

    emit(
        "alacritty_opacity",
        line_key_value(&cfg.join("alacritty/alacritty.toml"), "opacity = "),
    );
    emit(
        "alacritty_blur",
        line_key_value(&cfg.join("alacritty/alacritty.toml"), "blur = "),
    );
    emit(
        "kitty_background_opacity",
        line_key_value(&cfg.join("kitty/kitty.conf"), "background_opacity "),
    );
    emit(
        "kitty_background_blur",
        line_key_value(&cfg.join("kitty/kitty.conf"), "background_blur "),
    );

    // The Zen profile is auto-detected (see tracked_apps::zen_config_paths).
    let zen_paths = crate::tracked_apps::zen_config_paths();
    let zen_user_js = zen_paths
        .first()
        .map(|rel| live_path(rel))
        .unwrap_or_else(|| home_dir().join(".zen/user.js"));
    for (key, pref) in [
        (
            "zen_transparent_browser",
            "browser.tabs.allow_transparent_browser",
        ),
        ("zen_linux_transparency", "zen.widget.linux.transparency"),
        (
            "zen_custom_stylesheets",
            "toolkit.legacyUserProfileCustomizations.stylesheets",
        ),
        ("zen_active_theme", "extensions.activeThemeID"),
        ("zen_content_theme", "browser.theme.content-theme"),
        ("zen_toolbar_theme", "browser.theme.toolbar-theme"),
        (
            "zen_transparency_color",
            "mod.sameerasw.zen_transparency_color",
        ),
        ("zen_bg_color_enabled", "mod.sameerasw.zen_bg_color_enabled"),
        ("zen_bg_opacity", "mod.sameerasw.zen_bg_opacity"),
        ("zen_window_scheme", "zen.view.window.scheme"),
    ] {
        emit(key, user_js_pref(&zen_user_js, pref));
    }

    emit(
        "btop_rounded_corners",
        line_key_value(&cfg.join("btop/btop.conf"), "rounded_corners = "),
    );
    emit(
        "zed_theme_mode",
        json_nested_value(&cfg.join("zed/settings.json"), "theme", "mode"),
    );
    emit(
        "zed_theme_light",
        json_nested_value(&cfg.join("zed/settings.json"), "theme", "light"),
    );
    emit(
        "zed_theme_dark",
        json_nested_value(&cfg.join("zed/settings.json"), "theme", "dark"),
    );
    emit(
        "code_theme",
        json_key_value(&cfg.join("Code/User/settings.json"), "workbench.colorTheme"),
    );
    emit(
        "cursor_theme",
        json_key_value(
            &cfg.join("Cursor/User/settings.json"),
            "workbench.colorTheme",
        ),
    );

    // Obsidian used to dump the whole appearance.json (raw `{}` or fragments).
    // Only the two meaningful strings are reported now.
    let obsidian_json = home_dir().join("obsidian/vault/.obsidian/appearance.json");
    emit("obsidian_theme", json_key_value(&obsidian_json, "theme"));
    emit(
        "obsidian_css_theme",
        json_key_value(&obsidian_json, "cssTheme"),
    );

    if let Ok(operations) = crate::operation::list() {
        emit("apply_operation_busy", Some(operations["busy"].to_string()));
        let interrupted = operations["operations"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|record| record["interrupted"] == true)
            .filter_map(|record| record["id"].as_str())
            .collect::<Vec<_>>()
            .join(",");
        emit("apply_recovery_required", Some(interrupted));
    }
    out
}
