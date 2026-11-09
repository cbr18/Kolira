//! Read-only pre-apply report and content-bound confirmation.
use crate::{
    change_plan::{hash, Entry, Plan},
    field, paths, profile, theme,
};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

fn bytes(entry: &Entry) -> &[u8] {
    if let Entry::File { bytes, .. } = entry {
        bytes
    } else {
        &[]
    }
}
fn mode(entry: &Entry) -> Option<String> {
    match entry {
        Entry::File { mode, .. } | Entry::Directory { mode } => Some(format!("{mode:04o}")),
        _ => None,
    }
}
fn link(entry: &Entry) -> Option<String> {
    if let Entry::Symlink { target } = entry {
        Some(target.to_string_lossy().into())
    } else {
        None
    }
}
fn text_diff(path: &str, old: &[u8], new: &[u8]) -> Result<(String, bool)> {
    if old.contains(&0)
        || new.contains(&0)
        || std::str::from_utf8(old).is_err()
        || std::str::from_utf8(new).is_err()
    {
        return Ok((String::new(), true));
    }
    if old.len() + new.len() > 1024 * 1024 {
        return Ok((String::new(), true));
    }
    let mut a = tempfile::NamedTempFile::new()?;
    a.write_all(old)?;
    let mut b = tempfile::NamedTempFile::new()?;
    b.write_all(new)?;
    let diff = which::which("diff").or_else(|_| which::which("/usr/bin/diff"))?;
    let (_, output) = paths::output_bounded_for(
        Command::new(diff)
            .args([
                "-u",
                "--label",
                &format!("live/{path}"),
                "--label",
                &format!("theme/{path}"),
            ])
            .arg(a.path())
            .arg(b.path()),
        std::time::Duration::from_secs(3),
    )
    .context("Could not calculate text diff")?;
    let output = String::from_utf8_lossy(&output).to_string();
    // Keep Qt text layout bounded; all bytes still participate in the token.
    Ok((
        output.lines().take(1500).collect::<Vec<_>>().join("\n"),
        output.lines().count() > 1500,
    ))
}
fn label(path: &Path) -> String {
    path.strip_prefix(paths::home_dir())
        .map(|p| format!("~/{}", p.display()))
        .unwrap_or_else(|_| path.display().to_string())
}

pub fn build(args: &Value) -> Result<Value> {
    build_impl(args, true)
}
fn build_impl(args: &Value, include_text_diff: bool) -> Result<Value> {
    let _lock = crate::operation::MutationGuard::acquire()?;
    let name = crate::gui_api::str_arg(args, "name")?;
    let source = paths::profile_dir(name)?;
    if !source.is_dir() {
        bail!("Profile not found: {name}");
    }
    let mut selected = crate::gui_api::component_selection(&args["components"])?;
    let package = theme::is_package(&source)
        .then(|| theme::load(&source))
        .transpose()?;
    if package.is_some() && selected.is_some() {
        bail!("Theme packages apply everything they define");
    }
    if package.is_none() && args["use_settings"] == true && selected.is_none() {
        let settings = crate::gui_api::settings();
        let disabled: Vec<String> =
            serde_json::from_value(settings["disabled_components"].clone()).unwrap_or_default();
        let mut ids: Vec<String> = if settings["default_components_mode"] == "custom" {
            serde_json::from_value(settings["custom_components"].clone()).unwrap_or_default()
        } else {
            crate::custom_components::merged_component_defs()
                .into_iter()
                .map(|c| c.id)
                .collect()
        };
        ids.retain(|id| !disabled.contains(id));
        if ids.is_empty() {
            bail!("Select at least one component in Settings");
        }
        selected = Some(ids);
    }
    let selection = selected.clone();
    let mut rels = profile::resolve_cli_paths(selected)?;
    let mut plan = Plan::default();
    plan.track_tree(&source)?;
    plan.read(&paths::settings_file())?;
    plan.read(&paths::theme_root().join("components.json"))?;
    let mut missing = json!([]);
    let mut dependencies = json!([]);
    let mut kept = vec![];
    if let Some(package) = &package {
        for asset in package.assets.iter().filter(|a| a.kind != "wallpaper") {
            theme::validate_asset_restore(&source, &asset.path)?;
            if !rels.contains(&asset.path) {
                rels.push(asset.path.clone());
            }
        }
        let absent = theme::check(name)?;
        kept = absent
            .iter()
            .filter(|item| theme::KEPT_WHEN_MISSING.contains(&item.field.as_str()))
            .map(|item| item.field.clone())
            .collect();
        missing = serde_json::to_value(absent)?;
        dependencies = serde_json::to_value(theme::needed(name)?)?;
    }
    if package.is_none() {
        let absent: Vec<Value> = field::capture(&field::Base::Snapshot(source.clone()))
            .into_iter()
            .filter(|(id, value)| !theme::available(id, value))
            .filter(|(id, value)| {
                !(id == "kvantum_theme"
                    && source
                        .join(format!(".config/Kvantum/{}", field::value_str(value)))
                        .is_dir())
            })
            .map(|(id, value)| json!({"field":id,"name":field::value_str(&value),"source":""}))
            .collect();
        missing = json!(absent);
    }
    let keep_wallpaper = args["keep_wallpaper"].as_bool().unwrap_or(false)
        || package.as_ref().is_some_and(|p| p.wallpaper.is_none());
    let old_layout = plan.text(&paths::live_path(crate::wallpaper::APPLETSRC_REL))?;
    let kept_values = profile::live_values(&kept);
    let mut journal = crate::undo::plan_revert(&mut plan)?;
    profile::plan_restore(name, &source, &rels, &mut plan)?;
    profile::plan_post_restore(
        &source,
        &rels,
        keep_wallpaper,
        &kept_values,
        old_layout.as_deref(),
        &mut plan,
    )?;
    let mut journal_rels = rels.clone();
    journal_rels.push(".config/konsolerc".into());
    crate::undo::plan_record_prefs(&mut journal, &journal_rels, &mut plan)?;
    if package.is_none() && source.join("theme-bundle.json").is_file() {
        let data: Value = serde_json::from_slice(&fs::read(source.join("theme-bundle.json"))?)?;
        let defs = crate::bundle_view::components();
        let mut values: Vec<_> = data["components"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(id, value)| {
                defs.get(id)
                    .and_then(|d| d["field"].as_str())
                    .filter(|f| !f.is_empty())
                    .map(|f| (f.to_string(), value.clone()))
            })
            .collect();
        values.sort_by_key(|(id, _)| {
            field::APPLY_FIRST
                .iter()
                .position(|s| *s == id)
                .unwrap_or(field::APPLY_FIRST.len())
        });
        for (id, value) in values {
            field::plan_value(&id, &value, &mut plan)?;
        }
        for (id, value) in data["applications"].as_object().into_iter().flatten() {
            crate::app_themes::plan_apply(
                id,
                value.as_str().context("Application theme must be text")?,
                &mut plan,
            )?;
        }
    }
    let mut runtime_changes = vec![];
    let mut runtime_state = std::collections::BTreeMap::new();
    if !field::no_apply() {
        let mut wanted = journal.gsettings.clone();
        if rels.iter().any(|rel| rel == ".config/gtk-3.0/settings.ini") {
            let text = plan
                .text(&paths::live_path(".config/gtk-3.0/settings.ini"))?
                .unwrap_or_default();
            let gtk = package
                .as_ref()
                .and_then(|p| p.gsettings_gtk_theme.clone())
                .filter(|_| !kept.iter().any(|id| id == "gtk_theme"))
                .or_else(|| field::ini_get(&text, "Settings", "gtk-theme-name"));
            let dark = field::ini_get(&text, "Settings", "gtk-application-prefer-dark-theme");
            for (key, value) in field::gtk_desktop_keys(gtk.as_deref(), dark.as_deref()) {
                wanted.insert(key.into(), value);
            }
            for (key, ini) in [
                ("icon-theme", "gtk-icon-theme-name"),
                ("cursor-theme", "gtk-cursor-theme-name"),
            ] {
                if let Some(value) =
                    field::ini_get(&text, "Settings", ini).filter(|s| !s.is_empty())
                {
                    wanted.insert(key.into(), value);
                }
            }
        }
        for key in ["gtk-theme", "color-scheme", "icon-theme", "cursor-theme"] {
            let old = crate::undo::gsettings_get(key);
            runtime_state.insert(key, old.clone());
            if let Some(value) = wanted.get(key) {
                if old.as_deref() != Some(value.as_str()) {
                    runtime_changes.push(json!({"key":key,"before":old,"after":value}));
                }
            }
        }
    }
    let changes: Vec<_> = plan.changed().map(|(path, _)| path.clone()).collect();
    let actions = crate::actions::for_changes(&source, &changes);
    let source_digest = source_signature(&source)?;
    let mut files = vec![];
    for (path, new) in plan.changed() {
        if path == &paths::active_file() || matches!(new, Entry::Directory { .. }) {
            continue;
        }
        let old = plan.before.get(path).context("Original missing")?;
        if matches!(old, Entry::Directory { .. }) {
            continue;
        }
        let operation = if matches!(new, Entry::Missing) {
            "delete"
        } else if matches!(old, Entry::Missing) {
            "create"
        } else {
            "modify"
        };
        let display = label(path);
        let (diff, truncated) = if include_text_diff {
            text_diff(&display, bytes(old), bytes(new))?
        } else {
            (String::new(), false)
        };
        files.push(json!({"path":display,"operation":operation,"diff":diff,"truncated":truncated,"before_bytes":bytes(old).len(),"after_bytes":bytes(new).len(),"before_sha256":hash(bytes(old)),"after_sha256":hash(bytes(new)),"symlink":matches!(new,Entry::Symlink{..}),"before_mode":mode(old),"after_mode":mode(new),"before_link":link(old),"after_link":link(new),"reasons":plan.reasons.get(path).cloned().unwrap_or_default()}));
    }
    let builtin: Vec<_> = crate::tracked_apps::get_tracked_apps()
        .into_iter()
        .map(|a| a.id)
        .collect();
    let custom_components: Vec<_> = crate::custom_components::load_custom_components()?
        .into_iter()
        .filter(|app| {
            !builtin.contains(&app.id) && selection.as_ref().is_none_or(|ids| ids.contains(&app.id))
        })
        .map(|app| app.id)
        .collect();
    let fingerprints: Vec<Value> = files
        .iter()
        .cloned()
        .map(|mut file| {
            if let Some(object) = file.as_object_mut() {
                object.remove("diff");
                object.remove("truncated");
            }
            file
        })
        .collect();
    let canonical = json!({"name":name,"components":selection,"keep_wallpaper":args["keep_wallpaper"].as_bool().unwrap_or(false),"inputs":plan.inputs,"changes":fingerprints,"runtime_state":runtime_state,"runtime_changes":runtime_changes,"missing":missing,"dependencies":dependencies,"actions":actions,"kept":kept});
    let token = hash(&serde_json::to_vec(&canonical)?);
    Ok(
        json!({"name":name,"token":token,"source_digest":source_digest,"keep_wallpaper":args["keep_wallpaper"].as_bool().unwrap_or(false),"components":selection,"files":files,"file_count":files.len(),"missing":missing,"dependencies":dependencies,"actions":actions,"kept_fields":kept,"runtime_changes":runtime_changes,"restart_plasma":true,"preserves_personal_settings":true,"custom_components":custom_components}),
    )
}

pub fn check_confirmation(args: &Value) -> Result<()> {
    if let Some(expected) = args["preview_token"].as_str() {
        let current = build_impl(args, false)?;
        if current["token"] != expected {
            bail!("PREVIEW_STALE: theme or live settings changed; review the updated preview before applying");
        }
    }
    Ok(())
}

thread_local! {static CONFIRMATION:std::cell::RefCell<Option<Value>>=const{std::cell::RefCell::new(None)};}
struct ConfirmationReset(Option<Value>);
impl Drop for ConfirmationReset {
    fn drop(&mut self) {
        CONFIRMATION.with(|context| *context.borrow_mut() = self.0.take());
    }
}
pub fn confirmed<T>(args: &Value, work: impl FnOnce() -> Result<T>) -> Result<T> {
    if args["preview_token"].is_string() && args["install_missing"] == true {
        bail!("Install dependencies first, then refresh the preview before applying");
    }
    check_confirmation(args)?;
    let context = if args["preview_token"].is_string() {
        let mut context = args.clone();
        context["source_digest"] = build_impl(args, false)?["source_digest"].clone();
        Some(context)
    } else {
        None
    };
    let _reset = ConfirmationReset(CONFIRMATION.with(|old| old.replace(context)));
    work()
}
pub(crate) fn verify_before_write(prepared: &Path) -> Result<()> {
    let args = CONFIRMATION.with(|context| context.borrow().clone());
    if let Some(args) = args {
        check_confirmation(&args)?;
        if source_signature(prepared)? != args["source_digest"].as_str().unwrap_or("") {
            bail!("PREVIEW_STALE: prepared theme differs from the reviewed theme");
        }
    }
    Ok(())
}
fn source_signature(root: &Path) -> Result<String> {
    fn visit(
        root: &Path,
        path: &Path,
        values: &mut std::collections::BTreeMap<PathBuf, String>,
    ) -> Result<()> {
        let entry = crate::change_plan::entry(path)?;
        let value = match &entry {
            Entry::File { bytes, mode } => format!("file:{mode}:{}", hash(bytes)),
            _ => serde_json::to_string(&entry)?,
        };
        values.insert(path.strip_prefix(root)?.to_path_buf(), value);
        if matches!(entry, Entry::Directory { .. }) {
            for child in fs::read_dir(path)? {
                visit(root, &child?.path(), values)?;
            }
        }
        Ok(())
    }
    let mut values = std::collections::BTreeMap::new();
    visit(root, root, &mut values)?;
    Ok(hash(&serde_json::to_vec(&values)?))
}
