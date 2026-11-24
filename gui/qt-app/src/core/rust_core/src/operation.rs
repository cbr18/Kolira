//! Cross-process appearance lock and durable profile-apply recovery.
use crate::{gui_api::write_bytes, paths};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::{Path, PathBuf},
};

thread_local! {
    static DEPTH: RefCell<usize> = const { RefCell::new(0) };
    static CURRENT: RefCell<Option<Record>> = const { RefCell::new(None) };
    static HOOK_ERRORS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static LAST_ATTEMPT: RefCell<Option<String>> = const { RefCell::new(None) };
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    pub profile: String,
    pub state: String,
    pub started: String,
    pub updated: String,
    pub backup: Option<PathBuf>,
    pub error: Option<String>,
    pub recovery_errors: Vec<String>,
    #[serde(default)]
    pub previous_profile: String,
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default)]
    pub report: serde_json::Value,
    #[serde(default)]
    pub restored_operation: Option<String>,
}
fn default_source() -> String {
    "cli".into()
}
impl Record {
    fn is_live(&self) -> bool {
        self.report["kind"] == "live"
    }
}
thread_local! {
    static SOURCE: RefCell<String> = RefCell::new(default_source());
}
pub fn with_source<T>(source: &str, work: impl FnOnce() -> Result<T>) -> Result<T> {
    if !["cli", "gui", "mcp", "schedule"].contains(&source) {
        bail!("Invalid operation source");
    }
    let previous = SOURCE.with(|s| s.replace(source.into()));
    let result = work();
    SOURCE.with(|s| s.replace(previous));
    result
}
/// Save partial results before any fallible follow-up so failure keeps details.
pub fn note_report(report: serde_json::Value) -> Result<()> {
    CURRENT.with(|c| {
        let mut context = c.borrow_mut();
        if let Some(record) = context.as_mut() {
            record.report = report;
            save(record)?;
        }
        Ok(())
    })
}
fn records_dir() -> PathBuf {
    paths::theme_root().join("operations")
}
fn lock_path() -> PathBuf {
    paths::theme_root().join("appearance.lock")
}
fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}
fn save(record: &Record) -> Result<()> {
    let _lock = crate::gui_api::lock_file(&paths::theme_root().join("operation-history.lock"))?;
    private_dir(&records_dir())?;
    write_bytes(
        &records_dir().join(format!("{}.json", record.id)),
        &serde_json::to_vec_pretty(record)?,
        Some(0o600),
    )
}
fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
fn records() -> Result<Vec<Record>> {
    let _lock = crate::gui_api::lock_file(&paths::theme_root().join("operation-history.lock"))?;
    records_unlocked()
}
fn records_unlocked() -> Result<Vec<Record>> {
    if !records_dir().exists() {
        return Ok(vec![]);
    }
    let mut result = vec![];
    for entry in fs::read_dir(records_dir())? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "json") {
            let mut record = serde_json::from_slice::<Record>(&fs::read(&path)?)
                .with_context(|| format!("Invalid operation record {}", path.display()))?;
            if let Some(backup) = &record.backup {
                record.backup = Some(paths::migrated_backup_path(backup));
            }
            result.push(record);
        }
    }
    result.sort_by(|a, b| b.started.cmp(&a.started));
    Ok(result)
}
fn pending(record: &Record) -> bool {
    matches!(
        record.state.as_str(),
        "preparing" | "applying" | "recovering" | "recovery_failed"
    )
}
fn open_lock() -> Result<fs::File> {
    fs::create_dir_all(paths::theme_root())?;
    Ok(fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path())?)
}
/// Reentrant only within the current thread, never across independent workers.
pub struct MutationGuard {
    _file: Option<fs::File>,
}
impl MutationGuard {
    pub fn acquire() -> Result<Self> {
        if DEPTH.with(|depth| *depth.borrow() > 0) {
            DEPTH.with(|depth| *depth.borrow_mut() += 1);
            return Ok(Self { _file: None });
        }
        let file = open_lock()?;
        file.try_lock().map_err(|e| {
            anyhow!("Another appearance operation is running; retry after it finishes ({e})")
        })?;
        // Only a live owner blocks another mutation. Interrupted records retain
        // their diagnostics and backups, but must not become a permanent lock.
        DEPTH.with(|depth| *depth.borrow_mut() = 1);
        Ok(Self { _file: Some(file) })
    }
}
impl Drop for MutationGuard {
    fn drop(&mut self) {
        DEPTH.with(|depth| *depth.borrow_mut() -= 1);
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Node {
    kind: String,
    mode: u32,
    hash: String,
    link: Option<PathBuf>,
}
#[derive(Serialize, Deserialize)]
struct Snapshot {
    #[serde(default)]
    tmux_before: crate::tmux_runtime::State,
    #[serde(default)]
    tmux_after: Option<crate::tmux_runtime::State>,
    roots: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    missing_parents: Vec<PathBuf>,
    #[serde(default)]
    gsettings: BTreeMap<String, String>,
    before: BTreeMap<PathBuf, Node>,
    #[serde(default)]
    after: Option<BTreeMap<PathBuf, Node>>,
    #[serde(default)]
    gsettings_after: BTreeMap<String, String>,
    #[serde(default)]
    exact_paths: bool,
}
fn hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn collect(
    path: &Path,
    nodes: &mut BTreeMap<PathBuf, Node>,
    files: Option<&Path>,
    recurse: bool,
) -> Result<()> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e).with_context(|| format!("Cannot inspect {}", path.display())),
    };
    let mut node = Node {
        kind: String::new(),
        mode: meta.permissions().mode() & 0o7777,
        hash: String::new(),
        link: None,
    };
    if meta.file_type().is_symlink() {
        node.kind = "symlink".into();
        node.link = Some(fs::read_link(path)?);
    } else if meta.is_dir() {
        node.kind = "directory".into();
    } else if meta.is_file() {
        node.kind = "file".into();
        let bytes = fs::read(path).with_context(|| format!("Cannot back up {}", path.display()))?;
        node.hash = hash(&bytes);
        if let Some(dir) = files {
            write_bytes(
                &dir.join(hash(path.as_os_str().as_encoded_bytes())),
                &bytes,
                Some(0o600),
            )?;
        }
    } else {
        bail!(
            "Unsupported special file in apply footprint: {}",
            path.display()
        );
    }
    let directory = node.kind == "directory";
    nodes.insert(path.into(), node);
    if directory && recurse {
        for entry in fs::read_dir(path)? {
            collect(&entry?.path(), nodes, files, true)?;
        }
    }
    Ok(())
}
fn collect_all(roots: &[PathBuf], files: Option<&Path>) -> Result<BTreeMap<PathBuf, Node>> {
    let mut nodes = BTreeMap::new();
    for root in roots {
        collect(root, &mut nodes, files, true)?;
    }
    Ok(nodes)
}
fn current_nodes(snapshot: &Snapshot) -> Result<BTreeMap<PathBuf, Node>> {
    if !snapshot.exact_paths {
        return collect_all(&snapshot.roots, None);
    }
    let journal = paths::theme_root().join("apply-journal");
    let mut nodes = BTreeMap::new();
    for root in &snapshot.roots {
        collect(root, &mut nodes, None, *root == journal)?;
    }
    Ok(nodes)
}
fn manifest(backup: &Path) -> PathBuf {
    backup.join("transaction/manifest.json")
}
fn save_snapshot(backup: &Path, snapshot: &Snapshot) -> Result<()> {
    write_bytes(
        &manifest(backup),
        &serde_json::to_vec(snapshot)?,
        Some(0o600),
    )
}
fn normalize_roots(roots: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut expanded = BTreeSet::new();
    for root in roots {
        // Follow an existing symlink root/parent as well: destination tools may
        // write through it. The symlink itself is retained separately.
        let mut ancestor = root.as_path();
        while !ancestor.exists() && ancestor.parent().is_some() {
            ancestor = ancestor.parent().unwrap();
        }
        if let Ok(real) = ancestor.canonicalize() {
            if let Ok(suffix) = root.strip_prefix(ancestor) {
                expanded.insert(if suffix.as_os_str().is_empty() {
                    real
                } else {
                    real.join(suffix)
                });
            }
        }
        expanded.insert(root);
    }
    let mut result: Vec<PathBuf> = vec![];
    let mut ordered: Vec<_> = expanded.into_iter().collect();
    ordered.sort_by_key(|p| p.components().count());
    for path in ordered {
        if !result
            .iter()
            .any(|p| path.starts_with(p) && !p.is_symlink())
        {
            result.push(path);
        }
    }
    result
}
/// Called after preparation, immediately before any live mutation.
pub fn prepare(backup: &Path, roots: Vec<PathBuf>) -> Result<()> {
    prepare_inner(
        backup,
        roots,
        ["gtk-theme", "color-scheme", "icon-theme", "cursor-theme"],
        false,
    )
}

pub(crate) fn prepare_live<'a>(
    backup: &Path,
    roots: Vec<PathBuf>,
    settings: impl IntoIterator<Item = &'a str>,
) -> Result<()> {
    prepare_inner(backup, roots, settings, true)
}

fn prepare_inner<'a>(
    backup: &Path,
    roots: Vec<PathBuf>,
    settings: impl IntoIterator<Item = &'a str>,
    track_new_parents: bool,
) -> Result<()> {
    if CURRENT.with(|c| c.borrow().as_ref().is_none_or(|r| r.backup.is_some())) {
        return Ok(());
    }
    private_dir(backup)?;
    private_dir(&backup.join("transaction"))?;
    private_dir(&backup.join("transaction/files"))?;
    let roots = normalize_roots(roots);
    for root in &roots {
        if backup.starts_with(root) || records_dir().starts_with(root) || root == Path::new("/") {
            bail!("Apply footprint includes its own backup/state: {}; narrow the custom component paths",root.display());
        }
    }
    let before = collect_all(&roots, Some(&backup.join("transaction/files")))?;
    let gsettings = settings
        .into_iter()
        .filter_map(|key| crate::undo::gsettings_get(key).map(|value| (key.into(), value)))
        .collect();
    // A file write may create parent directories. Don't snapshot their other
    // contents or remove files that appear there after Apply.
    let missing_parents = if track_new_parents {
        let mut missing: Vec<_> = roots
            .iter()
            .flat_map(|path| {
                path.ancestors()
                    .skip(1)
                    .take_while(|parent| {
                        fs::symlink_metadata(parent)
                            .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
                    })
                    .map(Path::to_path_buf)
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        missing.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
        missing
    } else {
        vec![]
    };
    save_snapshot(
        backup,
        &Snapshot {
            tmux_before: if roots.iter().any(|p| {
                crate::four_apps::paths_for("tmux").iter().any(|r| {
                    let live = paths::live_path(r);
                    p == &live || live.starts_with(p)
                })
            }) {
                crate::tmux_runtime::capture()?
            } else {
                crate::tmux_runtime::State::new()
            },
            tmux_after: None,
            roots,
            missing_parents,
            before,
            after: None,
            gsettings,
            gsettings_after: BTreeMap::new(),
            exact_paths: false,
        },
    )?;
    CURRENT.with(|c| -> Result<()> {
        let mut record = c.borrow_mut();
        let record = record.as_mut().context("No current operation")?;
        record.backup = Some(backup.into());
        record.state = "applying".into();
        record.updated = now();
        save(record)
    })
}
pub fn current_id() -> Option<String> {
    CURRENT.with(|c| c.borrow().as_ref().map(|r| r.id.clone()))
}
pub fn last_attempt_id() -> Option<String> {
    LAST_ATTEMPT.with(|id| id.borrow().clone())
}
pub fn note_hook_error(message: String) {
    HOOK_ERRORS.with(|e| e.borrow_mut().push(message));
}
pub fn hook_checkpoint() -> Result<()> {
    let errors = HOOK_ERRORS.with(|e| std::mem::take(&mut *e.borrow_mut()));
    if !errors.is_empty() {
        bail!("External commands failed: {}", errors.join("; "));
    }
    Ok(())
}
fn read_snapshot(backup: &Path) -> Result<Snapshot> {
    Ok(serde_json::from_slice(&fs::read(manifest(backup))?)?)
}
fn checkpoint(backup: &Path) -> Result<()> {
    let mut snapshot = read_snapshot(backup)?;
    snapshot.after = Some(current_nodes(&snapshot)?);
    snapshot.gsettings_after = gtk_values(snapshot.gsettings.keys().map(String::as_str));
    snapshot.tmux_after = Some(if snapshot.tmux_before.is_empty() {
        crate::tmux_runtime::State::new()
    } else {
        crate::tmux_runtime::capture()?
    });
    save_snapshot(backup, &snapshot)
}

pub(crate) fn gtk_values<'a>(keys: impl IntoIterator<Item = &'a str>) -> BTreeMap<String, String> {
    keys.into_iter()
        .filter_map(|key| crate::undo::gsettings_get(key).map(|value| (key.into(), value)))
        .collect()
}

pub(crate) fn copy_changed_backup(backup: &Path, target: &Path) -> Result<bool> {
    let original = read_snapshot(backup)?;
    let after = original
        .after
        .as_ref()
        .context("Missing operation checkpoint")?;
    let journal = paths::theme_root().join("apply-journal");
    let mut roots = BTreeSet::new();
    let mut missing_parents: BTreeSet<_> = original.missing_parents.into_iter().collect();
    for path in original.before.keys().chain(after.keys()) {
        if original.before.get(path) == after.get(path) {
            continue;
        }
        if path.starts_with(&journal) {
            roots.insert(journal.clone());
        } else if !original.before.contains_key(path)
            && after.get(path).is_some_and(|node| node.kind == "directory")
        {
            missing_parents.insert(path.clone());
        } else {
            roots.insert(path.clone());
        }
    }
    let mut missing_parents: Vec<_> = missing_parents.into_iter().collect();
    missing_parents.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    let before: BTreeMap<_, _> = original
        .before
        .into_iter()
        .filter(|(path, _)| {
            roots.contains(path) || (roots.contains(&journal) && path.starts_with(&journal))
        })
        .collect();
    let gsettings: BTreeMap<_, _> = original
        .gsettings
        .into_iter()
        .filter(|(key, value)| {
            original
                .gsettings_after
                .get(key)
                .is_some_and(|after| after != value)
        })
        .collect();
    let tmux_changed = original.tmux_after.as_ref().is_some_and(|after| {
        !crate::tmux_runtime::changes(&original.tmux_before, after).is_empty()
    });
    if roots.is_empty() && gsettings.is_empty() && !tmux_changed {
        return Ok(false);
    }
    let files = target.join("transaction/files");
    private_dir(&files)?;
    for (path, node) in &before {
        if node.kind != "file" {
            continue;
        }
        let file = hash(path.as_os_str().as_encoded_bytes());
        let source = backup.join("transaction/files").join(&file);
        let target = files.join(&file);
        if fs::hard_link(&source, &target).is_err() {
            fs::copy(&source, &target)?;
        }
    }
    save_snapshot(
        target,
        &Snapshot {
            roots: roots.into_iter().collect(),
            before,
            after: None,
            gsettings,
            gsettings_after: BTreeMap::new(),
            tmux_before: if tmux_changed {
                original.tmux_before
            } else {
                Default::default()
            },
            tmux_after: None,
            missing_parents,
            exact_paths: true,
        },
    )?;
    verify_backup(target)?;
    Ok(true)
}
fn remove_node(path: &Path) -> Result<()> {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return Ok(());
    };
    if meta.is_dir() && !meta.file_type().is_symlink() {
        fs::remove_dir(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}
fn restore_desktop(backup: &Path, force: bool, live: bool, wallpaper: bool) -> Result<()> {
    let snapshot = read_snapshot(backup)?;
    let expected = if snapshot.tmux_before.is_empty() {
        crate::tmux_runtime::State::new()
    } else if force {
        crate::tmux_runtime::capture()?
    } else {
        snapshot
            .tmux_after
            .clone()
            .unwrap_or_else(|| snapshot.tmux_before.clone())
    };
    crate::tmux_runtime::restore(&crate::tmux_runtime::changes(
        &snapshot.tmux_before,
        &expected,
    ))?;
    restore_gtk(&snapshot.gsettings)?;
    if live {
        if wallpaper {
            crate::wallpaper::refresh_after_restore()?;
        }
        crate::profile::refresh_live_after_rollback(&snapshot.roots);
        Ok(())
    } else {
        crate::profile::recover_after_rollback()
    }
}
pub(crate) fn restore_gtk(settings: &BTreeMap<String, String>) -> Result<()> {
    for (key, value) in settings {
        if !crate::field::no_apply() {
            let status = paths::status_bounded(std::process::Command::new("gsettings").args([
                "set",
                "org.gnome.desktop.interface",
                key,
                value,
            ]));
            if !status.is_some_and(|s| s.success()) {
                bail!("Could not restore GSettings {key}");
            }
            if crate::undo::gsettings_get(key).as_deref() != Some(value.as_str()) {
                bail!("GSettings verification failed: {key}");
            }
        }
    }
    Ok(())
}
fn restore(backup: &Path, force: bool, pause: bool) -> Vec<String> {
    let result = (|| -> Result<Vec<String>> {
        let snapshot = read_snapshot(backup)?;
        let current = current_nodes(&snapshot)?;
        let expected = snapshot.after.as_ref().unwrap_or(&snapshot.before);
        let mut keys: BTreeSet<PathBuf> = snapshot
            .before
            .keys()
            .chain(current.keys())
            .chain(expected.keys())
            .cloned()
            .collect();
        let changed: Vec<PathBuf> = keys
            .iter()
            .filter(|p| snapshot.before.get(*p) != current.get(*p))
            .cloned()
            .collect();
        let mut conflicts: Vec<_> = changed
            .iter()
            .filter(|p| !force && current.get(*p) != expected.get(*p))
            .map(|p| format!("Changed after interruption or checkpoint: {}", p.display()))
            .collect();
        if !force {
            conflicts.extend(crate::tmux_runtime::conflicts(
                &snapshot.tmux_before,
                snapshot
                    .tmux_after
                    .as_ref()
                    .unwrap_or(&snapshot.tmux_before),
            )?);
        }
        // Check every conflict before restoring anything, including removals.
        if !conflicts.is_empty() {
            return Ok(conflicts);
        }
        // Verify every original before changing any live file.
        for (path, node) in &snapshot.before {
            if node.kind == "file" {
                let bytes = fs::read(
                    backup
                        .join("transaction/files")
                        .join(hash(path.as_os_str().as_encoded_bytes())),
                )?;
                if hash(&bytes) != node.hash {
                    bail!("Corrupt backup for {}", path.display());
                }
            }
        }
        if pause {
            crate::profile::pause_for_recovery();
        }
        // Remove only newly created entries, deepest first; never recursively
        // remove a directory that may contain another application's files.
        let mut remove: Vec<_> = keys
            .iter()
            .filter(|p| !snapshot.before.contains_key(*p))
            .cloned()
            .collect();
        remove.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
        for path in remove {
            remove_node(&path)?;
        }
        let mut restore: Vec<_> = std::mem::take(&mut keys)
            .into_iter()
            .filter(|p| snapshot.before.get(p) != current.get(p))
            .collect();
        restore.sort_by_key(|p| p.components().count());
        for path in restore {
            let Some(node) = snapshot.before.get(&path) else {
                continue;
            };
            match node.kind.as_str() {
                "directory" => {
                    if path.is_symlink() || path.is_file() {
                        remove_node(&path)?;
                    }
                    fs::create_dir_all(&path)?;
                    fs::set_permissions(&path, fs::Permissions::from_mode(node.mode))?;
                }
                "file" => {
                    if path.is_dir() && !path.is_symlink() {
                        fs::remove_dir(&path)?;
                    }
                    let bytes = fs::read(
                        backup
                            .join("transaction/files")
                            .join(hash(path.as_os_str().as_encoded_bytes())),
                    )?;
                    if hash(&bytes) != node.hash {
                        bail!("Corrupt backup for {}", path.display());
                    }
                    write_bytes(&path, &bytes, Some(node.mode))?;
                }
                "symlink" => {
                    remove_node(&path)?;
                    fs::create_dir_all(path.parent().context("symlink parent")?)?;
                    symlink(node.link.as_ref().context("symlink target")?, &path)?;
                }
                _ => bail!("Invalid backup node"),
            }
        }
        for path in &snapshot.missing_parents {
            // An empty directory created for this write can be cleaned up.
            // Keep any other files created there since the operation.
            match fs::remove_dir(path) {
                Ok(()) => {}
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::NotFound
                            | std::io::ErrorKind::DirectoryNotEmpty
                            | std::io::ErrorKind::NotADirectory
                    ) => {}
                Err(e) => {
                    return Err(e).with_context(|| format!("Cannot clean up {}", path.display()))
                }
            }
        }
        let actual = current_nodes(&snapshot)?;
        let all: BTreeSet<_> = snapshot.before.keys().chain(actual.keys()).collect();
        let errors = all
            .into_iter()
            .filter(|p| actual.get(*p) != snapshot.before.get(*p))
            .map(|p| format!("Recovery verification failed: {}", p.display()))
            .collect();
        Ok(errors)
    })();
    result.unwrap_or_else(|e| vec![format!("{e:#}")])
}

pub fn run<T>(profile: &str, work: impl FnOnce() -> Result<T>) -> Result<T> {
    let _lock = match MutationGuard::acquire() {
        Ok(lock) => lock,
        Err(error) => {
            let source = SOURCE.with(|s| s.borrow().clone());
            let message = format!("{error:#}");
            let id = rejected_attempt(
                profile,
                &source,
                &message,
                message.contains("Another appearance operation"),
            )?;
            LAST_ATTEMPT.with(|last| *last.borrow_mut() = Some(id.clone()));
            bail!("{message}; operation {id}");
        }
    };
    if current_id().is_some() {
        return work();
    }
    private_dir(&records_dir())?;
    let unique = tempfile::Builder::new()
        .prefix("apply-")
        .tempdir_in(records_dir())?;
    let id = unique
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let record = Record {
        id,
        profile: profile.into(),
        state: "preparing".into(),
        started: now(),
        updated: now(),
        backup: None,
        error: None,
        recovery_errors: vec![],
        previous_profile: crate::profile::get_active_profile().unwrap_or_default(),
        source: SOURCE.with(|s| s.borrow().clone()),
        report: serde_json::Value::Null,
        restored_operation: None,
    };
    LAST_ATTEMPT.with(|last| *last.borrow_mut() = Some(record.id.clone()));
    save(&record)?;
    CURRENT.with(|c| *c.borrow_mut() = Some(record));
    HOOK_ERRORS.with(|e| e.borrow_mut().clear());
    let outcome = work().and_then(|value| {
        hook_checkpoint()?;
        Ok(value)
    });
    if let Ok(changed) = changed_paths() {
        CURRENT.with(|c| {
            if let Some(record) = c.borrow_mut().as_mut() {
                if !record.report.is_object() {
                    record.report = serde_json::json!({});
                }
                record.report["changed_paths"] = serde_json::json!(changed);
            }
        });
    }
    let mut record = CURRENT.with(|c| c.borrow_mut().take()).unwrap();
    if let Some(backup) = &record.backup {
        if let Err(e) = checkpoint(backup) {
            record.error = Some(format!("Cannot checkpoint recovery state: {e:#}"));
            record.state = "recovery_failed".into();
            record.updated = now();
            save(&record)?;
            crate::profile::recover_desktop_shell();
            bail!(
                "{}; operation {}; backup {}",
                record.error.unwrap(),
                record.id,
                backup.display()
            );
        }
    }
    match outcome {
        Ok(value) => {
            record.state = if record.report.get("skip_reason").is_some() {
                "skipped"
            } else {
                "succeeded"
            }
            .into();
            record.updated = now();
            if record.state == "succeeded" {
                if let Err(error) = crate::restore_points::record_automatic(&record) {
                    if !record.report.is_object() {
                        record.report = serde_json::json!({});
                    }
                    record.report["restore_point_error"] = format!("{error:#}").into();
                }
            }
            save(&record)?;
            // Bounded retention for the apply's own footprint (best-effort):
            // never fails an otherwise successful apply. The just-finished
            // operation is protected from eviction inside the sweep.
            crate::retention::run_after_apply(&record.id);
            Ok(value)
        }
        Err(error) => {
            record.error = Some(format!("{error:#}"));
            if let Some(backup) = &record.backup {
                record.state = "recovering".into();
                save(&record)?;
                let live = record.is_live();
                record.recovery_errors = restore(backup, false, !live);
                if record.recovery_errors.is_empty() {
                    if let Err(e) =
                        restore_desktop(backup, false, live, record.report["wallpaper"] == true)
                    {
                        record
                            .recovery_errors
                            .push(format!("Desktop recovery: {e:#}"));
                    }
                    // Restore bytes again if a live tool normalized its config.
                    record.recovery_errors.extend(restore(backup, true, false));
                } else if !live {
                    crate::profile::recover_desktop_shell();
                }
                record.state = if record.recovery_errors.is_empty() {
                    "rolled_back"
                } else {
                    "recovery_failed"
                }
                .into();
            } else {
                record.state = "failed_preparation".into();
            }
            record.updated = now();
            save(&record)?;
            bail!(
                "Apply failed: {error:#}; operation {}; {}; backup {}; recovery errors: {}",
                record.id,
                record.state,
                record.backup.as_deref().unwrap_or(Path::new("")).display(),
                if record.recovery_errors.is_empty() {
                    "none".into()
                } else {
                    record.recovery_errors.join("; ")
                }
            );
        }
    }
}

/// Record an attempt rejected before acquiring appearance ownership. This
/// changes only history metadata and never bypasses the appearance lock.
pub fn rejected_attempt(
    profile: &str,
    source: &str,
    message: &str,
    skipped: bool,
) -> Result<String> {
    private_dir(&records_dir())?;
    let unique = tempfile::Builder::new()
        .prefix("apply-")
        .tempdir_in(records_dir())?;
    let id = unique
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let record = Record {
        id: id.clone(),
        profile: profile.into(),
        state: if skipped {
            "skipped"
        } else {
            "failed_preparation"
        }
        .into(),
        started: now(),
        updated: now(),
        backup: None,
        error: (!skipped).then(|| message.into()),
        recovery_errors: vec![],
        previous_profile: crate::profile::get_active_profile().unwrap_or_default(),
        source: source.into(),
        report: serde_json::json!({"message":message,"skip_reason":if skipped {Some(message)} else {None}}),
        restored_operation: None,
    };
    save(&record)?;
    Ok(id)
}

pub fn list() -> Result<serde_json::Value> {
    list_page(0, 10, true)
}

pub(crate) fn list_page(offset: usize, limit: usize, verify: bool) -> Result<serde_json::Value> {
    let file = lock_path().exists().then(open_lock).transpose()?;
    let busy = file.as_ref().is_some_and(|file| file.try_lock().is_err());
    let mut values = vec![];
    let records = records()?;
    let last_profile_apply = records
        .iter()
        .find(|record| {
            record.state == "succeeded" && record.restored_operation.is_none() && !record.is_live()
        })
        .map(|record| record.profile.clone());
    let completed = records.iter().filter(|record| !pending(record)).count();
    let mut index = 0;
    for record in records {
        if pending(&record) {
            if offset != 0 {
                continue;
            }
        } else {
            let include = index >= offset && index < offset.saturating_add(limit);
            index += 1;
            if !include {
                continue;
            }
        }
        let interrupted = pending(&record) && !busy;
        let mut value = serde_json::to_value(record)?;
        value["interrupted"] = interrupted.into();
        let backup = value["backup"].as_str().map(PathBuf::from);
        let health = backup.as_deref().map(|path| {
            if verify {
                verify_backup(path)
            } else {
                fs::metadata(manifest(path)).map(|_| ()).map_err(Into::into)
            }
        });
        value["backup_status"] = match &health {
            None => "not_created",
            Some(Ok(())) => "available",
            Some(Err(_)) => "unavailable",
        }
        .into();
        value["backup_error"] = health
            .and_then(Result::err)
            .map(|e| format!("{e:#}"))
            .into();
        value["can_restore"] = (!busy && value["backup_status"] == "available").into();
        values.push(value);
    }
    let next = offset.saturating_add(limit);
    Ok(
        serde_json::json!({"busy":busy,"operations":values,"last_profile_apply":last_profile_apply,
        "next_offset":(next < completed).then_some(next)}),
    )
}

fn verify_backup(backup: &Path) -> Result<()> {
    let snapshot = read_snapshot(backup)?;
    for (path, node) in snapshot.before {
        if node.kind == "file" {
            let bytes = fs::read(
                backup
                    .join("transaction/files")
                    .join(hash(path.as_os_str().as_encoded_bytes())),
            )?;
            if hash(&bytes) != node.hash {
                bail!("Corrupt backup for {}", path.display());
            }
        }
    }
    Ok(())
}

/// Explicit restoration of a completed change, with a new safety transaction.
pub fn restore_completed(id: &str, confirmed: bool) -> Result<serde_json::Value> {
    paths::validate_profile_name(id)?;
    if !confirmed {
        bail!("Restoration replaces current appearance; explicit confirmation required");
    }
    let _lock = MutationGuard::acquire()?;
    let source: Record =
        serde_json::from_slice(&fs::read(records_dir().join(format!("{id}.json")))?)?;
    if pending(&source) {
        bail!("Use operation.recover for interrupted operations");
    }
    let backup = source
        .backup
        .as_deref()
        .context("Operation has no backup")?;
    restore_saved_backup(
        backup,
        id,
        &source.previous_profile,
        source.is_live(),
        source.report["wallpaper"] == true,
    )
}

pub(crate) fn note_restored(id: &str) {
    CURRENT.with(|c| c.borrow_mut().as_mut().unwrap().restored_operation = Some(id.into()));
}

pub(crate) fn restore_saved_backup(
    backup: &Path,
    id: &str,
    profile: &str,
    live: bool,
    wallpaper: bool,
) -> Result<serde_json::Value> {
    let _lock = MutationGuard::acquire()?;
    verify_backup(backup)?;
    let snapshot = read_snapshot(backup)?;
    run(profile, || {
        note_restored(id);
        let safety = crate::profile::unique_dir(&paths::backups_dir(), "pre-restore")?;
        if live {
            note_report(serde_json::json!({"kind":"live","wallpaper":wallpaper}))?;
            prepare_live(
                &safety,
                snapshot.roots,
                snapshot.gsettings.keys().map(String::as_str),
            )?;
        } else {
            prepare_live(
                &safety,
                snapshot.roots,
                snapshot.gsettings.keys().map(String::as_str),
            )?;
        }
        let errors = restore(backup, true, !live);
        if !errors.is_empty() {
            bail!("Restoration failed: {}", errors.join("; "));
        }
        restore_desktop(backup, true, live, wallpaper)?;
        let errors = restore(backup, true, false);
        if !errors.is_empty() {
            bail!("Restoration verification: {}", errors.join("; "));
        }
        let mut result = serde_json::json!({"operation_id":current_id(),"restored_operation":id,"backup":safety});
        if live {
            result["kind"] = "live".into();
            result["wallpaper"] = wallpaper.into();
        }
        note_report(result.clone())?;
        Ok(result)
    })
}
pub fn recover(id: &str, force: bool) -> Result<Record> {
    paths::validate_profile_name(id)?;
    let _lock = MutationGuard::acquire()?;
    let mut record: Record =
        serde_json::from_slice(&fs::read(records_dir().join(format!("{id}.json")))?)?;
    if !pending(&record) {
        bail!("Operation {id} does not require recovery");
    }
    if record.state == "preparing" && record.backup.is_none() {
        record.state = "cancelled_preparation".into();
        record.updated = now();
        save(&record)?;
        return Ok(record);
    }
    let backup = record.backup.as_ref().context("Operation has no backup")?;
    record.state = "recovering".into();
    save(&record)?;
    let live = record.is_live();
    record.recovery_errors = restore(backup, force, !live);
    if record.recovery_errors.is_empty() {
        if let Err(e) = restore_desktop(backup, force, live, record.report["wallpaper"] == true) {
            record
                .recovery_errors
                .push(format!("Desktop recovery: {e:#}"));
        }
        record.recovery_errors.extend(restore(backup, true, false));
    } else if !live {
        crate::profile::recover_desktop_shell();
    }
    record.state = if record.recovery_errors.is_empty() {
        "rolled_back"
    } else {
        "recovery_failed"
    }
    .into();
    record.updated = now();
    save(&record)?;
    Ok(record)
}

/// Compare the recovery snapshot with live files before offering restarts.
pub(crate) fn changed_paths() -> Result<Vec<PathBuf>> {
    let backup = CURRENT.with(|context| {
        context
            .borrow()
            .as_ref()
            .and_then(|record| record.backup.clone())
    });
    let Some(backup) = backup else {
        return Ok(vec![]);
    };
    let snapshot = read_snapshot(&backup)?;
    let current = collect_all(&snapshot.roots, None)?;
    let all: BTreeSet<_> = snapshot
        .before
        .keys()
        .chain(current.keys())
        .cloned()
        .collect();
    Ok(all
        .into_iter()
        .filter(|path| snapshot.before.get(path) != current.get(path))
        .collect())
}

/// Include servers that appeared after initial preparation before touching them.
pub(crate) fn note_tmux_before(state: &crate::tmux_runtime::State) -> Result<()> {
    let backup = CURRENT
        .with(|c| c.borrow().as_ref().and_then(|r| r.backup.clone()))
        .context("tmux writes require a prepared appearance operation")?;
    let mut snapshot = read_snapshot(&backup)?;
    for (key, value) in state {
        snapshot
            .tmux_before
            .entry(key.clone())
            .or_insert_with(|| value.clone());
    }
    save_snapshot(&backup, &snapshot)
}
