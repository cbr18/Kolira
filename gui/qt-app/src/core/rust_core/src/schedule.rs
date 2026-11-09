//! Local-session scheduler. Claims are durable before apply (at most once).
use crate::{
    gui_api::{lock_file, str_arg, write_bytes},
    operation, paths,
};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Datelike, Duration, Local, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    process::{Command, Stdio},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    pub theme: String,
    pub time: String,
    pub days: Vec<u32>,
    pub enabled: bool,
    #[serde(default)]
    pub keep_wallpaper: bool,
}
#[derive(Default, Serialize, Deserialize)]
struct State {
    enabled: bool,
    rules: Vec<Rule>,
    #[serde(default)]
    results: BTreeMap<String, Value>,
    #[serde(default)]
    claims: BTreeMap<String, String>,
}
fn file() -> PathBuf {
    paths::theme_root().join("schedule.json")
}
fn lock() -> Result<fs::File> {
    lock_file(&paths::theme_root().join("schedule.lock"))
}
fn load() -> Result<State> {
    match fs::read(file()) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes).context("Invalid schedule.json")?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(State::default()),
        Err(e) => Err(e.into()),
    }
}
fn save(state: &State) -> Result<()> {
    write_bytes(&file(), &serde_json::to_vec_pretty(state)?, None)
}
fn clock(rule: &Rule) -> Result<(u32, u32)> {
    let bytes = rule.time.as_bytes();
    if bytes.len() != 5
        || bytes[2] != b':'
        || !bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 2 || b.is_ascii_digit())
    {
        bail!("Time must be HH:MM");
    }
    let hour = rule.time[..2].parse::<u32>()?;
    let minute = rule.time[3..].parse::<u32>()?;
    if hour > 23 || minute > 59 {
        bail!("Time must be HH:MM (00:00–23:59)");
    }
    Ok((hour, minute))
}
fn validate(rule: &Rule) -> Result<()> {
    paths::validate_profile_name(&rule.id)?;
    paths::validate_profile_name(&rule.theme)?;
    if !paths::profile_dir(&rule.theme)?.is_dir() {
        bail!("Theme does not exist: {}", rule.theme);
    }
    clock(rule)?;
    if rule.days.is_empty() || rule.days.iter().any(|d| !(1..=7).contains(d)) {
        bail!("Select ISO weekdays 1–7");
    }
    Ok(())
}
fn event<T: TimeZone>(rule: &Rule, now: &DateTime<T>) -> String {
    format!("{}T{}", now.date_naive().format("%Y-%m-%d"), rule.time)
}
fn due<T: TimeZone>(rule: &Rule, now: &DateTime<T>) -> bool {
    let Ok((h, m)) = clock(rule) else {
        return false;
    };
    rule.enabled
        && rule.days.contains(&now.weekday().number_from_monday())
        && now.hour() == h
        && now.minute() == m
}
/// Search real instants, so nonexistent DST times are skipped and repeated
/// local times share a claim. Bound is eight days, even across DST transitions.
fn next(rule: &Rule, now: DateTime<Local>, claimed: Option<&String>) -> Option<String> {
    if !rule.enabled {
        return None;
    }
    (1..=8 * 24 * 60)
        .map(|m| now + Duration::minutes(m))
        .find(|at| due(rule, at) && claimed.is_none_or(|key| key < &event(rule, at)))
        .map(|at| {
            at.with_second(0)
                .unwrap()
                .with_nanosecond(0)
                .unwrap()
                .to_rfc3339()
        })
}
fn service_owned(service: &str) -> bool {
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
        return false;
    }
    paths::output_bounded_for(
        Command::new("qdbus6").args([
            "org.freedesktop.DBus",
            "/",
            "org.freedesktop.DBus.NameHasOwner",
            service,
        ]),
        std::time::Duration::from_secs(2),
    )
    .is_some_and(|out| out.0.success() && String::from_utf8_lossy(&out.1).trim() == "true")
}
fn session_available() -> bool {
    crate::field::no_apply()
        || (std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .split(':')
            .any(|v| v == "KDE")
            && (std::env::var_os("WAYLAND_DISPLAY").is_some()
                || std::env::var_os("DISPLAY").is_some())
            && ["org.kde.KWin", "org.kde.plasmashell"]
                .iter()
                .all(|service| service_owned(service)))
}
fn daemon_lock() -> Result<fs::File> {
    fs::create_dir_all(paths::theme_root())?;
    Ok(fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(paths::theme_root().join("schedule-daemon.lock"))?)
}
fn running() -> Result<bool> {
    Ok(daemon_lock()?.try_lock().is_err())
}
fn autostart(enabled: bool) -> Result<()> {
    // Desktop Entry escaping is independent of shell quoting; Exec has no shell.
    let exe = std::env::current_exe()?.to_string_lossy().into_owned();
    if exe.contains(['\n', '\r', '%']) {
        bail!("Unsupported executable path for desktop autostart");
    }
    let quoted = exe
        .replace('\\', "\\\\\\\\")
        .replace('"', "\\\\\"")
        .replace('`', "\\\\`")
        .replace('$', "\\\\$");
    write_bytes(&paths::config_home().join("autostart/kde-theme-manager-schedule.desktop"),
        format!("[Desktop Entry]\nType=Application\nName=KDE Theme Manager Schedule\nOnlyShowIn=KDE;\nExec=\"{quoted}\" schedule daemon\nHidden={}\n", !enabled).as_bytes(), None)
}
pub fn rpc(method: &str, args: &Value) -> Result<Value> {
    let _lock = lock()?;
    let mut state = load()?;
    match method {
        "list" => {}
        "put" => {
            let rule: Rule =
                serde_json::from_value(args["rule"].clone()).context("Invalid schedule rule")?;
            validate(&rule)?;
            if let Some(old) = state.rules.iter_mut().find(|r| r.id == rule.id) {
                *old = rule;
            } else {
                state.rules.push(rule);
            }
            state.rules.sort_by(|a, b| a.id.cmp(&b.id));
            save(&state)?;
        }
        "delete" => {
            let id = str_arg(args, "id")?;
            paths::validate_profile_name(id)?;
            state.rules.retain(|r| r.id != id);
            // Keep the claim: deleting/recreating a rule must not replay today.
            state.results.remove(id);
            save(&state)?;
        }
        "enable" => {
            let enabled = args["enabled"]
                .as_bool()
                .context("enabled must be boolean")?;
            autostart(enabled)?;
            state.enabled = enabled;
            save(&state)?;
            if enabled && !running()? && !crate::field::no_apply() && session_available() {
                Command::new(std::env::current_exe()?)
                    .args(["schedule", "daemon"])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()?;
            }
        }
        _ => bail!("Unknown schedule method"),
    }
    let now = Local::now();
    let rules: Vec<_> = state
        .rules
        .iter()
        .map(|r| {
            let mut v = serde_json::to_value(r).unwrap();
            v["next"] = if state.enabled {
                json!(next(r, now, state.claims.get(&r.id)))
            } else {
                Value::Null
            };
            v["last"] = state.results.get(&r.id).cloned().unwrap_or(Value::Null);
            v
        })
        .collect();
    Ok(json!({"enabled":state.enabled,"running":running()?,"rules":rules}))
}

pub fn tick(now: DateTime<Local>) -> Result<Value> {
    let jobs = {
        let _lock = lock()?;
        let mut state = load()?;
        if !state.enabled {
            return Ok(json!([]));
        }
        let mut jobs: Vec<_> = state
            .rules
            .iter()
            .filter(|r| {
                due(r, &now)
                    && state
                        .claims
                        .get(&r.id)
                        .is_none_or(|key| key < &event(r, &now))
            })
            .cloned()
            .collect();
        jobs.sort_by(|a, b| a.id.cmp(&b.id));
        for rule in &jobs {
            let key = event(rule, &now);
            state.claims.insert(rule.id.clone(), key.clone());
            state.results.insert(rule.id.clone(),json!({"event":key,"state":"claimed","time":Utc::now(),"message":"Claimed before apply; interrupted claims are never retried automatically"}));
        }
        save(&state)?;
        jobs
    };
    let mut results = vec![];
    for (index, rule) in jobs.iter().enumerate() {
        let mut operation_id = None;
        let result = if index > 0 {
            let message = "Skipped: another rule has priority at this time";
            operation_id = Some(operation::rejected_attempt(
                &rule.theme,
                "schedule",
                message,
                true,
            )?);
            Ok(message)
        } else {
            operation::with_source("schedule", || {
                operation::run(&rule.theme, || {
                    operation_id = operation::current_id();
                    operation::note_report(
                        json!({"schedule_rule":rule.id,"schedule_event":event(rule,&now)}),
                    )?;
                    if !session_available() {
                        operation::note_report(
                            json!({"schedule_rule":rule.id,"skip_reason":"no_plasma_session"}),
                        )?;
                        return Ok("Skipped: no Plasma session");
                    }
                    let dir = paths::profile_dir(&rule.theme)?;
                    if !dir.is_dir() {
                        bail!("Scheduled theme is missing: {}", rule.theme);
                    }
                    if crate::theme::is_package(&dir)
                        && (!crate::theme::check(&rule.theme)?.is_empty()
                            || !crate::theme::needed(&rule.theme)?.is_empty())
                    {
                        bail!("Missing dependencies; install them manually before the next event");
                    }
                    let report = crate::bundle_view::apply(
                        &json!({"name":rule.theme,"keep_wallpaper":rule.keep_wallpaper,"skip_dependencies":true,"source":"schedule"}),
                    )?;
                    operation::note_report(report)?;
                    Ok("Applied")
                })
            })
        };
        let skipped_busy = result
            .as_ref()
            .err()
            .is_some_and(|e| e.to_string().contains("Another appearance operation"));
        if operation_id.is_none() {
            operation_id = operation::last_attempt_id();
        }
        let value = match result {
            Ok(message) => {
                json!({"event":event(rule,&now),"state":if message.starts_with("Skipped:") {"skipped"} else {"succeeded"},"message":message,"operation_id":operation_id,"time":Utc::now()})
            }
            Err(error) => {
                json!({"event":event(rule,&now),"state":if skipped_busy {"skipped"} else {"failed"},"message":format!("{error:#}"),"operation_id":operation_id,"time":Utc::now()})
            }
        };
        let _lock = lock()?;
        let mut state = load()?;
        if state.rules.iter().any(|r| r.id == rule.id) {
            state.results.insert(rule.id.clone(), value.clone());
            save(&state)?;
        }
        results.push(value);
    }
    Ok(json!(results))
}
pub fn daemon() -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    let executable = std::env::current_exe()?;
    let identity = fs::metadata(&executable)?;
    let file = daemon_lock()?;
    if file.try_lock().is_err() {
        return Ok(());
    }
    let mut last = Local::now();
    let startup = std::time::Instant::now();
    let mut had_bus = false;
    let mut skipped_minute = last.format("%Y-%m-%dT%H:%M").to_string();
    loop {
        // Starting/restarting or waking does not catch up missed events.
        std::thread::sleep(std::time::Duration::from_secs(10));
        let now = Local::now();
        let Ok(installed) = fs::metadata(&executable) else {
            return Ok(());
        };
        if (identity.dev(), identity.ino()) != (installed.dev(), installed.ino()) {
            drop(file);
            Command::new(&executable)
                .args(["schedule", "daemon"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
            return Ok(());
        }
        let bus_alive = crate::field::no_apply() || service_owned("org.freedesktop.DBus");
        if (!bus_alive && had_bus) || (!bus_alive && startup.elapsed().as_secs() > 120) {
            return Ok(());
        }
        had_bus |= bus_alive;
        if !load()?.enabled {
            return Ok(());
        }
        if (now.timestamp() - last.timestamp()).abs() > 30 {
            skipped_minute = now.format("%Y-%m-%dT%H:%M").to_string();
        }
        if now.format("%Y-%m-%dT%H:%M").to_string() != skipped_minute {
            tick(now)?;
        }
        last = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn weekdays_dst_repeated_time_and_clock_validation() {
        let mut r = Rule {
            id: "morning".into(),
            theme: "probe".into(),
            time: "08:30".into(),
            days: vec![1],
            enabled: true,
            keep_wallpaper: false,
        };
        let monday = DateTime::parse_from_rfc3339("2026-11-09T08:30:00+10:00").unwrap();
        assert!(due(&r, &monday));
        assert!(!due(&r, &(monday + Duration::days(1))));
        let repeated = DateTime::parse_from_rfc3339("2026-11-09T08:30:00+09:00").unwrap();
        assert_eq!(event(&r, &monday), event(&r, &repeated));
        r.time = "24:00".into();
        assert!(clock(&r).is_err());
        r.time = "8:30".into();
        assert!(clock(&r).is_err());
        r.time = "１２:０".into();
        assert!(clock(&r).is_err());
    }
}
