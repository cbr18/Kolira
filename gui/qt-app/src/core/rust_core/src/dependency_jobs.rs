//! Dependency jobs have a durable result and an explicit cancellation token.
//! Download workers run in their own process group; package transactions are
//! allowed to finish safely in their terminal instead of being killed mid-write.
use crate::{dependency_registry as registry, paths};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, ExitCode},
    time::{Duration, Instant},
};

pub fn event(stage: &str, message: &str) {
    if let Ok(id) = std::env::var("KTM_DEP_OPERATION") {
        if let Ok(dir) = job_dir(&id) {
            let _ = registry::atomic_json(
                &dir.join("progress.json"),
                &json!({"stage":stage,"message":message}),
            );
        }
    }
    if std::env::var("KTM_DEP_EVENTS").as_deref() == Ok("1") {
        // A closed GUI pipe must not panic or interrupt a committed installation.
        let _ = writeln!(
            std::io::stdout(),
            "{}",
            json!({"event": "progress", "stage": stage, "message": message})
        );
    }
}

fn jobs_dir() -> PathBuf {
    paths::theme_root().join("dependency-operations")
}
fn job_dir(id: &str) -> Result<PathBuf> {
    if id.len() != 24 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("invalid operation id");
    }
    Ok(jobs_dir().join(id))
}
pub fn cancel(id: &str) -> Result<()> {
    let dir = job_dir(id)?;
    if !dir.join("state.json").is_file() {
        bail!("unknown operation");
    }
    fs::write(dir.join("cancel"), b"cancel")?;
    Ok(())
}
fn process_identity(pid: u32) -> Option<String> {
    fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()?
        .rsplit_once(") ")?
        .1
        .split_whitespace()
        .nth(19)
        .map(str::to_string)
}
pub fn state(id: &str) -> Result<Value> {
    let path = job_dir(id)?.join("state.json");
    let mut value: Value = serde_json::from_slice(&fs::read(&path)?)?;
    if value["status"] == "running" {
        let live = value["pid"]
            .as_u64()
            .and_then(|p| process_identity(p as u32));
        if live.as_deref() != value["process_start"].as_str() || live.is_none() {
            value["status"] = json!("interrupted");
            registry::atomic_json(&path, &value)?;
        }
    }
    if let Ok(bytes) = fs::read(job_dir(id)?.join("progress.json")) {
        if let Ok(progress) = serde_json::from_slice::<Value>(&bytes) {
            value["progress"] = progress;
        }
    }
    Ok(value)
}

/// Start the existing verified job executor and return its durable id rather
/// than blocking MCP's stdio request loop during installation.
pub fn start(action: &str, ids: &[String], confirmed: bool) -> Result<Value> {
    use std::io::Read;
    use std::process::Stdio;
    if !matches!(action, "install" | "remove" | "reinstall") || ids.is_empty() {
        bail!("Select dependencies and install, remove or reinstall");
    }
    if ids.iter().any(|id| id.starts_with('-')) {
        bail!("Invalid dependency id");
    }
    fs::create_dir_all(jobs_dir())?;
    let mut output = tempfile::NamedTempFile::new_in(jobs_dir())?;
    let errors = tempfile::NamedTempFile::new_in(jobs_dir())?;
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args(["dependencies", action])
        .args(ids)
        .arg("--events=jsonl");
    if confirmed {
        command.arg("--confirm-shared");
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(output.reopen()?)
        .stderr(errors.reopen()?)
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut text = String::new();
    loop {
        output.as_file_mut().read_to_string(&mut text)?;
        for line in text.lines() {
            if let Ok(value) = serde_json::from_str::<Value>(line) {
                if let Some(id) = value["operation"]
                    .as_str()
                    .filter(|_| value["event"] == "started")
                {
                    let dir = job_dir(id)?;
                    output
                        .persist(dir.join("events.jsonl"))
                        .map_err(|e| e.error)?;
                    errors
                        .persist(dir.join("stderr.log"))
                        .map_err(|e| e.error)?;
                    return state(id);
                }
            }
        }
        if let Some(status) = child.try_wait()? {
            let error = fs::read_to_string(errors.path()).unwrap_or_default();
            bail!("Dependency job did not start ({status}): {error}");
        }
        if Instant::now() >= deadline {
            // Do not kill a potentially privileged transaction on transport timeout.
            let log = output.keep().map_err(|e| e.error)?.1;
            let diagnostics = errors.keep().map_err(|e| e.error)?.1;
            bail!(
                "Job startup timed out; inspect {} and {} before retrying",
                log.display(),
                diagnostics.display()
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

pub fn recover() -> Result<()> {
    if !jobs_dir().is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(jobs_dir())?.flatten() {
        let id = entry.file_name().to_string_lossy().to_string();
        if job_dir(&id).is_ok() && entry.path().join("state.json").is_file() {
            state(&id)?;
        }
    }
    Ok(())
}

fn stop_group(child: &mut std::process::Child) {
    let group = format!("-{}", child.id());
    let _ = Command::new("kill")
        .args(["-TERM", "--", &group])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    let deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < deadline {
        if child.try_wait().ok().flatten().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let _ = Command::new("kill")
        .args(["-KILL", "--", &group])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    let _ = child.wait();
}

pub fn run(action: &str, ids: &[String], confirmed: bool) -> Result<bool> {
    use std::os::unix::process::CommandExt;
    if !matches!(action, "install" | "remove" | "reinstall") || ids.is_empty() {
        bail!("select dependencies and install, remove or reinstall");
    }
    if std::env::var("KTM_THEME_NO_APPLY").as_deref() == Ok("1") {
        bail!("mutations disabled by KTM_THEME_NO_APPLY=1");
    }
    // Validate the whole selection before performing its first mutation.
    for id in ids {
        let r = registry::get(id)?;
        if action != "install" && !r.can_remove {
            bail!("{}: {}", r.dependency.name, r.removal_reason);
        }
        if action != "install" && !r.themes.is_empty() && !confirmed {
            bail!(
                "confirmation required for affected themes: {}",
                r.themes.iter().cloned().collect::<Vec<_>>().join(", ")
            );
        }
        if action != "remove" && !r.can_install {
            bail!("{} has no automatic installation method", r.dependency.name);
        }
    }
    fs::create_dir_all(jobs_dir())?;
    let id = format!(
        "{:024x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
            ^ u128::from(std::process::id())
    );
    let id = &id[id.len() - 24..];
    let dir = job_dir(id)?;
    fs::create_dir(&dir)?;
    let mut state = json!({"id":id, "action":action, "status":"running", "results":[], "pid":std::process::id(), "process_start":process_identity(std::process::id())});
    registry::atomic_json(&dir.join("state.json"), &state)?;
    println!("{}", json!({"event":"started", "operation":id}));
    std::io::stdout().flush()?;
    let mut all_ok = true;
    for (index, resource_id) in ids.iter().enumerate() {
        if dir.join("cancel").exists() {
            break;
        }
        let resource = registry::get(resource_id)?;
        println!(
            "{}",
            json!({"event":"progress", "resource":resource_id,"stage":"starting","message":resource.dependency.name,"current":index+1,"total":ids.len()})
        );
        let result_path = dir.join("worker.json");
        let _ = fs::remove_file(&result_path);
        let staging = tempfile::Builder::new()
            .prefix("staging-")
            .tempdir_in(&dir)?;
        let mut child = Command::new(std::env::current_exe()?)
            .args(["dependencies", "worker", action, resource_id])
            .env("KTM_DEP_RESULT", &result_path)
            .env("KTM_DEP_EVENTS", "1")
            .env("TMPDIR", staging.path())
            .env("KTM_DEP_CANCEL", dir.join("cancel"))
            .env("KTM_DEP_OPERATION", id)
            .process_group(0)
            .spawn()?;
        let package = crate::deps::needs_root(&resource.dependency);
        let deadline = Instant::now() + Duration::from_secs(if package { 3700 } else { 1800 });
        let mut cancelled = false;
        loop {
            if child.try_wait()?.is_some() {
                break;
            }
            if dir.join("cancel").exists() {
                if !cancelled && package {
                    event("waiting", "Cancellation requested; waiting for the package transaction to finish safely");
                }
                cancelled = true;
                if !package {
                    stop_group(&mut child);
                    break;
                }
            }
            if Instant::now() > deadline {
                if !package {
                    stop_group(&mut child);
                }
                // Package workers have their own safe terminal deadline; never
                // send a process-group signal to a privileged transaction.
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if !package {
            stop_group(&mut child);
        }
        let result = fs::read(&result_path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .unwrap_or_else(|| {
                let mut message = if cancelled {
                    "Cancelled"
                } else {
                    "Installation interrupted or timed out; refresh before retrying"
                }
                .to_string();
                if action == "reinstall" && !package {
                    message.push_str("; ");
                    message.push_str(
                        &registry::recover_reinstall(resource_id)
                            .unwrap_or_else(|e| format!("{e:#}")),
                    );
                }
                json!({"ok":false,"message":message})
            });
        let ok = result["ok"].as_bool().unwrap_or(false);
        all_ok &= ok;
        state["results"]
            .as_array_mut()
            .context("results")?
            .push(json!({"id":resource_id,"ok":ok,"message":result["message"]}));
        registry::atomic_json(&dir.join("state.json"), &state)?;
        println!(
            "{}",
            json!({"event":"item", "id":resource_id,"ok":ok,"message":result["message"]})
        );
    }
    let cancelled = dir.join("cancel").exists();
    all_ok &= !cancelled;
    state["status"] = json!(if cancelled {
        "cancelled"
    } else if all_ok {
        "success"
    } else {
        "failed"
    });
    registry::atomic_json(&dir.join("state.json"), &state)?;
    registry::refresh(None)?;
    println!(
        "{}",
        json!({"event":"finished", "ok":all_ok,"operation":id,"status":state["status"],"results":state["results"]})
    );
    Ok(all_ok)
}

pub fn worker(action: &str, id: &str) -> Result<()> {
    let p = std::env::var_os("KTM_DEP_RESULT")
        .context("worker must be started by a dependency operation")?;
    if !matches!(action, "install" | "remove" | "reinstall") {
        bail!("invalid worker action");
    }
    if std::env::var("KTM_THEME_NO_APPLY").as_deref() == Ok("1") {
        bail!("mutations disabled by KTM_THEME_NO_APPLY=1");
    }
    let resource = registry::get(id)?;
    if !crate::deps::needs_root(&resource.dependency) {
        // If the GUI kills its CLI (or the CLI crashes), the download worker
        // must not continue installing invisibly. A pid start time prevents reuse.
        let operation = std::env::var("KTM_DEP_OPERATION").context("missing parent operation")?;
        let parent = state(&operation)?;
        let pid = parent["pid"].as_u64().context("missing parent pid")? as u32;
        let identity = parent["process_start"]
            .as_str()
            .context("missing parent identity")?
            .to_string();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(200));
            if process_identity(pid).as_deref() != Some(&identity) {
                let group = format!("-{}", std::process::id());
                let _ = Command::new("kill").args(["-KILL", "--", &group]).status();
                break;
            }
        });
    }
    let result = (|| {
        let r = registry::get(id)?;
        if action == "reinstall" {
            return registry::reinstall(id, true);
        }
        if action == "remove" {
            event("removing", &r.dependency.name);
            registry::remove(id, true)?;
        }
        if matches!(action, "install" | "reinstall") {
            if crate::deps::needs_root(&r.dependency) {
                registry::install_package(&r.dependency)
            } else {
                crate::deps::install(&r.dependency)
            }
        } else {
            Ok(format!("Removed {}", r.dependency.name))
        }
    })();
    registry::atomic_json(
        &PathBuf::from(p),
        &match &result {
            Ok(m) => json!({"ok":true,"message":m}),
            Err(e) => json!({"ok":false,"message":format!("{e:#}")}),
        },
    )?;
    result.map(|_| ())
}

/// Run a package command in the terminal, publishing its result before exiting.
/// No shell snippets and no extra Enter prompt are involved.
pub fn terminal_worker(result: &str, command: &[String]) -> Result<()> {
    let (program, args) = command.split_first().context("missing package command")?;
    if !matches!(program.as_str(), "sudo" | "yay" | "paru") {
        bail!("unsupported package manager");
    }
    let status = Command::new(program).args(args).status()?;
    registry::atomic_json(
        &PathBuf::from(result),
        &json!({"ok":status.success(),"code":status.code()}),
    )?;
    if !status.success() {
        bail!("package manager failed; see terminal output");
    }
    Ok(())
}

pub fn cli(args: &[String]) -> ExitCode {
    let result = (|| -> Result<bool> {
        let sub = args.first().map(String::as_str).unwrap_or("list");
        match sub {
            "list"|"refresh"|"register-app" => {
                let python = args.iter().find_map(|a| a.strip_prefix("--python="));
                if sub == "register-app" && python.is_none() { bail!("register-app requires --python=PATH"); }
                let mut list = registry::refresh(python)?;
                if let Some(theme) = args.iter().find_map(|a| a.strip_prefix("--theme=")) { paths::validate_profile_name(theme)?; list.retain(|r| r.themes.contains(theme)); }
                if args.iter().any(|a| a == "--app") { list.retain(|r| r.application); }
                println!("{}", serde_json::to_string(&list)?);
                Ok(true)
            }
            "install"|"remove"|"reinstall" => run(sub, &args.iter().skip(1).filter(|a| !a.starts_with("--")).cloned().collect::<Vec<_>>(), args.iter().any(|a| a == "--confirm-shared")),
            "cancel" => { cancel(args.get(1).context("missing operation id")?)?; Ok(true) }
            "operation" => { println!("{}", state(args.get(1).context("missing operation id")?)?); Ok(true) }
            "remember" => {
                let theme = args.iter().find_map(|a| a.strip_prefix("--theme=")).context("missing --theme")?;
                let policy = args.iter().find_map(|a| a.strip_prefix("--policy=")).context("missing --policy")?;
                let policy = crate::automation::Policy::parse(policy).context("invalid policy")?;
                let selected: Vec<&str> = args.iter().filter_map(|a| a.strip_prefix("--selected=")).collect();
                let groups: Vec<&str> = args.iter().filter_map(|a| a.strip_prefix("--group=")).collect();
                let needed = crate::theme::needed(theme)?;
                for group in &groups {
                    if !matches!(*group, "store"|"packages") { bail!("unknown dependency policy group"); }
                    let items: Vec<_> = needed.iter().filter(|n| {
                        let key = if crate::deps::needs_root(&n.dependency) {"packages"} else {"store"};
                        key == *group
                    }).collect();
                    if items.is_empty() { bail!("no dependencies in policy group {group}"); }
                    if policy == crate::automation::Policy::Always && !items.iter().all(|n| selected.contains(&n.resource_id.as_str())) {
                        bail!("partial selections cannot be remembered for {group}");
                    }
                }
                if !groups.is_empty() { crate::automation::set(&groups.iter().map(|g| (*g, policy)).collect::<Vec<_>>())?; }
                Ok(true)
            }
            "worker" => { worker(args.get(1).context("missing action")?, args.get(2).context("missing id")?)?; Ok(true) }
            "terminal-worker" => { terminal_worker(args.get(1).context("missing result path")?, args.get(2..).context("missing command")?)?; Ok(true) }
            _ => bail!("Usage: dependencies list|refresh|register-app|install|remove|reinstall|cancel|operation"),
        }
    })();
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(2),
        Err(e) => {
            eprintln!("{e:#}");
            ExitCode::from(1)
        }
    }
}
