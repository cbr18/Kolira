//! Bundle metadata, legacy compatibility and validation for all adapters.
use crate::{
    field::{self, Base},
    paths,
};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};
pub fn components() -> BTreeMap<String, Value> {
    serde_json::from_str(include_str!("../data/bundle_components.json"))
        .expect("valid component map")
}
pub fn metadata(name: &str) -> Result<Value> {
    let dir = paths::profile_dir(name)?;
    if !crate::theme::is_package(&dir) {
        let file = dir.join("theme-bundle.json");
        if file.is_file() {
            let mut metadata: Value =
                serde_json::from_slice(&fs::read(file)?).context("Invalid bundle metadata")?;
            metadata
                .as_object_mut()
                .context("Invalid bundle metadata")?
                .insert("legacy".into(), json!(true));
            return Ok(metadata);
        }
    }
    let package = if crate::theme::is_package(&dir) {
        Some(crate::theme::load(&dir)?)
    } else {
        None
    };
    let mut values = json!({});
    for (id, component) in components() {
        let Some(f) = component["field"].as_str().and_then(field::field) else {
            continue;
        };
        let raw = if let Some(p) = &package {
            p.values.get(&f.id).cloned()
        } else {
            field::read_raw(f, &Base::Snapshot(dir.clone()))
        };
        let raw = if f.id == "plasma_theme" && raw.as_ref().is_none_or(|v| v.is_null() || v == "") {
            Some(json!("default"))
        } else {
            raw
        };
        if let Some(value) = raw.and_then(|v| field::coerce(f, v)) {
            if value != json!("") {
                values[&id] = value;
            }
        }
    }
    Ok(match package {
        Some(package) => {
            json!({"schema":package.schema,"name":name,"legacy":false,"description":package.description,"createdAt":package.created,"components":values,"values":package.values,"applications":{},"wallpaper":package.wallpaper.unwrap_or_default(),"assets":package.assets,"sources":package.sources})
        }
        None => {
            json!({"schema":"theme-bundle/1","name":name,"legacy":true,"components":values,"applications":{},"wallpaper":""})
        }
    })
}
pub fn validate(name: &str) -> Result<Value> {
    let dir = paths::profile_dir(name)?;
    if crate::theme::is_package(&dir) {
        let missing = crate::theme::check(name)?;
        return Ok(
            json!({"ok":missing.is_empty(),"message":if missing.is_empty(){"Theme bundle is complete".into()}else{format!("Missing theme references: {}",missing.iter().map(|m|if m.source.is_empty(){m.name.clone()}else{format!("{} ({})",m.name,m.source)}).collect::<Vec<_>>().join("; "))},"missing":missing}),
        );
    }
    let data = metadata(name)?;
    let defs = components();
    let mut missing = vec![];
    for (id, value) in data["components"].as_object().into_iter().flatten() {
        if let Some(def) = defs.get(id) {
            if def["choice"] == true && value != &Value::Null && value != &json!("") {
                if let Some(field) = def["field"].as_str() {
                    let choices = crate::inspector_view::choices(field)?;
                    let value = field::value_str(value);
                    if !choices.is_empty() && !choices.iter().any(|v| v["value"] == value) {
                        missing.push(format!(
                            "{}: {value}",
                            field::field(field).map(|f| f.label.as_str()).unwrap_or(id)
                        ));
                    }
                }
            }
        }
    }
    let apps = crate::app_themes::rpc("all", &json!({}))?;
    for (id, theme) in data["applications"].as_object().into_iter().flatten() {
        if let Some(app) = apps
            .as_array()
            .and_then(|apps| apps.iter().find(|a| a["id"] == id.as_str()))
        {
            if let Some(themes) = app["themes"].as_array() {
                if !themes.is_empty() && !themes.iter().any(|t| t["id"] == *theme) {
                    missing.push(format!(
                        "{}: {}",
                        app["name"].as_str().unwrap_or(id),
                        field::value_str(theme)
                    ));
                }
            }
        }
    }
    let wallpaper = data["wallpaper"].as_str().unwrap_or("");
    let saved = dir.join("wallpaper");
    let has_copy = fs::read_dir(saved)
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| e.path().is_file() && e.file_name() != "source");
    if !wallpaper.is_empty() && !has_copy && !std::path::Path::new(wallpaper).is_file() {
        missing.push(format!("Wallpaper: {wallpaper}"));
    }
    Ok(
        json!({"ok":missing.is_empty(),"message":if missing.is_empty(){"Theme bundle is complete".into()}else{format!("Missing theme references: {}",missing.join("; "))}}),
    )
}
pub fn apply(args: &Value) -> Result<Value> {
    if args["source"] == "mcp"
        && args["preview_token"]
            .as_str()
            .is_none_or(|token| token.trim().is_empty())
    {
        anyhow::bail!("MCP apply requires preview_token; review profile.preview before applying");
    }
    let name = crate::gui_api::str_arg(args, "name")?;
    if args["source"] == "gui" && args["components"].is_null() && args["preview_token"].is_null() {
        crate::theme::prepare_legacy(name)?;
    }
    crate::operation::with_source(args["source"].as_str().unwrap_or("cli"), || {
        crate::operation::run(name, || {
            let report = crate::preview::confirmed(args, || apply_inner(args))?;
            crate::operation::note_report(report.clone())?;
            Ok(report)
        })
    })
}
fn apply_inner(args: &Value) -> Result<Value> {
    let name = crate::gui_api::str_arg(args, "name")?;
    let dir = paths::profile_dir(name)?;
    crate::gui_api::component_selection(&args["components"])?;
    if crate::theme::is_package(&dir) {
        if args["components"].is_array() {
            anyhow::bail!("Theme packages apply everything they define; component selection is only for legacy snapshots");
        }
        let options = crate::theme::ApplyOptions {
            keep_wallpaper: args["keep_wallpaper"].as_bool().unwrap_or(false),
            install_missing: args["install_missing"].as_bool().unwrap_or(false),
            skip_dependencies: args["preview_token"].is_string()
                || args["skip_dependencies"].as_bool().unwrap_or(false),
            sync_apps: !args["without_sync"].as_bool().unwrap_or(false),
        };
        return Ok(serde_json::to_value(crate::theme::apply(name, &options)?)?);
    }
    let legacy = metadata(name)?;
    let components = crate::gui_api::component_selection(&args["components"])?;
    let backup = crate::profile::apply_profile_cli_keeping(
        name,
        components,
        args["keep_wallpaper"].as_bool().unwrap_or(false),
    )?;
    let mut errors = vec![];
    let mut values = vec![];
    let defs = components_map();
    // Former GUI-only theme-bundle/1 metadata, after the snapshot pass.
    if dir.join("theme-bundle.json").is_file() {
        for (id, value) in legacy["components"].as_object().into_iter().flatten() {
            if let Some(field) = defs.get(id).and_then(|v| v["field"].as_str()) {
                if !field.is_empty() {
                    values.push((field.to_string(), value.clone()));
                }
            }
        }
        errors.extend(field::apply(&values).errors);
        for (id, value) in legacy["applications"].as_object().into_iter().flatten() {
            let result = crate::app_themes::rpc("apply", &json!({"id":id,"value":value}));
            match result {
                Ok(value) if value["ok"] == true => {}
                Ok(value) => errors.push(
                    value["message"]
                        .as_str()
                        .unwrap_or("Application theme failed")
                        .into(),
                ),
                Err(e) => errors.push(format!("{e:#}")),
            }
        }
    }
    if !errors.is_empty() {
        anyhow::bail!("Legacy bundle apply failed: {}", errors.join("; "));
    }
    Ok(
        json!({"operation_id":crate::operation::current_id(),"applied":crate::profile::get_profile_components(name)?,"backup":backup,"missing":[],"errors":errors,"skipped":[],"installed":[],"dependencies":[],"actions":[]}),
    )
}
fn components_map() -> BTreeMap<String, Value> {
    components()
}

/// Read card fingerprints in one process, independently of live settings.
pub fn summaries(args: &Value) -> Result<Value> {
    let names = args["names"].as_array().context("names must be an array")?;
    let mut out = json!({});
    for name in names {
        let name = name.as_str().context("profile name must be a string")?;
        paths::validate_profile_name(name)?;
        out[name] = summary(name).unwrap_or_else(|e| json!({"error":format!("{e:#}")}));
    }
    Ok(out)
}

pub fn resource_previews(args: &Value) -> Result<Value> {
    let id = crate::gui_api::str_arg(args, "id")?;
    let profile = args["profile"]
        .as_str()
        .map(paths::profile_dir)
        .transpose()?;
    let profile = profile.as_deref().filter(|path| !path.is_symlink());
    let options = args["options"]
        .as_array()
        .context("options must be an array")?;
    let mut previews = BTreeMap::new();
    for value in options.iter().take(512).filter_map(Value::as_str) {
        if let Some(image) = resource_preview(id, value, profile) {
            previews.insert(value, image);
        }
    }
    Ok(json!(previews))
}

fn resource_preview(id: &str, value: &str, profile: Option<&Path>) -> Option<Value> {
    if id == "icon_theme" {
        return preview_icon(profile, value);
    }
    let value = value.strip_prefix("__aurorae__svg__").unwrap_or(value);
    if value.is_empty() || value.starts_with('.') || value.contains(['/', '\\']) {
        return None;
    }
    let places: &[(&str, &str)] = match id {
        "cursor_theme" => &[(".local/share/icons", "icons"), (".icons", "icons")],
        "plasma_global" => &[(".local/share/plasma/look-and-feel", "plasma/look-and-feel")],
        "plasma_style" => &[(".local/share/plasma/desktoptheme", "plasma/desktoptheme")],
        "decoration_theme" => &[(".local/share/aurorae/themes", "aurorae/themes")],
        "gtk_theme" => &[(".local/share/themes", "themes"), (".themes", "themes")],
        _ => return None,
    };
    places.iter().find_map(|(relative, system)| {
        let files = profile
            .and_then(|root| local_resource(root, &root.join(relative).join(value)))
            .into_iter()
            .chain([
                paths::live_path(relative).join(value),
                Path::new("/usr/share").join(system).join(value),
            ]);
        files.into_iter().find_map(|root| {
            [
                "contents/previews/preview.png",
                "preview.png",
                "screenshot.png",
                "thumbnail.png",
            ]
            .iter()
            .find_map(|file| preview_image(&root, &root.join(file)))
        })
    })
}

/// Read the desktop identifiers shown in the library header.
pub fn live_summary() -> Value {
    let mut summary = json!({});
    for id in ["color_scheme", "icon_theme", "cursor_theme", "plasma_theme"] {
        if let Some(value) = field::field(id)
            .and_then(|definition| {
                field::read_raw(definition, &Base::Live)
                    .and_then(|value| field::coerce(definition, value))
            })
            .filter(|value| !field::value_str(value).is_empty())
        {
            summary[id] = value;
        }
    }
    summary
}

/// Verify plausible matches against the same read-only merge used by Apply.
pub fn live_matches(args: &Value) -> Result<Value> {
    use crate::{
        change_plan::{Entry, Plan},
        profile, theme,
    };
    let _lock = crate::operation::MutationGuard::acquire()?;
    let names = args["names"].as_array().context("names must be an array")?;
    let mut live = live_summary();
    let actual: BTreeMap<_, _> = field::capture(&Base::Live).into_iter().collect();
    let rels = profile::resolve_cli_paths(None)?;
    let old_layout = fs::read_to_string(paths::live_path(crate::wallpaper::APPLETSRC_REL)).ok();
    let mut states = BTreeMap::new();
    for name in names {
        let name = name.as_str().context("profile name must be a string")?;
        let dir = paths::profile_dir(name)?;
        let check = || -> Result<&str> {
            anyhow::ensure!(
                !dir.is_symlink() && theme::is_package(&dir),
                "Package unavailable"
            );
            let package = theme::load(&dir)?;
            let dynamic_accent = package.values.get("accent_from_wallpaper") == Some(&json!(true));
            let mut observed = 0;
            let mut unreadable = false;
            let mut different = false;
            let saved = Base::Snapshot(dir.clone());
            for (id, value) in &package.values {
                if dynamic_accent && id == "accent_color" {
                    continue;
                }
                let Some(definition) = field::field(id) else {
                    unreadable = true;
                    continue;
                };
                let expected = field::coerce(definition, value.clone());
                let Some(expected) = expected else {
                    unreadable = true;
                    continue;
                };
                match actual.get(id) {
                    Some(value)
                        if value != &expected
                            && !(definition.kind == "font"
                                && value.as_str().and_then(|s| s.split(',').next())
                                    == expected.as_str().and_then(|s| s.split(',').next())) =>
                    {
                        different = true
                    }
                    Some(_) => observed += 1,
                    None => unreadable = true,
                }
                // The manifest can describe a value whose snapshot is incomplete.
                unreadable |= field::read(definition, &saved).as_ref() != Some(&expected);
            }
            if different {
                return Ok("different");
            }
            if let Some(gtk) = &package.gsettings_gtk_theme {
                match crate::undo::live_gtk_theme() {
                    Some(value) if value != *gtk => return Ok("different"),
                    Some(_) => observed += 1,
                    None => unreadable = true,
                }
            }
            if let Some(relative) = &package.wallpaper {
                let image =
                    local_resource(&dir, &dir.join(relative)).context("Wallpaper unavailable")?;
                let live = crate::wallpaper::plasma_wallpaper_from(&paths::live_path(
                    crate::wallpaper::APPLETSRC_REL,
                ))
                .context("Live wallpaper unavailable")?;
                let len = fs::metadata(&image)?.len();
                if fs::metadata(&live)?.len() != len {
                    return Ok("different");
                }
                anyhow::ensure!(len <= 32 * 1024 * 1024, "Wallpaper is too large to compare");
                if fs::read(live)? != fs::read(image)? {
                    return Ok("different");
                }
                observed += 1;
            }
            if unreadable || observed == 0 || !theme::missing_resources(&dir, &package).is_empty() {
                return Ok("unknown");
            }
            let mut plan = Plan::default();
            let mut tracked = rels.clone();
            for asset in package
                .assets
                .iter()
                .filter(|asset| asset.kind != "wallpaper")
            {
                theme::validate_asset_restore(&dir, &asset.path)?;
                if !tracked.contains(&asset.path) {
                    tracked.push(asset.path.clone());
                }
            }
            profile::plan_restore(name, &dir, &tracked, &mut plan)?;
            profile::plan_post_restore(
                &dir,
                &tracked,
                package.wallpaper.is_none(),
                &[],
                old_layout.as_deref(),
                &mut plan,
            )?;
            crate::app_sync::plan_apps(&dir, &package, &mut plan)?;
            let marker = paths::active_file();
            let policies: BTreeMap<_, _> = tracked
                .iter()
                .map(|relative| {
                    (
                        paths::live_path(relative),
                        crate::appearance::policy(relative),
                    )
                })
                .collect();
            let globals = paths::live_path(".config/kdeglobals");
            let changed = plan.changed().any(|(path, entry)| {
                if *path == marker || matches!(entry, Entry::Directory { .. }) {
                    return false;
                }
                if let (Some(Entry::File { bytes: old, .. }), Entry::File { bytes: new, .. }) =
                    (plan.before.get(path), entry)
                {
                    use crate::appearance::Policy;
                    if matches!(
                        policies.get(path),
                        Some(
                            Policy::Ini(_)
                                | Policy::Kv(_)
                                | Policy::KwinRc
                                | Policy::KwinRules
                                | Policy::Appletsrc
                                | Policy::PanelViews
                        )
                    ) {
                        if let (Ok(old), Ok(new)) =
                            (std::str::from_utf8(old), std::str::from_utf8(new))
                        {
                            let ignore_accent = dynamic_accent && *path == globals;
                            return ini_values(old, ignore_accent)
                                != ini_values(new, ignore_accent);
                        }
                    }
                }
                true
            });
            Ok(if changed { "different" } else { "match" })
        };
        states.insert(name, check().unwrap_or("unknown"));
    }
    let matches: Vec<_> = states
        .iter()
        .filter(|(_, state)| **state == "match")
        .map(|(name, _)| *name)
        .collect();
    live["profiles"] = json!(states);
    live["matches"] = json!(matches);
    Ok(live)
}

fn ini_values(text: &str, ignore_accent: bool) -> BTreeMap<(String, String), Vec<String>> {
    let mut values = BTreeMap::<_, Vec<_>>::new();
    for section in crate::appearance::parse_ini(text) {
        for line in &section.lines {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            if ignore_accent && section.group() == "General" && key.trim() == "AccentColor" {
                continue;
            }
            values
                .entry((section.group().to_owned(), key.trim().to_owned()))
                .or_default()
                .push(value.trim().to_owned());
        }
    }
    values
}

fn summary(name: &str) -> Result<Value> {
    let dir = paths::profile_dir(name)?;
    if fs::symlink_metadata(&dir)?.file_type().is_symlink() {
        anyhow::bail!("Profile directory must not be a symlink");
    }
    let package = if crate::theme::is_package(&dir) {
        Some(crate::theme::load(&dir)?)
    } else {
        None
    };
    let base = Base::Snapshot(dir.clone());
    let read = |id: &str| -> Value {
        let Some(f) = field::field(id) else {
            return Value::Null;
        };
        let raw = match &package {
            Some(p) => p.values.get(id).cloned(),
            None => field::read_raw(f, &base),
        };
        raw.and_then(|v| field::coerce(f, v)).unwrap_or(Value::Null)
    };
    let display = |id| {
        let value = read(id);
        if field::field(id).is_some_and(|f| f.kind == "color") {
            field::color_hex(&value)
        } else {
            field::value_str(&value)
        }
    };
    let accent = display("accent_color");
    let accent_from_wallpaper = read("accent_from_wallpaper").as_bool();
    let preview_accent = if accent_from_wallpaper == Some(true) || accent.is_empty() {
        display("selection_bg")
    } else {
        accent.clone()
    };
    let window_bg = display("window_bg");
    let view_bg = display("view_bg");
    let text_color = display("text_color");
    let swatches: Vec<_> = [&window_bg, &view_bg, &accent, &text_color]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    let wallpaper = package
        .as_ref()
        .and_then(|package| package.wallpaper.as_deref());
    let base16_name = package
        .as_ref()
        .and_then(|p| p.palette.as_ref().map(|palette| &palette.name));
    let missing = package
        .as_ref()
        .map(|p| crate::theme::missing_resources(&dir, p));
    let wallpaper_asset = wallpaper.and_then(|path| preview_wallpaper(&dir, path));
    let icon = preview_icon(Some(&dir), &display("icon_theme"));
    let preview_font = |id| {
        let spec = display(id);
        let mut values = spec.split(',');
        let family = values.next().filter(|item| !item.is_empty());
        let points = values
            .next()
            .and_then(|item| item.parse::<f64>().ok())
            .filter(|size| (4.0..=72.0).contains(size));
        json!({"family":family,"points":points})
    };
    let window_opacity = (read("translucency_enabled") == true)
        .then(|| {
            read("translucency_inactive")
                .as_f64()
                .map(|n| n.clamp(0.0, 100.0) / 100.0)
        })
        .flatten();
    let radius = (display("decoration_library") == "org.kde.klassy")
        .then(|| read("klassy_radius").as_f64().map(|n| n.clamp(0.0, 48.0)))
        .flatten();
    let mut details = BTreeMap::<&str, Vec<Value>>::new();
    for definition in field::registry() {
        if !definition.profile_tracked {
            continue;
        }
        let value = read(&definition.id);
        if value.is_null() || value.as_str() == Some("") {
            continue;
        }
        let group = if matches!(
            definition.id.as_str(),
            "widget_style"
                | "kvantum_theme"
                | "kvantum_translucent"
                | "gtk_theme"
                | "gtk_dark"
                | "gtk_font"
        ) {
            "toolkits"
        } else {
            match definition.section.as_str() {
                "windows" => "windows",
                "colors" => "palette",
                "apps" => "applications",
                _ => "desktop",
            }
        };
        let value = if definition.kind == "color" {
            json!(field::color_hex(&value))
        } else {
            value
        };
        let unit = if definition.kind == "font" {
            "font"
        } else {
            ""
        };
        details
            .entry(group)
            .or_default()
            .push(json!({"label":definition.label,"value":value,"unit":unit}));
    }
    let target_apps: Vec<_> = crate::tracked_apps::get_tracked_apps()
        .into_iter()
        .filter(|app| {
            app.category != "desktop"
                && app.category != "system"
                && app
                    .config_paths
                    .iter()
                    .any(|path| crate::profile::theme_file(&dir, path).is_file())
        })
        .map(|app| app.name)
        .collect();
    let author = local_resource(&dir, &dir.join(crate::theme::MANIFEST))
        .and_then(|file| fs::read(file).ok())
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|meta| meta["author"].as_str().map(str::to_owned));
    let (mut size, mut count) = (0u64, 0u64);
    let mut pending = vec![dir];
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path).into_iter().flatten().flatten() {
            let Ok(meta) = entry.path().symlink_metadata() else {
                continue;
            };
            if meta.is_dir() {
                pending.push(entry.path())
            } else if meta.is_file() {
                size = size.saturating_add(meta.len());
                count += 1
            }
        }
    }
    Ok(
        json!({"accent":accent,"accentFromWallpaper":accent_from_wallpaper,"swatches":swatches,"scheme":display("color_scheme"),"base16Name":base16_name,
        "previewColors":{"window":window_bg,"view":view_bg,"accent":preview_accent,"text":text_color},
        "lookAndFeel":display("look_and_feel"),"plasmaTheme":display("plasma_theme"),
        "kvantum":display("kvantum_theme"),"radius":display("klassy_radius"),
        "blur":display("blur_strength"),"blurEnabled":read("blur_enabled").as_bool().unwrap_or(false),
        "kitty":display("kitty_opacity"),"iconTheme":display("icon_theme"),
        "gtkTheme":display("gtk_theme"),"decoration":display("decoration_theme"),
        "decorationDisabled":read("decoration_disabled") == true,
        "cursorTheme":display("cursor_theme"),"widgetStyle":display("widget_style"),
        "previewStyle":{"windowOpacity":window_opacity,"terminalOpacity":read("kitty_opacity"),
            "radius":radius,"font":preview_font("ui_font"),"titleFont":preview_font("title_font")},
        "iconPreview":icon,"detailFields":details,"detailApps":target_apps,"author":author,
        "wallpaperName":wallpaper.and_then(|path| Path::new(path).file_name()),
        "wallpaperPreview":wallpaper_asset,"unavailableResources":missing,
        "sizeBytes":size,"fileCount":count}),
    )
}

fn preview_wallpaper(dir: &Path, relative: &str) -> Option<Value> {
    preview_image(dir, &dir.join(relative))
}

fn preview_icon(dir: Option<&Path>, theme: &str) -> Option<Value> {
    if theme.is_empty() || theme.starts_with('.') || theme.contains(['/', '\\']) {
        return None;
    }
    let locations = dir
        .and_then(|root| local_resource(root, &root.join(".local/share/icons").join(theme)))
        .into_iter()
        .chain([
            paths::data_home().join("icons").join(theme),
            paths::home_dir().join(".icons").join(theme),
            Path::new("/usr/share/icons").join(theme),
        ]);
    let samples = [
        "48x48/places/folder.svg",
        "48x48/places/folder.png",
        "32x32/places/folder.svg",
        "32x32/places/folder.png",
        "places/48/folder.svg",
        "places/48/folder.png",
        "places/32/folder.svg",
        "scalable/places/folder.svg",
        "places/symbolic/folder-symbolic.svg",
    ];
    locations.into_iter().find_map(|root| {
        samples
            .iter()
            .find_map(|sample| preview_image(&root, &root.join(sample)))
    })
}

fn local_resource(root: &Path, path: &Path) -> Option<PathBuf> {
    let file = path.canonicalize().ok()?;
    file.starts_with(root.canonicalize().ok()?).then_some(file)
}

fn preview_image(dir: &Path, path: &Path) -> Option<Value> {
    let file = local_resource(dir, path)?;
    if !file
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "bmp" | "svg"
            )
        })
    {
        return None;
    }
    let metadata = fs::metadata(&file).ok()?;
    if !metadata.is_file() || metadata.len() > 32 * 1024 * 1024 {
        return None;
    }
    let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(json!({"path":file,"revision":format!("{}-{}",modified.as_nanos(),metadata.len())}))
}
