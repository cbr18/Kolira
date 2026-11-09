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
fn prune() -> Result<()> {
    let _lock = crate::gui_api::lock_file(&paths::theme_root().join("operation-history.lock"))?;
    for record in records_unlocked()?
        .into_iter()
        .filter(|r| !pending(r))
        .skip(10)
    {
        match fs::remove_file(records_dir().join(format!("{}.json", record.id))) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
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
        Self::acquire_with_pending(false)
    }
    fn acquire_with_pending(allow_pending: bool) -> Result<Self> {
        if DEPTH.with(|depth| *depth.borrow() > 0) {
            DEPTH.with(|depth| *depth.borrow_mut() += 1);
            return Ok(Self { _file: None });
        }
        let file = open_lock()?;
        file.try_lock().map_err(|e| {
            anyhow!("Another appearance operation is running; retry after it finishes ({e})")
        })?;
        if !allow_pending {
            if let Some(record) = records()?.into_iter().find(pending) {
                bail!("Interrupted appearance operation {} requires recovery; run kolira operations recover {} (backup: {})",record.id,record.id,record.backup.as_deref().unwrap_or(Path::new("")).display());
            }
        }
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
    roots: Vec<PathBuf>,
    #[serde(default)]
    gsettings: BTreeMap<String, String>,
    before: BTreeMap<PathBuf, Node>,
    #[serde(default)]
    after: Option<BTreeMap<PathBuf, Node>>,
}
fn hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn collect(path: &Path, nodes: &mut BTreeMap<PathBuf, Node>, files: Option<&Path>) -> Result<()> {
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
    if directory {
        for entry in fs::read_dir(path)? {
            collect(&entry?.path(), nodes, files)?;
        }
    }
    Ok(())
}
fn collect_all(roots: &[PathBuf], files: Option<&Path>) -> Result<BTreeMap<PathBuf, Node>> {
    let mut nodes = BTreeMap::new();
    for root in roots {
        collect(root, &mut nodes, files)?;
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
    let gsettings = ["gtk-theme", "color-scheme", "icon-theme", "cursor-theme"]
        .into_iter()
        .filter_map(|key| crate::undo::gsettings_get(key).map(|value| (key.into(), value)))
        .collect();
    save_snapshot(
        backup,
        &Snapshot {
            roots,
            before,
            after: None,
            gsettings,
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
    snapshot.after = Some(collect_all(&snapshot.roots, None)?);
    save_snapshot(backup, &snapshot)
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
fn restore_desktop(backup: &Path) -> Result<()> {
    let snapshot = read_snapshot(backup)?;
    for (key, value) in snapshot.gsettings {
        if !crate::field::no_apply() {
            let status = paths::status_bounded(std::process::Command::new("gsettings").args([
                "set",
                "org.gnome.desktop.interface",
                &key,
                &value,
            ]));
            if !status.is_some_and(|s| s.success()) {
                bail!("Could not restore GSettings {key}");
            }
            if crate::undo::gsettings_get(&key).as_deref() != Some(value.as_str()) {
                bail!("GSettings verification failed: {key}");
            }
        }
    }
    crate::profile::recover_after_rollback()
}
fn restore(backup: &Path, force: bool, pause: bool) -> Vec<String> {
    let result = (|| -> Result<Vec<String>> {
        let snapshot = read_snapshot(backup)?;
        let current = collect_all(&snapshot.roots, None)?;
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
        let conflicts: Vec<_> = changed
            .iter()
            .filter(|p| !force && current.get(*p) != expected.get(*p))
            .map(|p| format!("Changed after interruption or checkpoint: {}", p.display()))
            .collect();
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
        let actual = collect_all(&snapshot.roots, None)?;
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
            save(&record)?;
            prune()?;
            Ok(value)
        }
        Err(error) => {
            record.error = Some(format!("{error:#}"));
            if let Some(backup) = &record.backup {
                record.state = "recovering".into();
                save(&record)?;
                record.recovery_errors = restore(backup, false, true);
                if record.recovery_errors.is_empty() {
                    if let Err(e) = restore_desktop(backup) {
                        record
                            .recovery_errors
                            .push(format!("Desktop recovery: {e:#}"));
                    }
                    // Restore bytes again if a live tool normalized its config.
                    record.recovery_errors.extend(restore(backup, true, false));
                } else {
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
            prune()?;
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
    prune()?;
    Ok(id)
}

pub fn list() -> Result<serde_json::Value> {
    let file = lock_path().exists().then(open_lock).transpose()?;
    let busy = file.as_ref().is_some_and(|file| file.try_lock().is_err());
    let mut values = vec![];
    for record in records()? {
        let interrupted = pending(&record) && !busy;
        let mut value = serde_json::to_value(record)?;
        value["interrupted"] = interrupted.into();
        let backup = value["backup"].as_str().map(PathBuf::from);
        let health = backup.as_deref().map(verify_backup);
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
    Ok(serde_json::json!({"busy":busy,"operations":values}))
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
    verify_backup(backup)?;
    let snapshot = read_snapshot(backup)?;
    run(&source.previous_profile, || {
        CURRENT.with(|c| c.borrow_mut().as_mut().unwrap().restored_operation = Some(id.into()));
        let safety = crate::profile::unique_dir(&paths::backups_dir(), "pre-restore")?;
        prepare(&safety, snapshot.roots)?;
        let errors = restore(backup, true, true);
        if !errors.is_empty() {
            bail!("Restoration failed: {}", errors.join("; "));
        }
        restore_desktop(backup)?;
        let errors = restore(backup, true, false);
        if !errors.is_empty() {
            bail!("Restoration verification: {}", errors.join("; "));
        }
        let result = serde_json::json!({"operation_id":current_id(),"restored_operation":id,"backup":safety});
        note_report(result.clone())?;
        Ok(result)
    })
}
pub fn recover(id: &str, force: bool) -> Result<Record> {
    paths::validate_profile_name(id)?;
    let _lock = MutationGuard::acquire_with_pending(true)?;
    let mut record: Record =
        serde_json::from_slice(&fs::read(records_dir().join(format!("{id}.json")))?)?;
    if !pending(&record) {
        bail!("Operation {id} does not require recovery");
    }
    if record.state == "preparing" && record.backup.is_none() {
        record.state = "cancelled_preparation".into();
        record.updated = now();
        save(&record)?;
        prune()?;
        return Ok(record);
    }
    let backup = record.backup.as_ref().context("Operation has no backup")?;
    record.state = "recovering".into();
    save(&record)?;
    record.recovery_errors = restore(backup, force, true);
    if record.recovery_errors.is_empty() {
        if let Err(e) = restore_desktop(backup) {
            record
                .recovery_errors
                .push(format!("Desktop recovery: {e:#}"));
        }
        record.recovery_errors.extend(restore(backup, true, false));
    } else {
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
    prune()?;
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
