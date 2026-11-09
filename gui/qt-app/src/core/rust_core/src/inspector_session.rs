//! Rust-owned staging, file snapshots, apply and Undo/Revert for Inspector.
use crate::{
    field::{self, Base},
    gui_api::{str_arg, write_bytes},
    paths,
};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

#[derive(Default, Serialize, Deserialize)]
struct State {
    #[serde(default)]
    revision: u64,
    staged: BTreeMap<String, Value>,
    configs: Vec<(String, String, String)>,
    order: Vec<(String, String)>,
    undo: Vec<Undo>,
    originals: BTreeMap<String, Snapshot>,
    #[serde(default)]
    apps: BTreeMap<String, String>,
    next: usize,
}
#[derive(Clone, Serialize, Deserialize)]
struct Snapshot {
    file: Option<String>,
    mode: u32,
}
#[derive(Serialize, Deserialize)]
struct Undo {
    field: Option<String>,
    label: String,
    path: String,
    old: Snapshot,
    raw: Value,
}
fn field_def(id: &str) -> Result<&'static field::FieldDef> {
    field::field(id).ok_or_else(|| anyhow!("Unknown field: {id}"))
}
fn snapshot(state: &mut State, dir: &Path, path: &Path) -> Result<Snapshot> {
    let mode = fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o7777)
        .unwrap_or(0o600);
    if !path.exists() {
        return Ok(Snapshot { file: None, mode });
    }
    if !path.is_file() {
        bail!("Not a regular config file: {}", path.display());
    }
    let name = format!("file-{}", state.next);
    state.next += 1;
    write_bytes(&dir.join(&name), &fs::read(path)?, Some(mode))?;
    Ok(Snapshot {
        file: Some(name),
        mode,
    })
}
fn original(state: &mut State, dir: &Path, path: &Path) -> Result<()> {
    let key = path.to_string_lossy().to_string();
    if state.originals.contains_key(&key) {
        return Ok(());
    }
    let snap = snapshot(state, dir, path)?;
    if let Some(file) = &snap.file {
        let home = paths::home_dir();
        let rel = path
            .strip_prefix(&home)
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| {
                Path::new("_external").join(path.strip_prefix("/").unwrap_or(path))
            });
        let backup_root = paths::backups_dir().join(dir.file_name().context("session directory")?);
        fs::create_dir_all(&backup_root)?;
        fs::set_permissions(&backup_root, fs::Permissions::from_mode(0o700))?;
        let backup = backup_root.join(rel);
        write_bytes(&backup, &fs::read(dir.join(file))?, Some(snap.mode))?;
    }
    state.originals.insert(key, snap);
    Ok(())
}
fn restore(dir: &Path, path: &Path, snapshot: &Snapshot) -> Result<()> {
    match &snapshot.file {
        Some(file) => write_bytes(path, &fs::read(dir.join(file))?, Some(snapshot.mode)),
        None => {
            if path.is_file() || path.is_symlink() {
                fs::remove_file(path)?;
            } else if path.exists() {
                bail!(
                    "Refusing to remove a non-file during restore: {}",
                    path.display()
                );
            }
            Ok(())
        }
    }
}
fn config_path(relative: &str) -> Result<PathBuf> {
    let p = Path::new(relative);
    if p.is_absolute()
        || p.components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        bail!("Invalid config path: {relative}");
    }
    let known = crate::tracked_apps::component_config_paths();
    if !known
        .iter()
        .flat_map(|(_, paths)| paths)
        .any(|root| p == Path::new(root) || p.starts_with(root))
    {
        bail!("Config path is not registered: {relative}");
    }
    Ok(paths::live_path(relative))
}
fn normalize(f: &field::FieldDef, value: &Value) -> Result<Value> {
    if f.kind == "font" && value.is_object() {
        let current = field::read(f, &Base::Live)
            .map(|v| field::value_str(&v))
            .unwrap_or_default();
        let family = value["family"].as_str().unwrap_or("").trim();
        let size = value["size"].as_f64().unwrap_or(10.0);
        if f.file.contains("gtk") {
            return Ok(json!(if size == 0.0 {
                family.into()
            } else {
                format!("{family} {size}")
            }));
        }
        let mut parts: Vec<String> = current.split(',').map(str::to_string).collect();
        if parts.len() < 2 {
            return Ok(json!(format!(
                "{family},{size},-1,5,50,0,0,0,0,0,0,0,0,0,0,1"
            )));
        }
        parts[0] = family.into();
        parts[1] = size.to_string();
        return Ok(json!(parts.join(",")));
    }
    field::normalize_input(f, value)
        .ok_or_else(|| anyhow!("Invalid value for {}: {value}", f.label))
}
enum Change {
    Field(String, Value),
    App(String, String),
    Wallpaper(String),
}
fn prepare_component(id: &str, value: &Value) -> Result<Change> {
    if id == "wallpaper" {
        return Ok(Change::Wallpaper(
            value
                .as_str()
                .context("Wallpaper path must be a string")?
                .into(),
        ));
    }
    if let Some(def) = crate::bundle_view::components().get(id) {
        let field = def["field"].as_str().context("Component is not writable")?;
        return Ok(Change::Field(
            field.into(),
            normalize(field_def(field)?, value)?,
        ));
    }
    if !crate::app_themes::definitions()
        .iter()
        .any(|v| v["id"] == id)
    {
        bail!("Unknown component: {id}");
    }
    let value = value
        .as_str()
        .context("Application theme must be a string")?;
    let meta = crate::app_themes::rpc("metadata", &json!({"id":id}))?;
    if meta["capabilities"]["apply_theme"] != true {
        bail!(
            "{} themes cannot be applied automatically",
            meta["name"].as_str().unwrap_or(id)
        );
    }
    if crate::app_themes::rpc("detect", &json!({"id":id}))? != json!(true) {
        bail!("{} is not installed", meta["name"].as_str().unwrap_or(id));
    }
    Ok(Change::App(id.into(), value.into()))
}
fn commit_change(state: &mut State, change: Change) -> Result<(bool, String)> {
    match change {
        Change::Field(id, value) => {
            if !state.staged.contains_key(&id) {
                state.order.push(("field".into(), id.clone()));
            }
            let label = field_def(&id)?.label.clone();
            state.staged.insert(id, value);
            Ok((true, format!("{label} staged — press Apply")))
        }
        Change::App(id, value) => {
            let result = crate::app_themes::rpc("apply", &json!({"id":id,"value":value}))?;
            let ok = result["ok"].as_bool().unwrap_or(false);
            if ok {
                state.apps.insert(id, value);
            }
            Ok((ok, result["message"].as_str().unwrap_or("").into()))
        }
        Change::Wallpaper(path) => {
            crate::wallpaper::apply_image(&path)?;
            state.apps.insert("wallpaper".into(), path);
            Ok((true, "Wallpaper set".into()))
        }
    }
}
fn config_token(relative: &str, key: &str) -> String {
    serde_json::to_string(&(relative, key)).expect("string pair serializes")
}

fn apply(state: &mut State, dir: &Path) -> Result<(bool, String)> {
    let app_count = state.apps.len();
    if state.staged.is_empty() && state.configs.is_empty() {
        if app_count > 0 {
            state.apps.clear();
            return Ok((true, format!("Applied {app_count} app theme(s)")));
        }
        return Ok((false, "Nothing to apply".into()));
    }
    let mut pending = BTreeMap::new();
    let mut messages = vec![];
    let mut applied = 0;
    for (id, value) in state.staged.clone() {
        let f = field_def(&id)?;
        let prepare = (|| {
            let path = if f.source == "kvantum" {
                state
                    .staged
                    .get("kvantum_theme")
                    .and_then(|value| field::kvantum_file(&field::value_str(value), &Base::Live))
                    .or_else(|| field::file_for(f, &Base::Live))
            } else {
                field::file_for(f, &Base::Live)
            }
            .context("No target file")?;
            original(state, dir, &path)?;
            if matches!(
                id.as_str(),
                "gtk_theme" | "gtk_dark" | "icon_theme" | "cursor_theme"
            ) {
                for rel in [
                    ".config/gtk-3.0/settings.ini",
                    ".config/gtk-4.0/settings.ini",
                ] {
                    original(state, dir, &paths::live_path(rel))?;
                }
            }
            let old = snapshot(state, dir, &path)?;
            Ok::<_, anyhow::Error>((
                path.clone(),
                old,
                if f.source == "kvantum" {
                    field::ini_get(&fs::read_to_string(&path)?, &f.group, &f.key)
                        .and_then(|value| field::coerce(f, json!(value)))
                        .unwrap_or(Value::Null)
                } else {
                    field::read(f, &Base::Live).unwrap_or(Value::Null)
                },
            ))
        })();
        match prepare {
            Ok(data) => {
                pending.insert(id, (value, data));
            }
            Err(e) => messages.push(format!("{}: {e:#}", f.label)),
        }
    }
    let values: Vec<(String, Value)> = state
        .order
        .iter()
        .filter(|(kind, _)| kind == "field")
        .filter_map(|(_, id)| pending.get(id).map(|(v, _)| (id.clone(), v.clone())))
        .collect();
    let report = field::apply(&values);
    let failed: std::collections::BTreeSet<_> = report
        .errors
        .iter()
        .filter_map(|error| error.split_once(':').map(|(id, _)| id.to_string()))
        .collect();
    messages.extend(report.errors);
    for id in report.applied {
        if let Some((_, (path, old, raw))) = pending.remove(&id) {
            state.undo.push(Undo {
                label: field_def(&id)?.label.clone(),
                field: Some(id.clone()),
                path: path.to_string_lossy().to_string(),
                old,
                raw,
            });
            if !failed.contains(&id) {
                state.staged.remove(&id);
            }
            applied += 1;
        }
    }
    for (relative, key, value) in state.configs.clone() {
        let result = (|| {
            let path = config_path(&relative)?;
            let (text, raw) = crate::config_edit::plan(&path, &key, &value)?;
            original(state, dir, &path)?;
            let old = snapshot(state, dir, &path)?;
            write_bytes(&path, text.as_bytes(), None)?;
            state.undo.push(Undo {
                field: None,
                label: format!("{relative} · {key}"),
                path: path.to_string_lossy().to_string(),
                old,
                raw,
            });
            Ok::<_, anyhow::Error>(())
        })();
        match result {
            Ok(()) => {
                state
                    .configs
                    .retain(|(r, k, _)| r != &relative || k != &key);
                applied += 1;
            }
            Err(e) => messages.push(format!("{relative} · {key}: {e:#}")),
        }
    }
    state.order.retain(|(kind, id)| {
        if kind == "field" {
            state.staged.contains_key(id)
        } else {
            state
                .configs
                .iter()
                .any(|(r, k, _)| config_token(r, k) == *id)
        }
    });
    let ok = messages.is_empty() && state.staged.is_empty() && state.configs.is_empty();
    let mut message = format!("Applied {applied} change(s)");
    if app_count > 0 {
        message.push_str(&format!(" + {app_count} app theme(s)"));
        state.apps.clear();
    }
    if !messages.is_empty() {
        message.push_str(&format!(" — {}", messages.join("; ")));
    }
    Ok((ok, message))
}
fn undo(state: &mut State, dir: &Path) -> Result<(bool, String)> {
    if let Some((kind, id)) = state.order.pop() {
        let label = if kind == "field" {
            state.staged.remove(&id);
            field_def(&id)?.label.clone()
        } else {
            let (r, k): (String, String) =
                serde_json::from_str(&id).context("invalid config staging token")?;
            state.configs.retain(|(rel, key, _)| rel != &r || key != &k);
            format!("{r} · {k}")
        };
        return Ok((true, format!("Unstaged: {label}")));
    }
    let Some(entry) = state.undo.last() else {
        return Ok((false, "Nothing to undo".into()));
    };
    let result = if let Some(id) = &entry.field {
        if !entry.raw.is_null() {
            let report = field::apply(&[(id.clone(), entry.raw.clone())]);
            if !report.errors.is_empty() {
                return Ok((false, format!("Undo failed: {}", report.errors.join("; "))));
            }
            Ok(())
        } else {
            restore(dir, Path::new(&entry.path), &entry.old)
        }
    } else {
        restore(dir, Path::new(&entry.path), &entry.old)
    };
    if let Err(e) = result {
        return Ok((false, format!("Undo failed: {e:#}")));
    }
    let entry = state.undo.pop().unwrap();
    Ok((true, format!("Undone: {}", entry.label)))
}
fn revert(state: &mut State, dir: &Path) -> Result<(bool, String)> {
    let discarded = state.staged.len() + state.configs.len();
    state.apps.clear();
    state.staged.clear();
    state.configs.clear();
    state.order.clear();
    if state.originals.is_empty() {
        return Ok((
            true,
            if discarded > 0 {
                format!("Discarded {discarded} staged change(s)")
            } else {
                "No changes in this session".into()
            },
        ));
    }
    let mut errors = vec![];
    let mut restored = 0;
    for (path, snapshot) in state.originals.clone() {
        let existed = Path::new(&path).exists() || Path::new(&path).is_symlink();
        match restore(dir, Path::new(&path), &snapshot) {
            Ok(()) => {
                restored += usize::from(snapshot.file.is_some() || existed);
                state.originals.remove(&path);
            }
            Err(e) => errors.push(format!("{path}: {e:#}")),
        }
    }
    if errors.is_empty() {
        let mut values = BTreeMap::new();
        for entry in &state.undo {
            if let Some(id) = &entry.field {
                let f = field_def(id)?;
                if !f.command.is_empty() || !f.hook.is_empty() {
                    if let Some(value) = field::read(f, &Base::Live) {
                        values.insert(id.clone(), value);
                    }
                }
            }
        }
        errors.extend(field::apply(&values.into_iter().collect::<Vec<_>>()).errors);
        state.undo.clear();
    }
    let mut messages = vec![];
    if discarded > 0 {
        messages.push(format!("Discarded {discarded} staged change(s)"));
    }
    messages.push(format!("Reverted {restored} file(s)"));
    messages.extend(errors.clone());
    Ok((errors.is_empty(), messages.join("; ")))
}
pub fn rpc(op: &str, args: &Value) -> Result<Value> {
    // Lock order: global appearance lock, then Inspector session lock.
    let _appearance_lock = if matches!(
        op,
        "apply" | "undo" | "revert" | "app" | "wallpaper" | "component" | "create_profile"
    ) {
        Some(crate::operation::MutationGuard::acquire()?)
    } else {
        None
    };
    if !matches!(
        op,
        "state"
            | "stage"
            | "component"
            | "app"
            | "wallpaper"
            | "config"
            | "apply"
            | "undo"
            | "revert"
            | "reset"
            | "create_profile"
            | "close"
    ) {
        bail!("unknown inspector operation {op}");
    }
    if op == "close" && args["session"].is_null() {
        return Ok(json!({"closed":false}));
    }
    let root = paths::cache_home().join("kde-theme-profile/inspector-sessions");
    fs::create_dir_all(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    let dir = if let Some(id) = args["session"].as_str() {
        paths::validate_profile_name(id)?;
        let path = root.join(id);
        if path.is_symlink() || !path.is_dir() {
            bail!("Unknown inspector session");
        }
        path
    } else {
        tempfile::Builder::new()
            .prefix("inspector-")
            .tempdir_in(&root)?
            .keep()
    };
    let _lock = crate::gui_api::lock_file(&dir.join("lock"))?;
    let path = dir.join("state.json");
    if args["session"].is_string() && !path.is_file() {
        bail!("Unknown inspector session");
    }
    if op == "close" {
        fs::remove_dir_all(&dir)?;
        return Ok(json!({"closed":true}));
    }

    let mut state: State = if path.exists() {
        serde_json::from_slice(&fs::read(&path)?)?
    } else {
        State::default()
    };
    let mut data = Value::Null;
    let outcome = (|| match op {
        "create_profile" => {
            let name = str_arg(args, "name")?;
            paths::validate_profile_name(name)?;
            paths::profile_dir(name)?;
            let components = args["components"].as_object().cloned().unwrap_or_default();
            let applications = args["applications"]
                .as_object()
                .cloned()
                .unwrap_or_default();
            let planned = components
                .iter()
                .chain(applications.iter())
                .map(|(id, value)| prepare_component(id, value))
                .collect::<Result<Vec<_>>>()?;
            for change in planned {
                let (ok, message) = commit_change(&mut state, change)?;
                if !ok {
                    return Ok((false, message));
                }
            }
            if !components.is_empty() {
                let (ok, message) = apply(&mut state, &dir)?;
                if !ok {
                    return Ok((false, message));
                }
            }
            let options = crate::theme::SaveOptions {
                description: Some(args["description"].as_str().unwrap_or("").into()),
                include_wallpaper: args["include_wallpaper"].as_bool().unwrap_or(true),
            };
            crate::theme::save(name, &options)?;
            crate::dependency_registry::refresh(None)?;
            data = crate::bundle_view::metadata(name)?;
            Ok((true, format!("Saved profile: {name}")))
        }
        "component" | "app" => commit_change(
            &mut state,
            prepare_component(str_arg(args, "id")?, &args["value"])?,
        ),
        "wallpaper" => commit_change(&mut state, prepare_component("wallpaper", &args["value"])?),
        "reset" => {
            let id = str_arg(args, "id")?;
            let f = field_def(id)?;
            let target = str_arg(args, "target")?;
            if target == "default" {
                bail!("This field has no portable default; pick a value instead");
            }
            if target != "profile" {
                bail!("Unknown reset target: {target}");
            }
            let active = crate::profile::get_active_profile()?;
            if active.is_empty() {
                bail!("No active profile");
            }
            let path = paths::profile_dir(&active)?;
            let value = if crate::theme::is_package(&path) {
                crate::theme::load(&path)?.values.get(id).cloned()
            } else {
                field::read_raw(f, &Base::Snapshot(path))
            }
            .with_context(|| format!("{} is not stored in the profile", f.label))?;
            let value = normalize(f, &value)?;
            if !state.staged.contains_key(id) {
                state.order.push(("field".into(), id.into()));
            }
            state.staged.insert(id.into(), value);
            Ok((true, format!("{} staged — press Apply", f.label)))
        }
        "stage" => {
            let id = str_arg(args, "id")?;
            let f = field_def(id)?;
            let value = normalize(f, &args["value"])?;
            if !state.staged.contains_key(id) {
                state.order.push(("field".into(), id.into()));
            }
            state.staged.insert(id.into(), value);
            Ok((true, format!("{} staged — press Apply", f.label)))
        }
        "config" => {
            let relative = str_arg(args, "path")?;
            let key = str_arg(args, "key")?;
            let value = str_arg(args, "value")?;
            crate::config_edit::plan(&config_path(relative)?, key, value)?;
            if let Some(item) = state
                .configs
                .iter_mut()
                .find(|(r, k, _)| r == relative && k == key)
            {
                item.2 = value.into();
            } else {
                state
                    .configs
                    .push((relative.into(), key.into(), value.into()));
                state
                    .order
                    .push(("config".into(), config_token(relative, key)));
            }
            Ok((true, format!("{key} staged — press Apply")))
        }
        "apply" => apply(&mut state, &dir),
        "undo" => undo(&mut state, &dir),
        "revert" => revert(&mut state, &dir),
        "state" => Ok((true, String::new())),
        _ => bail!("unknown inspector operation {op}"),
    })();
    let (ok, message) = outcome.unwrap_or_else(|e| (false, format!("{e:#}")));
    state.revision += 1;
    write_bytes(&path, &serde_json::to_vec(&state)?, Some(0o600))?;
    Ok(
        json!({"session":dir.file_name().unwrap().to_string_lossy(),"revision":state.revision,"data":data,"ok":ok,"message":message,"staged":state.staged,"apps":state.apps,"configs":state.configs,"undoDepth":state.undo.len()}),
    )
}

pub fn staged(session: Option<&str>) -> Result<BTreeMap<String, Value>> {
    let Some(id) = session else {
        return Ok(BTreeMap::new());
    };
    paths::validate_profile_name(id)?;
    let file = paths::cache_home()
        .join("kde-theme-profile/inspector-sessions")
        .join(id)
        .join("state.json");
    let state: State = serde_json::from_slice(&fs::read(file)?)?;
    Ok(state.staged)
}
