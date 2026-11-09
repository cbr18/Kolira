//! Failures and real cross-process contention, always in a disposable HOME.
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;
struct Home(TempDir);
impl Home {
    fn new() -> Self {
        let h = Self(tempfile::tempdir().unwrap());
        h.write(
            ".config/kdeglobals",
            "# personal\n[General]\nColorScheme=Old\nBrowserApplication=keep\n",
        );
        h.write(".config/kde-theme-profile/active-profile", "previous\n");
        h.write(
            ".config/kde-theme-profile/themes/probe/theme.json",
            r#"{"schema":"theme-profile/theme@2","name":"probe"}"#,
        );
        h.write(
            ".config/kde-theme-profile/themes/probe/.config/kdeglobals",
            "[General]\nColorScheme=New\n",
        );
        h
    }
    fn path(&self, p: &str) -> PathBuf {
        self.0.path().join(p)
    }
    fn write(&self, p: &str, text: &str) {
        let path = self.path(p);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn executable(&self, name: &str, script: &str) {
        self.write(&format!("bin/{name}"), script);
        fs::set_permissions(
            self.path(&format!("bin/{name}")),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_theme-profile"));
        c.args(args)
            .env("HOME", self.0.path())
            .env("XDG_CONFIG_HOME", self.path(".config"))
            .env("XDG_DATA_HOME", self.path(".local/share"))
            .env("XDG_CACHE_HOME", self.path(".cache"))
            .env("PATH", self.path("bin"))
            .env("KTM_OFFLINE", "1")
            .env("KTM_THEME_NO_APPLY", "1");
        c
    }
    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }
    fn apply(&self) -> Output {
        self.run(&["apply", "probe", "--without-dependencies", "--json"])
    }
    fn records(&self) -> Value {
        let out = self.run(&["operations", "list"]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
    fn rpc(&self, method: &str, args: Value) -> Value {
        let mut child = self
            .command(&["core", method])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(args.to_string().as_bytes())
            .unwrap();
        serde_json::from_slice(&child.wait_with_output().unwrap().stdout).unwrap()
    }
    fn blocked_apply(&self) -> Running {
        self.executable("plasma-apply-colorscheme","#!/bin/sh\nprintf '%s' \"$$\" > \"$HOME/hook.pid\"\n: > \"$HOME/entered\"\nexec /bin/sleep 20\n");
        let child = self
            .command(&["apply", "probe", "--without-dependencies", "--json"])
            .env_remove("KTM_THEME_NO_APPLY")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut running = Running { child, hook: None };
        let deadline = Instant::now() + Duration::from_secs(10);
        while !self.path("entered").exists() {
            assert!(Instant::now() < deadline, "hook did not start");
            assert!(
                running.child.try_wait().unwrap().is_none(),
                "apply exited before its hook"
            );
            thread::sleep(Duration::from_millis(20));
        }
        running.hook = Some(
            fs::read_to_string(self.path("hook.pid"))
                .unwrap()
                .parse()
                .unwrap(),
        );
        running
    }
}
struct Running {
    child: Child,
    hook: Option<u32>,
}
impl Running {
    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(pid) = self.hook.take() {
            let _ = Command::new("/usr/bin/kill")
                .args(["-KILL", &pid.to_string()])
                .status();
        }
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        self.kill();
    }
}

#[test]
fn successful_apply_links_private_record_backup_and_json_report() {
    let h = Home::new();
    let out = h.apply();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    let record = &h.records()["operations"][0];
    assert_eq!(record["id"], report["operation_id"]);
    assert_eq!(record["state"], "succeeded");
    assert_eq!(record["backup"], report["backup"]);
    let backup = PathBuf::from(record["backup"].as_str().unwrap());
    assert!(backup.join("transaction/manifest.json").is_file());
    assert_eq!(
        fs::metadata(&backup).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(backup.join("transaction/manifest.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert!(!h
        .run(&["operations", "recover", record["id"].as_str().unwrap()])
        .status
        .success());
}

#[test]
fn invalid_preparation_changes_no_live_files() {
    let h = Home::new();
    let before = fs::read(h.path(".config/kdeglobals")).unwrap();
    h.write(
        ".config/kde-theme-profile/themes/probe/.config/alacritty/alacritty.toml",
        "[window\ninvalid TOML\n",
    );
    let out = h.apply();
    assert!(!out.status.success());
    assert_eq!(fs::read(h.path(".config/kdeglobals")).unwrap(), before);
    assert_eq!(
        fs::read_to_string(h.path(".config/kde-theme-profile/active-profile")).unwrap(),
        "previous\n"
    );
    assert_eq!(h.records()["operations"][0]["state"], "failed_preparation");
}

#[test]
fn external_command_failure_restores_original_bytes_permissions_marker_and_journal() {
    let h = Home::new();
    let before = fs::read(h.path(".config/kdeglobals")).unwrap();
    fs::set_permissions(
        h.path(".config/kdeglobals"),
        fs::Permissions::from_mode(0o640),
    )
    .unwrap();
    h.executable("plasma-apply-colorscheme", "#!/bin/sh\n[ \"$1\" != New ]\n");
    let out = h
        .command(&["apply", "probe", "--without-dependencies", "--json"])
        .env_remove("KTM_THEME_NO_APPLY")
        .output()
        .unwrap();
    assert!(!out.status.success());
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(
        error.contains("External commands failed") && error.contains("rolled_back"),
        "{error}"
    );
    assert_eq!(fs::read(h.path(".config/kdeglobals")).unwrap(), before);
    assert_eq!(
        fs::metadata(h.path(".config/kdeglobals"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o640
    );
    assert_eq!(
        fs::read_to_string(h.path(".config/kde-theme-profile/active-profile")).unwrap(),
        "previous\n"
    );
    assert!(!h.path(".config/kde-theme-profile/apply-journal").exists());
    assert_eq!(h.records()["operations"][0]["recovery_errors"], json!([]));
}

#[test]
fn write_failure_after_snapshot_rolls_back_earlier_writes() {
    let h = Home::new();
    let before = fs::read(h.path(".config/kdeglobals")).unwrap();
    h.write(
        ".config/gtk-3.0/settings.ini",
        "[Settings]\ngtk-theme-name=Old\n",
    );
    h.write(
        ".config/kde-theme-profile/themes/probe/.config/gtk-3.0/settings.ini",
        "[Settings]\ngtk-theme-name=New\n",
    );
    h.executable("systemctl","#!/bin/sh\nif [ \"$2\" = stop ] && [ \"$3\" = plasma-plasmashell.service ]; then\n/bin/rm -f \"$HOME/.config/gtk-3.0/settings.ini\"\n/bin/mkdir \"$HOME/.config/gtk-3.0/settings.ini\"\nfi\nexit 0\n");
    let out = h
        .command(&["apply", "probe", "--without-dependencies", "--json"])
        .env_remove("KTM_THEME_NO_APPLY")
        .output()
        .unwrap();
    assert!(!out.status.success());
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(error.contains("rolled_back"), "{error}");
    assert_eq!(fs::read(h.path(".config/kdeglobals")).unwrap(), before);
    assert_eq!(
        fs::read_to_string(h.path(".config/gtk-3.0/settings.ini")).unwrap(),
        "[Settings]\ngtk-theme-name=Old\n"
    );
}

#[test]
fn concurrent_cli_core_and_inspector_are_refused_without_losing_staging() {
    let h = Home::new();
    let session = h.rpc("inspector.state", json!({}))["data"]["session"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        h.rpc(
            "inspector.stage",
            json!({"session":session,"id":"window_bg","value":"#112233"})
        )["data"]["ok"],
        true
    );
    let mut running = h.blocked_apply();
    assert_eq!(h.records()["busy"], true);
    assert_eq!(
        h.rpc(
            "profile.apply",
            json!({"name":"probe","skip_dependencies":true})
        )["ok"],
        false
    );
    let second = h.apply();
    assert!(!second.status.success());
    assert!(String::from_utf8_lossy(&second.stderr).contains("Another appearance operation"));
    assert_eq!(
        h.rpc("inspector.apply", json!({"session":session}))["ok"],
        false
    );
    assert_eq!(
        h.rpc("inspector.state", json!({"session":session}))["data"]["staged"]["window_bg"],
        "17,34,51"
    );
    let field = h.run(&["field", "set", "window_bg=#ffffff"]);
    assert!(!field.status.success());
    running.kill();
    assert!(h.records()["operations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["interrupted"] == true));
    let blocked = h.apply();
    assert!(String::from_utf8_lossy(&blocked.stderr).contains("requires recovery"));
}

#[test]
fn killed_apply_is_detected_and_default_recovery_preserves_later_personal_changes() {
    let h = Home::new();
    let before = fs::read(h.path(".config/kdeglobals")).unwrap();
    let mut running = h.blocked_apply();
    running.kill();
    let record = h.records()["operations"][0].clone();
    let id = record["id"].as_str().unwrap();
    assert_eq!(record["interrupted"], true);
    h.write(
        ".config/kdeglobals",
        "[General]\nBrowserApplication=edited-after-crash\n",
    );
    let recovery = h.run(&["operations", "recover", id]);
    assert!(!recovery.status.success());
    assert!(fs::read_to_string(h.path(".config/kdeglobals"))
        .unwrap()
        .contains("edited-after-crash"));
    let result: Value = serde_json::from_slice(&recovery.stdout).unwrap();
    assert_eq!(result["state"], "recovery_failed");
    assert!(!result["recovery_errors"].as_array().unwrap().is_empty());
    let forced = h.run(&["operations", "recover", id, "--force"]);
    assert!(
        forced.status.success(),
        "{} / {}",
        String::from_utf8_lossy(&forced.stdout),
        String::from_utf8_lossy(&forced.stderr)
    );
    assert_eq!(fs::read(h.path(".config/kdeglobals")).unwrap(), before);
    assert_eq!(
        h.records()["operations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap()["state"],
        "rolled_back"
    );
    assert!(PathBuf::from(record["backup"].as_str().unwrap()).is_dir());
    assert!(h.apply().status.success());
}

#[test]
fn corrupt_backup_is_reported_and_never_used_for_restoration() {
    let h = Home::new();
    let mut running = h.blocked_apply();
    running.kill();
    let record = h.records()["operations"][0].clone();
    let id = record["id"].as_str().unwrap();
    let files = PathBuf::from(record["backup"].as_str().unwrap()).join("transaction/files");
    let original = fs::read_dir(files).unwrap().next().unwrap().unwrap().path();
    fs::write(&original, "corrupt original").unwrap();
    let live = fs::read(h.path(".config/kdeglobals")).unwrap();
    let recovery = h.run(&["operations", "recover", id, "--force"]);
    assert!(!recovery.status.success());
    let result: Value = serde_json::from_slice(&recovery.stdout).unwrap();
    assert_eq!(result["state"], "recovery_failed");
    assert!(result["recovery_errors"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e.as_str().unwrap().contains("Corrupt backup")));
    assert_eq!(fs::read(h.path(".config/kdeglobals")).unwrap(), live);
    assert!(original.exists());
}

#[test]
fn interrupted_preparation_can_be_acknowledged_without_touching_live_files() {
    let h = Home::new();
    let live = fs::read(h.path(".config/kdeglobals")).unwrap();
    h.write(".config/kde-theme-profile/operations/apply-preparation.json", &json!({
        "id":"apply-preparation","profile":"probe","state":"preparing","started":"2026-11-08T00:00:00Z",
        "updated":"2026-11-08T00:00:00Z","backup":null,"error":null,"recovery_errors":[]
    }).to_string());
    assert_eq!(h.records()["operations"][0]["interrupted"], true);
    assert!(!h.apply().status.success());
    let out = h.run(&["operations", "recover", "apply-preparation"]);
    assert!(out.status.success());
    let result: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["state"], "cancelled_preparation");
    assert_eq!(fs::read(h.path(".config/kdeglobals")).unwrap(), live);
    assert!(h.apply().status.success());
}

#[test]
fn plasmashell_restart_failure_has_separate_recovery_diagnostics() {
    let h = Home::new();
    let before = fs::read(h.path(".config/kdeglobals")).unwrap();
    h.executable(
        "systemctl",
        "#!/bin/sh\nif [ \"$2\" = is-active ]; then exit 1; fi\nexit 0\n",
    );
    let out = h
        .command(&["apply", "probe", "--without-dependencies", "--json"])
        .env_remove("KTM_THEME_NO_APPLY")
        .output()
        .unwrap();
    assert!(!out.status.success());
    let record = &h.records()["operations"][0];
    assert_eq!(record["state"], "recovery_failed");
    assert!(record["error"].as_str().unwrap().contains("plasmashell"));
    assert!(record["recovery_errors"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e.as_str().unwrap().contains("plasmashell")));
    assert_eq!(fs::read(h.path(".config/kdeglobals")).unwrap(), before);
}

#[test]
fn broad_custom_component_cannot_copy_its_backup_into_itself() {
    let h = Home::new();
    h.write(
        ".config/kde-theme-profile/components.json",
        r#"{"components":[{"id":"broad","config_paths":[".config"]}]}"#,
    );
    let before = fs::read(h.path(".config/kdeglobals")).unwrap();
    let out = h.apply();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("footprint"));
    assert_eq!(fs::read(h.path(".config/kdeglobals")).unwrap(), before);
    assert!(h
        .path(".config/kde-theme-profile/backups")
        .read_dir()
        .unwrap()
        .next()
        .is_none());
}
