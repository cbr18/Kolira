//! Offline theme authoring and versioned workspace instructions.
use crate::{
    field::{self, Base},
    gui_api::{str_arg, write_bytes},
    paths, theme,
};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs};
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn install() -> Result<Value> {
    let _lock = crate::gui_api::lock_file(&paths::theme_root().join("agent-instructions.lock"))?;
    let dir = paths::profiles_dir();
    fs::create_dir_all(&dir)?;
    let manifest = paths::theme_root().join("agent-instructions.json");
    let mut hashes: BTreeMap<String, String> = match fs::read(&manifest) {
        Ok(bytes) => serde_json::from_slice(&bytes).context("Invalid instruction tracking file")?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
        Err(e) => return Err(e.into()),
    };
    let mut results = vec![];
    for (name, text) in [
        ("AGENTS.md", include_str!("../data/theme-agents.md")),
        ("CLAUDE.md", "# kolira workspace\n\n@AGENTS.md\n"),
    ] {
        let path = dir.join(name);
        if path.is_symlink() {
            bail!("Instruction file is a symlink: {}", path.display());
        }
        let old = match fs::read(&path) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        let managed = old
            .as_ref()
            .is_none_or(|b| hashes.get(name) == Some(&digest(b)) || b == text.as_bytes());
        if managed {
            if let Some(bytes) = old.as_ref().filter(|b| b.as_slice() != text.as_bytes()) {
                let backup =
                    crate::profile::unique_dir(&paths::backups_dir(), "agent-instructions")?;
                write_bytes(&backup.join(name), bytes, None)?;
            }
            write_bytes(&path, text.as_bytes(), None)?;
            hashes.insert(name.into(), digest(text.as_bytes()));
            results.push(json!({"file":path,"state":"updated"}));
        } else {
            let candidate = dir.join(format!("{name}.kit-new"));
            write_bytes(&candidate, text.as_bytes(), None)?;
            results
                .push(json!({"file":path,"state":"preserved_user_changes","candidate":candidate}));
        }
    }
    write_bytes(&manifest, &serde_json::to_vec_pretty(&hashes)?, None)?;
    Ok(json!({"version":1,"files":results}))
}
pub fn capabilities() -> Value {
    json!({"api":1,"version":env!("CARGO_PKG_VERSION"),"theme_schema":theme::SCHEMA,
        "offline_prepare":"agent.prepare","schema":"agent.schema","history":"operation.list",
        "restore":"operation.restore","schedule":["schedule.list","schedule.put","schedule.delete","schedule.enable"],
        "prepare_generators":["kde","kitty","alacritty"],"instructions":paths::profiles_dir().join("AGENTS.md"),
        "apply_requires_explicit_user_intent":true,"no_apply_disables_file_writes":false})
}
pub fn schema() -> Value {
    json!({"schema":theme::SCHEMA,"package_schema":serde_json::from_str::<Value>(include_str!("../data/theme-schema.json")).expect("valid package schema"),"fields":field::registry().iter().map(|f| json!({"id":f.id,"label":f.label,"type":f.kind,"source":f.source,"file":f.file,"profile_tracked":f.profile_tracked,"choices":f.choices,"offline":f.profile_tracked && f.source != "kvantum" && !f.file.is_empty()})).collect::<Vec<_>>(),"components":crate::bundle_view::components(),
        "reserved_workspace_paths":["AGENTS.md","AGENTS.override.md","CLAUDE.md","CLAUDE.local.md","AGENTS.md.kit-new","AGENTS.override.md.kit-new","CLAUDE.md.kit-new","CLAUDE.local.md.kit-new"],"name_pattern":"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$","prepare":{"name":"required string","values":"field-id to typed value","description":"optional string","use_palette":"optional bool; uses saved palette without applying"},
        "assets":"Local user resources may be copied into the package; publication requires license review"})
}
pub fn prepare(args: &Value) -> Result<Value> {
    let object = args
        .as_object()
        .context("Preparation arguments must be an object")?;
    if object
        .keys()
        .any(|k| !["name", "values", "description", "use_palette"].contains(&k.as_str()))
    {
        bail!("Unknown preparation argument");
    }
    if let Some(value) = args.get("description") {
        if !value.is_string() {
            bail!("description must be string");
        }
    }

    let name = str_arg(args, "name")?;
    paths::validate_profile_name(name)?;
    let _lock = crate::operation::MutationGuard::acquire()?;
    let target = paths::profile_dir(name)?;
    if fs::symlink_metadata(&target).is_ok() || crate::profile::case_duplicate_of(name).is_some() {
        bail!("Theme already exists: {name}");
    }
    let values = args["values"]
        .as_object()
        .context("values must be an object")?;
    if let Some(value) = args.get("use_palette") {
        if !value.is_boolean() {
            bail!("use_palette must be boolean");
        }
    }
    let mut normalized = vec![];
    for (id, value) in values {
        let f = field::field(id).with_context(|| format!("Unknown field: {id}"))?;
        if !f.profile_tracked || f.source == "kvantum" || f.file.is_empty() {
            bail!("Field cannot be prepared offline: {id}");
        }
        normalized.push((
            f,
            field::normalize_input(f, value).with_context(|| format!("Invalid value for {id}"))?,
        ));
    }
    fs::create_dir_all(paths::profiles_dir())?;
    let temp = tempfile::Builder::new()
        .prefix(".prepare-")
        .tempdir_in(paths::profiles_dir())?;
    let base = Base::Snapshot(temp.path().into());
    let mut files = BTreeMap::new();
    for (f, value) in &normalized {
        let path = field::file_for(f, &base).context("Field has no snapshot path")?;
        let rel = path
            .strip_prefix(temp.path())?
            .to_string_lossy()
            .into_owned();
        let text = files.entry(rel).or_insert_with(String::new);
        *text = field::patched(f, text, value)?;
    }
    for (rel, text) in files {
        let normalized = crate::appearance::normalize(&rel, &text, false)?;
        if normalized.is_empty() {
            bail!("Field file is outside theme appearance policy: {rel}");
        }
        for (rel, text) in normalized {
            write_bytes(&temp.path().join(rel), text.as_bytes(), None)?;
        }
    }
    if args["use_palette"] == true {
        let palette = crate::palette::load(&crate::palette::live_palette_path())?;
        palette.validate()?;
        let scheme = crate::palette::kde_scheme_id(&palette);
        write_bytes(
            &temp
                .path()
                .join(format!(".local/share/color-schemes/{scheme}.colors")),
            crate::palette::kde_color_scheme(&palette).as_bytes(),
            None,
        )?;
        let path = temp.path().join(".config/kdeglobals");
        let existing = fs::read_to_string(&path).unwrap_or_default();
        write_bytes(
            &path,
            field::set_ini_text(&existing, "General", "ColorScheme", &scheme).as_bytes(),
            None,
        )?;
        let kitty = temp.path().join(crate::appearance::KITTY_THEME);
        let existing = fs::read_to_string(&kitty).unwrap_or_default();
        write_bytes(
            &kitty,
            crate::palette::kitty_conf_with_palette(&existing, &palette).as_bytes(),
            None,
        )?;
        let alacritty = temp.path().join(crate::appearance::ALACRITTY_THEME);
        let existing = fs::read_to_string(&alacritty).unwrap_or_default();
        write_bytes(
            &alacritty,
            crate::palette::alacritty_with_palette(&existing, &palette)?.as_bytes(),
            None,
        )?;
        crate::palette::save(
            &palette,
            &temp.path().join(crate::palette::LIVE_PALETTE_REL),
        )?;
    }
    let package = theme::build(
        name,
        temp.path(),
        &theme::SaveOptions {
            description: args["description"].as_str().map(str::to_string),
            include_wallpaper: false,
        },
    )?;
    if package.values.is_empty() && package.assets.is_empty() {
        bail!("No theme appearance values were produced");
    }
    for (f, _) in normalized {
        if !package.values.contains_key(&f.id) {
            bail!("Field was excluded by appearance-only policy: {}", f.id);
        }
    }
    // temp and destination share a filesystem; the package appears atomically.
    fs::rename(temp.path(), &target)?;
    Ok(
        json!({"name":name,"package":package,"path":target,"missing":theme::check(name)?,"live_appearance_changed":false}),
    )
}
