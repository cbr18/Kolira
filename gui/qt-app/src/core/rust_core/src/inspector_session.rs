//! Rust-owned staging, file snapshots, apply and Undo/Revert for Inspector.
use crate::{
    field::{self, Base},
    gui_api::{str_arg, write_bytes},
    operation, paths, wallpaper,
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

#[derive(Clone, Default, Serialize, Deserialize)]
struct State {
    #[serde(default)]
    revision: u64,
    staged: BTreeMap<String, Value>,
    #[serde(default)]
    field_baselines: BTreeMap<String, Option<Value>>,
    configs: Vec<(String, String, String)>,
    #[serde(default)]
    config_baselines: BTreeMap<String, (Value, Value)>,
    order: Vec<(String, String)>,
    undo: Vec<Undo>,
    originals: BTreeMap<String, Snapshot>,
    #[serde(default)]
    apps: BTreeMap<String, String>,
    #[serde(default)]
    app_drafts: BTreeMap<String, AppDraft>,
    #[serde(default)]
    wallpaper_draft: Option<WallpaperDraft>,
    next: usize,
}

impl State {
    fn has_draft(&self) -> bool {
        !self.staged.is_empty()
            || !self.configs.is_empty()
            || !self.app_drafts.is_empty()
            || self.wallpaper_draft.is_some()
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct WallpaperDraft {
    before: wallpaper::DesktopWallpapers,
    path: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct AppDraft {
    target: PathBuf,
    before: Value,
    after: Value,
    value: String,
}
#[derive(Clone, Serialize, Deserialize)]
struct Snapshot {
    file: Option<String>,
    mode: u32,
}
#[derive(Clone, Serialize, Deserialize)]
struct Undo {
    field: Option<String>,
    label: String,
    path: String,
    old: Snapshot,
    raw: Value,
    #[serde(default)]
    wallpaper: bool,
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
        return Ok(json!(field::qt_font_value(&current, family, Some(size))));
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
fn commit_change(state: &mut State, change: Change, stage: bool) -> Result<(bool, String)> {
    match change {
        Change::Field(id, value) => stage_field(state, &id, value),
        Change::App(id, value) if stage => stage_app(state, id, value),
        Change::App(id, value) => {
            let result = crate::app_themes::rpc("apply", &json!({"id":id,"value":value}))?;
            let ok = result["ok"].as_bool().unwrap_or(false);
            if ok {
                state.apps.insert(id, value);
            }
            Ok((ok, result["message"].as_str().unwrap_or("").into()))
        }
        Change::Wallpaper(path) if stage => stage_wallpaper(state, &path),
        Change::Wallpaper(path) => {
            crate::wallpaper::apply_image(&path)?;
            state.apps.insert("wallpaper".into(), path);
            Ok((true, "Wallpaper set".into()))
        }
    }
}

fn stage_wallpaper(state: &mut State, path: &str) -> Result<(bool, String)> {
    let path = wallpaper::draft_image(path)?;
    let current = wallpaper::observe_draft()?;
    let before = state
        .wallpaper_draft
        .as_ref()
        .map(|draft| &draft.before)
        .unwrap_or(&current);
    if wallpaper::matches_image(&current, &path) || wallpaper::matches_image(before, &path) {
        unstage(state, "wallpaper", "wallpaper")?;
        return Ok((true, "Unstaged: Wallpaper".into()));
    }
    if state.wallpaper_draft.is_none() {
        state.order.push(("wallpaper".into(), "wallpaper".into()));
    }
    state.wallpaper_draft = Some(WallpaperDraft {
        before: before.clone(),
        path,
    });
    Ok((true, "Wallpaper staged — press Apply".into()))
}

fn stage_app(state: &mut State, id: String, value: String) -> Result<(bool, String)> {
    use crate::app_themes;
    app_themes::validate_draft_theme(&id, &value)?;
    let (target, observed, after) = app_themes::observe_draft(&id, &value)?;
    let label = app_themes::theme_label(&id)?;
    let before = state
        .app_drafts
        .get(&id)
        .filter(|draft| draft.target == target)
        .map_or(observed, |draft| draft.before.clone());
    if before == after {
        unstage(state, "app", &id)?;
        return Ok((true, format!("Unstaged: {label}")));
    }
    if !state.app_drafts.contains_key(&id) {
        state.order.push(("app".into(), id.clone()));
    }
    state.app_drafts.insert(
        id,
        AppDraft {
            target,
            before,
            after,
            value,
        },
    );
    Ok((true, format!("{label} staged — press Apply")))
}

fn stage_field(state: &mut State, id: &str, value: Value) -> Result<(bool, String)> {
    let definition = field_def(id)?;
    if !state.field_baselines.contains_key(id) {
        if let Ok(baseline) = observe_draft_field(state, definition) {
            state.field_baselines.insert(id.into(), baseline);
        }
    }
    if state.field_baselines.get(id).and_then(Option::as_ref) == Some(&value) {
        unstage(state, "field", id)?;
        return Ok((true, format!("Unstaged: {}", definition.label)));
    }
    if !state.staged.contains_key(id) {
        state.order.push(("field".into(), id.into()));
    }
    if id == "kvantum_theme" && state.staged.get(id) != Some(&value) {
        invalidate_kvantum_baselines(state);
    }
    state.staged.insert(id.into(), value);
    Ok((true, format!("{} staged — press Apply", definition.label)))
}

fn observe_draft_field(state: &State, definition: &field::FieldDef) -> Result<Option<Value>> {
    if definition.source == "kvantum" {
        if let Some(selected) = state.staged.get("kvantum_theme") {
            let path = field::kvantum_file(&field::value_str(selected), &Base::Live);
            return field::observe_file(definition, path.as_deref());
        }
    }
    field::observe_live(definition)
}

fn invalidate_kvantum_baselines(state: &mut State) {
    state
        .field_baselines
        .retain(|id, _| field::field(id).is_none_or(|definition| definition.source != "kvantum"));
}

fn config_token(relative: &str, key: &str) -> String {
    serde_json::to_string(&(relative, key)).expect("string pair serializes")
}

fn unstage(state: &mut State, kind: &str, id: &str) -> Result<()> {
    if kind == "field" {
        let removed = state.staged.remove(id);
        state.field_baselines.remove(id);
        if id == "kvantum_theme"
            && removed.is_some()
            && field::observe_live(field_def(id)?).ok().flatten() != removed
        {
            invalidate_kvantum_baselines(state);
        }
    } else if kind == "app" {
        state.app_drafts.remove(id);
    } else if kind == "wallpaper" {
        state.wallpaper_draft = None;
    } else {
        let (relative, key): (String, String) = serde_json::from_str(id)?;
        state
            .configs
            .retain(|(r, k, _)| r != &relative || k != &key);
        state.config_baselines.remove(id);
    }
    state.order.retain(|(k, key)| k != kind || key != id);
    Ok(())
}

fn stage_config(
    state: &mut State,
    relative: &str,
    key: &str,
    value: &str,
) -> Result<(bool, String)> {
    let id = config_token(relative, key);
    let (current, proposed) =
        crate::config_edit::observe_edit(&config_path(relative)?, key, value)?;
    let proposed = proposed?;
    let baseline = state
        .config_baselines
        .entry(id.clone())
        .or_insert((current, proposed.clone()));
    if proposed == baseline.0 {
        unstage(state, "config", &id)?;
        return Ok((true, format!("Unstaged: {key}")));
    }
    baseline.1 = proposed;
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
        state.order.push(("config".into(), id));
    }
    Ok((true, format!("{key} staged — press Apply")))
}

#[derive(Serialize)]
struct DraftConflict {
    kind: String,
    id: String,
    label: String,
    source: Option<String>,
    before: Value,
    current: Value,
    after: Value,
    can_keep: bool,
}

impl DraftConflict {
    fn token(&self) -> String {
        use sha2::{Digest, Sha256};
        let bytes = serde_json::to_vec(self).expect("draft conflict serializes");
        format!("{:x}", Sha256::digest(bytes))
    }
}

fn conflict_report(conflicts: &[DraftConflict]) -> Value {
    json!({"conflicts": conflicts.iter().map(|entry| {
        let mut row = json!(entry);
        row["token"] = json!(entry.token());
        row
    }).collect::<Vec<_>>()})
}

fn refresh_draft(state: &mut State) -> Result<Vec<DraftConflict>> {
    let mut converged = Vec::new();
    let mut conflicts = Vec::new();
    let mut field_baselines = Vec::new();
    let mut config_baselines = Vec::new();
    for (kind, id) in &state.order {
        if kind == "wallpaper" {
            if let Some(draft) = &state.wallpaper_draft {
                let current = wallpaper::observe_draft()?;
                if wallpaper::matches_image(&current, &draft.path) {
                    converged.push((kind.clone(), id.clone()));
                } else if !wallpaper::same_wallpapers(&current, &draft.before) {
                    conflicts.push(DraftConflict {
                        kind: kind.clone(), id: id.clone(), label: "Wallpaper".into(), source: None,
                        before: json!({"wallpapers": draft.before, "display":wallpaper::describe_wallpapers(&draft.before)}),
                        current: json!({"wallpapers":current, "display":wallpaper::describe_wallpapers(&current)}),
                        after: json!(draft.path), can_keep: true,
                    });
                }
            }
            continue;
        }
        let (label, source, before, current, after, can_keep) = if kind == "field" {
            let definition = field_def(id)?;
            let current = observe_draft_field(state, definition)
                .with_context(|| format!("Could not read Live: {}", definition.label))?;
            let Some(proposed) = state.staged.get(id) else {
                continue;
            };
            if !state.field_baselines.contains_key(id) {
                field_baselines.push((id.clone(), current.clone()));
            }
            (
                definition.label.clone(),
                None,
                state
                    .field_baselines
                    .get(id)
                    .cloned()
                    .unwrap_or_else(|| current.clone())
                    .unwrap_or(Value::Null),
                current.unwrap_or(Value::Null),
                proposed.clone(),
                true,
            )
        } else if kind == "app" {
            let Some(draft) = state.app_drafts.get(id) else {
                continue;
            };
            let label = crate::app_themes::theme_label(id)?.to_owned();
            let (target, current, _) = crate::app_themes::observe_draft(id, &draft.value)
                .with_context(|| format!("Could not read Live: {label}"))?;
            let source = Some(target.to_string_lossy().into_owned());
            if target != draft.target {
                conflicts.push(DraftConflict {
                    kind: kind.clone(),
                    id: id.clone(),
                    label,
                    source,
                    before: draft.before.clone(),
                    current,
                    after: draft.after.clone(),
                    can_keep: false,
                });
                continue;
            }
            (
                label,
                source,
                draft.before.clone(),
                current,
                draft.after.clone(),
                true,
            )
        } else {
            let (relative, key): (String, String) = serde_json::from_str(id)?;
            let Some((_, _, value)) = state
                .configs
                .iter()
                .find(|(r, k, _)| r == &relative && k == &key)
            else {
                continue;
            };
            let (current, proposed) =
                crate::config_edit::observe_edit(&config_path(&relative)?, &key, value)
                    .with_context(|| format!("Could not read Live: {relative} · {key}"))?;
            let (before, after) = if let Some(baseline) = state.config_baselines.get(id) {
                baseline.clone()
            } else {
                let proposed = proposed.as_ref().map_err(|error| anyhow!("{error:#}"))?;
                let baseline = (current.clone(), proposed.clone());
                config_baselines.push((id.clone(), baseline.clone()));
                baseline
            };
            (
                key,
                Some(relative),
                before,
                current,
                after.clone(),
                proposed.as_ref().is_ok_and(|value| value == &after),
            )
        };
        if current == after {
            converged.push((kind.clone(), id.clone()));
        } else if current != before || !can_keep {
            conflicts.push(DraftConflict {
                kind: kind.clone(),
                id: id.clone(),
                label,
                source,
                before,
                current,
                after,
                can_keep,
            });
        }
    }
    state.field_baselines.extend(field_baselines);
    state.config_baselines.extend(config_baselines);
    for (kind, id) in converged {
        unstage(state, &kind, &id)?;
    }
    Ok(conflicts)
}

fn resolve_draft(state: &mut State, args: &Value) -> Result<Vec<DraftConflict>> {
    let choice = str_arg(args, "choice")?;
    anyhow::ensure!(
        matches!(choice, "live" | "draft"),
        "Unknown draft resolution"
    );
    let conflicts = refresh_draft(state)?;
    let reviewed = str_arg(args, "token")?;
    let selected = conflicts.iter().find(|entry| entry.token() == reviewed);
    if let Some(entry) = selected {
        if choice == "live" {
            unstage(state, &entry.kind, &entry.id)?;
        } else if entry.can_keep {
            if entry.kind == "field" {
                let current = (!entry.current.is_null()).then(|| entry.current.clone());
                state.field_baselines.insert(entry.id.clone(), current);
            } else if entry.kind == "app" {
                if let Some(draft) = state.app_drafts.get_mut(&entry.id) {
                    draft.before = entry.current.clone();
                }
            } else if entry.kind == "wallpaper" {
                if let Some(draft) = state.wallpaper_draft.as_mut() {
                    draft.before = serde_json::from_value(entry.current["wallpapers"].clone())?;
                }
            } else {
                state.config_baselines.insert(
                    entry.id.clone(),
                    (entry.current.clone(), entry.after.clone()),
                );
            }
        }
    }
    refresh_draft(state)
}

fn draft_target(state: &State, f: &field::FieldDef) -> Option<PathBuf> {
    if f.source == "kvantum" {
        if let Some(selected) = state.staged.get("kvantum_theme") {
            return field::kvantum_file(&field::value_str(selected), &Base::Live);
        }
    }
    field::file_for(f, &Base::Live)
}

fn draft_footprint(state: &State) -> Result<Vec<PathBuf>> {
    let mut roots = Vec::new();
    for id in state.staged.keys() {
        let f = field_def(id)?;
        roots.push(
            draft_target(state, f).with_context(|| format!("No target file for {}", f.label))?,
        );
        if field::GTK_MIRROR.iter().any(|(mirrored, _)| mirrored == id) {
            roots.extend(field::GTK_SETTINGS.iter().copied().map(paths::live_path));
        }
        if !field::no_apply() {
            if f.source == "kvantum" {
                roots.push(paths::live_path(".config/Kvantum/kvantum.kvconfig"));
            }
            if id == "look_and_feel" {
                // Plasma writes the overrides and its kdedefaults layer.
                // See plasma-workspace/libklookandfeel/klookandfeelmanager.cpp.
                roots.extend(
                    [
                        ".config/kcminputrc",
                        ".config/kwinrc",
                        ".config/plasmarc",
                        ".config/plasmashellrc",
                        ".config/ksplashrc",
                        ".config/kglobalshortcutsrc",
                        ".config/plasma-org.kde.plasma.desktop-appletsrc",
                        ".config/kdedefaults/package",
                        ".config/kdedefaults/kdeglobals",
                        ".config/kdedefaults/kcminputrc",
                        ".config/kdedefaults/kwinrc",
                        ".config/kdedefaults/plasmarc",
                        ".config/kdedefaults/ksplashrc",
                    ]
                    .map(paths::live_path),
                );
            }
        }
    }
    for (relative, _, _) in &state.configs {
        roots.push(config_path(relative)?);
    }
    let (_, app_paths) = plan_app_drafts(state)?;
    roots.extend(app_paths.into_iter().map(|(_, path)| path));
    if let Some(draft) = &state.wallpaper_draft {
        wallpaper::draft_image(&draft.path)?;
        let path = wallpaper::draft_layout_path()?;
        wallpaper::wait_for_wallpaper_config(&path, &draft.before)?;
        roots.extend([path, paths::live_path(".config/kdeglobals")]);
    }
    Ok(roots)
}

fn plan_app_drafts(state: &State) -> Result<(crate::change_plan::Plan, Vec<(String, PathBuf)>)> {
    let mut plan = crate::change_plan::Plan::default();
    let mut paths = Vec::new();
    for (id, draft) in &state.app_drafts {
        crate::app_themes::validate_draft_theme(id, &draft.value)?;
        let target = crate::app_themes::plan_apply(id, &draft.value, &mut plan)?;
        let label = crate::app_themes::theme_label(id)?;
        anyhow::ensure!(
            target == draft.target,
            "Application config changed: {label}"
        );
        paths.push((label.to_owned(), target));
    }
    Ok((plan, paths))
}

fn apply_app_drafts(state: &mut State, dir: &Path) -> Result<usize> {
    let (plan, paths) = plan_app_drafts(state)?;
    let mut undo = Vec::new();
    for (label, path) in paths {
        original(state, dir, &path)?;
        undo.push(Undo {
            label,
            field: None,
            path: path.to_string_lossy().into_owned(),
            old: snapshot(state, dir, &path)?,
            raw: Value::Null,
            wallpaper: false,
        });
    }
    plan.commit()?;
    state.undo.extend(undo);
    let applied = state.app_drafts.len();
    state.app_drafts.clear();
    Ok(applied)
}

fn apply_transaction(state: &mut State, dir: &Path, source: &str) -> Result<(bool, String)> {
    if !state.has_draft() {
        return apply(state, dir);
    }
    // Keep the staged changes and prior undo history if the transaction fails.
    let mut draft = state.clone();
    let message = operation::with_source(source, || {
        operation::run("", || {
            operation::note_report(json!({"kind":"live"}))?;
            let id = operation::current_id().context("No Live operation")?;
            let broad = draft.staged.contains_key("look_and_feel");
            let settings = [
                ("gtk_theme", "gtk-theme"),
                ("gtk_dark", "color-scheme"),
                ("icon_theme", "icon-theme"),
                ("cursor_theme", "cursor-theme"),
            ]
            .into_iter()
            .filter_map(|(field, key)| (broad || draft.staged.contains_key(field)).then_some(key));
            operation::prepare_live(
                &paths::backups_dir().join(id),
                draft_footprint(&draft)?,
                settings,
            )?;
            let (ok, message) = apply(&mut draft, dir)?;
            anyhow::ensure!(ok, "{message}");
            Ok(message)
        })
    })?;
    *state = draft;
    Ok((true, message))
}

fn apply_wallpaper_draft(state: &mut State, dir: &Path, image: &str) -> Result<()> {
    let image = wallpaper::draft_image(image)?;
    let path = wallpaper::draft_layout_path()?;
    let current = wallpaper::observe_draft()?;
    wallpaper::wait_for_wallpaper_config(&path, &current)?;
    original(state, dir, &path)?;
    let old = snapshot(state, dir, &path)?;
    operation::note_report(json!({"kind":"live","wallpaper":true}))?;
    wallpaper::apply_draft_image(&path, &image, &current)?;
    state.undo.push(Undo {
        label: "Wallpaper".into(),
        field: None,
        path: path.to_string_lossy().into_owned(),
        old,
        raw: Value::Null,
        wallpaper: true,
    });
    state.wallpaper_draft = None;
    Ok(())
}

fn apply(state: &mut State, dir: &Path) -> Result<(bool, String)> {
    let app_count = state.apps.len();
    if !state.has_draft() {
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
            let path = draft_target(state, f).context("No target file")?;
            original(state, dir, &path)?;
            if field::GTK_MIRROR
                .iter()
                .any(|(mirrored, _)| *mirrored == id)
            {
                for rel in field::GTK_SETTINGS {
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
                wallpaper: false,
            });
            if !failed.contains(&id) {
                state.staged.remove(&id);
                state.field_baselines.remove(&id);
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
                wallpaper: false,
            });
            Ok::<_, anyhow::Error>(())
        })();
        match result {
            Ok(()) => {
                state
                    .config_baselines
                    .remove(&config_token(&relative, &key));
                state
                    .configs
                    .retain(|(r, k, _)| r != &relative || k != &key);
                applied += 1;
            }
            Err(e) => messages.push(format!("{relative} · {key}: {e:#}")),
        }
    }
    if messages.is_empty() && !state.app_drafts.is_empty() {
        applied += apply_app_drafts(state, dir)?;
    }
    if messages.is_empty() {
        if let Some(path) = state
            .wallpaper_draft
            .as_ref()
            .map(|draft| draft.path.clone())
        {
            apply_wallpaper_draft(state, dir, &path)?;
            applied += 1;
        }
    }
    state.order.retain(|(kind, id)| {
        if kind == "field" {
            state.staged.contains_key(id)
        } else if kind == "app" {
            state.app_drafts.contains_key(id)
        } else if kind == "wallpaper" {
            state.wallpaper_draft.is_some()
        } else {
            state
                .configs
                .iter()
                .any(|(r, k, _)| config_token(r, k) == *id)
        }
    });
    let ok = messages.is_empty() && !state.has_draft();
    let mut message = format!("Applied {applied} change(s)");
    if app_count > 0 {
        message.push_str(&format!(" + {app_count} app theme(s)"));
        state.apps.clear();
    }
    Ok((
        ok,
        if messages.is_empty() {
            message
        } else {
            messages.join("; ")
        },
    ))
}
fn undo(state: &mut State, dir: &Path) -> Result<(bool, String)> {
    if let Some((kind, id)) = state.order.pop() {
        let label = if kind == "field" {
            unstage(state, "field", &id)?;
            field_def(&id)?.label.clone()
        } else if kind == "app" {
            unstage(state, "app", &id)?;
            crate::app_themes::theme_label(&id)?.to_owned()
        } else if kind == "wallpaper" {
            unstage(state, "wallpaper", &id)?;
            "Wallpaper".into()
        } else {
            let (r, k): (String, String) =
                serde_json::from_str(&id).context("invalid config staging token")?;
            state.configs.retain(|(rel, key, _)| rel != &r || key != &k);
            state.config_baselines.remove(&id);
            format!("{r} · {k}")
        };
        return Ok((true, format!("Unstaged: {label}")));
    }
    let Some(entry) = state.undo.last() else {
        return Ok((false, "Nothing to undo".into()));
    };
    let result = if entry.wallpaper {
        restore_wallpaper_entry(dir, Path::new(&entry.path), &entry.old)
    } else if let Some(id) = &entry.field {
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

fn restore_wallpaper_entry(dir: &Path, path: &Path, snapshot: &Snapshot) -> Result<()> {
    let before = fs::read_to_string(
        dir.join(
            snapshot
                .file
                .as_ref()
                .context("Missing wallpaper snapshot")?,
        ),
    )?;
    let live = wallpaper::observe_draft()?;
    wallpaper::wait_for_wallpaper_config(path, &live)?;
    let current = fs::read_to_string(path)?;
    let restored = crate::layout::restore_wallpaper_selection(&current, &before);
    if restored == current {
        return Ok(());
    }
    write_bytes(path, restored.as_bytes(), None)?;
    let result = wallpaper::refresh_after_restore();
    if let Err(error) = result {
        write_bytes(path, current.as_bytes(), None)?;
        wallpaper::refresh_after_restore().context("Could not return to the current wallpaper")?;
        return Err(error);
    }
    Ok(())
}

fn discard(state: &mut State) -> usize {
    let discarded = state.staged.len()
        + state.configs.len()
        + state.app_drafts.len()
        + usize::from(state.wallpaper_draft.is_some());
    state.staged.clear();
    state.field_baselines.clear();
    state.configs.clear();
    state.config_baselines.clear();
    state.app_drafts.clear();
    state.wallpaper_draft = None;
    state.order.clear();
    discarded
}

fn revert(state: &mut State, dir: &Path) -> Result<(bool, String)> {
    let discarded = discard(state);
    state.apps.clear();
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
        let path_name = Path::new(&path);
        let result =
            if path_name == paths::live_path(wallpaper::APPLETSRC_REL) && snapshot.file.is_some() {
                restore_wallpaper_entry(dir, path_name, &snapshot)
            } else {
                restore(dir, path_name, &snapshot)
            };
        match result {
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
        "apply"
            | "undo"
            | "revert"
            | "app"
            | "wallpaper"
            | "component"
            | "create_profile"
            | "refresh"
            | "resolve"
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
            | "discard"
            | "refresh"
            | "resolve"
            | "create_profile"
            | "close"
    ) {
        bail!("unknown inspector operation {op}");
    }
    if op == "close" && args["session"].is_null() {
        return Ok(json!({"closed":false}));
    }
    let root = paths::cache_home().join("kolira/inspector-sessions");
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
                let (ok, message) = commit_change(&mut state, change, false)?;
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
            args["stage"] == true,
        ),
        "wallpaper" => commit_change(
            &mut state,
            prepare_component("wallpaper", &args["value"])?,
            args["stage"] == true,
        ),
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
            stage_field(&mut state, id, normalize(f, &value)?)
        }
        "stage" => {
            let id = str_arg(args, "id")?;
            let f = field_def(id)?;
            stage_field(&mut state, id, normalize(f, &args["value"])?)
        }
        "config" => stage_config(
            &mut state,
            str_arg(args, "path")?,
            str_arg(args, "key")?,
            str_arg(args, "value")?,
        ),
        "refresh" | "resolve" => {
            let conflicts = if op == "refresh" {
                refresh_draft(&mut state)?
            } else {
                resolve_draft(&mut state, args)?
            };
            data = conflict_report(&conflicts);
            Ok((true, String::new()))
        }
        "apply" => {
            if args["check_live"] != true
                && state.app_drafts.is_empty()
                && state.wallpaper_draft.is_none()
            {
                // Older callers expect to retry only the writes that failed.
                return apply(&mut state, &dir);
            }
            let had_draft = state.has_draft();
            let conflicts = refresh_draft(&mut state)?;
            if !conflicts.is_empty() {
                data = conflict_report(&conflicts);
                return Ok((false, String::new()));
            }
            if had_draft && !state.has_draft() && state.apps.is_empty() {
                return Ok((true, "Live already matches the draft".into()));
            }
            apply_transaction(&mut state, &dir, args["source"].as_str().unwrap_or("cli"))
        }
        "undo" => undo(&mut state, &dir),
        "revert" => revert(&mut state, &dir),
        "discard" => {
            let count = if let Some(section) = args.get("section") {
                let section = section.as_str().context("Invalid draft category")?;
                anyhow::ensure!(
                    section == "wallpaper"
                        || field::registry().iter().any(|f| f.section == section),
                    "Unknown draft category: {section}"
                );
                let ids: Vec<_> = state
                    .staged
                    .keys()
                    .filter(|id| field::field(id).is_some_and(|f| f.section == section))
                    .cloned()
                    .collect();
                for id in &ids {
                    unstage(&mut state, "field", id)?;
                }
                let mut count = ids.len();
                if section == "apps" {
                    let ids: Vec<_> = state.app_drafts.keys().cloned().collect();
                    for id in &ids {
                        unstage(&mut state, "app", id)?;
                    }
                    count += ids.len();
                }
                if section == "wallpaper" && state.wallpaper_draft.is_some() {
                    unstage(&mut state, "wallpaper", "wallpaper")?;
                    count += 1;
                }
                count
            } else {
                discard(&mut state)
            };
            Ok((true, format!("Discarded {count} staged change(s)")))
        }
        "state" => Ok((true, String::new())),
        _ => bail!("unknown inspector operation {op}"),
    })();
    let (ok, message) = outcome.unwrap_or_else(|e| (false, format!("{e:#}")));
    state.revision += 1;
    write_bytes(&path, &serde_json::to_vec(&state)?, Some(0o600))?;
    let draft: Vec<_> = state
        .order
        .iter()
        .filter_map(|(kind, id)| {
            if kind == "field" {
                let field = field::field(id)?;
                Some(
                    json!({"label":field.label,"section":field.section,"before":state.field_baselines.get(id),
                "after":state.staged.get(id)?}),
                )
            } else if kind == "app" {
                let draft = state.app_drafts.get(id)?;
                let before = draft.before.get("theme").unwrap_or(&draft.before);
                let after = draft.after.get("theme").unwrap_or(&draft.after);
                Some(json!({"label":crate::app_themes::theme_label(id).ok()?,"section":"apps","before":before,"after":after}))
            } else if kind == "wallpaper" {
                let draft = state.wallpaper_draft.as_ref()?;
                Some(json!({"label":"Wallpaper","section":"wallpaper","before":wallpaper::describe_wallpapers(&draft.before),"after":Path::new(&draft.path).file_name()?.to_str()?}))
            } else {
                let (relative, key): (String, String) = serde_json::from_str(id).ok()?;
                let (_, _, value) = state
                    .configs
                    .iter()
                    .find(|(r, k, _)| r == &relative && k == &key)?;
                Some(json!({"label":key,"source":relative,"after":value}))
            }
        })
        .collect();
    let app_draft_count = state.app_drafts.len();
    let mut apps = state.apps;
    apps.extend(
        state
            .app_drafts
            .into_iter()
            .map(|(id, draft)| (id, draft.value)),
    );
    Ok(
        json!({"session":dir.file_name().unwrap().to_string_lossy(),"revision":state.revision,"data":data,"ok":ok,"message":message,"staged":state.staged,"apps":apps,"appDraftCount":app_draft_count,"wallpaperDraft":state.wallpaper_draft.as_ref().map(|draft| &draft.path),"configs":state.configs,"draft":draft,"undoDepth":state.undo.len()}),
    )
}

pub fn staged(session: Option<&str>) -> Result<BTreeMap<String, Value>> {
    let Some(id) = session else {
        return Ok(BTreeMap::new());
    };
    paths::validate_profile_name(id)?;
    let file = paths::cache_home()
        .join("kolira/inspector-sessions")
        .join(id)
        .join("state.json");
    let state: State = serde_json::from_slice(&fs::read(file)?)?;
    Ok(state.staged)
}

/// Replay an Inspector draft into a private profile capture.
/// The caller owns the appearance lock and publishes only a complete package.
pub(crate) fn capture_draft(
    id: &str,
    snapshot: &Path,
    include_wallpaper: bool,
) -> Result<BTreeMap<String, Value>> {
    use crate::profile::snapshot_relative;
    paths::validate_profile_name(id)?;
    let session = paths::cache_home()
        .join("kolira/inspector-sessions")
        .join(id);
    if session.is_symlink() || !session.is_dir() {
        bail!("Unknown inspector session");
    }
    let _lock = crate::gui_api::lock_file(&session.join("lock"))?;
    let state: State = serde_json::from_slice(&fs::read(session.join("state.json"))?)?;
    let base = Base::Snapshot(snapshot.into());
    let mut fields: Vec<_> = state.staged.iter().collect();
    fields.sort_by_key(|(id, _)| {
        field::APPLY_FIRST
            .iter()
            .position(|field| field == id)
            .unwrap_or(field::APPLY_FIRST.len())
    });
    for (id, value) in fields {
        field::write_snapshot_value(snapshot, id, value)?;
    }
    for (relative, key, value) in state.configs {
        config_path(&relative)?;
        let path = base.resolve(&relative);
        let rel = snapshot_relative(snapshot, &path)?;
        let (patched, _) = crate::config_edit::plan(&path, &key, &value)?;
        let normalized = crate::appearance::for_theme(rel, &patched).with_context(|| {
            format!("Draft edit is outside theme appearance policy: {relative} · {key}")
        })?;
        write_bytes(&path, normalized.as_bytes(), None)?;
        let (check, _) = crate::config_edit::plan(&path, &key, &value)?;
        anyhow::ensure!(
            check == normalized,
            "Draft edit is outside theme appearance policy: {relative} · {key}"
        );
    }
    for (id, draft) in state.app_drafts {
        crate::app_themes::capture_draft(&id, &draft.value, &draft.target, snapshot)?;
    }
    if let Some(draft) = state.wallpaper_draft.filter(|_| include_wallpaper) {
        let image = wallpaper::draft_image(&draft.path)?;
        let media = snapshot.join("wallpaper");
        if media.is_dir() {
            fs::remove_dir_all(media)?;
        }
        crate::profile::capture_wallpaper_file(snapshot, Path::new(&image))?;
        let layout = snapshot.join(wallpaper::APPLETSRC_REL);
        snapshot_relative(snapshot, &layout)?;
        if layout.is_file() {
            let text = crate::layout::with_wallpaper_image(
                &fs::read_to_string(&layout)?,
                &wallpaper::image_url(&image),
            );
            write_bytes(&layout, text.as_bytes(), None)?;
        }
    }
    Ok(state.staged)
}
