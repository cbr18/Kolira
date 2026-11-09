//! Application theme adapters shared by GUI, CLI and MCP.
use crate::{
    field,
    gui_api::{str_arg, write_bytes},
    paths,
};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
};

pub fn definitions() -> &'static Vec<Value> {
    static ALL: OnceLock<Vec<Value>> = OnceLock::new();
    ALL.get_or_init(|| {
        serde_json::from_str(include_str!("../data/app_adapters.json")).expect("valid app metadata")
    })
}
fn definition(id: &str) -> Result<&'static Value> {
    definitions()
        .iter()
        .find(|a| a["id"] == id)
        .context("Unknown application adapter")
}
fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}
fn text(path: &Path) -> String {
    fs::read(path)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default()
}
fn dirs(path: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    paths.sort_by_key(|p| {
        p.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase()
    });
    paths
}
fn files(path: &Path, suffix: &str) -> Vec<PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.to_string_lossy().ends_with(suffix))
        .collect();
    paths.sort_by_key(|p| {
        p.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase()
    });
    paths
}
pub fn vaults() -> Vec<PathBuf> {
    ["obsidian", "Obsidian"]
        .iter()
        .flat_map(|root| dirs(&paths::home_dir().join(root)))
        .filter(|p| p.join(".obsidian").is_dir())
        .collect()
}
fn settings(a: &Value) -> Option<PathBuf> {
    if a["id"] == "obsidian" {
        vaults()
            .first()
            .map(|p| p.join(".obsidian/appearance.json"))
    } else {
        a["settings_rel"].as_str().map(paths::live_path)
    }
}
fn config(a: &Value) -> Option<PathBuf> {
    strings(&a["config_paths"])
        .into_iter()
        .map(|p| paths::live_path(&p))
        .find(|p| p.is_file())
}
fn has_config(a: &Value) -> bool {
    if a["id"] == "obsidian" {
        !vaults().is_empty()
    } else {
        strings(&a["config_paths"])
            .iter()
            .any(|p| paths::live_path(p).exists())
    }
}
fn executable(a: &Value) -> Option<Vec<String>> {
    let mut binaries = strings(&a["binaries"]);
    if binaries.is_empty() {
        binaries = strings(&a["launch_command"]).into_iter().take(1).collect();
    }
    for bin in binaries {
        if let Ok(path) = which::which(bin) {
            return Some(vec![path.to_string_lossy().into()]);
        }
    }
    for id in strings(&a["flatpak_ids"]) {
        if crate::tracked_apps::is_flatpak_installed(&id) {
            return Some(vec!["flatpak".into(), "run".into(), id]);
        }
    }
    None
}
fn ok() -> Value {
    json!({"ok":true,"message":""})
}
fn err(message: impl ToString, status: &str) -> Value {
    let message = message.to_string();
    json!({"ok":false,"status":status,"message":message,"message_template":message,"message_args":[]})
}
fn err_with_args(template: &str, args: &[&str], status: &str) -> Value {
    let mut message = String::new();
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let mut digits = String::new();
            while chars.peek().is_some_and(|c| c.is_ascii_digit()) {
                digits.push(chars.next().unwrap());
            }
            if let Some(value) = digits
                .parse::<usize>()
                .ok()
                .and_then(|n| n.checked_sub(1))
                .and_then(|i| args.get(i))
            {
                message.push_str(value)
            } else {
                message.push('%');
                message.push_str(&digits)
            }
        } else {
            message.push(c)
        }
    }
    json!({"ok":false,"status":status,"message":message,"message_template":template,"message_args":args})
}
fn path_parts(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| (*s).to_string()).collect()
}
fn get(text: &str, path: &[&str]) -> Value {
    field::json_get(text, &path_parts(path)).unwrap_or(Value::Null)
}
fn string(value: &Value) -> String {
    if value.is_null() {
        String::new()
    } else {
        field::value_str(value)
    }
}
fn theme(id: &str, label: &str, source: &str) -> Value {
    json!({"id":id,"label":label,"source":source})
}
fn unique(values: Vec<Value>) -> Vec<Value> {
    let mut out: Vec<Value> = vec![];
    for value in values {
        if let Some(index) = out.iter().position(|v| v["id"] == value["id"]) {
            out[index] = value;
        } else {
            out.push(value);
        }
    }
    out
}
pub fn backup(path: &Path) -> Result<Option<PathBuf>> {
    if !path.is_file() {
        return Ok(None);
    }
    let relative = path
        .strip_prefix(paths::home_dir())
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| Path::new("_external").join(path.file_name().unwrap_or_default()));
    let root = paths::backups_dir();
    fs::create_dir_all(&root)?;
    let dir = tempfile::Builder::new()
        .prefix("app-theme-")
        .tempdir_in(root)?
        .keep();
    let target = dir.join(relative);
    let mode = fs::metadata(path)?.permissions();
    write_bytes(&target, &fs::read(path)?, None)?;
    fs::set_permissions(&target, mode)?;
    Ok(Some(target))
}

fn installed(a: &Value) -> Result<Value> {
    let id = a["id"].as_str().unwrap_or("");
    let mut result = vec![];
    match id {
        "vscode" | "cursor" => {
            let builtins = if id == "cursor" {
                vec!["Cursor Dark Midnight", "Cursor Light Midnight"]
            } else {
                vec![
                    "Default Dark Modern",
                    "Default Light Modern",
                    "Visual Studio Dark",
                    "Visual Studio Light",
                    "Abyss",
                    "Kimbie Dark",
                    "Monokai",
                    "Solarized Dark",
                    "Solarized Light",
                ]
            };
            result.extend(
                builtins
                    .into_iter()
                    .map(|name| theme(name, name, "built-in")),
            );
            let mut roots = vec![paths::home_dir()
                .join(a["extensions_rel"].as_str().unwrap_or(".vscode/extensions"))];
            if id == "vscode" {
                roots.push(paths::home_dir().join(".vscode-oss/extensions"));
            }
            for root in roots {
                for dir in dirs(&root) {
                    let file = dir.join("package.json");
                    let Ok(package) = serde_json::from_str::<Value>(&text(&file)) else {
                        continue;
                    };
                    let extension = package["name"]
                        .as_str()
                        .unwrap_or_else(|| dir.file_name().unwrap().to_str().unwrap_or(""));
                    for item in package["contributes"]["themes"]
                        .as_array()
                        .into_iter()
                        .flatten()
                    {
                        let label =
                            item["label"]
                                .as_str()
                                .map(str::to_string)
                                .unwrap_or_else(|| {
                                    Path::new(item["path"].as_str().unwrap_or("theme"))
                                        .file_stem()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .into()
                                });
                        if !label.is_empty() {
                            let theme_id = item["id"].as_str().unwrap_or(&label);
                            result.push(theme(
                                theme_id,
                                &label,
                                &format!(
                                    "{extension}:{}",
                                    dir.file_name().unwrap().to_string_lossy()
                                ),
                            ));
                        }
                    }
                }
            }
        }
        "zed" => {
            for name in strings(&a["built_in_themes"]) {
                result.push(theme(&name, &name, "built-in"));
            }
            let mut all = files(&paths::live_path(".config/zed/themes"), ".json");
            for dir in dirs(&paths::data_home().join("zed/extensions/installed")) {
                all.extend(files(&dir.join("themes"), ".json"));
            }
            for file in all {
                let value: Value =
                    serde_json::from_str(&crate::config_edit::strip_jsonc(&text(&file)))
                        .unwrap_or(Value::Null);
                let mut names: Vec<String> = value["themes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v["name"].as_str().map(str::to_string))
                    .collect();
                if names.is_empty() {
                    if let Some(name) = value["name"].as_str() {
                        names.push(name.into());
                    }
                }
                if names.is_empty() {
                    names.push(
                        file.file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into(),
                    );
                }
                for name in names {
                    result.push(theme(&name, &name, &file.to_string_lossy()));
                }
            }
        }
        "obsidian" => {
            let Some(vault) = vaults().into_iter().next() else {
                return Ok(err("Obsidian vault not found", "not_installed"));
            };
            result.push(theme("obsidian", "Obsidian", "built-in"));
            for dir in dirs(&vault.join(".obsidian/themes")) {
                if !dir.join("theme.css").is_file() && !dir.join("manifest.json").is_file() {
                    continue;
                }
                let value: Value =
                    serde_json::from_str(&text(&dir.join("manifest.json"))).unwrap_or(Value::Null);
                let name = dir.file_name().unwrap().to_string_lossy();
                result.push(theme(
                    &name,
                    value["name"].as_str().unwrap_or(&name),
                    &dir.to_string_lossy(),
                ));
            }
        }
        "btop" | "telegram" => {
            let roots = if id == "btop" {
                vec![
                    paths::live_path(".config/btop/themes"),
                    PathBuf::from("/usr/share/btop/themes"),
                ]
            } else {
                vec![
                    paths::live_path(".config/telegram-desktop/themes"),
                    paths::live_path(".config/noctalia/Assets/Templates"),
                    paths::live_path(".local/share/TelegramDesktop/tdata/theme"),
                ]
            };
            let suffix = if id == "btop" {
                ".theme"
            } else {
                ".tdesktop-theme"
            };
            for root in roots {
                for path in files(&root, suffix) {
                    let name = path.file_stem().unwrap().to_string_lossy();
                    result.push(theme(&name, &name, &path.to_string_lossy()));
                }
            }
        }
        _ => {
            return Ok(err_with_args(
                "%1 themes cannot be enumerated",
                &[a["name"].as_str().unwrap_or(id)],
                "unsupported",
            ))
        }
    }
    let mut value = ok();
    value["themes"] = json!(unique(result));
    Ok(value)
}
fn current(a: &Value) -> Result<Value> {
    let id = a["id"].as_str().unwrap_or("");
    let mut out = ok();
    let name = a["name"].as_str().unwrap_or(id);
    if id == "btop" {
        let path = paths::live_path(".config/btop/btop.conf");
        if !path.is_file() {
            return Ok(err("btop config not found", "not_installed"));
        }
        let value = field::ini_get(&text(&path), "", "color_theme")
            .unwrap_or_default()
            .trim_matches('"')
            .to_string();
        out["value"] = json!(value);
        out["label"] = out["value"].clone();
        return Ok(out);
    }
    if id == "discord" {
        let Some(path) = config(a) else {
            return Ok(err("Discord settings.json not found", "not_installed"));
        };
        let value: Value = match serde_json::from_str(&text(&path)) {
            Ok(v) => v,
            Err(e) => {
                return Ok(err_with_args(
                    "Could not parse Discord settings.json: %1",
                    &[&e.to_string()],
                    "parse_error",
                ))
            }
        };
        let selected = string(
            value
                .get("BACKGROUND_COLOR")
                .filter(|v| !v.is_null() && **v != json!(""))
                .unwrap_or(&value["theme"]),
        );
        let lower = selected.to_lowercase();
        out["label"] = json!(
            if matches!(lower.as_str(), "#ffffff" | "#fff" | "light" | "white") {
                "Light"
            } else if !selected.is_empty() {
                "Dark"
            } else {
                ""
            }
        );
        out["value"] = json!(selected);
        return Ok(out);
    }
    let Some(path) = settings(a) else {
        return Ok(err_with_args(
            "%1 does not expose the current theme",
            &[name],
            "unsupported",
        ));
    };
    if id != "obsidian" && !path.is_file() {
        return Ok(err_with_args(
            "Settings file not found: %1",
            &[a["settings_rel"].as_str().unwrap_or("")],
            "not_installed",
        ));
    }
    let content = text(&path);
    let value = match id {
        "zed" => {
            let selected = get(&content, &["theme"]);
            if selected.is_string() {
                out["mode"] = json!("dark");
                selected
            } else {
                let mode = get(&content, &["theme", "mode"])
                    .as_str()
                    .unwrap_or("system")
                    .to_string();
                out["mode"] = json!(mode);
                let mut chosen = get(&content, &["theme", &mode]);
                if (chosen.is_null() || chosen == json!("")) && mode == "system" {
                    chosen = get(&content, &["theme", "dark"]);
                    if chosen.is_null() {
                        chosen = get(&content, &["theme", "light"]);
                    }
                }
                chosen
            }
        }
        "vscode" | "cursor" => get(&content, &["workbench.colorTheme"]),
        "obsidian" => {
            let v = get(&content, &["theme"]);
            if v.is_null() || v == json!("") {
                json!("obsidian")
            } else {
                v
            }
        }
        _ => Value::Null,
    };
    let selected = string(&value);
    out["value"] = json!(selected);
    out["label"] = out["value"].clone();
    if matches!(id, "vscode" | "cursor") {
        if let Some(item) = installed(a)?["themes"]
            .as_array()
            .and_then(|items| items.iter().find(|item| item["id"] == value))
        {
            out["label"] = item["label"].clone();
        }
    }
    Ok(out)
}
fn apply(a: &Value, theme_id: &str) -> Result<Value> {
    let _lock = crate::operation::MutationGuard::acquire()?;
    if theme_id.is_empty() {
        return Ok(err("Theme is empty", "unsupported"));
    }
    let id = a["id"].as_str().unwrap_or("");
    let name = a["name"].as_str().unwrap_or(id);
    if !a["capabilities"]["apply_theme"].as_bool().unwrap_or(false) {
        return Ok(err_with_args(
            "%1 themes cannot be applied automatically",
            &[name],
            "unsupported",
        ));
    }
    let path = if id == "btop" {
        paths::live_path(".config/btop/btop.conf")
    } else {
        let Some(path) = settings(a) else {
            return Ok(err("Obsidian vault not found", "not_installed"));
        };
        path
    };
    if id == "btop" && !path.is_file() {
        return Ok(err("btop config not found", "not_installed"));
    }
    let mut content = text(&path);
    if content.is_empty() {
        content = "{}".into();
    }
    let updated = patch_theme(id, &content, theme_id)?;
    backup(&path)?;
    write_bytes(&path, updated.as_bytes(), None)?;
    Ok(
        json!({"ok":true,"message":format!("{} theme set to {theme_id}",if id=="zed"{"Zed dark"}else{name})}),
    )
}
fn spawn(argv: &[String]) -> Result<()> {
    if crate::field::no_apply() {
        return Ok(());
    }
    let (program, args) = argv.split_first().context("missing executable")?;
    Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}
pub fn rpc(operation: &str, args: &Value) -> Result<Value> {
    if operation == "all" {
        return rows();
    }
    if operation == "backup" {
        return Ok(json!(backup(Path::new(str_arg(args, "path")?))?));
    }
    if operation == "vaults" {
        return Ok(json!(vaults()));
    }
    let a = definition(str_arg(args, "id")?)?;
    let result = (|| {
        Ok(match operation {
            "metadata" => a.clone(),
            "detect" => json!(executable(a).is_some()),
            "executable" => json!(executable(a)),
            "has_config" => json!(has_config(a)),
            "config" => json!(config(a)),
            "current" => current(a)?,
            "themes" => installed(a)?,
            "apply" => apply(a, str_arg(args, "value")?)?,
            "open_config" => {
                let mut target = config(a)
                    .or_else(|| {
                        strings(&a["config_paths"])
                            .first()
                            .map(|p| paths::live_path(p))
                    })
                    .context("No known config file")?;
                if !target.is_file() && !target.is_dir() {
                    target = target.parent().context("config folder not found")?.into();
                }
                if !target.is_dir() && !target.is_file() {
                    bail!("config folder not found");
                }
                spawn(&["xdg-open".into(), target.to_string_lossy().into()])?;
                json!({"ok":true,"message":format!("Opened {}",target.display())})
            }
            "launch" | "open_theme_settings" => {
                let mut argv = executable(a).context("Application not installed")?;
                if operation == "open_theme_settings" {
                    match a["id"].as_str().unwrap() {
                        "vscode" | "cursor" => argv.push("workbench.action.selectTheme".into()),
                        "zed" => argv.push("--new".into()),
                        "obsidian" => argv.push("obsidian://appearance".into()),
                        "discord" | "telegram" => {}
                        _ => {
                            return Ok(err(
                                "Application has no known theme settings command",
                                "unsupported",
                            ))
                        }
                    }
                } else {
                    argv.extend(strings(&a["launch_command"]).into_iter().skip(1));
                    argv.extend(strings(&args["arguments"]));
                }
                spawn(&argv)?;
                json!({"ok":true,"message":format!("Launched {}",a["name"].as_str().unwrap())})
            }
            _ => bail!("unknown application operation {operation}"),
        })
    })();
    Ok(result.unwrap_or_else(|e: anyhow::Error| {
        err(
            format!("{e:#}"),
            if operation == "apply" {
                "apply_failed"
            } else {
                "launch_failed"
            },
        )
    }))
}
fn rows() -> Result<Value> {
    let mut values = vec![];
    for a in definitions() {
        let id = a["id"].as_str().unwrap();
        let detected = executable(a).is_some();
        let (mut status, mut message) = ("ok".to_string(), String::new());
        let mut selected = json!({});
        let mut themes = json!([]);
        let mut status_template = String::new();
        let mut status_args = json!([]);
        if !detected {
            status = "not_installed".into();
            message = if has_config(a) {
                "Not installed — only an old config was found"
            } else {
                "Application not detected"
            }
            .into();
        } else if a["capabilities"]["current_theme"] == true {
            let v = current(a)?;
            if v["ok"] == true {
                selected = v;
            } else {
                status = v["status"].as_str().unwrap_or("warning").into();
                message = v["message"].as_str().unwrap_or("").into();
                status_template = v["message_template"].as_str().unwrap_or(&message).into();
                status_args = v["message_args"].clone();
            }
        }
        if detected && a["capabilities"]["installed_themes"] == true {
            let v = installed(a)?;
            if v["ok"] == true {
                themes = v["themes"].clone();
            } else if status == "ok" {
                status = v["status"].as_str().unwrap_or("warning").into();
                message = v["message"].as_str().unwrap_or("").into();
                status_template = v["message_template"].as_str().unwrap_or(&message).into();
                status_args = v["message_args"].clone();
            }
        }
        let path = config(a).or_else(|| {
            strings(&a["config_paths"])
                .first()
                .map(|p| paths::live_path(p))
        });
        values.push(json!({"id":id,"name":a["name"],"icon":a["icon"],"installed":detected,"integration":a["integration"],"capabilities":a["capabilities"],"configPath":path.as_ref().map(|p|p.to_string_lossy().to_string()).unwrap_or_default(),"configDisplay":path.as_ref().and_then(|p|p.file_name()).map(|s|s.to_string_lossy().to_string()).unwrap_or_default(),"currentValue":selected["value"].as_str().unwrap_or(""),"currentDisplay":selected["label"].as_str().or(selected["value"].as_str()).unwrap_or(""),"mode":selected["mode"].as_str().unwrap_or(""),"themes":themes,"status":status,"statusText":message,"statusTemplate":if status_template.is_empty(){&message}else{&status_template},"statusArgs":status_args}));
    }
    Ok(json!(values))
}

fn patch_theme(id: &str, content: &str, theme_id: &str) -> Result<String> {
    Ok(match id {
        "btop" => field::set_ini_text(
            content,
            "",
            "color_theme",
            &serde_json::to_string(theme_id)?,
        ),
        "zed" => {
            let mode = field::json_set(content, &path_parts(&["theme", "mode"]), &json!("dark"))?;
            field::json_set(&mode, &path_parts(&["theme", "dark"]), &json!(theme_id))?
        }
        "obsidian" => field::json_set(content, &path_parts(&["theme"]), &json!(theme_id))?,
        _ => field::json_set(
            content,
            &path_parts(&["workbench.colorTheme"]),
            &json!(theme_id),
        )?,
    })
}
pub(crate) fn plan_apply(
    id: &str,
    theme_id: &str,
    plan: &mut crate::change_plan::Plan,
) -> Result<()> {
    let a = definitions()
        .iter()
        .find(|a| a["id"] == id)
        .context("Unknown application")?;
    if theme_id.is_empty() || a["capabilities"]["apply_theme"] != true {
        anyhow::bail!("Application theme cannot be applied: {id}");
    }
    let path = if id == "btop" {
        paths::live_path(".config/btop/btop.conf")
    } else {
        settings(a).context("Application settings not found")?
    };
    if id == "btop" && !plan.is_file(&path)? {
        anyhow::bail!("btop config not found");
    }
    let text = plan.text(&path)?.unwrap_or_default();
    plan.write_text(
        &path,
        patch_theme(id, if text.is_empty() { "{}" } else { &text }, theme_id)?,
        "application",
    )
}
