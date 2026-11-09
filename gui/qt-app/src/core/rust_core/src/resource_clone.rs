//! Derive editable local resources from installed bases and the live look.
use crate::{
    field::{self, Base},
    gui_api::write_bytes,
    paths,
};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{fs, path::PathBuf};
fn directories() -> Vec<PathBuf> {
    let mut out = vec![paths::data_home()];
    out.extend(
        std::env::var("XDG_DATA_DIRS")
            .unwrap_or_else(|_| "/usr/local/share:/usr/share".into())
            .split(':')
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from),
    );
    out
}
pub fn create(args: &Value) -> Result<Value> {
    let name = crate::gui_api::str_arg(args, "name")?;
    paths::validate_profile_name(name)?;
    let staged = crate::inspector_session::staged(args["session"].as_str())?;
    let value = |id: &str| -> Value {
        staged
            .get(id)
            .cloned()
            .or_else(|| field::field(id).and_then(|f| field::read(f, &Base::Live)))
            .unwrap_or(Value::Null)
    };
    let scheme = field::value_str(&value("color_scheme"));
    let kvantum = field::value_str(&value("kvantum_theme"));
    if scheme.is_empty() {
        bail!("No installed base color scheme");
    }
    if std::path::Path::new(&scheme).components().count() != 1 || scheme == "." || scheme == ".." {
        bail!("Invalid base color scheme");
    }
    let color_source = directories()
        .iter()
        .map(|root| root.join("color-schemes").join(format!("{scheme}.colors")))
        .find(|p| p.is_file())
        .with_context(|| format!("Base color scheme not found: {scheme}"))?;
    let color_target = paths::data_home()
        .join("color-schemes")
        .join(format!("{name}.colors"));
    if color_target.exists() {
        bail!("Color scheme already exists: {name}");
    }
    let kv_source = field::kvantum_file(&kvantum, &Base::Live)
        .with_context(|| format!("Base Kvantum theme not found: {kvantum}"))?;
    let root = paths::data_home().join("Kvantum").join(name);
    let kv_target = root.join(format!("{name}.kvconfig"));
    if kv_target.exists() {
        bail!("Kvantum theme already exists: {name}");
    }
    let mut color = fs::read_to_string(color_source)?;
    let live = fs::read_to_string(paths::live_path(".config/kdeglobals")).unwrap_or_default();
    if let Some(groups) = crate::config_edit::ini_parse(&live).as_object() {
        for (group, keys) in groups {
            if group.starts_with("Colors:") || group == "WM" {
                if let Some(keys) = keys.as_object() {
                    for (key, value) in keys {
                        color =
                            field::set_ini_text(&color, group, key, value.as_str().unwrap_or(""));
                    }
                }
            }
        }
    }
    color = field::set_ini_text(&color, "General", "Name", name);
    color = field::set_ini_text(&color, "General", "ColorScheme", name);
    let mut kv = fs::read_to_string(&kv_source)?;
    for id in [
        "kvantum_translucent",
        "kvantum_blurring",
        "kvantum_popup_blurring",
    ] {
        let v = value(id);
        if !v.is_null() {
            let f = field::field(id).context("missing Kvantum field")?;
            kv = field::set_ini_text(&kv, &f.group, &f.key, &field::value_str(&v));
        }
    }
    write_bytes(&color_target, color.as_bytes(), None)?;
    write_bytes(&kv_target, kv.as_bytes(), None)?;
    for entry in fs::read_dir(kv_source.parent().context("Kvantum parent")?)? {
        let path = entry?.path();
        if path.is_file()
            && matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("svg" | "png")
            )
        {
            let target = root.join(path.file_name().context("asset name")?);
            fs::copy(&path, &target)?;
            fs::set_permissions(target, fs::metadata(path)?.permissions())?;
        }
    }
    let assets = [
        color_target.to_string_lossy().to_string(),
        kv_target.to_string_lossy().to_string(),
    ];
    Ok(
        json!({"assets":assets,"message":format!("Created custom theme assets: {}",assets.join(", "))}),
    )
}
