//! Bundle metadata, legacy compatibility and validation for all adapters.
use crate::{
    field::{self, Base},
    paths,
};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs};
pub fn components() -> BTreeMap<String, Value> {
    serde_json::from_str(include_str!("../data/bundle_components.json"))
        .expect("valid component map")
}
pub fn metadata(name: &str) -> Result<Value> {
    let dir = paths::profile_dir(name)?;
    if !crate::theme::is_package(&dir) {
        let file = dir.join("theme-bundle.json");
        if file.is_file() {
            return serde_json::from_slice(&fs::read(file)?).context("Invalid bundle metadata");
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
    let name = crate::gui_api::str_arg(args, "name")?;
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
    let swatches: Vec<_> = [
        display("window_bg"),
        display("view_bg"),
        accent.clone(),
        display("text_color"),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect();
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
        json!({"accent":accent,"swatches":swatches,"scheme":display("color_scheme"),
        "lookAndFeel":display("look_and_feel"),"plasmaTheme":display("plasma_theme"),
        "kvantum":display("kvantum_theme"),"radius":display("klassy_radius"),
        "blur":display("blur_strength"),"blurEnabled":read("blur_enabled").as_bool().unwrap_or(false),
        "kitty":display("kitty_opacity"),"iconTheme":display("icon_theme"),
        "gtkTheme":display("gtk_theme"),"decoration":display("decoration_theme"),"sizeBytes":size,"fileCount":count}),
    )
}
