//! Managed recovery points. The operation history and safety transactions
//! have their own lifetime.
use crate::{gui_api, operation, paths, profile, theme};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

const COUNT: usize = 20;
const BYTES: u64 = 2 * 1024 * 1024 * 1024;
const DESKTOP_KEYS: [&str; 5] = [
    "gtk-theme",
    "color-scheme",
    "icon-theme",
    "cursor-theme",
    "font-name",
];

#[derive(Clone, Serialize, Deserialize)]
struct Point {
    id: String,
    name: String,
    created: String,
    pinned: bool,
    bytes: u64,
    #[serde(flatten)]
    data: Data,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Data {
    Manual {
        digest: String,
        gtk: BTreeMap<String, String>,
    },
    Automatic {
        profile: String,
        previous_profile: String,
        live: bool,
        wallpaper: bool,
    },
}

fn root() -> PathBuf {
    paths::theme_root().join("restore-points")
}

fn point_dir(id: &str) -> Result<PathBuf> {
    paths::validate_profile_name(id)?;
    ensure!(
        id.starts_with("apply-") || id.starts_with("point-"),
        "Invalid restore point"
    );
    Ok(root().join(id))
}

fn read(dir: &Path) -> Result<Point> {
    ensure!(
        fs::symlink_metadata(dir).is_ok_and(|meta| meta.is_dir()),
        "Restore point is unavailable"
    );
    let path = dir.join("point.json");
    ensure!(!path.is_symlink(), "Invalid restore point metadata");
    let point: Point = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(point_dir(&point.id)? == dir, "Restore point ID mismatch");
    Ok(point)
}

fn write(dir: &Path, point: &Point) -> Result<()> {
    gui_api::write_bytes(
        &dir.join("point.json"),
        &serde_json::to_vec(point)?,
        Some(0o600),
    )
}

fn list() -> Result<Vec<Point>> {
    let root = root();
    if !root.exists() {
        return Ok(vec![]);
    }
    let mut points = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with("point-") || name.starts_with("apply-"))
        {
            points.push(read(&entry.path())?);
        }
    }
    points.sort_by(|a, b| (&b.created, &b.id).cmp(&(&a.created, &a.id)));
    Ok(points)
}

fn name(args: &Value) -> Result<String> {
    let name = gui_api::str_arg(args, "name")?.trim();
    ensure!(
        !name.is_empty() && name.chars().count() <= 120 && !name.chars().any(char::is_control),
        "Invalid restore point name"
    );
    Ok(name.to_owned())
}

fn measure(path: &Path) -> Result<u64> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() {
        return Ok(metadata.len());
    }
    let mut size = 0u64;
    for entry in fs::read_dir(path)? {
        size = size.saturating_add(measure(&entry?.path())?);
    }
    Ok(size)
}

fn expired(points: &[Point], protect: &str, count: usize, bytes: u64) -> Result<Vec<String>> {
    let sum = |items: &[Point]| {
        items
            .iter()
            .fold(0u64, |total, p| total.saturating_add(p.bytes))
    };
    let mut size = sum(points);
    let mut unpinned = points.iter().filter(|p| !p.pinned).count();
    let mut removals = vec![];
    for point in points.iter().rev().filter(|p| !p.pinned && p.id != protect) {
        if unpinned <= count && size <= bytes {
            break;
        }
        unpinned -= 1;
        size = size.saturating_sub(point.bytes);
        removals.push(point.id.clone());
    }
    ensure!(
        size <= bytes || protect.is_empty(),
        "Restore point storage is full. Free space by deleting a point or unpinning an older one."
    );
    Ok(removals)
}

fn remove(id: &str) -> Result<()> {
    let dir = point_dir(id)?;
    read(&dir)?;
    let trash = tempfile::Builder::new()
        .prefix(".removed-")
        .tempdir_in(root())?;
    fs::rename(dir, trash.path().join("data"))?;
    trash.close()?;
    Ok(())
}

fn prepare() -> Result<tempfile::TempDir> {
    let root = root();
    if !root.exists() {
        fs::create_dir_all(&root)?;
    }
    ensure!(
        fs::symlink_metadata(&root)?.is_dir(),
        "Invalid restore point directory"
    );
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    Ok(tempfile::Builder::new()
        .prefix(".staging-")
        .tempdir_in(root)?)
}

fn publish(staged: tempfile::TempDir, mut point: Point) -> Result<Point> {
    point.bytes = measure(staged.path())?;
    let mut points = list()?;
    points.insert(0, point.clone());
    let removals = expired(&points, &point.id, COUNT, BYTES)?;
    write(staged.path(), &point)?;
    let target = point_dir(&point.id)?;
    ensure!(!target.exists(), "Restore point already exists");
    fs::rename(staged.path(), target)?;
    for id in removals {
        remove(&id)?;
    }
    Ok(point)
}

pub(crate) fn record_automatic(record: &operation::Record) -> Result<()> {
    let Some(backup) = &record.backup else {
        return Ok(());
    };
    let staged = prepare()?;
    if !operation::copy_changed_backup(backup, staged.path())? {
        return Ok(());
    }
    publish(
        staged,
        Point {
            id: record.id.clone(),
            name: record.profile.clone(),
            created: record.started.clone(),
            pinned: false,
            bytes: 0,
            data: Data::Automatic {
                profile: record.profile.clone(),
                previous_profile: record.previous_profile.clone(),
                live: record.report["kind"] == "live",
                wallpaper: record.report["wallpaper"] == true,
            },
        },
    )?;
    Ok(())
}

pub(crate) fn rpc(method: &str, args: &Value) -> Result<Value> {
    if method == "list" {
        let points = list()?;
        let mut items = Vec::new();
        for point in points {
            let mut item = serde_json::to_value(point)?;
            let data = item.as_object_mut().context("Invalid point")?;
            data.remove("digest");
            data.remove("gtk");
            items.push(item);
        }
        return Ok(json!({"points":items,"max_count":COUNT,"max_bytes":BYTES}));
    }
    let _lock = operation::MutationGuard::acquire()?;
    if method == "create" {
        let title = name(args)?;
        let staged = prepare()?;
        let id = staged
            .path()
            .file_name()
            .context("Point name")?
            .to_string_lossy()
            .replacen(".staging-", "point-", 1);
        let snapshot = staged.path().join("appearance");
        fs::create_dir(&snapshot)?;
        let rels = profile::resolve_cli_paths(None)?;
        profile::capture_snapshot(&snapshot, &id, &rels, true, true)?;
        theme::build(
            &id,
            &snapshot,
            &theme::SaveOptions {
                description: None,
                include_wallpaper: true,
            },
        )?;
        let digest = crate::preview::source_signature(&snapshot)?;
        let point = publish(
            staged,
            Point {
                id,
                name: title,
                created: chrono::Utc::now().to_rfc3339(),
                pinned: false,
                bytes: 0,
                data: Data::Manual {
                    digest,
                    gtk: operation::gtk_values(DESKTOP_KEYS),
                },
            },
        )?;
        return Ok(json!({"id":point.id}));
    }
    let id = gui_api::str_arg(args, "id")?;
    let dir = point_dir(id)?;
    let mut point = read(&dir)?;
    match method {
        "rename" => {
            point.name = name(args)?;
            write(&dir, &point)?;
        }
        "pin" => {
            point.pinned = args["pinned"].as_bool().context("pinned must be boolean")?;
            write(&dir, &point)?;
            for id in expired(&list()?, "", COUNT, BYTES)? {
                remove(&id)?;
            }
        }
        "delete" => {
            ensure!(
                args["confirmed"] == true,
                "Deleting a restore point requires confirmation"
            );
            remove(id)?;
        }
        "restore" => {
            ensure!(
                args["confirmed"] == true,
                "Restoring appearance requires confirmation"
            );
            return operation::with_source(
                args["source"].as_str().unwrap_or("cli"),
                || match point.data {
                    Data::Manual { digest, gtk } => {
                        let snapshot = dir.join("appearance");
                        ensure!(
                            !snapshot.is_symlink()
                                && crate::preview::source_signature(&snapshot)? == digest,
                            "Restore point data has changed"
                        );
                        profile::restore_appearance_point(id, &snapshot, &gtk)
                    }
                    Data::Automatic {
                        previous_profile,
                        live,
                        wallpaper,
                        ..
                    } => operation::restore_saved_backup(
                        &dir,
                        id,
                        &previous_profile,
                        live,
                        wallpaper,
                    ),
                },
            );
        }
        _ => bail!("Unknown restore point operation"),
    }
    Ok(json!({"id":id}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(id: &str, pinned: bool, bytes: u64) -> Point {
        Point {
            id: id.into(),
            name: String::new(),
            created: String::new(),
            pinned,
            bytes,
            data: Data::Manual {
                digest: String::new(),
                gtk: BTreeMap::new(),
            },
        }
    }

    #[test]
    fn retention_counts_pinned_bytes_and_eviction_protects_new_point() {
        let points = [
            point("new", false, 4),
            point("pinned", true, 5),
            point("old", false, 3),
        ];
        assert_eq!(expired(&points, "new", 1, 10).unwrap(), ["old"]);
        assert!(expired(&points, "new", 1, 8).is_err());
        assert_eq!(expired(&points, "", 20, 8).unwrap(), ["old", "new"]);
    }
}
