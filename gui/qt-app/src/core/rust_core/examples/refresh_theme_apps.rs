//! Maintenance of repository / staging packages; never modifies live app configs.
use anyhow::{Context, Result};
use kolira_core::{
    appearance, browser_appearance, discord, field, four_apps, konsole,
    palette::{self, contrast, Palette, Rgb, Variant, MIN_TEXT_CONTRAST},
    terminal_apps, third_apps,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

fn put(root: &Path, rel: &str, text: &str) -> Result<()> {
    let out = root.join(rel);
    fs::create_dir_all(out.parent().context("output parent")?)?;
    let mut temporary = tempfile::NamedTempFile::new_in(out.parent().context("output parent")?)?;
    let permissions = fs::metadata(&out)
        .map(|m| m.permissions())
        .unwrap_or_else(|_| fs::Permissions::from_mode(0o644));
    temporary.as_file().set_permissions(permissions)?;
    temporary.write_all(text.as_bytes())?;
    temporary.persist(&out).map_err(|e| e.error)?;
    Ok(())
}
fn document(root: &Path, rel: &str, value: &Value) -> Result<()> {
    put(root, rel, &(serde_json::to_string_pretty(value)? + "\n"))
}
fn patch(root: &Path, rel: &str, key: &str, value: &Value) -> Result<()> {
    let text = fs::read_to_string(root.join(rel)).unwrap_or_else(|_| "{}".into());
    put(root, rel, &field::json_set(&text, &[key.into()], value)?)
}
fn derive(root: &Path, package: &Value) -> Result<Palette> {
    if package["palette"].is_object() {
        return Ok(serde_json::from_value(package["palette"].clone())?);
    }
    let scheme = package["assets"]
        .as_array()
        .context("assets")?
        .iter()
        .find(|v| v["kind"] == "color-scheme")
        .context("colour scheme")?;
    let text = fs::read_to_string(root.join(scheme["path"].as_str().context("scheme path")?))?;
    let color = |group, key, fallback: &str| -> Result<Rgb> {
        let s = field::ini_get(&text, group, key).unwrap_or_else(|| fallback.into());
        if s.starts_with('#') {
            return Rgb::parse(&s);
        }
        let v: Vec<u8> = s
            .split(',')
            .map(str::parse)
            .collect::<std::result::Result<_, _>>()?;
        anyhow::ensure!(v.len() == 3, "invalid KDE RGB");
        Ok(Rgb(v[0], v[1], v[2]))
    };
    let bg = color("Colors:View", "BackgroundNormal", "#202020")?;
    let fg = color("Colors:View", "ForegroundNormal", "#eeeeee")?;
    let window = color("Colors:Window", "BackgroundNormal", &bg.hex())?;
    let dim = color("Colors:View", "ForegroundInactive", &fg.mix(bg, 0.5).hex())?;
    let accent = color("Colors:Selection", "BackgroundNormal", "#7aaeef")?;
    let red = color("Colors:View", "ForegroundNegative", "#e06c75")?;
    let orange = color("Colors:View", "ForegroundNeutral", "#d19a66")?;
    let green = color("Colors:View", "ForegroundPositive", "#98c379")?;
    let blue = color("Colors:View", "ForegroundLink", &accent.hex())?;
    let purple = color("Colors:View", "ForegroundVisited", "#c678dd")?;
    Ok(Palette {
        name: package["name"].as_str().context("theme name")?.into(),
        author: "Kolira contributors (from this theme's KDE colours)".into(),
        variant: if bg.luminance() < 0.5 {
            Variant::Dark
        } else {
            Variant::Light
        },
        base: [
            bg,
            window,
            bg.mix(fg, 0.16),
            dim,
            fg.mix(bg, 0.25),
            fg,
            fg.mix(window, 0.1),
            fg,
            red,
            orange,
            orange.mix(fg, 0.35),
            green,
            green.mix(blue, 0.5),
            blue,
            purple,
            red.mix(orange, 0.5),
        ]
        .iter()
        .map(|c| c.hex())
        .collect(),
        accent: accent.hex(),
        source: format!("theme:{}", package["name"].as_str().unwrap()),
    })
}

fn zed_theme(p: &Palette, label: &str) -> Value {
    let editor = p.slot(0x00);
    let panel = p.slot(0x01);
    let accent = p.accent_rgb();
    let hover = panel.mix(accent, 0.08);
    let selected = panel.mix(accent, 0.16);
    let selection = editor.mix(accent, 0.18);
    let search_match = editor.mix(p.slot(0x0a), 0.25);
    let backgrounds = [editor, panel, hover, selected, selection, search_match];
    let min_contrast = |color| {
        backgrounds
            .iter()
            .map(|bg| contrast(color, *bg))
            .fold(f64::INFINITY, f64::min)
    };
    let anchor = [Rgb(0, 0, 0), Rgb(255, 255, 255)]
        .into_iter()
        .max_by(|a, b| min_contrast(*a).total_cmp(&min_contrast(*b)))
        .expect("black or white");
    let readable = |color: Rgb| {
        (0..=100)
            .map(|step| color.mix(anchor, f64::from(step) / 100.0))
            .find(|mixed| min_contrast(*mixed) >= MIN_TEXT_CONTRAST)
            .unwrap_or(anchor)
    };
    let ink = |slot| readable(p.slot(slot));
    let text = ink(0x05);
    let muted = ink(0x03);
    let accent_ink = readable(accent);
    let warning = ink(0x0a);

    let mut style: serde_json::Map<String, Value> = [
        ("background", editor),
        ("surface.background", panel),
        ("elevated_surface.background", panel),
        ("panel.background", panel),
        ("text", text),
        ("text.muted", muted),
        ("text.accent", accent_ink),
        ("icon.accent", accent_ink),
        ("link_text.hover", accent_ink),
        ("border", p.slot(0x02)),
        ("border.focused", accent_ink),
        ("panel.focused_border", accent_ink),
        ("pane.focused_border", accent_ink),
        ("element.hover", hover),
        ("element.selected", selected),
        ("ghost_element.hover", hover),
        ("ghost_element.selected", selected),
        ("created", ink(0x0b)),
        ("modified", warning),
        ("warning", warning),
        ("deleted", ink(0x08)),
        ("conflict", ink(0x09)),
        ("renamed", ink(0x0d)),
        ("error", ink(0x08)),
        ("success", ink(0x0b)),
        ("editor.background", editor),
        ("editor.foreground", text),
        ("editor.gutter.background", editor),
        ("editor.line_number", muted),
        ("editor.active_line_number", text),
        ("editor.active_line.background", panel),
        ("tab.active_background", editor),
        ("tab.inactive_background", panel),
        ("toolbar.background", panel),
        ("status_bar.background", panel),
        ("terminal.background", editor),
        ("terminal.foreground", text),
        ("search.match_background", search_match),
    ]
    .into_iter()
    .map(|(role, color)| (role.into(), color.hex().into()))
    .collect();
    style.insert(
        "players".into(),
        json!([{"cursor": accent_ink.hex(), "background": accent_ink.hex(), "selection": selection.hex()}]),
    );
    style.insert(
        "syntax".into(),
        json!({
            "comment": {"color": muted.hex()},
            "string": {"color": ink(0x0b).hex()},
            "keyword": {"color": ink(0x0e).hex()},
            "number": {"color": ink(0x09).hex()},
            "function": {"color": ink(0x0d).hex()},
            "type": {"color": warning.hex()},
        }),
    );
    json!({
        "$schema": "https://zed.dev/schema/themes/v0.2.0.json",
        "name": label,
        "author": "Kolira contributors",
        "themes": [{
            "name": label,
            "appearance": if p.variant == Variant::Dark { "dark" } else { "light" },
            "style": style,
        }],
    })
}

fn refresh(root: &Path) -> Result<()> {
    let mut package: Value = serde_json::from_slice(&fs::read(root.join("theme.json"))?)?;
    let mut p = derive(root, &package)?;
    p.validate()?;
    if p.source.contains("/home/") || p.source.contains("$HOME") {
        p.source = package["wallpaper"]
            .as_str()
            .map(|s| format!("theme:{s}"))
            .unwrap_or_default();
    }
    // Older local packages import a CSS filename outside today's registry.
    // Migrate the resource and its import together so export cannot orphan it.
    for gtk in ["gtk-3.0", "gtk-4.0"] {
        let legacy = root.join(format!(".config/{gtk}/theme-profile.css"));
        let host = format!(".config/{gtk}/gtk.css");
        if let (Ok(css), Ok(text)) = (
            fs::read_to_string(&legacy),
            fs::read_to_string(root.join(&host)),
        ) {
            if text.contains("theme-profile.css") {
                let owned = format!(".config/{gtk}/kolira.css");
                anyhow::ensure!(
                    !root.join(&owned).exists() || fs::read_to_string(root.join(&owned))? == css,
                    "Conflicting legacy/new GTK resources in {}",
                    root.display()
                );
                put(root, &owned, &css)?;
                put(
                    root,
                    &host,
                    &text.replace("theme-profile.css", "kolira.css"),
                )?;
            }
        }
    }
    let name = package["name"].as_str().context("name")?.to_string();
    let dark = p.variant == Variant::Dark;
    let label = format!("Kolira {name}");
    let b = &p.base;
    package["palette"] = serde_json::to_value(&p)?;
    // Refresh system colour resources too: stale Breeze GTK definitions affect
    // Firefox's native chrome even when no browser stylesheet is enabled.
    let kde = palette::kde_color_scheme(&p);
    let kde_id = palette::kde_scheme_id(&p);
    let kde_rel = format!(".local/share/color-schemes/{kde_id}.colors");
    put(root, &kde_rel, &kde)?;
    let selected_scheme = package["values"]["color_scheme"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            fs::read_to_string(root.join(".config/kdeglobals"))
                .ok()
                .and_then(|text| field::ini_get(&text, "General", "ColorScheme"))
        })
        .unwrap_or_else(|| kde_id.clone());
    package["values"]["color_scheme"] = selected_scheme.clone().into();
    package["values"]["accent_color"] = p.accent_rgb().kde().into();
    let mut globals = fs::read_to_string(root.join(".config/kdeglobals")).unwrap_or_default();
    // This is a user choice, independent from the saved app palette. Refreshing
    // colours must not turn off Plasma's wallpaper-derived accent.
    let wallpaper_accent = package["values"]["accent_from_wallpaper"]
        .as_bool()
        .unwrap_or_else(|| {
            field::ini_get(&globals, "General", "accentColorFromWallpaper").as_deref()
                == Some("true")
        });
    package["values"]["accent_from_wallpaper"] = wallpaper_accent.into();
    let mut general = false;
    globals = globals
        .lines()
        .filter(|line| {
            let t = line.trim();
            if t.starts_with('[') {
                general = t == "[General]";
            }
            !(general
                && t.split_once('=').is_some_and(|(key, _)| {
                    key.trim().eq_ignore_ascii_case("accentColorFromWallpaper")
                }))
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let mut group = "";
    for line in kde.lines() {
        if line.starts_with('[') {
            group = line.trim_start_matches('[').trim_end_matches(']');
        } else if group.starts_with("Colors:") || group == "WM" {
            if let Some((key, value)) = line.split_once('=') {
                globals = field::set_ini_text(&globals, group, key, value);
            }
        }
    }
    globals = field::set_ini_text(&globals, "General", "ColorScheme", &selected_scheme);
    globals = field::set_ini_text(&globals, "General", "AccentColor", &p.accent_rgb().kde());
    globals = field::set_ini_text(
        &globals,
        "General",
        "accentColorFromWallpaper",
        if wallpaper_accent { "true" } else { "false" },
    );
    put(root, ".config/kdeglobals", &globals)?;
    for f in field::registry()
        .iter()
        .filter(|f| f.file == ".config/kdeglobals" && f.kind == "color")
    {
        if let Some(value) = field::ini_get(&globals, &f.group, &f.key) {
            package["values"][&f.id] = value.into();
        }
    }
    let assets = package["assets"].as_array_mut().context("assets")?;
    assets.retain(|a| a["path"] != kde_rel);
    assets.push(json!({"kind":"color-scheme","name":kde_id,"path":kde_rel,"origin":"generated","redistributable":true,"license":"MIT (generated by Kolira)"}));
    if selected_scheme == kde_id {
        if let Some(sources) = package["sources"].as_object_mut() {
            sources.remove("color_scheme");
        }
    }
    for gtk in ["gtk-3.0", "gtk-4.0"] {
        for file in ["colors.css", "kolira.css"] {
            let rel = format!(".config/{gtk}/{file}");
            if let Ok(old) = fs::read_to_string(root.join(&rel)) {
                put(
                    root,
                    &rel,
                    &kolira_core::extras::gtk_colors_with_palette(&old, &p),
                )?;
            }
        }
    }
    let alacritty = ".config/alacritty/kolira.toml";
    let old = fs::read_to_string(root.join(alacritty)).unwrap_or_default();
    put(root, alacritty, &palette::alacritty_with_palette(&old, &p)?)?;
    let ghostty = ".config/ghostty/kolira";
    let old = fs::read_to_string(root.join(ghostty)).unwrap_or_default();
    put(
        root,
        ghostty,
        &kolira_core::extras::ghostty_palette_text(&old, &p),
    )?;
    let mut targets = package["app_targets"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    targets.retain(|target| {
        !matches!(
            target.as_str(),
            Some("telegram" | "kotatogram" | "chromium" | "chrome" | "google-chrome")
        )
    });
    for target in [
        "neovim", "obsidian", "zen", "firefox", "discord", "kitty", "konsole",
    ] {
        if !targets.iter().any(|v| v == target) {
            targets.push(target.into());
        }
    }
    for target in terminal_apps::TARGETS
        .iter()
        .chain(third_apps::TARGETS)
        .chain(four_apps::TARGETS)
        .chain(kolira_core::launcher_apps::TARGETS)
        .copied()
        .chain(["cava"])
    {
        if !targets.iter().any(|v| v == target) {
            targets.push(target.into());
        }
    }
    package["app_targets"] = targets.into();
    // These snapshots use built-in editor themes plus colour data, with no
    // marketplace code, downloads or third-party cache needed at Apply time.
    package["editor_themes"] = json!([]);
    let builtin = if dark {
        "Default Dark Modern"
    } else {
        "Default Light Modern"
    };
    let colors = json!({"editor.background":b[0],"editor.foreground":b[5],
        "editor.selectionBackground":b[2],"editor.lineHighlightBackground":b[1],
        "sideBar.background":b[1],"sideBar.foreground":b[5],"activityBar.background":b[1],
        "activityBar.foreground":b[5],"statusBar.background":b[1],"statusBar.foreground":b[5],
        "titleBar.activeBackground":b[1],"titleBar.activeForeground":b[5],
        "tab.activeBackground":b[0],"tab.activeForeground":b[5],
        "terminal.background":b[0],"terminal.foreground":b[5],"focusBorder":p.accent});
    let tokens = json!({"textMateRules":[
        {"scope":["comment"],"settings":{"foreground":b[3]}},
        {"scope":["string"],"settings":{"foreground":b[11]}},
        {"scope":["keyword","storage"],"settings":{"foreground":b[14]}},
        {"scope":["constant.numeric"],"settings":{"foreground":b[9]}},
        {"scope":["entity.name.function"],"settings":{"foreground":b[13]}}
    ]});
    for (dir, key) in [
        ("Cursor", "cursor_ide_theme"),
        ("Code", "vscode_theme"),
        ("VSCodium", "vscode_theme"),
    ] {
        let rel = format!(".config/{dir}/User/settings.json");
        patch(root, &rel, "workbench.colorTheme", &json!(builtin))?;
        patch(root, &rel, "workbench.colorCustomizations", &colors)?;
        patch(root, &rel, "editor.tokenColorCustomizations", &tokens)?;
        package["values"][key] = builtin.into();
    }
    let zed_rel = format!(".config/zed/themes/kolira-{name}.json");
    document(root, &zed_rel, &zed_theme(&p, &label))?;
    patch(
        root,
        ".config/zed/settings.json",
        "theme",
        &json!({"mode":if dark {"dark"} else {"light"},"dark":label,"light":label}),
    )?;
    for key in ["zed_dark", "zed_light"] {
        package["values"][key] = label.clone().into();
    }
    package["values"]["zed_mode"] = if dark { "dark" } else { "light" }.into();
    package["assets"]
        .as_array_mut()
        .context("assets")?
        .retain(|a| a["kind"] != "zed" || (a["redistributable"] == true && a["path"] != zed_rel));
    package["assets"]
        .as_array_mut()
        .unwrap()
        .push(json!({"kind":"zed","name":label,"path":zed_rel,
        "origin":"generated","redistributable":true,"license":"MIT (generated by Kolira)"}));
    if let Some(sources) = package["sources"].as_object_mut() {
        for key in ["vscode_theme", "cursor_ide_theme", "zed_dark", "zed_light"] {
            sources.remove(key);
        }
    }
    if let Some(deps) = package["dependencies"].as_array_mut() {
        deps.retain(|d| !matches!(d["kind"].as_str(), Some("vscode" | "cursor" | "zed")));
    }
    document(
        root,
        ".config/nvim/kolira-palette.json",
        &serde_json::to_value(&p)?,
    )?;
    let kitty = ".config/kitty/kolira.conf";
    let old = fs::read_to_string(root.join(kitty)).unwrap_or_default();
    put(root, kitty, &palette::kitty_conf_with_palette(&old, &p))?;
    for (_, rel) in discord::CLIENTS {
        if !rel.starts_with(".config/") {
            continue;
        }
        let css = format!("{rel}/themes/kolira.theme.css");
        if root.join(rel).join("settings/settings.json").is_file() {
            put(root, &css, &discord::css(&p, 1.0))?;
        } else if root.join(&css).is_file() {
            // With no saved selector leave the resource absent, so the normal
            // follow-up generates AND enables it while keeping personal URLs.
            fs::remove_file(root.join(&css))?;
            let _ = fs::remove_dir(root.join(rel).join("themes"));
            let _ = fs::remove_dir(root.join(rel));
        }
    }
    for path in browser_appearance::ROOTS {
        if root.join(path).exists() {
            fs::remove_dir_all(root.join(path))?;
        }
    }
    for path in [
        ".config/chromium",
        ".config/google-chrome",
        ".var/app/org.chromium.Chromium",
        ".var/app/com.google.Chrome",
    ] {
        if root.join(path).exists() {
            fs::remove_dir_all(root.join(path))?;
        }
    }
    for (path, zen) in [(".zen", true), (".mozilla/firefox", false)] {
        let profile = format!("{path}/kolira.default");
        put(
            root,
            &format!("{profile}/user.js"),
            &browser_appearance::accent(&browser_appearance::mode("", dark, zen), &p, zen),
        )?;
    }
    for key in ["zen_opacity", "zen_transparent"] {
        package["values"].as_object_mut().unwrap().remove(key);
    }
    for key in ["zen_window_scheme", "zen_sites_scheme"] {
        package["values"][key] = if dark { "0" } else { "1" }.into();
    }
    let scheme = palette::kde_scheme_id(&p);
    let rel = format!(".local/share/konsole/{scheme}.colorscheme");
    // Preserve an existing profile's font/opacity choices; update only its scheme.
    let konsole_dir = root.join(".local/share/konsole");
    fs::create_dir_all(&konsole_dir)?;
    let mut profiles: Vec<PathBuf> = fs::read_dir(&konsole_dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "profile"))
        .collect();
    profiles.sort();
    let previous = profiles
        .first()
        .and_then(|profile| fs::read_to_string(profile).ok())
        .and_then(|text| field::ini_get(&text, "Appearance", "ColorScheme"))
        .and_then(|name| fs::read_to_string(konsole_dir.join(format!("{name}.colorscheme"))).ok());
    let mut generated = konsole::colorscheme(&p);
    if let Some(previous) = previous {
        for key in ["Opacity", "Blur"] {
            if let Some(value) = field::ini_get(&previous, "General", key) {
                generated = field::set_ini_text(&generated, "General", key, &value);
            }
        }
    }
    put(root, &rel, &generated)?;
    if profiles.is_empty() {
        profiles.push(konsole_dir.join("Theme Profile.profile"));
    }
    for file in profiles {
        let old = fs::read_to_string(&file).unwrap_or_default();
        let mut out = String::new();
        let mut appearance = false;
        let mut written = false;
        for line in old.lines() {
            if line.starts_with('[') {
                if appearance && !written {
                    out.push_str(&format!("ColorScheme={scheme}\n"));
                    written = true;
                }
                appearance = line == "[Appearance]";
            }
            if appearance && line.starts_with("ColorScheme=") {
                out.push_str(&format!("ColorScheme={scheme}\n"));
                written = true;
            } else {
                out.push_str(line);
                out.push('\n');
            }
        }
        if !written {
            if !appearance {
                out.push_str("[Appearance]\n");
            }
            out.push_str(&format!("ColorScheme={scheme}\n"));
        }
        put(
            root,
            file.strip_prefix(root)?.to_str().context("profile path")?,
            &out,
        )?;
    }
    let assets = package["assets"].as_array_mut().unwrap();
    assets.retain(|a| !(a["kind"] == "konsole-scheme" && a["path"] == rel));
    assets.push(json!({"kind":"konsole-scheme","name":scheme,"path":rel,"origin":"generated","redistributable":true,"license":"MIT (generated by Kolira)"}));
    let mut resources = terminal_apps::bundle_resources(&p);
    resources.extend(third_apps::bundle_resources(&p));
    resources.extend(four_apps::bundle_resources(&p));
    resources.extend(kolira_core::launcher_apps::bundle_resources(&p));
    let old_walker = root.join(".config/walker/style.css");
    if old_walker.is_file() {
        fs::remove_file(old_walker)?;
    }
    for (rel, generated) in &resources {
        let old = fs::read_to_string(root.join(rel)).ok();
        let merged = if rel == kolira_core::launcher_apps::FUZZEL {
            generated.clone()
        } else if rel == ".config/btop/btop.conf" {
            let mut lines: Vec<_> = old
                .as_deref()
                .unwrap_or("")
                .lines()
                .filter(|l| !l.trim().starts_with("color_theme ="))
                .map(str::to_owned)
                .collect();
            lines.push("color_theme = \"kolira\"".into());
            lines.join("\n") + "\n"
        } else {
            appearance::applied(rel, generated, old.as_deref())?
        };
        let merged = if third_apps::TARGETS
            .iter()
            .any(|id| third_apps::paths_for(id).contains(rel))
        {
            appearance::for_theme(rel, &merged).context("Invalid generated native resource")?
        } else {
            merged
        };
        put(root, rel, &merged)?;
    }
    let omp = kolira_core::extras::OMP_THEME;
    let palette_hash = format!(
        "{:x}",
        Sha256::digest(
            [
                b"palette-consistency-v2".as_slice(),
                &serde_json::to_vec(&p)?
            ]
            .concat()
        )
    );
    if package["app_coverage_omp_palette"].as_str() != Some(&palette_hash) {
        if let Ok(old) = fs::read_to_string(root.join(omp)) {
            put(root, omp, &terminal_apps::omp(&p, &old)?)?;
        }
    }
    package["app_coverage_omp_palette"] = palette_hash.into();
    package["values"]["btop_theme"] = "kolira".into();
    let assets = package["assets"].as_array_mut().unwrap();
    assets.retain(|a| !(a["kind"] == "btop" && a["path"] == ".config/btop/themes/kolira.theme"));
    assets.push(json!({"kind":"btop","name":"kolira","path":".config/btop/themes/kolira.theme","origin":"generated","redistributable":true,"license":"MIT (generated by Kolira)"}));
    package["app_coverage_revision"] = 5.into();
    document(root, "theme.json", &package)?;
    if root.join("manifest.json").is_file() {
        let mut manifest: Value = serde_json::from_slice(&fs::read(root.join("manifest.json"))?)?;
        manifest["accent_from_wallpaper"] = wallpaper_accent.into();
        if let Some(components) = manifest["components"].as_array_mut() {
            components.retain(|c| {
                !matches!(
                    c["id"].as_str(),
                    Some(
                        "zen"
                            | "firefox"
                            | "neovim"
                            | "obsidian"
                            | "telegram"
                            | "kotatogram"
                            | "google-chrome"
                            | "chromium"
                            | "chrome"
                    )
                )
            });
            for (id, paths) in [
                ("zen", json!([".zen/kolira.default/user.js",])),
                (
                    "firefox",
                    json!([".mozilla/firefox/kolira.default/user.js",]),
                ),
                ("neovim", json!([".config/nvim/kolira-palette.json"])),
                ("obsidian", json!([])),
            ] {
                components.push(json!({"id":id,"paths":paths}));
            }
        }
        if let Some(components) = manifest["components"].as_array_mut() {
            components.retain(|c| c["id"] != "terminal-colours");
            components.push(
                json!({"id":"terminal-colours","paths":resources.keys().collect::<Vec<_>>()}),
            );
        }
        document(root, "manifest.json", &manifest)?;
    }
    if root.join("manifest.txt").is_file() {
        let old = fs::read_to_string(root.join("manifest.txt"))?;
        let mut text = old
            .lines()
            .filter(|l| {
                !l.starts_with("user=")
                    && !appearance::is_retired_app_path(l)
                    && !browser_appearance::is_profile_file(l)
                    && !l.contains("/.obsidian/")
                    && *l != ".config/nvim/kolira-palette.json"
                    && *l != ".config/walker/style.css"
                    && !resources.contains_key(*l)
            })
            .collect::<Vec<_>>()
            .join("\n");
        text.push_str("\n.config/nvim/kolira-palette.json\n.zen/kolira.default/user.js\n.mozilla/firefox/kolira.default/user.js\n");
        for rel in resources.keys() {
            text.push_str(rel);
            text.push('\n');
        }
        // Keep repeated maintenance runs stable.
        let mut seen = std::collections::HashSet::new();
        let text = text
            .lines()
            .filter(|l| seen.insert(l.to_string()))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        put(root, "manifest.txt", &text)?;
    }
    if root.join("export.json").is_file() {
        fn hashes(root: &Path, path: &Path, out: &mut BTreeMap<String, String>) -> Result<()> {
            for entry in fs::read_dir(path)? {
                let p = entry?.path();
                anyhow::ensure!(!p.is_symlink(), "symlink in exported theme");
                if p.is_dir() {
                    hashes(root, &p, out)?;
                } else if p.file_name().is_some_and(|n| n != "export.json") {
                    out.insert(
                        p.strip_prefix(root)?.to_string_lossy().into_owned(),
                        format!("{:x}", Sha256::digest(fs::read(p)?)),
                    );
                }
            }
            Ok(())
        }
        let mut sums = BTreeMap::new();
        hashes(root, root, &mut sums)?;
        let mut manifest: Value = serde_json::from_slice(&fs::read(root.join("export.json"))?)?;
        manifest["files"] = serde_json::to_value(sums)?;
        document(root, "export.json", &manifest)?;
    }
    println!("Updated {name}");
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    anyhow::ensure!(
        args.len() == 2,
        "Usage: refresh_theme_apps THEMES_DIRECTORY"
    );
    let root = Path::new(&args[1]).canonicalize()?;
    anyhow::ensure!(kolira_core::paths::profiles_dir().canonicalize().ok().as_ref() != Some(&root),"Use repository/staging packages; import them through kolira, never edit live themes directly");
    let mut dirs: Vec<_> = fs::read_dir(root)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("theme.json").is_file())
        .collect();
    dirs.sort();
    for dir in dirs {
        refresh(&dir)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn contents(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in fs::read_dir(dir).unwrap().flatten() {
                let file = entry.path();
                if file.is_dir() {
                    walk(root, &file, out);
                } else {
                    out.insert(
                        file.strip_prefix(root).unwrap().to_path_buf(),
                        fs::read(file).unwrap(),
                    );
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(root, root, &mut out);
        out
    }
    #[test]
    fn repeated_refresh_keeps_every_byte_and_existing_opt_ins() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let palette = Palette {
            name: "test".into(),
            author: "test".into(),
            variant: Variant::Dark,
            base: vec!["#202020".into(); 16],
            accent: "#7aaeef".into(),
            source: "manual".into(),
        };
        document(root,"theme.json",&json!({"schema":"kolira/theme@2","name":"test","values":{},"assets":[],"palette":palette,"app_targets":["cava","chromium","chrome","google-chrome"]})).unwrap();
        put(
            root,
            ".config/Cursor/User/settings.json",
            "{ // keep\\n\"workbench.colorCustomizations\":{\"old\":true}}"
                .replace("\\n", "\n")
                .as_str(),
        )
        .unwrap();
        put(
            root,
            ".config/gtk-4.0/gtk.css",
            "@import 'theme-profile.css';\n",
        )
        .unwrap();
        put(
            root,
            ".config/gtk-4.0/theme-profile.css",
            "window { color: #abcdef; }\n",
        )
        .unwrap();
        refresh(root).unwrap();
        assert_eq!(
            fs::read_to_string(root.join(".config/gtk-4.0/kolira.css")).unwrap(),
            "window { color: #abcdef; }\n"
        );
        assert_eq!(
            fs::read_to_string(root.join(".config/gtk-4.0/gtk.css")).unwrap(),
            "@import 'kolira.css';\n"
        );
        put(root, kolira_core::extras::OMP_THEME, "{\"blocks\":[{\"segments\":[{\"foreground\":\"#707070\",\"template\":\" keep \"}]}]}\n").unwrap();
        put(root, "manifest.txt", ".config/nvim/kolira-palette.json\n").unwrap();
        refresh(root).unwrap();
        let before = contents(root);
        refresh(root).unwrap();
        for (path, old) in before {
            assert_eq!(
                String::from_utf8_lossy(&old),
                String::from_utf8_lossy(&fs::read(root.join(&path)).unwrap()),
                "{}",
                path.display()
            );
        }
        let package: Value =
            serde_json::from_slice(&fs::read(root.join("theme.json")).unwrap()).unwrap();
        assert!(package["app_targets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "cava"));
        assert!(!package["app_targets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| matches!(v.as_str(), Some("chromium" | "chrome" | "google-chrome"))));
        for id in kolira_core::launcher_apps::TARGETS {
            assert!(package["app_targets"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v == id));
        }
        let palette: palette::Palette = serde_json::from_value(package["palette"].clone()).unwrap();
        for (rel, generated) in kolira_core::launcher_apps::bundle_resources(&palette) {
            assert_eq!(fs::read_to_string(root.join(rel)).unwrap(), generated);
        }
    }
    #[test]
    fn refresh_preserves_wallpaper_accent_and_removes_case_aliases() {
        for wallpaper in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path();
            let p = Palette {
                name: "test".into(),
                author: "".into(),
                variant: Variant::Dark,
                base: vec!["#202020".into(); 16],
                accent: "#7aaeef".into(),
                source: "manual".into(),
            };
            document(root,"theme.json",&json!({"schema":"kolira/theme@2","name":"test","palette":p,"values":{"accent_from_wallpaper":wallpaper},"assets":[]})).unwrap();
            document(
                root,
                "manifest.json",
                &json!({"accent_from_wallpaper":!wallpaper,"components":[]}),
            )
            .unwrap();
            put(
                root,
                ".config/kdeglobals",
                "[General]\naccentColorFromWallpaper=true\nAccentColorFromWallpaper=false\n",
            )
            .unwrap();
            for _ in 0..2 {
                refresh(root).unwrap();
            }
            let globals = fs::read_to_string(root.join(".config/kdeglobals")).unwrap();
            assert_eq!(
                globals
                    .lines()
                    .filter(|l| l
                        .split_once('=')
                        .is_some_and(|(k, _)| k.eq_ignore_ascii_case("accentColorFromWallpaper")))
                    .count(),
                1
            );
            assert_eq!(
                field::ini_get(&globals, "General", "accentColorFromWallpaper"),
                Some(wallpaper.to_string())
            );
            for file in ["theme.json", "manifest.json"] {
                let v: Value = serde_json::from_slice(&fs::read(root.join(file)).unwrap()).unwrap();
                assert_eq!(
                    if file == "theme.json" {
                        &v["values"]["accent_from_wallpaper"]
                    } else {
                        &v["accent_from_wallpaper"]
                    },
                    &json!(wallpaper)
                );
            }
        }
    }
    #[test]
    fn refresh_keeps_user_selected_breeze_scheme() {
        for scheme in ["BreezeLight", "BreezeDark"] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path();
            let p = Palette {
                name: "test".into(),
                author: "".into(),
                variant: Variant::Dark,
                base: vec!["#202020".into(); 16],
                accent: "#7aaeef".into(),
                source: "manual".into(),
            };
            document(root,"theme.json",&json!({"schema":"kolira/theme@2","name":"test","palette":p,"values":{"color_scheme":scheme},"assets":[]})).unwrap();
            put(
                root,
                ".config/kdeglobals",
                &format!("[General]\nColorScheme={scheme}\n"),
            )
            .unwrap();
            for _ in 0..2 {
                refresh(root).unwrap();
            }
            let globals = fs::read_to_string(root.join(".config/kdeglobals")).unwrap();
            assert_eq!(
                field::ini_get(&globals, "General", "ColorScheme").as_deref(),
                Some(scheme)
            );
            let v: Value =
                serde_json::from_slice(&fs::read(root.join("theme.json")).unwrap()).unwrap();
            assert_eq!(v["values"]["color_scheme"], scheme);
        }
    }
}
