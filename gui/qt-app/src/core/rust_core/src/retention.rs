//! Bounded retention for the on-disk artifacts an apply leaves behind: the
//! per-apply safety copies, the operation history that references them, the
//! installer's `deploy-kolira*` copies and the one-shot `pre-import/prune/…`
//! safety folders. Restore points keep their own retention
//! ([`crate::restore_points`]); the `apply-journal` is a single rewritable file.
//!
//! A `pre-apply` (and `pre-restore`) backup is never orphaned from its
//! operation: the two are evicted together as a pair so `restore_completed` /
//! `recover` never point at a deleted backup. Pending, current and
//! restore-referenced operations are protected. The sweep is best-effort — a
//! failure here never fails an otherwise successful apply.

use crate::{gui_api, operation, paths};
use anyhow::Result;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Paired `pre-apply`/`pre-restore` backups + their operation records.
const APPLY_COUNT: usize = 12;
const APPLY_BYTES: u64 = 1024 * 1024 * 1024; // 1 GiB
/// Installer copies kept regardless of age (`deploy-kolira-*`).
const DEPLOY_COUNT: usize = 5;
/// One-shot safety copies (`pre-import-`, `pre-prune-`, …) per prefix.
const SAFETY_COUNT: usize = 10;
const SAFETY_MAX_AGE_DAYS: u64 = 14;

/// One-shot safety folders that are not tied to an operation record. `pre-apply-`
/// and `pre-restore` are excluded: they belong to a record and are paired.
const UNPAIRED_PREFIXES: &[&str] = &[
    "pre-import-",
    "pre-prune-",
    "pre-split-",
    "pre-save-",
    "pre-normalize-",
    "palette-apply-",
    "agent-instructions-",
];

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn mtime_secs(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Recursive size of a directory tree (metadata only, no symlink following),
/// matching `restore_points::measure`.
fn dir_size(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if !meta.is_dir() {
        return meta.len();
    }
    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            total = total.saturating_add(dir_size(&entry.path()));
        }
    }
    total
}

fn is_pending_state(state: &str) -> bool {
    matches!(
        state,
        "preparing" | "applying" | "recovering" | "recovery_failed"
    )
}

/// A terminal record whose backup may be evicted. `recovery_failed` is excluded:
/// it is pending and must stay for an explicit `recover`.
fn is_evictable_state(state: &str) -> bool {
    matches!(
        state,
        "succeeded" | "skipped" | "rolled_back" | "cancelled_preparation"
    )
}

fn read_records(records_dir: &Path) -> Vec<operation::Record> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(records_dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        // A corrupt record must not abort the sweep of the healthy ones.
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(record) = serde_json::from_slice::<operation::Record>(&bytes) {
                out.push(record);
            }
        }
    }
    out
}

/// Move a path into a trash directory (atomic rename) so removal never leaves a
/// half-deleted tree the rest of the sweep could trip over.
fn move_to_trash(path: &Path, trash_dir: &Path, name: &str) {
    if path.exists() {
        let _ = fs::rename(path, trash_dir.join(name));
    }
}

/// Choose which paired backups to evict. `paired` is `(id, bytes)` for every
/// record with an existing backup; `protected` lists the ids that must stay.
/// Returns eviction candidates in the given order (oldest first expected).
fn plan_paired(
    paired: &[(String, u64)],
    protected: &BTreeSet<String>,
    keep_count: usize,
    keep_bytes: u64,
) -> Vec<String> {
    let mut surviving = paired.len();
    let mut bytes: u64 = paired.iter().map(|(_, b)| *b).sum();
    let mut removals = Vec::new();
    for (id, size) in paired {
        if protected.contains(id) {
            continue;
        }
        if surviving <= keep_count && bytes <= keep_bytes {
            break;
        }
        surviving -= 1;
        bytes = bytes.saturating_sub(*size);
        removals.push(id.clone());
    }
    removals
}

struct PairedOutcome {
    evicted_records: usize,
    evicted_backups: usize,
}

/// Evict oldest paired backups (with their operation records) down to the count
/// and byte budgets, then sweep unpaired safety folders and installer copies.
fn enforce(records_dir: &Path, backups_dir: &Path, current_id: &str) -> Result<PairedOutcome> {
    let records = read_records(records_dir);
    let now = now_secs();

    // Every id referenced as a restore source by a surviving record stays put,
    // so the audit chain never dangles.
    let referenced: BTreeSet<String> = records
        .iter()
        .filter_map(|r| r.restored_operation.clone())
        .collect();

    // Resolve each record's backup path once and measure it once.
    let mut sizes: BTreeMap<String, (PathBuf, u64, bool)> = BTreeMap::new();
    for record in &records {
        let Some(backup) = &record.backup else {
            continue;
        };
        let path = paths::migrated_backup_path(backup);
        let exists = path.is_dir();
        let size = if exists { dir_size(&path) } else { 0 };
        sizes.insert(record.id.clone(), (path, size, exists));
    }

    let mut paired: Vec<(String, u64)> = Vec::new(); // (id, bytes), records with a backup slot
    let mut protected: BTreeSet<String> = BTreeSet::new();
    for record in &records {
        let is_protected = is_pending_state(&record.state)
            || record.id == current_id
            || referenced.contains(&record.id);
        if is_protected {
            if sizes.contains_key(&record.id) {
                protected.insert(record.id.clone());
            }
            // Protected records still count toward the budget.
            if let Some((_, size, exists)) = sizes.get(&record.id) {
                if *exists {
                    paired.push((record.id.clone(), *size));
                }
            }
            continue;
        }
        if is_evictable_state(&record.state) {
            // Sort key is `started`; paired is reordered below.
            paired.push((
                record.id.clone(),
                sizes.get(&record.id).map(|(_, s, _)| *s).unwrap_or(0),
            ));
        }
    }

    // Oldest first by start time, so eviction removes the age-old ones.
    let start_of = |id: &str| {
        records
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.started.clone())
            .unwrap_or_default()
    };
    paired.sort_by_key(|(id, _)| start_of(id));

    let evict_ids = plan_paired(&paired, &protected, APPLY_COUNT, APPLY_BYTES);
    let evicted: BTreeSet<String> = evict_ids.iter().cloned().collect();

    // Backup directories still referenced by a surviving record: never swept as
    // "orphans" even when they match an age/count rule.
    let live_backups: BTreeSet<PathBuf> = records
        .iter()
        .filter(|r| !evicted.contains(&r.id))
        .filter_map(|r| r.backup.as_ref().map(|b| paths::migrated_backup_path(b)))
        .filter(|p| p.is_dir())
        .collect();

    let trash_dir = tempfile::Builder::new()
        .prefix(".retention-")
        .tempdir_in(backups_dir)?;
    let mut outcome = PairedOutcome {
        evicted_records: 0,
        evicted_backups: 0,
    };
    for id in &evict_ids {
        if let Some((backup, _, exists)) = sizes.get(id) {
            if *exists {
                move_to_trash(backup, trash_dir.path(), &format!("backup-{id}"));
                outcome.evicted_backups += 1;
            }
        }
        move_to_trash(
            &records_dir.join(format!("{id}.json")),
            trash_dir.path(),
            &format!("record-{id}.json"),
        );
        outcome.evicted_records += 1;
    }

    sweep_unpaired(backups_dir, &live_backups, now);
    sweep_deploy(backups_dir);
    Ok(outcome)
}

/// Remove one-shot safety folders (`pre-import-`, …) beyond count/age, and any
/// `pre-apply-*`/`pre-restore` left without a record (e.g. after a crash) older
/// than the age limit. `live_backups` (referenced by a surviving record) and
/// already-evicted dirs are never touched.
fn sweep_unpaired(backups_dir: &Path, live_backups: &BTreeSet<PathBuf>, now: u64) {
    let Ok(entries) = fs::read_dir(backups_dir) else {
        return;
    };
    let dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && !live_backups.contains(p))
        .collect();

    for prefix in UNPAIRED_PREFIXES {
        sweep_prefix(backups_dir, &dirs, prefix, now, SAFETY_COUNT, true);
    }
    // Orphan pre-apply/pre-restore backups left without a record (e.g. after a
    // crash): age-based only, no count cap.
    sweep_prefix(backups_dir, &dirs, "pre-apply-", now, usize::MAX, true);
    sweep_prefix(backups_dir, &dirs, "pre-restore", now, usize::MAX, true);
}

#[allow(clippy::too_many_arguments)]
fn sweep_prefix(
    backups_dir: &Path,
    dirs: &[PathBuf],
    prefix: &str,
    now: u64,
    keep: usize,
    apply_age: bool,
) {
    let mut matched: Vec<PathBuf> = dirs
        .iter()
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(prefix))
        })
        .cloned()
        .collect();
    if matched.is_empty() {
        return;
    }
    // Newest first by mtime.
    matched.sort_by(|a, b| mtime_secs(b).cmp(&mtime_secs(a)).then_with(|| a.cmp(b)));
    let max_age = SAFETY_MAX_AGE_DAYS * 24 * 60 * 60;
    let trash = tempfile::Builder::new()
        .prefix(".retention-sweep-")
        .tempdir_in(backups_dir);
    for (index, dir) in matched.iter().enumerate() {
        let too_old = apply_age && now.saturating_sub(mtime_secs(dir)) > max_age;
        let beyond = index >= keep;
        if let Ok(trash) = &trash {
            if too_old || beyond {
                let name = dir
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                move_to_trash(dir, trash.path(), &name);
            }
        }
    }
}

fn sweep_deploy(backups_dir: &Path) {
    let Ok(entries) = fs::read_dir(backups_dir) else {
        return;
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("deploy-kolira"))
        })
        .collect();
    if dirs.len() <= DEPLOY_COUNT {
        return;
    }
    dirs.sort_by(|a, b| mtime_secs(b).cmp(&mtime_secs(a)).then_with(|| a.cmp(b)));
    let trash = tempfile::Builder::new()
        .prefix(".retention-deploy-")
        .tempdir_in(backups_dir);
    if let Ok(trash) = trash {
        for dir in dirs.iter().skip(DEPLOY_COUNT) {
            let name = dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            move_to_trash(dir, trash.path(), &name);
        }
    }
}

/// Best-effort retention run after a successful apply. Never fails the apply.
pub(crate) fn run_after_apply(current_id: &str) {
    let _lock = match gui_api::lock_file(&paths::theme_root().join("operation-history.lock")) {
        Ok(lock) => lock,
        Err(_) => return,
    };
    let _ = enforce(
        &paths::theme_root().join("operations"),
        &paths::backups_dir(),
        current_id,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn write_record(dir: &Path, id: &str, state: &str, started: &str, backup: Option<&Path>) {
        let record = operation::Record {
            id: id.into(),
            profile: "p".into(),
            state: state.into(),
            started: started.into(),
            updated: started.into(),
            backup: backup.map(|p| p.to_path_buf()),
            error: None,
            recovery_errors: vec![],
            previous_profile: String::new(),
            source: "cli".into(),
            report: serde_json::Value::Null,
            restored_operation: None,
        };
        fs::write(
            dir.join(format!("{id}.json")),
            serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
    }

    fn make_backup(backups: &Path, name: &str, size: usize) -> PathBuf {
        let dir = backups.join(name);
        fs::create_dir_all(&dir).unwrap();
        // One file of the requested size approximates the backup footprint.
        File::create(dir.join("blob")).unwrap();
        let file = dir.join("blob");
        fs::write(&file, vec![0u8; size]).unwrap();
        dir
    }

    #[test]
    fn paired_plan_evicts_oldest_until_count_and_bytes_met() {
        let paired = vec![
            ("a".to_string(), 100),
            ("b".to_string(), 100),
            ("c".to_string(), 100),
            ("d".to_string(), 100),
        ];
        // keep 2 by count: a, b evicted.
        assert_eq!(
            plan_paired(&paired, &BTreeSet::new(), 2, 10_000),
            vec!["a", "b"]
        );
        // keep all by count but 250 bytes: a, b evicted (oldest first).
        assert_eq!(
            plan_paired(&paired, &BTreeSet::new(), 10, 250),
            vec!["a", "b"]
        );
    }

    #[test]
    fn paired_plan_protects_listed_ids() {
        let paired = vec![
            ("a".to_string(), 100),
            ("b".to_string(), 100),
            ("c".to_string(), 100),
        ];
        let protected: BTreeSet<String> = ["a".to_string()].into();
        // keep 1: a is protected, so b and c are evicted (a still counts).
        assert_eq!(plan_paired(&paired, &protected, 1, 10_000), vec!["b", "c"]);
    }

    #[test]
    fn paired_plan_keeps_everything_under_budget() {
        let paired = vec![("a".to_string(), 100), ("b".to_string(), 100)];
        assert!(plan_paired(&paired, &BTreeSet::new(), 12, 1_000_000).is_empty());
    }

    #[test]
    fn enforce_evicts_old_pair_together_and_protects_current() {
        let root = tempfile::tempdir().unwrap();
        let records = root.path().join("operations");
        let backups = root.path().join("backups");
        fs::create_dir_all(&records).unwrap();
        fs::create_dir_all(&backups).unwrap();

        // 15 succeeded pairs → count cap (12) must evict the 3 oldest.
        let mut ids = Vec::new();
        for i in 0..15 {
            let id = format!("apply-{i:02}");
            let backup = make_backup(&backups, &format!("pre-apply-p-{i:02}"), 1024);
            write_record(
                &records,
                &id,
                "succeeded",
                &format!("2026-01-{:02}T00:00:00Z", i + 1),
                Some(&backup),
            );
            ids.push(id);
        }
        let outcome = enforce(&records, &backups, "apply-99").unwrap();
        assert_eq!(outcome.evicted_records, 3);
        assert_eq!(outcome.evicted_backups, 3);
        // Oldest three gone (record + backup).
        for id in &ids[..3] {
            assert!(
                !records.join(format!("{id}.json")).exists(),
                "{id} record kept"
            );
        }
        for id in &ids[3..] {
            assert!(
                records.join(format!("{id}.json")).exists(),
                "{id} record evicted"
            );
        }
        // Corresponding backups removed for the evicted three.
        assert!(!backups.join("pre-apply-p-00").exists());
        assert!(backups.join("pre-apply-p-03").exists());
    }

    #[test]
    fn enforce_never_evicts_pending_or_current_or_referenced() {
        let root = tempfile::tempdir().unwrap();
        let records = root.path().join("operations");
        let backups = root.path().join("backups");
        fs::create_dir_all(&records).unwrap();
        fs::create_dir_all(&backups).unwrap();

        // Many succeeded pairs plus one pending and the current one.
        for i in 0..15 {
            let id = format!("apply-{i:02}");
            let backup = make_backup(&backups, &format!("pre-apply-p-{i:02}"), 1024);
            let state = if i == 0 { "applying" } else { "succeeded" };
            write_record(
                &records,
                &id,
                state,
                &format!("2026-02-{:02}T00:00:00Z", i + 1),
                Some(&backup),
            );
        }
        let current_backup = make_backup(&backups, "pre-apply-current", 1024);
        write_record(
            &records,
            "apply-current",
            "succeeded",
            "2026-03-01T00:00:00Z",
            Some(&current_backup),
        );

        enforce(&records, &backups, "apply-current").unwrap();

        // Pending and current survive with their backups.
        assert!(records.join("apply-00.json").exists());
        assert!(backups.join("pre-apply-p-00").exists());
        assert!(records.join("apply-current.json").exists());
        assert!(backups.join("pre-apply-current").exists());
    }

    #[test]
    fn unpaired_safety_folders_keep_newest_and_drop_by_age() {
        let root = tempfile::tempdir().unwrap();
        let backups = root.path().join("backups");
        fs::create_dir_all(&backups).unwrap();
        // 12 pre-import dirs: count cap (10) drops the 2 oldest.
        for i in 0..12 {
            fs::create_dir_all(backups.join(format!("pre-import-t{i:02}"))).unwrap();
        }
        sweep_unpaired(&backups, &BTreeSet::new(), now_secs());
        let remaining = fs::read_dir(&backups)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("pre-import-"))
            .count();
        assert_eq!(remaining, 10);
    }

    #[test]
    fn deploy_copies_capped_to_five() {
        let root = tempfile::tempdir().unwrap();
        let backups = root.path().join("backups");
        fs::create_dir_all(&backups).unwrap();
        for i in 0..8 {
            fs::create_dir_all(backups.join(format!("deploy-kolira-2026010{i}"))).unwrap();
        }
        sweep_deploy(&backups);
        let remaining = fs::read_dir(&backups)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("deploy-kolira"))
            .count();
        assert_eq!(remaining, DEPLOY_COUNT);
    }

    #[test]
    fn state_classification_matches_pending_and_evictable() {
        assert!(is_pending_state("applying"));
        assert!(is_pending_state("recovery_failed"));
        assert!(!is_evictable_state("applying"));
        assert!(is_evictable_state("succeeded"));
        assert!(is_evictable_state("rolled_back"));
        // recovery_failed is pending and must not be paired-evicted.
        assert!(!is_evictable_state("recovery_failed"));
    }
}
