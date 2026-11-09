//! JSON transport for GUI/MCP adapters. All application operations stay in Rust.
use crate::{custom_components, paths, tracked_apps, wallpaper};
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
    path::Path,
    process::ExitCode,
};

pub fn write_bytes(path: &Path, bytes: &[u8], mode: Option<u32>) -> Result<()> {
    let parent = path.parent().context("missing parent")?;
    fs::create_dir_all(parent)?;
    let permissions = mode.unwrap_or_else(|| {
        fs::metadata(path)
            .map(|m| m.permissions().mode() & 0o7777)
            .unwrap_or(0o600)
    });
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.as_file()
        .set_permissions(fs::Permissions::from_mode(permissions))?;
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

pub fn lock_file(path: &Path) -> Result<fs::File> {
    fs::create_dir_all(path.parent().context("lock parent")?)?;
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(std::fs::TryLockError::WouldBlock) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(10))
            }
            Err(e) => bail!("operation busy or unavailable: {e}"),
        }
    }
}

pub fn str_arg<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing string {key}"))
}

pub fn settings_defaults() -> Value {
    json!({"default_components_mode":"all","custom_components":[],"wallpaper_linked_to_profile":false,"wallpaper_directory":"","theme":"system","show_not_installed_warning":true,"network_enabled":true,"auto_apply_on_startup":false,"minimize_to_tray":false,"global_hotkey":"","confirm_before_apply":true,"create_backup_before_apply":true,"notify_on_success":true,"core_backend":"core","disabled_components":[]})
}
pub fn settings() -> Value {
    let mut result = settings_defaults();
    if let Ok(bytes) = fs::read(paths::settings_file()) {
        if let Ok(Value::Object(values)) = serde_json::from_slice(&bytes) {
            for (key, value) in values {
                result[&key] = value;
            }
        }
    }
    let backend = result["core_backend"]
        .as_str()
        .unwrap_or("core")
        .trim()
        .to_lowercase();
    result["core_backend"] = json!(if backend == "auto" { "auto" } else { "core" });
    result
}

pub fn component_selection(value: &Value) -> Result<Option<Vec<String>>> {
    if value.is_null() {
        return Ok(None);
    }
    let values = value.as_array().context("components must be an array")?;
    if values.is_empty() {
        bail!("Select at least one component");
    }
    let known = custom_components::merged_component_defs();
    let mut out = vec![];
    for value in values {
        let id = value.as_str().context("Invalid component id")?;
        if !known.iter().any(|app| app.id == id) {
            bail!("Unknown component: {id}");
        }
        if !out.iter().any(|v| v == id) {
            out.push(id.to_string());
        }
    }
    Ok(Some(out))
}

pub fn request(method: &str, args: &Value) -> Result<Value> {
    match method {
        "agent.capabilities" => Ok(crate::agent::capabilities()),
        "agent.schema" => Ok(crate::agent::schema()),
        "agent.prepare" => crate::agent::prepare(args),
        "agent.install" => crate::agent::install(),
        "dependency.list" => Ok(serde_json::to_value(crate::dependency_registry::refresh(
            None,
        )?)?),
        "dependency.start" => {
            let ids = args["ids"]
                .as_array()
                .context("ids must be an array")?
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_string)
                        .context("dependency id must be string")
                })
                .collect::<Result<Vec<_>>>()?;
            crate::dependency_jobs::start(
                str_arg(args, "action")?,
                &ids,
                args["confirmed"].as_bool().unwrap_or(false),
            )
        }
        "schedule.list" | "schedule.put" | "schedule.delete" | "schedule.enable" => {
            crate::schedule::rpc(method.strip_prefix("schedule.").unwrap(), args)
        }
        "operation.restore" => {
            crate::operation::with_source(args["source"].as_str().unwrap_or("cli"), || {
                crate::operation::restore_completed(
                    str_arg(args, "id")?,
                    args["confirmed"].as_bool().unwrap_or(false),
                )
            })
        }
        "operation.list" => crate::operation::list(),
        "operation.recover" => Ok(serde_json::to_value(crate::operation::recover(
            str_arg(args, "id")?,
            args["force"].as_bool().unwrap_or(false),
        )?)?),
        "registry.descriptions" => Ok(json!(tracked_apps::get_tracked_apps()
            .into_iter()
            .map(|app| app.description)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>())),

        "profile.selection" => Ok(json!(
            component_selection(&args["components"])?.map(|ids| ids.join(","))
        )),

        "desktop.load_blur" => {
            if !crate::field::no_apply() {
                paths::output_bounded_for(
                    std::process::Command::new("qdbus6").args([
                        "org.kde.KWin",
                        "/Effects",
                        "org.kde.kwin.Effects.loadEffect",
                        "blur",
                    ]),
                    std::time::Duration::from_secs(2),
                );
            }
            Ok(Value::Null)
        }
        "report.export" => {
            let file = Path::new(str_arg(args, "file")?);
            write_bytes(
                file,
                (serde_json::to_string_pretty(&args["data"])? + "\n").as_bytes(),
                None,
            )?;
            Ok(json!(file))
        }
        "profile.clear_stale" => Ok(json!(crate::profile::clear_stale_active(str_arg(
            args, "name"
        )?)?)),

        "desktop.recover" => {
            crate::profile::recover_desktop_shell();
            Ok(json!({"message":"Desktop recovery requested"}))
        }

        "wallpaper.directory" => Ok(json!(wallpaper::gui_wallpaper_dir())),

        "profile.summaries" => crate::bundle_view::summaries(args),
        "bundle.components" => Ok(json!(crate::bundle_view::components())),
        "bundle.metadata" => crate::bundle_view::metadata(str_arg(args, "name")?),
        "bundle.validate" => crate::bundle_view::validate(str_arg(args, "name")?),
        "profile.save_before_apply" => {
            let _lock = crate::operation::MutationGuard::acquire()?;
            let name = str_arg(args, "name")?;
            let path = paths::profile_dir(name)?;
            if fs::symlink_metadata(&path).is_ok()
                || crate::profile::case_duplicate_of(name).is_some()
            {
                bail!("A theme with this name already exists");
            }
            let options = crate::theme::SaveOptions {
                description: None,
                include_wallpaper: true,
            };
            crate::theme::save(name, &options)?;
            Ok(json!({"name":name}))
        }
        "profile.preview" => crate::preview::build(args),
        "profile.apply" => crate::bundle_view::apply(args),
        "profile.is_package" => Ok(json!(crate::theme::is_package(&paths::profile_dir(
            str_arg(args, "name")?
        )?))),

        "paths.migrate" => Ok(json!(paths::theme_root())),
        "profile.list" => Ok(serde_json::to_value(crate::profile::list_profiles()?)?),
        "profile.info" => Ok(serde_json::to_value(crate::profile::profile_info(
            str_arg(args, "name")?,
        )?)?),

        "field.choices" => Ok(json!(crate::inspector_view::choices(str_arg(args, "id")?)?)),
        "field.fonts" => Ok(json!(crate::inspector_view::fonts())),
        "field.validate" => crate::inspector_view::validate(),
        "profile.active" => Ok(json!(crate::profile::get_active_profile()?)),
        "field.reads" => {
            let base = if let Some(path) = args["base"].as_str() {
                crate::field::Base::Snapshot(path.into())
            } else {
                crate::field::Base::Live
            };
            let package = if let crate::field::Base::Snapshot(path) = &base {
                if crate::theme::is_package(path) {
                    Some(crate::theme::load(path)?)
                } else {
                    None
                }
            } else {
                None
            };
            let mut values = json!({});
            for f in crate::field::registry() {
                values[&f.id] = if let Some(p) = &package {
                    p.values.get(&f.id).cloned().unwrap_or(Value::Null)
                } else {
                    crate::field::read_raw(f, &base).unwrap_or(Value::Null)
                };
            }
            Ok(values)
        }

        "resource.clone" => crate::resource_clone::create(args),
        method if method.starts_with("app.") => crate::app_themes::rpc(&method[4..], args),
        method if method.starts_with("inspector.") => {
            crate::inspector_session::rpc(&method[10..], args)
        }
        method if method.starts_with("config.") => crate::config_edit::rpc(&method[7..], args),
        "field.read" => {
            let f = crate::field::field(str_arg(args, "id")?).context("Unknown field")?;
            let base = if let Some(path) = args["base"].as_str() {
                crate::field::Base::Snapshot(path.into())
            } else {
                crate::field::Base::Live
            };
            if let crate::field::Base::Snapshot(dir) = &base {
                if crate::theme::is_package(dir) {
                    return Ok(crate::theme::load(dir)?
                        .values
                        .get(&f.id)
                        .cloned()
                        .unwrap_or(Value::Null));
                }
            }
            Ok(crate::field::read_raw(f, &base).unwrap_or(Value::Null))
        }
        "field.file" => {
            let f = crate::field::field(str_arg(args, "id")?).context("Unknown field")?;
            let base = if let Some(path) = args["base"].as_str() {
                crate::field::Base::Snapshot(path.into())
            } else {
                crate::field::Base::Live
            };
            Ok(json!(crate::field::file_for(f, &base)))
        }
        "field.kvantum_file" => {
            let base = if let Some(path) = args["base"].as_str() {
                crate::field::Base::Snapshot(path.into())
            } else {
                crate::field::Base::Live
            };
            Ok(json!(crate::field::kvantum_file(
                str_arg(args, "theme")?,
                &base
            )))
        }
        "settings.defaults" => Ok(settings_defaults()),
        "settings.get" => Ok(settings()),
        "settings.set" => {
            let _lock = lock_file(&paths::theme_root().join("preferences.lock"))?;
            let update = args
                .get("settings")
                .and_then(Value::as_object)
                .context("settings must be an object")?;
            let mut current = settings();
            let defaults = settings_defaults();
            for (key, value) in update {
                if let Some(default) = defaults.get(key) {
                    let valid = match default {
                        Value::Bool(_) => value.is_boolean(),
                        Value::String(_) => value.is_string(),
                        Value::Array(_) => value
                            .as_array()
                            .is_some_and(|items| items.iter().all(Value::is_string)),
                        _ => false,
                    };
                    if !valid {
                        bail!("Invalid preference type: {key}");
                    }
                    current[key] = value.clone();
                }
            }
            let backend = current["core_backend"]
                .as_str()
                .unwrap_or("core")
                .trim()
                .to_lowercase();
            current["core_backend"] = json!(if backend == "auto" { "auto" } else { "core" });
            write_bytes(
                &paths::settings_file(),
                &(serde_json::to_string_pretty(&current)? + "\n").into_bytes(),
                Some(0o600),
            )?;
            Ok(current)
        }
        "registry.paths" => {
            let id = str_arg(args, "id")?;
            let defs = custom_components::merged_component_defs();
            Ok(json!(defs
                .iter()
                .find(|a| a.id == id)
                .map(|a| a.config_paths.clone())
                .unwrap_or_default()))
        }
        "registry.profile" => Ok(json!(match str_arg(args, "id")? {
            "zen" => tracked_apps::zen_profile_dir(),
            "firefox" => tracked_apps::firefox_profile_dir(),
            _ => None,
        })),
        "registry.detect" => {
            let def: tracked_apps::TrackedAppDef =
                serde_json::from_value(args["definition"].clone())?;
            Ok(serde_json::to_value(tracked_apps::detect_app_status(&def))?)
        }
        "registry.command" => Ok(json!(str_arg(args, "command")?.split('|').any(|c| c
            .split_whitespace()
            .next()
            .is_some_and(|bin| which::which(bin).is_ok())))),
        "registry.configs_exist" => Ok(json!(args["paths"]
            .as_array()
            .context("paths must be an array")?
            .iter()
            .filter_map(Value::as_str)
            .any(|p| paths::live_path(p).exists()))),
        "registry.custom" => Ok(serde_json::to_value(
            custom_components::load_custom_components().unwrap_or_default(),
        )?),
        "registry.definitions" => {
            let mut defs = tracked_apps::get_tracked_apps();
            if args["custom"].as_bool().unwrap_or(false) {
                defs = custom_components::merged_component_defs();
            }
            let customs = custom_components::load_custom_components().unwrap_or_default();
            let builtin_ids: std::collections::BTreeSet<_> = tracked_apps::get_tracked_apps()
                .into_iter()
                .map(|a| a.id)
                .collect();
            let values: Vec<Value> = defs
                .into_iter()
                .map(|app| {
                    let mut item = serde_json::to_value(&app).unwrap();
                    if let Some(c) = customs
                        .iter()
                        .find(|c| c.id == app.id && !builtin_ids.contains(&c.id))
                    {
                        item["is_custom"] = json!(true);
                        item["reload_command"] =
                            json!(c.reload_command.clone().unwrap_or_default());
                    }
                    item
                })
                .collect();
            Ok(json!(values))
        }
        "registry.components" => {
            let components = tracked_apps::detect_all_apps()?;
            let customs = custom_components::load_custom_components().unwrap_or_default();
            let builtin_ids: std::collections::BTreeSet<_> = tracked_apps::get_tracked_apps()
                .into_iter()
                .map(|a| a.id)
                .collect();
            let values: Vec<Value> = components
                .into_iter()
                .map(|app| {
                    let mut item = serde_json::to_value(&app).unwrap();
                    if let Some(c) = customs
                        .iter()
                        .find(|c| c.id == app.id && !builtin_ids.contains(&c.id))
                    {
                        item["is_custom"] = json!(true);
                        item["reload_command"] =
                            json!(c.reload_command.clone().unwrap_or_default());
                    }
                    item
                })
                .collect();
            Ok(json!(values))
        }
        "wallpaper.list" => Ok(json!(wallpaper::list_wallpapers_in(Path::new(str_arg(
            args,
            "directory"
        )?))?)),
        "wallpaper.current" => {
            let plasma = args["plasma"]
                .as_str()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| paths::live_path(wallpaper::APPLETSRC_REL));
            if let Some(path) = wallpaper::plasma_wallpaper_from(&plasma) {
                return Ok(json!(path));
            }
            let dir = args["directory"]
                .as_str()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(wallpaper::wallpaper_dir);
            let files = wallpaper::list_wallpapers_in(&dir).unwrap_or_default();
            let state = args["state"]
                .as_str()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| paths::cache_home().join("wallpaper_index"));
            let index = fs::read_to_string(state)
                .ok()
                .and_then(|s| s.trim().parse::<usize>().ok());
            Ok(index
                .and_then(|i| files.get(i).cloned())
                .map(Value::from)
                .unwrap_or(Value::Null))
        }
        "wallpaper.set" => {
            wallpaper::apply_image(str_arg(args, "path")?)?;
            Ok(json!({"message":"Wallpaper set"}))
        }

        _ => bail!("unknown core method: {method}"),
    }
}

pub fn cli(method: Option<&String>) -> ExitCode {
    let result = (|| {
        let method = method.context("core METHOD requires a method")?;
        let mut text = String::new();
        std::io::stdin().take(32 << 20).read_to_string(&mut text)?;
        let args: Value = serde_json::from_str(&text).context("invalid core request JSON")?;
        if !args.is_object() {
            bail!("core arguments must be an object");
        }
        request(method, &args)
    })();
    match result {
        Ok(data) => {
            println!("{}", json!({"api":1,"ok":true,"data":data}));
            ExitCode::SUCCESS
        }
        Err(e) => {
            println!("{}", json!({"api":1,"ok":false,"error":format!("{e:#}")}));
            ExitCode::from(1)
        }
    }
}
