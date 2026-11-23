//! Save scoped field edits to a theme package without applying it.
use crate::{appearance, browser_appearance, field, gui_api, obsidian, palette, paths, theme};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, os::unix::fs::MetadataExt, path::Path};

const FIELDS: &[(&str, &str)] = &[
    ("plasma_style", "plasma_theme"),
    ("color_scheme", "color_scheme"),
    ("accent_color", "accent_color"),
    ("accent_from_wallpaper", "accent_from_wallpaper"),
    ("icon_theme", "icon_theme"),
    ("cursor_theme", "cursor_theme"),
    ("ui_font", "ui_font"),
    ("animations", "animation_factor"),
    ("corner_radius", "klassy_radius"),
    ("blur_strength", "blur_strength"),
    ("translucency", "translucency_inactive"),
    ("widget_style", "widget_style"),
    ("gtk_theme", "gtk_theme"),
    ("gtk_dark", "gtk_dark"),
    ("kitty_opacity", "kitty_opacity"),
    ("kitty_blur", "kitty_blur"),
    ("alacritty_opacity", "alacritty_opacity"),
    ("alacritty_blur", "alacritty_blur"),
    ("vscode_theme", "vscode_theme"),
    ("cursor_ide_theme", "cursor_ide_theme"),
    ("zed_mode", "zed_mode"),
    ("zed_light", "zed_light"),
    ("zed_dark", "zed_dark"),
    ("btop_theme", "btop_theme"),
    ("btop_rounded", "btop_rounded"),
];

const DECORATION: &str = "decoration_theme";
const BASE16: &str = "base16_palette";
const APP_TARGETS: &str = "app_targets";
const ZEN_ACCENT: &str = "zen_accent_color";
const ZEN_MODE: &str = "zen_mode";
const ZEN_FIELDS: [&str; 2] = [ZEN_ACCENT, ZEN_MODE];
const ZEN_MODES: [(&str, &str); 2] = [("light", "Light"), ("dark", "Dark")];
const KDEGLOBALS: &str = ".config/kdeglobals";
const DECORATION_FIELDS: [&str; 3] = ["decoration_library", DECORATION, "decoration_disabled"];
const AURORAE: &str = "org.kde.kwin.aurorae";
const SVG_PREFIX: &str = "__aurorae__svg__";
const ZED_SELECTIONS: [&str; 2] = ["zed_light", "zed_dark"];
const ZED_MODES: [(&str, &str); 3] = [("system", "System"), ("light", "Light"), ("dark", "Dark")];

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Decoration {
    library: Option<String>,
    theme: Option<String>,
    disabled: Option<bool>,
}

impl Decoration {
    fn selected(library: &str, theme: String) -> Self {
        Self {
            library: Some(library.into()),
            theme: Some(theme),
            disabled: Some(false),
        }
    }

    fn fields(&self) -> [Option<Value>; 3] {
        [
            self.library.as_ref().map(|v| json!(v)),
            self.theme.as_ref().map(|v| json!(v)),
            self.disabled.map(Value::Bool),
        ]
    }

    fn label(&self) -> &str {
        if self.disabled == Some(true) {
            return "No decoration";
        }
        if let Some(theme) = &self.theme {
            return theme.trim_start_matches(SVG_PREFIX);
        }
        self.library.as_deref().unwrap_or("Window decoration")
    }

    fn same_choice(&self, other: &Self) -> bool {
        if self.disabled == Some(true) || other.disabled == Some(true) {
            return self.disabled == other.disabled;
        }
        self.library == other.library && self.theme == other.theme
    }

    fn aurorae_name(&self) -> Option<&str> {
        self.theme.as_deref()?.strip_prefix(SVG_PREFIX)
    }
}

fn stored_decoration(dir: &Path) -> Result<Option<Decoration>> {
    let text = read(dir, ".config/kwinrc")?
        .map(String::from_utf8)
        .transpose()?
        .unwrap_or_default();
    let value = |key| field::ini_get(&text, "org.kde.kdecoration2", key);
    let decoration = Decoration {
        library: value("library").filter(|s| !s.is_empty()),
        theme: value("theme").filter(|s| !s.is_empty()),
        disabled: value("NoPlugin")
            .and_then(|v| field::coerce(field::field("decoration_disabled")?, json!(v))?.as_bool()),
    };
    Ok((decoration != Decoration::default()).then_some(decoration))
}

fn decoration_choices(dir: &Path, manifest: &Value, current: Option<&Decoration>) -> Vec<Value> {
    let mut choices = vec![Decoration {
        disabled: Some(true),
        ..Decoration::default()
    }];
    for (library, theme) in [
        ("org.kde.breeze", "Breeze"),
        ("org.kde.klassy", "Klassy"),
        ("org.kde.oxygen", "Oxygen"),
    ] {
        if theme::available("decoration_library", &json!(library)) {
            choices.push(Decoration::selected(library, theme.into()));
        }
    }
    if theme::available("decoration_library", &json!(AURORAE)) {
        let mut names = BTreeSet::new();
        for folder in crate::deps::roots("aurorae") {
            for entry in fs::read_dir(folder).into_iter().flatten().flatten() {
                if entry.path().is_dir() {
                    if let Ok(name) = entry.file_name().into_string() {
                        let rc = entry.path().join(format!("{name}rc"));
                        if rc.is_file() || entry.path().join("metadata.json").is_file() {
                            names.insert(name);
                        }
                    }
                }
            }
        }
        for asset in ["assets", "inactive_assets"]
            .into_iter()
            .filter_map(|key| manifest[key].as_array())
            .flatten()
        {
            if local_asset(dir, asset, "aurorae") {
                if let Some(name) = asset["name"].as_str() {
                    names.insert(name.into());
                }
            }
        }
        choices.extend(
            names
                .into_iter()
                .map(|name| Decoration::selected(AURORAE, format!("{SVG_PREFIX}{name}"))),
        );
    }
    let mut matched = false;
    let mut result: Vec<_> = choices
        .into_iter()
        .map(|choice| {
            let value = if let Some(current) = current.filter(|d| d.same_choice(&choice)) {
                matched = true;
                current
            } else {
                &choice
            };
            json!({"label":choice.label(),"value":value,"available":true})
        })
        .collect();
    if let Some(current) = current.filter(|_| !matched) {
        let engine = current
            .library
            .as_ref()
            .is_none_or(|lib| theme::available("decoration_library", &json!(lib)));
        let asset = current.theme.as_ref().is_none_or(|name| {
            theme::available(DECORATION, &json!(name))
                || current.aurorae_name().is_some_and(|name| {
                    ["assets", "inactive_assets"]
                        .into_iter()
                        .filter_map(|key| manifest[key].as_array())
                        .flatten()
                        .any(|asset| asset["name"] == name && local_asset(dir, asset, "aurorae"))
                })
        });
        result.push(json!({"label":current.label(),"value":current,"available":engine && asset}));
    }
    result
}

fn read(root: &Path, relative: &str) -> Result<Option<Vec<u8>>> {
    let mut path = root.to_path_buf();
    let mut parts = Path::new(relative).components().peekable();
    while let Some(part) = parts.next() {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(meta) => {
                let expected_type = if parts.peek().is_some() {
                    meta.is_dir()
                } else {
                    meta.is_file()
                };
                ensure!(expected_type, "Unsupported theme path: {relative}");
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(Some(fs::read(path)?))
}

fn revision(dir: &Path) -> Result<String> {
    let mut hash = Sha256::new();
    let root = fs::symlink_metadata(dir).context("Theme is no longer available")?;
    ensure!(root.is_dir(), "Theme is not a local directory");
    hash.update(root.dev().to_le_bytes());
    hash.update(root.ino().to_le_bytes());
    let zen_files = zen_snapshot_files(dir)?;
    let obsidian_files = obsidian::snapshot_appearances(dir)?;
    let obsidian_files = obsidian_files
        .iter()
        .map(|path| {
            path.strip_prefix(dir)?
                .to_str()
                .context("Invalid Obsidian snapshot path")
        })
        .collect::<Result<Vec<_>>>()?;
    let files: BTreeSet<_> = FIELDS
        .iter()
        .filter_map(|(_, id)| field::field(id).map(|f| f.file.as_str()))
        .chain(
            appearance::TERMINAL_CONFIGS
                .into_iter()
                .flat_map(|(host, include)| [host, terminal_snapshot_path(dir, include)]),
        )
        .chain([theme::MANIFEST])
        .chain(field::GTK_SETTINGS.iter().copied())
        .chain(zen_files.iter().map(String::as_str))
        .chain(obsidian_files)
        .collect();
    for file in files {
        hash.update(file.as_bytes());
        if let Some(bytes) = read(dir, file)? {
            hash.update([1]);
            hash.update(Sha256::digest(bytes));
        } else {
            hash.update([0]);
        }
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn zen_snapshot_files(dir: &Path) -> Result<Vec<String>> {
    let mut paths = Vec::new();
    for root in browser_appearance::zen_roots() {
        let first = paths.len();
        let entries = match fs::read_dir(dir.join(root)) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        for entry in entries {
            let entry = entry?;
            if entry.file_type()?.is_dir()
                && fs::symlink_metadata(entry.path().join("user.js")).is_ok()
            {
                let relative = entry.path().join("user.js");
                paths.push(
                    relative
                        .strip_prefix(dir)?
                        .to_str()
                        .context("Invalid Zen snapshot path")?
                        .to_owned(),
                );
            }
        }
        paths[first..].sort();
    }
    Ok(paths)
}

fn zen_values(
    dir: &Path,
    package: &theme::Package,
) -> Result<(Option<palette::Rgb>, Option<bool>)> {
    let text = match zen_snapshot_files(dir)?.first() {
        Some(path) => read(dir, path)?
            .map(String::from_utf8)
            .transpose()?
            .unwrap_or_default(),
        None => String::new(),
    };
    let generated = package
        .palette
        .as_ref()
        .filter(|_| package.app_targets.iter().any(|app| app == "zen"));
    let accent = generated
        .filter(|_| !package.zen_overrides.accent)
        .map(|palette| palette.accent_rgb())
        .or_else(|| browser_appearance::zen_accent(&text));
    let dark = generated
        .filter(|_| !package.zen_overrides.mode)
        .map(|palette| palette.variant == palette::Variant::Dark)
        .or_else(|| browser_appearance::zen_mode(&text));
    Ok((accent, dark))
}

#[derive(Default)]
struct ZenEdits {
    // None leaves the field alone. Some(None) explicitly excludes it.
    accent: Option<Option<palette::Rgb>>,
    dark: Option<Option<bool>>,
}

impl ZenEdits {
    fn is_empty(&self) -> bool {
        self.accent.is_none() && self.dark.is_none()
    }
}

fn zen_changes(
    dir: &Path,
    package: &theme::Package,
    changes: &serde_json::Map<String, Value>,
) -> Result<ZenEdits> {
    if !ZEN_FIELDS.iter().any(|key| changes.contains_key(*key)) {
        return Ok(ZenEdits::default());
    }
    let (current_accent, current_dark) = zen_values(dir, package)?;
    let accent = changes
        .get(ZEN_ACCENT)
        .map(|value| match value {
            Value::Null => Ok(None),
            Value::String(color) => palette::Rgb::parse(color).map(Some),
            _ => bail!("Invalid Zen accent color"),
        })
        .transpose()?
        .filter(|value| *value != current_accent);
    let dark = changes
        .get(ZEN_MODE)
        .map(|value| match value {
            Value::Null => Ok(None),
            Value::String(mode) if mode == "dark" => Ok(Some(true)),
            Value::String(mode) if mode == "light" => Ok(Some(false)),
            _ => bail!("Invalid Zen theme mode"),
        })
        .transpose()?
        .filter(|value| *value != current_dark);
    Ok(ZenEdits { accent, dark })
}

fn patch_zen(
    dir: &Path,
    manifest: &mut Value,
    package: &mut theme::Package,
    edits: &ZenEdits,
) -> Result<()> {
    let mut files = zen_snapshot_files(dir)?;
    if files.is_empty() {
        files.push(".zen/default/user.js".into());
    }
    for file in files {
        let original = read(dir, &file)?.map(String::from_utf8).transpose()?;
        let mut text = original.clone().unwrap_or_default();
        if let Some(accent) = edits.accent {
            text = browser_appearance::set_zen_accent(&text, accent);
        }
        if let Some(dark) = edits.dark {
            text = browser_appearance::set_zen_mode(&text, dark);
        }
        // Keep an existing empty snapshot to preserve its priority over
        // snapshots from another Zen installation.
        if original.as_deref().unwrap_or_default() != text {
            gui_api::write_bytes(&dir.join(&file), text.as_bytes(), None)?;
        }
    }
    if edits.accent.is_some() {
        package.zen_overrides.accent = true;
    }
    if edits.dark.is_some() {
        package.zen_overrides.mode = true;
    }
    for (id, value) in [
        (
            ZEN_ACCENT,
            edits.accent.map(|value| value.map(|rgb| json!(rgb.hex()))),
        ),
        (
            "zen_window_scheme",
            edits
                .dark
                .map(|value| value.map(|dark| json!(u8::from(!dark)))),
        ),
    ] {
        if let Some(Some(value)) = value {
            package.values.insert(id.into(), value);
        } else if value.is_some() {
            package.values.remove(id);
        }
    }
    manifest["zen_overrides"] = json!(package.zen_overrides);
    Ok(())
}

fn tree_revision(dir: &Path) -> Result<String> {
    fn visit(path: &Path, hash: &mut Sha256) -> Result<()> {
        let node = fs::symlink_metadata(path)?;
        hash.update(
            path.file_name()
                .context("Missing theme path")?
                .as_encoded_bytes(),
        );
        for mark in [
            node.ino(),
            node.len(),
            node.ctime() as u64,
            node.ctime_nsec() as u64,
        ] {
            hash.update(mark.to_le_bytes());
        }
        if node.is_dir() {
            let mut children = fs::read_dir(path)?.collect::<std::io::Result<Vec<_>>>()?;
            children.sort_by_key(fs::DirEntry::file_name);
            for child in children {
                visit(&child.path(), hash)?;
            }
        }
        Ok(())
    }
    let mut hash = Sha256::new();
    visit(dir, &mut hash)?;
    Ok(format!("{:x}", hash.finalize()))
}

fn editor_state(dir: &Path, revision: &str) -> Result<Value> {
    let (manifest, package) = package(dir)?;
    let mut values = serde_json::Map::new();
    let mut bundled = serde_json::Map::new();
    for (component, id) in FIELDS {
        let f = field::field(id).context("Unknown theme field")?;
        let mut value = saved_value(dir, f, &package)?.unwrap_or(Value::Null);
        if f.kind == "color" && !value.is_null() {
            let hex = field::color_hex(&value);
            value = if hex.is_empty() {
                Value::Null
            } else {
                json!(hex)
            };
        } else if f.kind == "font" {
            if let Some(raw) = value.as_str() {
                value = json!(raw.split(',').next().unwrap_or_default());
            }
        }
        values.insert((*component).into(), value);
        if let Some(kind) = theme::bundled_kind_for_field(id) {
            let names: BTreeSet<_> = ["assets", "inactive_assets"]
                .into_iter()
                .filter_map(|key| manifest[key].as_array())
                .flatten()
                .filter(|asset| local_asset(dir, asset, kind))
                .filter_map(|asset| asset["name"].as_str())
                .collect();
            bundled.insert((*component).into(), json!(names));
        }
    }
    let decoration = stored_decoration(dir)?;
    values.insert(DECORATION.into(), json!(decoration));
    values.insert(BASE16.into(), json!(package.palette));
    values.insert(APP_TARGETS.into(), json!(package.app_targets));
    let (zen_accent, zen_dark) = zen_values(dir, &package)?;
    values.insert(ZEN_ACCENT.into(), json!(zen_accent.map(palette::Rgb::hex)));
    values.insert(
        ZEN_MODE.into(),
        json!(zen_dark.map(|dark| if dark { "dark" } else { "light" })),
    );
    let mut choices =
        json!({"decoration_theme":decoration_choices(dir, &manifest, decoration.as_ref())});
    let (obsidian_values, obsidian_choices) =
        obsidian::editor(dir, &package.obsidian_overrides, obsidian_palette(&package))?;
    for key in [obsidian::THEME_FIELD, obsidian::MODE_FIELD] {
        values.insert(key.into(), obsidian_values[key].clone());
        choices[key] = obsidian_choices[key].clone();
    }
    choices[ZEN_MODE] =
        json!(ZEN_MODES
            .map(|(value, label)| { json!({"value":value,"label":label,"available":true}) }));
    for (id, app) in [
        ("vscode_theme", "vscode"),
        ("cursor_ide_theme", "cursor"),
        ("btop_theme", "btop"),
    ] {
        choices[id] = app_choices(dir, &package, &manifest, id, app, values[id].as_str())?;
    }
    let zed_choices = app_choices(dir, &package, &manifest, "zed_dark", "zed", None)?;
    for id in ZED_SELECTIONS {
        choices[id] = zed_choices.clone();
        include_current_choice(&mut choices[id], id, values[id].as_str());
    }
    choices["zed_mode"] =
        json!(ZED_MODES
            .map(|(value, label)| { json!({"value":value,"label":label,"available":true}) }));
    include_current_choice(
        &mut choices["zed_mode"],
        "zed_mode",
        values["zed_mode"].as_str(),
    );
    let apps = crate::tracked_apps::get_tracked_apps();
    let targets: Vec<_> = crate::app_sync::TARGETS
        .iter()
        .map(|id| {
            let name = apps
                .iter()
                .find(|app| app.id == *id)
                .map_or(*id, |app| app.name.as_str());
            json!({"id": id, "name": name})
        })
        .collect();
    Ok(
        json!({"revision":revision,"values":values,"bundled":bundled,"choices":choices,
        "palettePresets":crate::gallery::bundled_palettes(),"appTargets":targets}),
    )
}

fn obsidian_palette(package: &theme::Package) -> Option<&palette::Palette> {
    package.palette.as_ref().filter(|_| {
        package
            .app_targets
            .iter()
            .any(|target| target == "obsidian")
    })
}

fn app_choices(
    dir: &Path,
    package: &theme::Package,
    manifest: &Value,
    field: &str,
    app: &str,
    current: Option<&str>,
) -> Result<Value> {
    let available = crate::app_themes::rpc("themes", &json!({"id":app}))?;
    let installed = available["themes"].as_array().into_iter().flatten();
    let cached = crate::app_themes::cached_color_themes(dir, package, app);
    let mut choices = std::collections::BTreeMap::new();
    for option in installed.chain(&cached) {
        if let Some(id) = option["id"].as_str() {
            let value = current
                .filter(|name| option["label"] == *name)
                .unwrap_or(id);
            choices.insert(
                value.to_string(),
                json!({"value":value,"label":option["label"],"available":true}),
            );
        }
    }
    if app == "btop" {
        for name in std::iter::once("Default").chain(
            ["assets", "inactive_assets"]
                .into_iter()
                .filter_map(|key| manifest[key].as_array())
                .flatten()
                .filter(|asset| local_asset(dir, asset, "btop"))
                .filter_map(|asset| asset["name"].as_str()),
        ) {
            choices.insert(
                name.to_string(),
                json!({"value":name,"label":name,"available":true}),
            );
        }
    }
    if app == "zed" {
        for asset in ["assets", "inactive_assets"]
            .into_iter()
            .filter_map(|key| manifest[key].as_array())
            .flatten()
            .filter(|asset| local_asset(dir, asset, "zed"))
        {
            if let Some(name) = asset["name"].as_str() {
                choices
                    .entry(name.into())
                    .or_insert_with(|| json!({"value":name,"label":name,"available":true}));
            }
        }
    }
    let mut choices = json!(choices.into_values().collect::<Vec<_>>());
    include_current_choice(&mut choices, field, current);
    Ok(choices)
}

fn include_current_choice(choices: &mut Value, field: &str, current: Option<&str>) {
    let choices = choices.as_array_mut().expect("editor choices");
    if let Some(name) =
        current.filter(|name| !choices.iter().any(|choice| choice["value"] == *name))
    {
        let available = field != "zed_mode" && theme::available(field, &json!(name));
        choices.push(json!({"value":name,"label":name,"available":available}));
    }
}

fn terminal_snapshot_path<'a>(dir: &Path, include: &'a str) -> &'a str {
    if fs::symlink_metadata(dir.join(include)).is_ok() {
        return include;
    }
    let (host, legacy) = if include == appearance::KITTY_THEME {
        (appearance::KITTY_HOST, ".config/kitty/theme-profile.conf")
    } else {
        (
            appearance::ALACRITTY_HOST,
            ".config/alacritty/theme-profile.toml",
        )
    };
    [legacy, host]
        .into_iter()
        .find(|path| fs::symlink_metadata(dir.join(path)).is_ok())
        .unwrap_or(include)
}

fn saved_value(dir: &Path, f: &field::FieldDef, package: &theme::Package) -> Result<Option<Value>> {
    if f.file == appearance::ZED_CONFIG {
        let text = read(dir, &f.file)?.map(String::from_utf8).transpose()?;
        return Ok(match text {
            Some(text) => appearance::zed_theme_value(&text, &f.path[1]),
            None => package
                .values
                .get(&f.id)
                .filter(|value| value.is_string())
                .cloned(),
        });
    }
    if !field::terminal_fields(&f.file).is_empty() {
        let relative = terminal_snapshot_path(dir, &f.file);
        let text = read(dir, relative)?.map(String::from_utf8).transpose()?;
        return Ok(text
            .as_deref()
            .and_then(|text| field::terminal_value(f, text)));
    }
    if theme::editor_selector_kind(&f.id).is_some() || f.file == appearance::BTOP_CONFIG {
        let Some(text) = read(dir, &f.file)?.map(String::from_utf8).transpose()? else {
            return Ok(None);
        };
        let raw = if f.file == appearance::BTOP_CONFIG {
            field::ini_get(&text, "", &f.key).map(|s| json!(s.trim_matches('"')))
        } else {
            field::json_get(&text, &f.path).filter(Value::is_string)
        };
        return Ok(raw
            .filter(|v| v != "")
            .and_then(|value| field::coerce(f, value)));
    }
    if f.id == "gtk_theme" {
        if let Some(name) = &package.gsettings_gtk_theme {
            return Ok(Some(json!(name)));
        }
    }
    Ok(package
        .values
        .get(&f.id)
        .and_then(|value| match f.kind.as_str() {
            "color" => field::normalize_input(f, value),
            _ => field::coerce(f, value.clone()),
        }))
}

fn validate_editor_settings(text: &str) -> Result<()> {
    let clean = crate::config_edit::strip_trailing_commas(&crate::config_edit::strip_jsonc(text));
    ensure!(
        serde_json::from_str::<Value>(&clean)?.is_object(),
        "Invalid editor settings"
    );
    Ok(())
}

fn patch_editor_settings(dir: &Path, f: &field::FieldDef, value: Option<&Value>) -> Result<()> {
    let old = read(dir, &f.file)?.map(String::from_utf8).transpose()?;
    let text = old.as_deref().unwrap_or("{}\n");
    let key = f.path.first().context("Missing editor settings key")?;
    validate_editor_settings(text)?;
    let edited = if let Some(value) = value {
        let change = serde_json::to_string(&json!({(key):value}))?;
        appearance::jsonc_overlay(&change, text, &[key])
    } else {
        appearance::jsonc_remove_member(text, key)
    };
    if edited != text {
        gui_api::write_bytes(&dir.join(&f.file), edited.as_bytes(), None)?;
    }
    Ok(())
}

fn patch_zed_settings(
    dir: &Path,
    manifest: &mut Value,
    package: &mut theme::Package,
    edits: &[(&field::FieldDef, Option<Value>, Option<Value>)],
) -> Result<()> {
    let old = read(dir, appearance::ZED_CONFIG)?
        .map(String::from_utf8)
        .transpose()?;
    let mut text = old.clone().unwrap_or_else(|| "{}\n".into());
    validate_editor_settings(&text)?;
    let previous: std::collections::BTreeMap<_, _> = ["zed_mode", "zed_light", "zed_dark"]
        .into_iter()
        .map(|id| {
            let f = field::field(id).expect("Zed theme field");
            Ok((id, saved_value(dir, f, package)?))
        })
        .collect::<Result<_>>()?;
    if old.is_none() {
        for (&id, value) in &previous {
            if let Some(value) = value {
                let key = &field::field(id).expect("Zed theme field").path[1];
                text = appearance::patch_zed_theme(&text, key, Some(value));
            }
        }
    }
    for (f, _, value) in edits
        .iter()
        .filter(|(f, _, _)| f.file == appearance::ZED_CONFIG)
    {
        text = appearance::patch_zed_theme(&text, &f.path[1], value.as_ref());
    }
    validate_editor_settings(&text)?;
    let path = dir.join(appearance::ZED_CONFIG);
    if appearance::jsonc_empty_object(&text) {
        if old.is_some() {
            fs::remove_file(path)?;
        }
    } else if Some(&text) != old.as_ref() {
        gui_api::write_bytes(&path, text.as_bytes(), None)?;
    }
    let mut before = BTreeSet::new();
    let mut selected = BTreeSet::new();
    for (id, old) in previous {
        let key = &field::field(id).expect("Zed theme field").path[1];
        let new = appearance::zed_theme_value(&text, key);
        if ZED_SELECTIONS.contains(&id) {
            before.extend(old.as_ref().and_then(Value::as_str).map(str::to_owned));
            before.extend(
                package
                    .values
                    .get(id)
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            );
            selected.extend(new.as_ref().and_then(Value::as_str).map(str::to_owned));
            if old != new {
                package.sources.remove(id);
            }
        }
        match new {
            Some(value) => {
                package.values.insert(id.into(), value);
            }
            None => {
                package.values.remove(id);
            }
        }
    }
    update_zed_references(dir, manifest, package, &before, &selected);
    Ok(())
}

fn update_zed_references(
    dir: &Path,
    manifest: &mut Value,
    package: &mut theme::Package,
    before: &BTreeSet<String>,
    selected: &BTreeSet<String>,
) {
    let cached = crate::app_themes::cached_color_themes(dir, package, "zed");
    let mut deps = manifest["dependencies"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for name in before.difference(selected) {
        select_bundled_asset(dir, manifest, "zed", Some(name), None);
        deps.retain(|item| item["kind"] != "zed" || item["name"] != name.as_str());
    }
    for name in selected {
        let bundled = select_bundled_asset(dir, manifest, "zed", None, Some(name))
            || cached
                .iter()
                .any(|theme| crate::app_themes::color_theme_matches(theme, name));
        if bundled {
            deps.retain(|item| item["kind"] != "zed" || item["name"] != name.as_str());
        }
        for id in ZED_SELECTIONS
            .into_iter()
            .filter(|id| package.values.get(*id).is_some_and(|v| v == name.as_str()))
        {
            if bundled {
                package.sources.remove(id);
            } else if let Some(source) = package
                .sources
                .get(id)
                .cloned()
                .or_else(|| theme::installed_editor_source(id, name))
            {
                if !deps
                    .iter()
                    .any(|item| item["kind"] == "zed" && item["name"] == name.as_str())
                {
                    deps.push(json!(crate::deps::editor_theme_dependency(
                        "zed",
                        name.clone(),
                        &source
                    )));
                }
                package.sources.insert(id.into(), source);
            }
        }
    }
    manifest["sources"] = json!(package.sources);
    manifest["dependencies"] = json!(deps);
}

fn update_editor_reference(
    dir: &Path,
    manifest: &mut Value,
    package: &mut theme::Package,
    field: &str,
    old: Option<&str>,
    next: Option<&str>,
) {
    let Some(editor) = theme::editor_selector_kind(field) else {
        return;
    };
    package.sources.remove(field);
    let recorded = package.values.get(field).and_then(Value::as_str);
    let mut dependencies = manifest["dependencies"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    dependencies.retain(|item| {
        item["kind"] != editor
            || ![old, recorded]
                .into_iter()
                .flatten()
                .any(|name| item["name"] == name)
    });
    if let Some(name) = next {
        let is_cached = crate::app_themes::cached_color_themes(dir, package, editor)
            .iter()
            .any(|item| crate::app_themes::color_theme_matches(item, name));
        if let Some(source) = theme::installed_editor_source(field, name).filter(|_| !is_cached) {
            if !dependencies
                .iter()
                .any(|item| item["kind"] == editor && item["name"] == name)
            {
                dependencies.push(json!(crate::deps::editor_theme_dependency(
                    editor,
                    name.into(),
                    &source
                )));
            }
            package.sources.insert(field.into(), source);
        }
    }
    manifest["dependencies"] = json!(dependencies);
    manifest["sources"] = json!(package.sources);
}

fn patch_terminal(dir: &Path, f: &field::FieldDef, value: Option<&Value>) -> Result<()> {
    let relative = terminal_snapshot_path(dir, &f.file);
    let old = read(dir, relative)?.map(String::from_utf8).transpose()?;
    let text = old.as_deref().unwrap_or_default();
    let mut edited = field::unset_terminal(f, text)?;
    if let Some(value) = value {
        edited = field::patched(f, &edited, value)?;
    }
    if edited != text {
        let path = dir.join(relative);
        if edited.trim().is_empty() && old.is_some() {
            fs::remove_file(&path)?;
            let fallback = terminal_snapshot_path(dir, &f.file);
            if fallback != relative && dir.join(fallback).is_file() {
                // The empty selected include masks an older snapshot file.
                gui_api::write_bytes(&path, b"", None)?;
            }
        } else {
            gui_api::write_bytes(&path, edited.as_bytes(), None)?;
        }
    }
    Ok(())
}

fn targets_change(value: &Value) -> Result<Vec<String>> {
    let targets: Vec<String> =
        serde_json::from_value(value.clone()).context("Invalid Base16 application list")?;
    let mut seen = BTreeSet::new();
    for target in &targets {
        ensure!(
            crate::app_sync::TARGETS.contains(&target.as_str()),
            "Unknown Base16 application: {target}"
        );
        ensure!(seen.insert(target), "Repeated Base16 application: {target}");
    }
    Ok(targets)
}

fn package(dir: &Path) -> Result<(Value, theme::Package)> {
    let bytes = read(dir, theme::MANIFEST)?.context("This theme uses a legacy format")?;
    let manifest: Value = serde_json::from_slice(&bytes)?;
    let package = theme::parse_package(&bytes)?;
    Ok((manifest, package))
}

pub(crate) fn editor(name: &str) -> Result<Value> {
    let dir = paths::profile_dir(name)?;
    let revision = revision(&dir)?;
    editor_state(&dir, &revision)
}

fn patch_ini(dir: &Path, file: &str, group: &str, key: &str, value: Option<&Value>) -> Result<()> {
    let path = dir.join(file);
    let old = read(dir, file)?.map(String::from_utf8).transpose()?;
    if old.is_none() && value.is_none() {
        return Ok(());
    }
    let text = old.as_deref().unwrap_or_default();
    let mut sections = appearance::parse_ini(text);
    for section in &mut sections {
        if section.group() == group {
            section
                .lines
                .retain(|line| appearance::base_key(line) != Some(key));
        }
    }
    sections.retain(|section| {
        section.group() != group || section.lines.iter().any(|line| !line.trim().is_empty())
    });
    let mut next = appearance::render_ini(&sections);
    if let Some(value) = value {
        next = field::set_ini_text(&next, group, key, &field::value_str(value));
    }
    if next.trim().is_empty() && old.is_some() {
        fs::remove_file(path)?;
    } else if next != text {
        gui_api::write_bytes(&path, next.as_bytes(), None)?;
    }
    Ok(())
}

fn local_asset(dir: &Path, asset: &Value, kind: &str) -> bool {
    asset["kind"] == kind
        && asset["path"]
            .as_str()
            .is_some_and(|path| theme::bundled_asset_present(dir, kind, path))
}

fn select_bundled_asset(
    dir: &Path,
    manifest: &mut Value,
    kind: &str,
    old: Option<&str>,
    next: Option<&str>,
) -> bool {
    let mut active = manifest["assets"].as_array().cloned().unwrap_or_default();
    let mut inactive = manifest["inactive_assets"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if let Some(old) = old {
        active.retain(|asset| {
            if asset["kind"] == kind && asset["name"] == old {
                if !inactive.contains(asset) {
                    inactive.push(asset.clone());
                }
                false
            } else {
                true
            }
        });
    }
    let mut bundled = false;
    if let Some(next) = next {
        inactive.retain(|asset| {
            if asset["name"] == next && local_asset(dir, asset, kind) {
                if !active.contains(asset) {
                    active.push(asset.clone());
                }
                false
            } else {
                true
            }
        });
        bundled = active
            .iter()
            .any(|asset| asset["name"] == next && local_asset(dir, asset, kind));
    }
    manifest["assets"] = json!(active);
    if inactive.is_empty() {
        manifest
            .as_object_mut()
            .expect("parsed theme manifest")
            .remove("inactive_assets");
    } else {
        manifest["inactive_assets"] = json!(inactive);
    }
    bundled
}

fn update_references(
    dir: &Path,
    manifest: &mut Value,
    id: &str,
    old: Option<&str>,
    next: Option<&str>,
) {
    let Some(kind) = crate::deps::reference_kind(id) else {
        return;
    };
    let bundled = theme::bundled_kind_for_field(id)
        .is_some_and(|asset_kind| select_bundled_asset(dir, manifest, asset_kind, old, next));
    update_dependency_refs(manifest, kind, old, next, bundled);
}

fn update_dependency_refs(
    manifest: &mut Value,
    kind: &str,
    old: Option<&str>,
    next: Option<&str>,
    bundled: bool,
) {
    if old == next && !bundled {
        return;
    }
    let mut dependencies = manifest["dependencies"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let matches = |dependency: &Value, name: &str| {
        dependency["kind"] == kind
            && dependency["name"].as_str().is_some_and(|entry| {
                entry == name || (kind == "widget-style" && entry.eq_ignore_ascii_case(name))
            })
    };
    dependencies.retain(|item| {
        !old.is_some_and(|name| matches(item, name))
            && !(bundled && next.is_some_and(|name| matches(item, name)))
    });
    if let Some(new) = next.filter(|_| !bundled) {
        if !dependencies.iter().any(|item| matches(item, new)) {
            if let Some(dep) = crate::deps::editor_reference(kind, new) {
                dependencies.push(json!(dep));
            }
        }
    }
    manifest["dependencies"] = json!(dependencies);
}

fn replace_color_scheme(
    dir: &Path,
    manifest: &Value,
    package: &mut theme::Package,
    name: Option<&str>,
) -> Result<()> {
    let contents = if let Some(name) = name {
        let bundled = manifest["assets"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|asset| asset["name"] == name && local_asset(dir, asset, "color-scheme"));
        if let Some(asset) = bundled {
            let path = asset["path"]
                .as_str()
                .context("Missing color scheme path")?;
            String::from_utf8(read(dir, path)?.context("Color scheme is no longer available")?)?
        } else {
            let path = crate::deps::locate("color-scheme", name)
                .context("Color scheme is no longer available")?;
            fs::read_to_string(path)?
        }
    } else {
        String::new()
    };
    let colors = appearance::parse_ini(&contents)
        .into_iter()
        .filter(|section| appearance::kde_palette_group(section.group()));
    if name.is_some() {
        ensure!(
            colors
                .clone()
                .any(|section| section.group().starts_with("Colors:")
                    && section
                        .lines
                        .iter()
                        .any(|line| appearance::base_key(line).is_some())),
            "The color scheme contains no KDE colors"
        );
    }
    for key in ["contrast", "frameContrast"] {
        let value = field::ini_get(&contents, "KDE", key).map(Value::String);
        patch_ini(dir, KDEGLOBALS, "KDE", key, value.as_ref())?;
    }
    patch_ini(dir, KDEGLOBALS, "General", "ColorSchemeHash", None)?;
    let old = read(dir, KDEGLOBALS)?.map(String::from_utf8).transpose()?;
    let mut sections = appearance::parse_ini(old.as_deref().unwrap_or_default());
    sections.retain(|section| !appearance::kde_palette_group(section.group()));
    sections.extend(colors);
    let text = appearance::render_ini(&sections);
    if text.trim().is_empty() {
        if old.is_some() {
            fs::remove_file(dir.join(KDEGLOBALS))?;
        }
    } else {
        gui_api::write_bytes(&dir.join(KDEGLOBALS), text.as_bytes(), None)?;
    }
    capture_scheme_fields(package, &text);
    Ok(())
}

fn capture_scheme_fields(package: &mut theme::Package, text: &str) {
    for f in field::registry()
        .iter()
        .filter(|f| f.file == KDEGLOBALS && appearance::kde_palette_group(&f.group))
    {
        if let Some(value) = field::ini_get(text, &f.group, &f.key) {
            package.values.insert(f.id.clone(), json!(value));
        } else {
            package.values.remove(&f.id);
        }
    }
}

fn palette_change(value: &Value) -> Result<Option<palette::Palette>> {
    if value.is_null() {
        return Ok(None);
    }
    let mut p: palette::Palette =
        serde_json::from_value(value.clone()).context("Invalid Base16 palette")?;
    p.validate()?;
    ensure!(
        p.name.len() <= 128 && !p.name.chars().any(char::is_control),
        "Invalid Base16 palette name"
    );
    p.accent = p.accent_rgb().hex();
    p.base = (0..16).map(|i| p.slot(i).hex()).collect();
    Ok(Some(p))
}

// The same renderers produce the comparison and replacement. A captured file
// with different colors remains an explicit override of palette generation.
fn refresh_rendered(
    dir: &Path,
    relative: &str,
    previous: &palette::Palette,
    next: &palette::Palette,
    render: impl Fn(&str, &palette::Palette) -> Result<String>,
) -> Result<()> {
    let Some(text) = read(dir, relative)
        .ok()
        .flatten()
        .and_then(|bytes| String::from_utf8(bytes).ok())
    else {
        return Ok(());
    };
    if render(&text, previous).is_ok_and(|expected| expected.trim_end() == text.trim_end()) {
        let updated = render(&text, next)?;
        if updated.trim_end() != text.trim_end() {
            gui_api::write_bytes(&dir.join(relative), updated.as_bytes(), None)?;
        }
    }
    Ok(())
}

fn app_palette_resources(p: &palette::Palette) -> std::collections::BTreeMap<String, String> {
    crate::terminal_apps::bundle_resources(p)
        .into_iter()
        .chain(crate::third_apps::bundle_resources(p))
        .chain(crate::four_apps::bundle_resources(p))
        .chain(crate::launcher_apps::bundle_resources(p))
        .collect()
}

fn refresh_app_palettes(
    dir: &Path,
    package: &theme::Package,
    previous: &palette::Palette,
    next: &palette::Palette,
) -> Result<()> {
    let before = app_palette_resources(previous);
    for (relative, next_text) in app_palette_resources(next) {
        if let Some(old) = before.get(&relative).filter(|old| **old != next_text) {
            refresh_rendered(dir, &relative, previous, next, |text, p| {
                let generated = if p == previous { old } else { &next_text };
                appearance::applied(&relative, generated, Some(text))
            })?;
        }
    }
    refresh_rendered(dir, appearance::KITTY_THEME, previous, next, |text, p| {
        Ok(palette::kitty_conf_with_palette(text, p))
    })?;
    refresh_rendered(
        dir,
        ".config/alacritty/kolira.toml",
        previous,
        next,
        palette::alacritty_with_palette,
    )?;
    refresh_rendered(
        dir,
        crate::extras::GHOSTTY_THEME,
        previous,
        next,
        |text, p| Ok(crate::extras::ghostty_palette_text(text, p)),
    )?;
    let id = palette::kde_scheme_id(previous);
    if palette::kde_scheme_id(next) == id {
        for asset in package.assets.iter().filter(|asset| {
            asset.kind == "konsole-scheme" && asset.name == id && asset.origin == "generated"
        }) {
            refresh_rendered(dir, &asset.path, previous, next, |text, p| {
                let mut scheme = crate::konsole::colorscheme(p);
                for key in ["Opacity", "Blur"] {
                    if let Some(value) = field::ini_get(text, "General", key) {
                        scheme = field::set_ini_text(&scheme, "General", key, &value);
                    }
                }
                Ok(scheme)
            })?;
        }
    }
    Ok(())
}

fn refresh_kde_palette(
    dir: &Path,
    package: &mut theme::Package,
    previous: &palette::Palette,
    next: &palette::Palette,
) -> Result<()> {
    let id = palette::kde_scheme_id(previous);
    if palette::kde_scheme_id(next) != id || package.values.get("color_scheme") != Some(&json!(id))
    {
        return Ok(());
    }
    let asset = package
        .assets
        .iter()
        .find(|a| a.kind == "color-scheme" && a.name == id && a.origin == "generated");
    let Some(asset) = asset else { return Ok(()) };
    let before = palette::kde_color_scheme(previous);
    if !read(dir, &asset.path)
        .ok()
        .flatten()
        .is_some_and(|data| data == before.as_bytes())
    {
        return Ok(());
    }
    let Some(data) = read(dir, KDEGLOBALS)? else {
        return Ok(());
    };
    let mut text = String::from_utf8(data)?;
    if field::ini_get(&text, "General", "ColorScheme").as_deref() != Some(id.as_str()) {
        return Ok(());
    }
    let after = palette::kde_color_scheme(next);
    gui_api::write_bytes(&dir.join(&asset.path), after.as_bytes(), None)?;
    for section in appearance::parse_ini(&before)
        .into_iter()
        .filter(|s| appearance::kde_palette_group(s.group()))
    {
        for line in &section.lines {
            if let Some((key, value)) = line.split_once('=') {
                let group = section.group();
                if field::ini_get(&text, group, key).as_deref() == Some(value) {
                    if let Some(updated) = field::ini_get(&after, group, key) {
                        text = field::set_ini_text(&text, group, key, &updated);
                    }
                }
            }
        }
    }
    let accent = previous.accent_rgb().kde();
    if field::ini_get(&text, "General", "AccentColor").as_deref() == Some(accent.as_str()) {
        let next_accent = next.accent_rgb().kde();
        text = field::set_ini_text(&text, "General", "AccentColor", &next_accent);
        package
            .values
            .insert("accent_color".into(), json!(next_accent));
    }
    gui_api::write_bytes(&dir.join(KDEGLOBALS), text.as_bytes(), None)?;
    patch_ini(dir, KDEGLOBALS, "General", "ColorSchemeHash", None)?;
    capture_scheme_fields(package, &text);
    Ok(())
}

fn refresh_palette_data(
    dir: &Path,
    relative: &str,
    previous: &Value,
    next: Option<&palette::Palette>,
) -> Result<()> {
    let stored = read(dir, relative)
        .ok()
        .flatten()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    if stored.as_ref() != Some(previous) {
        return Ok(());
    }
    let path = dir.join(relative);
    if let Some(p) = next {
        gui_api::write_bytes(
            &path,
            (serde_json::to_string_pretty(p)? + "\n").as_bytes(),
            None,
        )?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn write_palette(
    dir: &Path,
    package: &mut theme::Package,
    next: Option<palette::Palette>,
    keep_kde_scheme: bool,
) -> Result<()> {
    if let Some(previous) = package.palette.clone() {
        let saved = serde_json::to_value(&previous)?;
        refresh_palette_data(dir, palette::LIVE_PALETTE_REL, &saved, next.as_ref())?;
        if let Some(p) = &next {
            refresh_palette_data(dir, crate::neovim::DATA, &saved, Some(p))?;
            refresh_app_palettes(dir, package, &previous, p)?;
            if keep_kde_scheme {
                refresh_kde_palette(dir, package, &previous, p)?;
            }
        }
    }
    if next.is_none() {
        package.app_targets.clear();
    }
    package.palette = next;
    Ok(())
}

fn decoration_change(value: &Value, old: Option<&Decoration>) -> Result<Option<Decoration>> {
    if value.is_null() {
        return Ok(None);
    }
    let mut choice: Decoration =
        serde_json::from_value(value.clone()).context("Invalid window decoration selection")?;
    if Some(&choice) == old {
        return Ok(Some(choice));
    }
    if choice.disabled == Some(true) {
        ensure!(
            choice.library.is_none() && choice.theme.is_none(),
            "A disabled window decoration cannot select a theme"
        );
        return Ok(Some(choice));
    }
    let valid_id = |id: &str| {
        !id.is_empty()
            && id == id.trim()
            && !id.contains('/')
            && !id.starts_with('.')
            && !id.chars().any(char::is_control)
    };
    let library = choice
        .library
        .as_deref()
        .filter(|v| valid_id(v))
        .context("Choose a window decoration plugin")?;
    let label = choice
        .theme
        .as_deref()
        .filter(|v| valid_id(v))
        .context("Choose a window decoration theme")?;
    let compatible = match library {
        AURORAE => choice.aurorae_name().is_some_and(valid_id),
        "org.kde.breeze" => label == "Breeze",
        "org.kde.klassy" => label == "Klassy",
        "org.kde.oxygen" => label == "Oxygen",
        _ => choice.aurorae_name().is_none(),
    };
    ensure!(
        compatible,
        "The selected window decoration does not match its plugin"
    );
    choice.disabled = Some(false);
    Ok(Some(choice))
}

fn write_decoration(
    dir: &Path,
    manifest: &mut Value,
    package: &mut theme::Package,
    old: Option<&Decoration>,
    next: Option<&Decoration>,
) -> Result<()> {
    let from_aurorae = old.and_then(Decoration::aurorae_name);
    let to_aurorae = next.and_then(Decoration::aurorae_name);
    if from_aurorae != to_aurorae {
        let bundled = select_bundled_asset(dir, manifest, "aurorae", from_aurorae, to_aurorae);
        update_dependency_refs(manifest, "aurorae", from_aurorae, to_aurorae, bundled);
    }
    let from_library = old.and_then(|v| v.library.as_deref());
    let to_library = next.and_then(|v| v.library.as_deref());
    update_dependency_refs(manifest, "decoration", from_library, to_library, false);
    let values = next.map_or_else(|| Decoration::default().fields(), Decoration::fields);
    for (id, value) in DECORATION_FIELDS.into_iter().zip(values) {
        let f = field::field(id).context("Unknown window decoration field")?;
        patch_ini(dir, &f.file, &f.group, &f.key, value.as_ref())?;
        match value {
            Some(value) => {
                package.values.insert(id.into(), value);
            }
            None => {
                package.values.remove(id);
            }
        }
    }
    Ok(())
}

pub(crate) fn save(args: &Value) -> Result<Value> {
    let name = gui_api::str_arg(args, "name")?;
    let expected = gui_api::str_arg(args, "revision")?;
    let changes = args["changes"]
        .as_object()
        .context("Missing theme changes")?;
    let _lock = crate::operation::MutationGuard::acquire()?;
    let dir = paths::profile_dir(name)?;
    ensure!(
        revision(&dir)? == expected,
        "Theme changed on disk. Reopen it before saving."
    );
    let (mut manifest, mut package) = package(&dir)?;
    let edited_zen = zen_changes(&dir, &package, changes)?;
    let edited_obsidian = if [obsidian::MODE_FIELD, obsidian::THEME_FIELD]
        .iter()
        .any(|key| changes.contains_key(*key))
    {
        let (values, _) = obsidian::editor(
            &dir,
            &package.obsidian_overrides,
            obsidian_palette(&package),
        )?;
        let new = obsidian::edit(&package.obsidian_overrides, &values, changes)?;
        (new != package.obsidian_overrides).then_some(new)
    } else {
        None
    };
    let old_decoration = stored_decoration(&dir)?;
    let edited_decoration = changes
        .get(DECORATION)
        .map(|value| decoration_change(value, old_decoration.as_ref()))
        .transpose()?
        .filter(|value| *value != old_decoration);
    let edited_palette = changes
        .get(BASE16)
        .map(palette_change)
        .transpose()?
        .filter(|value| *value != package.palette);
    let edited_targets = changes
        .get(APP_TARGETS)
        .map(targets_change)
        .transpose()?
        .filter(|targets| *targets != package.app_targets);
    if let Some(targets) = &edited_targets {
        let palette = edited_palette.as_ref().unwrap_or(&package.palette);
        ensure!(
            targets.is_empty() || palette.is_some(),
            "Choose a Base16 palette first"
        );
    }
    let mut normalized = Vec::new();
    for (component, value) in changes {
        if matches!(
            component.as_str(),
            DECORATION
                | BASE16
                | APP_TARGETS
                | ZEN_ACCENT
                | ZEN_MODE
                | obsidian::MODE_FIELD
                | obsidian::THEME_FIELD
        ) {
            continue;
        }
        let Some((_, id)) = FIELDS.iter().find(|(edit, _)| edit == component) else {
            bail!("Field is not editable in a saved theme: {component}");
        };
        let f = field::field(id).context("Unknown theme field")?;
        let old = saved_value(&dir, f, &package)?;
        let new = if value.is_null() {
            None
        } else {
            match f.kind.as_str() {
                "int" | "float" => ensure!(
                    value.as_f64().is_some_and(|n| {
                        (f.kind == "float" || n.fract() == 0.0)
                            && f.min.is_none_or(|min| n >= min)
                            && f.max.is_none_or(|max| n <= max)
                    }),
                    "Invalid value for {}",
                    f.label
                ),
                "bool" => ensure!(value.is_boolean(), "Invalid value for {}", f.label),
                _ => {
                    let name = value
                        .as_str()
                        .with_context(|| format!("Invalid value for {}", f.label))?;
                    ensure!(
                        !name.trim().is_empty()
                            && name == name.trim()
                            && (f.kind != "font" || !name.contains(','))
                            && (f.kind == "font"
                                || theme::editor_selector_kind(id).is_some()
                                || (!name.starts_with('.') && !name.contains('/')))
                            && !name.chars().any(char::is_control),
                        "Invalid value for {}",
                        f.label
                    );
                    if f.id == "zed_mode" {
                        ensure!(
                            ZED_MODES.iter().any(|(mode, _)| mode == &name),
                            "Invalid Zed theme mode"
                        );
                    }
                }
            }
            if f.kind == "font" {
                Some(json!(field::qt_font_value(
                    old.as_ref().and_then(Value::as_str).unwrap_or_default(),
                    value.as_str().unwrap_or_default(),
                    None
                )))
            } else {
                Some(field::normalize_input(f, value).context("Invalid theme field value")?)
            }
        };
        if old != new {
            normalized.push((f, old, new));
        }
    }
    if normalized.is_empty()
        && edited_zen.is_empty()
        && edited_obsidian.is_none()
        && edited_decoration.is_none()
        && edited_palette.is_none()
        && edited_targets.is_none()
    {
        return editor_state(&dir, expected);
    }
    let source_tree = tree_revision(&dir)?;
    let temp = tempfile::Builder::new()
        .prefix(".edit-")
        .tempdir_in(paths::profiles_dir())?;
    let staged = temp.path().join("theme");
    crate::profile::copy_tree(&dir, &staged)?;
    if let Some(edited) = edited_palette {
        let keep_kde_scheme = !normalized.iter().any(|(f, _, _)| f.id == "color_scheme");
        write_palette(&staged, &mut package, edited, keep_kde_scheme)?;
        manifest["palette"] = json!(package.palette);
        manifest["app_targets"] = json!(package.app_targets);
    }
    if let Some(targets) = edited_targets {
        manifest["app_targets"] = json!(targets);
    }
    if let Some(edited) = edited_decoration {
        write_decoration(
            &staged,
            &mut manifest,
            &mut package,
            old_decoration.as_ref(),
            edited.as_ref(),
        )?;
    }
    if !edited_zen.is_empty() {
        patch_zen(&staged, &mut manifest, &mut package, &edited_zen)?;
    }
    if let Some(edited) = edited_obsidian {
        manifest["obsidian_overrides"] = json!(edited);
    }
    if normalized
        .iter()
        .any(|(f, _, _)| f.file == appearance::ZED_CONFIG)
    {
        patch_zed_settings(&staged, &mut manifest, &mut package, &normalized)?;
    }
    for (f, old, new) in normalized {
        if f.file == appearance::ZED_CONFIG {
            continue;
        }
        if theme::editor_selector_kind(&f.id).is_some() {
            update_editor_reference(
                &staged,
                &mut manifest,
                &mut package,
                &f.id,
                old.as_ref().and_then(Value::as_str),
                new.as_ref().and_then(Value::as_str),
            );
            patch_editor_settings(&staged, f, new.as_ref())?;
        } else if f.id == "btop_theme" {
            let quoted = new
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?
                .map(Value::String);
            patch_ini(&staged, &f.file, "", &f.key, quoted.as_ref())?;
        } else if f.id == "btop_rounded" {
            let boolean = new
                .as_ref()
                .and_then(Value::as_bool)
                .map(|value| json!(if value { "True" } else { "False" }));
            patch_ini(&staged, &f.file, "", &f.key, boolean.as_ref())?;
        } else {
            update_references(
                &staged,
                &mut manifest,
                &f.id,
                package.values.get(&f.id).and_then(Value::as_str),
                new.as_ref().and_then(Value::as_str),
            );
            if f.id == "color_scheme" {
                replace_color_scheme(
                    &staged,
                    &manifest,
                    &mut package,
                    new.as_ref().and_then(Value::as_str),
                )?;
            }
            if field::terminal_fields(&f.file).is_empty() {
                patch_ini(&staged, &f.file, &f.group, &f.key, new.as_ref())?;
            } else {
                patch_terminal(&staged, f, new.as_ref())?;
            }
            if let Some((_, key)) = field::GTK_MIRROR.iter().find(|(id, _)| *id == f.id) {
                for relative in field::GTK_SETTINGS {
                    patch_ini(&staged, relative, "Settings", key, new.as_ref())?;
                }
            }
            if f.id == "gtk_theme" {
                manifest
                    .as_object_mut()
                    .expect("parsed theme manifest")
                    .remove("gsettings_gtk_theme");
            }
        }
        match new {
            Some(value) => {
                package.values.insert(f.id.clone(), value);
            }
            None => {
                package.values.remove(&f.id);
            }
        }
    }
    manifest["values"] = json!(package.values);
    let path = staged.join(theme::MANIFEST);
    gui_api::write_bytes(
        &path,
        (serde_json::to_string_pretty(&manifest)? + "\n").as_bytes(),
        None,
    )?;
    let result = editor_state(&staged, &revision(&staged)?)?;
    ensure!(
        revision(&dir)? == expected && tree_revision(&dir)? == source_tree,
        "Theme changed on disk. Reopen it before saving."
    );
    use rustix::fs::{renameat_with, RenameFlags, CWD};
    renameat_with(CWD, &staged, CWD, &dir, RenameFlags::EXCHANGE)
        .context("Could not replace the saved theme atomically")?;
    Ok(result)
}
