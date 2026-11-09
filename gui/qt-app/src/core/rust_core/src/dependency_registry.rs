//! One local registry for source provenance, dependency requirements and ownership.
//! Legacy path records never imply ownership. Only newly created, fingerprinted
//! resources may be removed, after rechecking their location and contents.
use crate::{
    deps::{self, Dependency},
    paths,
};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Instance {
    pub path: String,
    pub ownership: String,
    #[serde(default)]
    pub fingerprint: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Resource {
    pub id: String,
    pub dependency: Dependency,
    #[serde(default)]
    pub requirements: Vec<Dependency>,
    pub themes: BTreeSet<String>,
    pub application: bool,
    pub instances: Vec<Instance>,
    pub installed: bool,
    pub checked_at: String,
    #[serde(default)]
    pub last_error: String,
    #[serde(default)]
    pub can_install: bool,
    #[serde(default)]
    pub can_remove: bool,
    #[serde(default)]
    pub removal_reason: String,
    #[serde(default)]
    pub package_managed: bool,
    #[serde(default)]
    pub install_method: String,
    #[serde(default)]
    pub remove_method: String,
}

#[derive(Serialize, Deserialize)]
struct Registry {
    schema: u32,
    #[serde(default)]
    sources: BTreeMap<String, Value>,
    #[serde(default)]
    resources: BTreeMap<String, Resource>,
    #[serde(default)]
    python: String,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            schema: 2,
            sources: BTreeMap::new(),
            resources: BTreeMap::new(),
            python: String::new(),
        }
    }
}

fn path() -> PathBuf {
    paths::theme_root().join("installed-assets.json")
}

pub fn atomic_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path.parent().context("registry parent")?;
    fs::create_dir_all(parent)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    if let Ok(meta) = fs::metadata(path) {
        tmp.as_file().set_permissions(meta.permissions())?;
    }
    serde_json::to_writer_pretty(&mut tmp, value)?;
    tmp.write_all(b"\n")?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

pub fn lock(name: &str) -> Result<File> {
    let dir = paths::theme_root().join("dependency-locks");
    fs::create_dir_all(&dir)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join(format!("{}.lock", hex(name.as_bytes()))))?;
    // Bounded contention also makes a GUI refresh fail visibly instead of hanging.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(std::fs::TryLockError::WouldBlock) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(25))
            }
            Err(e) => bail!("dependency registry busy or unavailable: {e}"),
        }
    }
}

fn read() -> Result<Registry> {
    let p = path();
    let bytes = match fs::read(&p) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Registry::default()),
        Err(e) => return Err(e.into()),
    };
    let value: Value = serde_json::from_slice(&bytes)
        .context("invalid installed-assets.json; original left untouched")?;
    if value.get("schema").is_some() {
        let reg: Registry = serde_json::from_value(value)?;
        if reg.schema != 2 {
            bail!("unsupported dependency registry schema {}", reg.schema);
        }
        return Ok(reg);
    }
    let sources = value
        .as_object()
        .context("legacy registry must be an object")?
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let backup = p.with_extension("json.pre-v2");
    if !backup.exists() {
        let mut file = tempfile::NamedTempFile::new_in(p.parent().context("registry parent")?)?;
        file.as_file()
            .set_permissions(fs::metadata(&p)?.permissions())?;
        file.write_all(&bytes)?;
        file.as_file().sync_all()?;
        file.persist_noclobber(backup).map_err(|e| e.error)?;
    }
    Ok(Registry {
        sources,
        ..Registry::default()
    })
}

pub fn source_entry(path: &Path) -> Option<(String, String)> {
    let _guard = lock("registry").ok()?;
    let reg = read().ok()?;
    let entry = reg.sources.get(&path.to_string_lossy().to_string())?;
    Some((
        entry.get("id")?.as_str()?.into(),
        entry
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("")
            .into(),
    ))
}

pub fn record_source(paths: &[PathBuf], id: &str, title: &str) -> Result<()> {
    let _guard = lock("registry")?;
    let mut reg = read()?;
    for p in paths {
        reg.sources.insert(
            p.display().to_string(),
            serde_json::json!({"store":"kde-store", "id":id, "title":title}),
        );
    }
    atomic_json(&path(), &reg)
}

fn hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn resource_id(d: &Dependency) -> String {
    // A single editor extension can provide multiple named themes.
    let identity = if d.id.is_empty() {
        format!("name:{}", d.name)
    } else {
        d.id.clone()
    };
    let identity = if matches!(d.source.as_str(), "vscode-marketplace" | "open-vsx") {
        identity.to_lowercase()
    } else {
        identity
    };
    let source = if matches!(d.source.as_str(), "vscode-marketplace" | "open-vsx") {
        "vsx"
    } else {
        &d.source
    };
    let kind = if matches!(d.source.as_str(), "package" | "aur") {
        "package"
    } else {
        &d.kind
    };
    hex(format!("{source}\0{kind}\0{identity}").as_bytes())[..24].into()
}

/// Immediate entries a provider may create. Existing entries are never adopted.
pub fn inventory(d: &Dependency) -> BTreeSet<PathBuf> {
    let roots = provider_roots(d);
    roots
        .into_iter()
        .flat_map(|root| {
            fs::read_dir(root)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
        })
        .collect()
}

fn provider_roots(d: &Dependency) -> Vec<PathBuf> {
    match d.kind.as_str() {
        "zed" => vec![paths::config_home().join("zed/themes")],
        "vscode" => vec![
            paths::home_dir().join(".vscode/extensions"),
            paths::home_dir().join(".vscode-oss/extensions"),
        ],
        "cursor" => vec![paths::home_dir().join(".cursor/extensions")],
        kind => deps::roots(kind)
            .into_iter()
            .filter(|p| !p.starts_with("/usr"))
            .collect(),
    }
}

fn safe_location(d: &Dependency, path: &Path) -> bool {
    provider_roots(d).iter().any(|root| {
        path.parent() == Some(root.as_path())
            && root.canonicalize().ok().as_deref() == Some(root.as_path())
            && path.file_name().is_some()
            && fs::symlink_metadata(path).is_ok_and(|m| !m.file_type().is_symlink())
    })
}

/// Includes names, bytes, modes and link targets without following symlinks.
pub fn fingerprint(path: &Path) -> Result<String> {
    fn collect(root: &Path, path: &Path, hash: &mut Sha256) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let meta = fs::symlink_metadata(path)?;
        if meta.file_type().is_symlink() {
            hash.update(b"symlink");
            hash.update(path.strip_prefix(root)?.as_os_str().as_encoded_bytes());
            hash.update(fs::read_link(path)?.as_os_str().as_encoded_bytes());
            return Ok(());
        }
        if !(meta.is_file() || meta.is_dir()) {
            bail!("unsupported resource entry {}", path.display());
        }
        hash.update(path.strip_prefix(root)?.as_os_str().as_encoded_bytes());
        hash.update(meta.permissions().mode().to_le_bytes());
        hash.update(if meta.is_dir() {
            b"dir".as_slice()
        } else {
            b"file".as_slice()
        });
        if meta.is_file() {
            let mut f = File::open(path)?;
            std::io::copy(&mut f, hash)?;
        } else {
            let mut children = fs::read_dir(path)?
                .map(|e| e.map(|e| e.path()))
                .collect::<std::io::Result<Vec<_>>>()?;
            children.sort();
            for child in children {
                collect(root, &child, hash)?;
            }
        }
        Ok(())
    }
    let mut hash = Sha256::new();
    collect(path, path, &mut hash)?;
    Ok(format!("{:x}", hash.finalize()))
}

fn add_requirement(reg: &mut Registry, d: &Dependency, theme: Option<&str>, app: bool) {
    let id = resource_id(d);
    let r = reg.resources.entry(id.clone()).or_insert_with(|| Resource {
        id,
        dependency: d.clone(),
        ..Resource::default()
    });
    if theme.is_some() || app {
        r.dependency = d.clone();
    }
    if !r
        .requirements
        .iter()
        .any(|x| x.name == d.name && x.kind == d.kind)
    {
        r.requirements.push(d.clone());
    }
    if let Some(name) = theme {
        r.themes.insert(name.into());
    }
    r.application |= app;
}

fn is_installed(d: &Dependency) -> bool {
    if d.source == "runtime" {
        return match d.kind.as_str() {
            "python" => crate::paths::output_bounded(std::process::Command::new(&d.id).args(["-c", "import sys; assert sys.version_info >= (3,11)"])).is_some_and(|(s,_)| s.success()),
            "pyside6" => crate::paths::output_bounded(std::process::Command::new(&d.id).args(["-c", "import PySide6; from PySide6.QtCore import QVersionNumber; assert QVersionNumber.compare(QVersionNumber.fromString(PySide6.__version__), QVersionNumber(6,7)) >= 0"])).is_some_and(|(s,_)| s.success()),
            "qml-module" => qml_module_path(&d.id).is_some(),
            _ => which::which(&d.name).is_ok(),
        };
    }
    deps::installed(d)
}

fn qml_module_path(id: &str) -> Option<PathBuf> {
    let mut roots: Vec<PathBuf> = ["QML_IMPORT_PATH", "QML2_IMPORT_PATH"]
        .iter()
        .filter_map(std::env::var_os)
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .collect();
    roots.extend([
        PathBuf::from("/usr/lib/qt6/qml"),
        PathBuf::from("/usr/lib64/qt6/qml"),
        PathBuf::from("/usr/share/qt6/qml"),
    ]);
    roots
        .into_iter()
        .map(|p| p.join(id.replace('.', "/")).join("qmldir"))
        .find(|p| p.is_file())
}

fn update_resource(r: &mut Resource) {
    r.installed = r.requirements.iter().all(is_installed);
    r.checked_at = chrono::Utc::now().to_rfc3339();
    r.instances.retain(|i| Path::new(&i.path).exists());
    for d in &r.requirements {
        let mut located = deps::locate_all(&d.kind, &d.name);
        if matches!(d.kind.as_str(), "python" | "pyside6") && Path::new(&d.id).exists() {
            located.push(PathBuf::from(&d.id));
        }
        if d.kind == "qml-module" {
            located.extend(qml_module_path(&d.id));
        }
        if d.kind == "command" {
            located.extend(which::which(&d.name).ok());
        }
        if d.kind == "zed" {
            located.extend(crate::theme::zed_theme_file_pub(&d.name));
        }
        if matches!(d.kind.as_str(), "vscode" | "cursor") {
            located.extend(inventory(d).into_iter().filter(|p| {
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase()
                    .starts_with(&format!("{}-", d.id.to_lowercase()))
            }));
        }
        for p in located {
            if !r
                .instances
                .iter()
                .any(|i| i.path == p.display().to_string())
            {
                r.instances.push(Instance {
                    path: p.display().to_string(),
                    ownership: "external".into(),
                    fingerprint: String::new(),
                });
            }
        }
    }
    r.can_install = !r.application
        && !r.dependency.id.is_empty()
        && matches!(
            r.dependency.source.as_str(),
            "kde-store" | "zed" | "vscode-marketplace" | "open-vsx" | "package" | "aur"
        );
    let owned: Vec<_> = r
        .instances
        .iter()
        .filter(|i| i.ownership == "managed")
        .collect();
    r.can_remove = !r.application
        && !owned.is_empty()
        && owned.iter().all(|i| {
            safe_location(&r.dependency, Path::new(&i.path))
                && fingerprint(Path::new(&i.path)).is_ok_and(|f| f == i.fingerprint)
        });
    if r.package_managed && deps::needs_root(&r.dependency) && package_present(&r.dependency.id) {
        r.can_remove = !r.application;
    }
    r.removal_reason = if r.can_remove {
        ""
    } else if r.application {
        "Required by the application"
    } else if owned.is_empty() {
        "No verified manager-owned files"
    } else {
        "Files changed or location is unsafe"
    }
    .into();
    r.install_method = match r.dependency.source.as_str() {
        "runtime" => "installer",
        "package" | "aur" => "package-manager",
        "vscode-marketplace" | "open-vsx" => "editor-cli",
        "zed" => "zed-user-themes",
        "kde-store" => "kde-store",
        _ => "manual",
    }
    .into();
    r.remove_method = if !r.can_remove {
        "protected"
    } else if deps::needs_root(&r.dependency) {
        "package-manager"
    } else if matches!(r.dependency.kind.as_str(), "vscode" | "cursor") {
        "editor-cli"
    } else {
        "managed-files"
    }
    .into();
}

pub fn refresh(python: Option<&str>) -> Result<Vec<Resource>> {
    crate::dependency_jobs::recover()?;
    let _guard = lock("registry")?;
    let mut reg = read()?;
    if let Some(p) = python {
        reg.python = p.into();
    }
    for r in reg.resources.values_mut() {
        r.themes.clear();
        r.application = false;
        r.requirements.clear();
    }
    let root = paths::profiles_dir();
    if root.is_dir() {
        for entry in fs::read_dir(root)?.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !entry.path().join(crate::theme::MANIFEST).is_file() {
                continue;
            }
            let package =
                crate::theme::load(&entry.path()).with_context(|| format!("theme {name}"))?;
            for d in &package.dependencies {
                add_requirement(&mut reg, d, Some(&name), false);
            }
        }
    }
    if !reg.python.is_empty() {
        for (kind, name) in [("python", "Python"), ("pyside6", "PySide6")] {
            let d = Dependency {
                kind: kind.into(),
                name: name.into(),
                source: "runtime".into(),
                id: reg.python.clone(),
                title: name.into(),
                exact: true,
                ..Dependency::default()
            };
            add_requirement(&mut reg, &d, None, true);
        }
        for name in [
            "qdbus6",
            "kreadconfig6",
            "kwriteconfig6",
            "plasma-apply-wallpaperimage",
            "kpackagetool6",
            "magick",
        ] {
            let d = Dependency {
                kind: "command".into(),
                name: name.into(),
                source: "runtime".into(),
                id: name.into(),
                title: name.into(),
                exact: true,
                ..Dependency::default()
            };
            add_requirement(&mut reg, &d, None, true);
        }
        for name in ["org.kde.kirigami", "org.kde.desktop"] {
            let d = Dependency {
                kind: "qml-module".into(),
                name: name.into(),
                source: "runtime".into(),
                id: name.into(),
                title: name.into(),
                exact: true,
                ..Dependency::default()
            };
            add_requirement(&mut reg, &d, None, true);
        }
    }
    // Retain historical resources for reinstall, without retaining stale theme links.
    for r in reg.resources.values_mut() {
        if r.requirements.is_empty() {
            r.requirements.push(r.dependency.clone());
        }
        update_resource(r);
    }
    atomic_json(&path(), &reg)?;
    Ok(reg.resources.into_values().collect())
}

pub fn get(id: &str) -> Result<Resource> {
    refresh(None)?
        .into_iter()
        .find(|r| r.id == id)
        .context("unknown dependency id")
}

pub fn record_result(
    d: &Dependency,
    before: &BTreeSet<PathBuf>,
    result: &Result<String>,
) -> Result<()> {
    let after = inventory(d);
    let _guard = lock("registry")?;
    let mut reg = read()?;
    add_requirement(&mut reg, d, None, false);
    let r = reg
        .resources
        .get_mut(&resource_id(d))
        .context("resource missing")?;
    r.last_error = result
        .as_ref()
        .err()
        .map(|e| format!("{e:#}"))
        .unwrap_or_default();
    let expected = deps::locate_all(&d.kind, &d.name);
    let editor_preexisting = matches!(d.kind.as_str(), "vscode" | "cursor")
        && before.iter().any(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
                .starts_with(&format!("{}-", d.id.to_lowercase()))
        });
    for p in after.difference(before).filter(|p| {
        let name = p.file_name().unwrap_or_default().to_string_lossy();
        match d.kind.as_str() {
            "zed" => name.starts_with(&format!("{}--", d.id)),
            "vscode" | "cursor" => name
                .to_lowercase()
                .starts_with(&format!("{}-", d.id.to_lowercase())),
            _ => expected.contains(p),
        }
    }) {
        if safe_location(d, p) {
            if let Ok(f) = fingerprint(p) {
                if !r
                    .instances
                    .iter()
                    .any(|i| i.path == p.display().to_string())
                {
                    r.instances.push(Instance {
                        path: p.display().to_string(),
                        ownership: if editor_preexisting {
                            "external"
                        } else {
                            "managed"
                        }
                        .into(),
                        fingerprint: f,
                    });
                }
            }
        }
    }
    update_resource(r);
    atomic_json(&path(), &reg)
}

pub fn remove(id: &str, confirm_shared: bool) -> Result<String> {
    let _resource_guard = lock(id)?;
    remove_unlocked(id, confirm_shared)
}

fn remove_unlocked(id: &str, confirm_shared: bool) -> Result<String> {
    let r = get(id)?;
    if !r.can_remove {
        bail!("{}", r.removal_reason);
    }
    if !r.themes.is_empty() && !confirm_shared {
        bail!(
            "confirm affected themes: {}",
            r.themes.iter().cloned().collect::<Vec<_>>().join(", ")
        );
    }
    if deps::needs_root(&r.dependency) {
        if !r.package_managed || !safe_package(&r.dependency.id) {
            bail!("external package is protected");
        }
        deps::run_in_terminal(&format!("sudo pacman -R {}", r.dependency.id))?;
        if package_present(&r.dependency.id) {
            bail!("package is still installed");
        }
        refresh(None)?;
        return Ok(format!("Removed {}", r.dependency.id));
    }
    let owned: Vec<_> = r
        .instances
        .iter()
        .filter(|i| i.ownership == "managed")
        .collect();
    // Check the whole removal set before changing anything.
    for i in &owned {
        let p = Path::new(&i.path);
        if !safe_location(&r.dependency, p) || fingerprint(p)? != i.fingerprint {
            bail!("resource changed: {}", p.display());
        }
    }
    if matches!(r.dependency.kind.as_str(), "vscode" | "cursor") {
        crate::stores::uninstall(&r.dependency.kind, &r.dependency.id)?;
        if owned.iter().any(|i| Path::new(&i.path).exists()) {
            bail!("editor reported success but managed extension files remain");
        }
    } else {
        for i in owned {
            let p = Path::new(&i.path);
            if p.is_dir() {
                fs::remove_dir_all(p)?;
            } else {
                fs::remove_file(p)?;
            }
        }
    }
    refresh(None)?;
    Ok(format!("Removed {}", r.dependency.name))
}

/// Keep a recoverable copy before reinstalling a user resource. A failed
/// provider must not turn a working installation into an absent one.
pub fn reinstall(id: &str, confirmed: bool) -> Result<String> {
    let _guard = lock(id)?;
    let original = get(id)?;
    if !original.can_remove {
        bail!("{}", original.removal_reason);
    }
    if deps::needs_root(&original.dependency) {
        let command = deps::root_command(&[&original.dependency])
            .context("no package manager")?
            .replace("--needed ", "");
        deps::run_in_terminal(&command)?;
        if !deps::installed(&original.dependency) {
            bail!("package is still missing after reinstall");
        }
        return Ok(format!("Reinstalled {}", original.dependency.name));
    }
    fs::create_dir_all(paths::backups_dir())?;
    let backup = tempfile::Builder::new()
        .prefix("dependency-reinstall-")
        .tempdir_in(paths::backups_dir())?
        .keep();
    let owned: Vec<_> = original
        .instances
        .iter()
        .filter(|i| i.ownership == "managed")
        .collect();
    for (index, instance) in owned.iter().enumerate() {
        crate::profile::copy_tree(Path::new(&instance.path), &backup.join(index.to_string()))?;
    }
    // A durable description makes an interrupted reinstall recoverable too.
    atomic_json(&backup.join("resource.json"), &original)?;
    remove_unlocked(id, confirmed)?;
    let before = inventory(&original.dependency);
    let result = deps::install_unrecorded(&original.dependency).and_then(|message| {
        if !original.requirements.iter().all(is_installed) {
            bail!("some required variants are still missing");
        }
        Ok(message)
    });
    record_result(&original.dependency, &before, &result)?;
    if let Err(error) = result {
        // Only remove freshly created files whose fingerprints still match
        // the failed attempt. External/changed paths are never overwritten.
        let failed = get(id)?;
        if failed.can_remove {
            remove_unlocked(id, true)?;
        }
        for (index, instance) in owned.iter().enumerate() {
            let target = Path::new(&instance.path);
            if target.exists() || target.is_symlink() {
                bail!(
                    "reinstall failed: {error:#}; recovery files kept at {}",
                    backup.display()
                );
            }
            crate::profile::copy_tree(&backup.join(index.to_string()), target)?;
        }
        let _registry_guard = lock("registry")?;
        let mut reg = read()?;
        let mut restored = original;
        restored.last_error = format!("Reinstall failed; previous files restored: {error:#}");
        update_resource(&mut restored);
        reg.resources.insert(id.into(), restored);
        atomic_json(&path(), &reg)?;
        bail!("reinstall failed; previous files restored: {error:#}");
    }
    fs::remove_dir_all(&backup)?;
    result
}

/// Recover the saved installation after its worker was cancelled or died.
/// Any path that now contains different bytes is left for explicit recovery.
pub fn recover_reinstall(id: &str) -> Result<String> {
    let _guard = lock(id)?;
    let mut candidates = Vec::new();
    for entry in fs::read_dir(paths::backups_dir())?.flatten() {
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with("dependency-reinstall-")
        {
            continue;
        }
        let manifest = entry.path().join("resource.json");
        let Ok(bytes) = fs::read(&manifest) else {
            continue;
        };
        let Ok(resource) = serde_json::from_slice::<Resource>(&bytes) else {
            continue;
        };
        if resource.id == id {
            candidates.push((fs::metadata(&manifest)?.modified()?, entry.path(), resource));
        }
    }
    candidates.sort_by_key(|(time, _, _)| *time);
    let (_, backup, mut original) = candidates
        .pop()
        .context("reinstall recovery copy was not created")?;
    let owned: Vec<_> = original
        .instances
        .iter()
        .filter(|i| i.ownership == "managed")
        .collect();
    for (index, instance) in owned.iter().enumerate() {
        let target = Path::new(&instance.path);
        let allowed = provider_roots(&original.dependency).iter().any(|root| {
            target.parent() == Some(root.as_path())
                && root.canonicalize().ok().as_deref() == Some(root.as_path())
        });
        if !allowed {
            bail!("unsafe recovery path; backup kept at {}", backup.display());
        }
        if target.exists() || target.is_symlink() {
            if !safe_location(&original.dependency, target)
                || fingerprint(target)? != instance.fingerprint
            {
                bail!("resource changed; backup kept at {}", backup.display());
            }
        } else {
            let saved = backup.join(index.to_string());
            if fingerprint(&saved)? != instance.fingerprint {
                bail!("recovery copy changed: {}", backup.display());
            }
            crate::profile::copy_tree(&saved, target)?;
        }
    }
    let _reg_guard = lock("registry")?;
    let mut reg = read()?;
    original.last_error = "Reinstall interrupted; previous files restored".into();
    update_resource(&mut original);
    reg.resources.insert(id.into(), original);
    atomic_json(&path(), &reg)?;
    Ok("Previous files restored".into())
}

fn safe_package(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"@._+-".contains(&b))
}
pub(crate) fn package_present(id: &str) -> bool {
    safe_package(id)
        && paths::output_bounded(
            std::process::Command::new("pacman")
                .args(["-Q", id])
                .stderr(std::process::Stdio::null()),
        )
        .is_some_and(|(s, _)| s.success())
}
pub fn install_package(d: &Dependency) -> Result<String> {
    if std::env::var("KTM_THEME_NO_APPLY").as_deref() == Ok("1") {
        bail!("installation disabled by KTM_THEME_NO_APPLY=1");
    }
    let _guard = lock(&resource_id(d))?;
    let existed = package_present(&d.id);
    let command = deps::root_command(&[d]).context("no supported package manager")?;
    let result = deps::run_in_terminal(&command).and_then(|term| {
        if !package_present(&d.id) || !deps::installed(d) {
            bail!("package command finished but {} is still missing", d.name);
        }
        Ok(format!("Installed {} in {term}", d.name))
    });
    record_package_result(d, existed, &result)?;
    result
}

pub(crate) fn record_package_result(
    d: &Dependency,
    existed: bool,
    result: &Result<String>,
) -> Result<()> {
    let _reg_guard = lock("registry")?;
    let mut reg = read()?;
    add_requirement(&mut reg, d, None, false);
    let r = reg
        .resources
        .get_mut(&resource_id(d))
        .context("missing resource")?;
    r.package_managed |= result.is_ok() && !existed;
    r.last_error = result
        .as_ref()
        .err()
        .map(|e| format!("{e:#}"))
        .unwrap_or_default();
    update_resource(r);
    atomic_json(&path(), &reg)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ids_share_editor_extension_but_not_target_application() {
        let d = Dependency {
            kind: "vscode".into(),
            source: "open-vsx".into(),
            id: "Publisher.Theme".into(),
            name: "Dark".into(),
            ..Dependency::default()
        };
        let mut other = d.clone();
        other.source = "vscode-marketplace".into();
        other.id = "publisher.theme".into();
        other.name = "Light".into();
        assert_eq!(resource_id(&d), resource_id(&other));
        other.kind = "cursor".into();
        assert_ne!(resource_id(&d), resource_id(&other));
    }
    #[test]
    fn fingerprints_detect_changes_and_hash_links_without_following_them() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a"), "first").unwrap();
        let first = fingerprint(dir.path()).unwrap();
        fs::write(dir.path().join("a"), "second").unwrap();
        assert_ne!(first, fingerprint(dir.path()).unwrap());
        let before = fingerprint(dir.path()).unwrap();
        std::os::unix::fs::symlink("/does-not-exist", dir.path().join("link")).unwrap();
        assert_ne!(before, fingerprint(dir.path()).unwrap());
    }
}
