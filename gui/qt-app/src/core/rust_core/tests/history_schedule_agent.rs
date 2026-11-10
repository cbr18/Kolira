//! Public contracts in disposable HOME/XDG; no real desktop is modified.
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
};
use tempfile::TempDir;
struct Home(TempDir);
impl Home {
    fn new() -> Self {
        let h = Self(tempfile::tempdir().unwrap());
        h.write(
            ".config/kdeglobals",
            "[KDE]\nwidgetStyle=Old\n[General]\nBrowserApplication=personal\n",
        );
        h.write(".config/kolira/active-profile", "previous\n");
        h.write(
            ".config/kolira/themes/probe/theme.json",
            r#"{"schema":"kolira/theme@2","name":"probe"}"#,
        );
        h.write(
            ".config/kolira/themes/probe/.config/kdeglobals",
            "[KDE]\nwidgetStyle=Breeze\n",
        );
        h
    }
    fn path(&self, p: &str) -> PathBuf {
        self.0.path().join(p)
    }
    fn write(&self, p: &str, s: &str) {
        let p = self.path(p);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, s).unwrap();
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_kolira"));
        c.args(args)
            .env("HOME", self.0.path())
            .env("XDG_CONFIG_HOME", self.path(".config"))
            .env("XDG_DATA_HOME", self.path(".local/share"))
            .env("XDG_CACHE_HOME", self.path(".cache"))
            .env("KTM_THEME_NO_APPLY", "1")
            .env("KTM_OFFLINE", "1")
            .env("PATH", "/nonexistent");
        c
    }
    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }
    fn rpc(&self, method: &str, args: Value) -> Value {
        let mut c = self
            .command(&["core", method])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        c.stdin
            .take()
            .unwrap()
            .write_all(args.to_string().as_bytes())
            .unwrap();
        let out = c.wait_with_output().unwrap();
        serde_json::from_slice(&out.stdout)
            .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&out.stderr)))
    }
    fn ok(&self, method: &str, args: Value) -> Value {
        let v = self.rpc(method, args);
        assert_eq!(v["ok"], true, "{v}");
        v["data"].clone()
    }
}
#[test]
fn history_restores_completed_change_with_safety_copy_and_full_report() {
    let h = Home::new();
    let before = fs::read(h.path(".config/kdeglobals")).unwrap();
    let preview = h.ok("profile.preview", json!({"name":"probe"}));
    let applied = h.ok(
        "profile.apply",
        json!({"name":"probe","source":"mcp","skip_dependencies":true,"preview_token":preview["token"]}),
    );
    let history = h.ok("operation.list", json!({}));
    let row = &history["operations"][0];
    assert_eq!(row["id"], applied["operation_id"]);
    assert_eq!(row["source"], "mcp");
    assert_eq!(row["previous_profile"], "previous");
    assert_eq!(row["backup_status"], "available");
    assert!(row["report"]["changed_paths"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v.as_str().unwrap().ends_with("kdeglobals")));
    let id = row["id"].as_str().unwrap();
    assert_eq!(h.rpc("operation.restore", json!({"id":id}))["ok"], false);
    let restored = h.ok("operation.restore", json!({"id":id,"confirmed":true}));
    assert_eq!(fs::read(h.path(".config/kdeglobals")).unwrap(), before);
    assert!(PathBuf::from(restored["backup"].as_str().unwrap())
        .join("transaction/manifest.json")
        .is_file());
    assert_eq!(
        h.ok("operation.list", json!({}))["operations"][0]["restored_operation"],
        id
    );
}
#[test]
fn corrupt_completed_backup_disables_restore_without_touching_live_files() {
    let h = Home::new();
    let report = h.ok(
        "profile.apply",
        json!({"name":"probe","skip_dependencies":true}),
    );
    let dir = PathBuf::from(report["backup"].as_str().unwrap()).join("transaction/files");
    let path = fs::read_dir(dir).unwrap().next().unwrap().unwrap().path();
    fs::write(path, "corrupted").unwrap();
    let before = fs::read(h.path(".config/kdeglobals")).unwrap();
    assert_eq!(
        h.ok("operation.list", json!({}))["operations"][0]["can_restore"],
        false
    );
    assert_eq!(
        h.rpc(
            "operation.restore",
            json!({"id":report["operation_id"],"confirmed":true})
        )["ok"],
        false
    );
    assert_eq!(fs::read(h.path(".config/kdeglobals")).unwrap(), before);
}
#[test]
fn ten_completed_records_are_retained_but_backups_survive_eviction() {
    let h = Home::new();
    let first = h.ok(
        "profile.apply",
        json!({"name":"probe","skip_dependencies":true}),
    );
    for _ in 0..10 {
        h.ok(
            "profile.apply",
            json!({"name":"probe","skip_dependencies":true}),
        );
    }
    let rows = h.ok("operation.list", json!({}))["operations"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(rows.len(), 10);
    assert!(!rows.iter().any(|r| r["id"] == first["operation_id"]));
    assert!(PathBuf::from(first["backup"].as_str().unwrap()).is_dir());
}
#[test]
fn scheduler_claims_once_orders_conflicts_and_keeps_rules_after_restart() {
    use chrono::{Datelike, Local};
    let h = Home::new();
    let now = Local::now();
    let time = now.format("%H:%M").to_string();
    let day = now.weekday().number_from_monday();
    for id in ["a", "b"] {
        h.ok(
            "schedule.put",
            json!({"rule":{"id":id,"theme":"probe","time":time,"days":[day],"enabled":true}}),
        );
    }
    h.ok("schedule.enable", json!({"enabled":true}));
    assert!(h
        .path(".config/autostart/kolira-schedule.desktop")
        .is_file());
    let out = h.run(&["schedule", "tick"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result[0]["state"], "succeeded");
    assert_eq!(result[1]["state"], "skipped");
    let second = h.run(&["schedule", "tick"]);
    assert_eq!(
        serde_json::from_slice::<Value>(&second.stdout).unwrap(),
        json!([])
    );
    let state = h.ok("schedule.list", json!({}));
    assert_eq!(state["rules"].as_array().unwrap().len(), 2);
    assert_eq!(state["rules"][0]["last"]["state"], "succeeded");
    assert!(h.ok("operation.list", json!({}))["operations"]
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["source"] == "schedule"));
    assert_eq!(
        h.rpc(
            "schedule.put",
            json!({"rule":{"id":"bad","theme":"probe","time":"24:00","days":[1],"enabled":true}})
        )["ok"],
        false
    );
    h.ok("schedule.delete", json!({"id":"a"}));
    assert_eq!(
        h.ok("schedule.list", json!({}))["rules"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn offline_authoring_and_instruction_updates_preserve_live_state_and_user_edits() {
    let h = Home::new();
    let before = fs::read(h.path(".config/kdeglobals")).unwrap();
    h.ok("agent.install", json!({}));
    let file = h.path(".config/kolira/themes/AGENTS.md");
    let original = fs::read_to_string(&file).unwrap();
    fs::write(&file, format!("{original}\nUser additions\n")).unwrap();
    h.ok("agent.install", json!({}));
    assert!(fs::read_to_string(&file)
        .unwrap()
        .ends_with("User additions\n"));
    assert!(file.with_file_name("AGENTS.md.kit-new").is_file());
    let out = h.ok(
        "agent.prepare",
        json!({"name":"offline","values":{"widget_style":"Breeze"}}),
    );
    assert_eq!(out["live_appearance_changed"], false);
    assert_eq!(out["package"]["values"]["widget_style"], "Breeze");
    assert_eq!(fs::read(h.path(".config/kdeglobals")).unwrap(), before);
    assert_eq!(
        fs::read_to_string(h.path(".config/kolira/active-profile")).unwrap(),
        "previous\n"
    );
    assert_eq!(
        h.rpc(
            "agent.prepare",
            json!({"name":"../escape","values":{"widget_style":"Breeze"}})
        )["ok"],
        false
    );
    assert_eq!(
        h.rpc(
            "agent.prepare",
            json!({"name":"bad","values":{"unknown":"x"}})
        )["ok"],
        false
    );
    assert!(!h.path(".config/kolira/themes/bad").exists());
    let out = h.run(&[
        "export",
        "offline",
        &format!("--out={}", h.path("offline.zip").display()),
        "--json",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let zip = zip::ZipArchive::new(fs::File::open(h.path("offline.zip")).unwrap()).unwrap();
    assert!(!zip
        .file_names()
        .any(|n| n.ends_with("AGENTS.md") || n.ends_with("CLAUDE.md")));
}

#[test]
fn palette_authoring_uses_tracked_terminal_includes_and_keeps_requested_opacity() {
    let h = Home::new();
    let colors: Vec<_> = (0..16)
        .map(|i| format!("#{:02x}{:02x}{:02x}", i * 16, i * 16, i * 16))
        .collect();
    h.write(
        ".config/kolira/palette.json",
        &json!({"name":"test","variant":"dark","base":colors,"accent":"#ff8800"}).to_string(),
    );
    let result = h.ok(
        "agent.prepare",
        json!({"name":"palette-probe","values":{"kitty_opacity":0.85},"use_palette":true}),
    );
    assert_eq!(result["package"]["values"]["kitty_opacity"], 0.85);
    assert!(result["package"]["palette"].is_object());
    assert!(h
        .path(".config/kolira/themes/palette-probe/.config/kitty/kolira.conf")
        .is_file());
    assert!(h
        .path(".config/kolira/themes/palette-probe/.config/alacritty/kolira.toml")
        .is_file());
    assert!(!h.path(".config/kitty/kolira.conf").exists());
}

#[test]
fn forced_import_cannot_replace_root_workspace_instructions() {
    let h = Home::new();
    h.ok("agent.install", json!({}));
    let path = h.path(".config/kolira/themes/AGENTS.md");
    let original = fs::read(&path).unwrap();
    h.ok(
        "agent.prepare",
        json!({"name":"safe-export","values":{"widget_style":"Breeze"}}),
    );
    let archive = h.path("safe.zip");
    let out = h.run(&[
        "export",
        "safe-export",
        &format!("--out={}", archive.display()),
        "--json",
    ]);
    assert!(out.status.success());
    for name in [
        "AGENTS.md",
        "agents.md",
        "CLAUDE.md",
        "AGENTS.md.kit-new",
        "README.md",
        "readme.md",
        "README.md.kit-new",
    ] {
        let out = h.run(&[
            "import",
            archive.to_str().unwrap(),
            &format!("--name={name}"),
            "--force",
            "--json",
        ]);
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("reserved"));
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(
            h.rpc(
                "agent.prepare",
                json!({"name":name,"values":{"widget_style":"Breeze"}})
            )["ok"],
            false
        );
    }
}

#[test]
fn interrupted_record_survives_history_pruning_until_explicit_recovery() {
    let h = Home::new();
    for n in 0..12 {
        h.write(&format!(".config/kolira/operations/apply-old-{n}.json"),&json!({"id":format!("apply-old-{n}"),"profile":"probe","state":"succeeded","started":format!("2020-01-{:02}T00:00:00Z",n+1),"updated":"2020-01-01T00:00:00Z","backup":null,"error":null,"recovery_errors":[]}).to_string());
    }
    h.write(".config/kolira/operations/apply-pending.json",&json!({"id":"apply-pending","profile":"probe","state":"preparing","started":"2019-01-01T00:00:00Z","updated":"2019-01-01T00:00:00Z","backup":null,"error":null,"recovery_errors":[]}).to_string());
    let applied = h.rpc(
        "profile.apply",
        json!({"name":"probe","skip_dependencies":true}),
    );
    assert_eq!(applied["ok"], true);
    let rows = h.ok("operation.list", json!({}))["operations"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(rows.len(), 11);
    assert!(rows
        .iter()
        .any(|r| r["id"] == "apply-pending" && r["interrupted"] == true));
    h.ok("operation.recover", json!({"id":"apply-pending"}));
    assert_eq!(
        h.ok("operation.list", json!({}))["operations"]
            .as_array()
            .unwrap()
            .len(),
        10
    );
}

#[test]
fn failed_completed_restore_keeps_current_bytes_in_its_safety_transaction() {
    let h = Home::new();
    let applied = h.ok(
        "profile.apply",
        json!({"name":"probe","skip_dependencies":true}),
    );
    h.write(
        ".config/kdeglobals",
        "[KDE]\nwidgetStyle=Later\n[General]\nBrowserApplication=personal-later\n",
    );
    let later = fs::read(h.path(".config/kdeglobals")).unwrap();
    h.write(
        "bin/systemctl",
        "#!/bin/sh\ncase \"$*\" in *is-active*) exit 1;; esac\nexit 0\n",
    );
    fs::set_permissions(h.path("bin/systemctl"), fs::Permissions::from_mode(0o755)).unwrap();
    let result = h
        .command(&[
            "operations",
            "restore",
            applied["operation_id"].as_str().unwrap(),
            "--confirm",
        ])
        .env_remove("KTM_THEME_NO_APPLY")
        .env("PATH", h.path("bin"))
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(fs::read(h.path(".config/kdeglobals")).unwrap(), later);
    let rows = h.ok("operation.list", json!({}));
    let restore = rows["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["restored_operation"] == applied["operation_id"])
        .unwrap();
    assert_eq!(restore["state"], "recovery_failed");
    assert!(!restore["recovery_errors"].as_array().unwrap().is_empty());
    assert_eq!(restore["backup_status"], "available");
}

#[test]
fn missing_plasma_is_a_visible_skip_and_deleted_theme_is_an_audited_failure() {
    use chrono::{Datelike, Local};
    for missing_session in [true, false] {
        let h = Home::new();
        let now = Local::now();
        h.ok("schedule.put",json!({"rule":{"id":"probe","theme":"probe","time":now.format("%H:%M").to_string(),"days":[now.weekday().number_from_monday()],"enabled":true}}));
        h.ok("schedule.enable", json!({"enabled":true}));
        let before = fs::read(h.path(".config/kdeglobals")).unwrap();
        let mut command = h.command(&["schedule", "tick"]);
        if missing_session {
            command
                .env_remove("KTM_THEME_NO_APPLY")
                .env("XDG_CURRENT_DESKTOP", "unavailable");
        } else {
            fs::rename(
                h.path(".config/kolira/themes/probe"),
                h.path(".config/kolira/themes/renamed"),
            )
            .unwrap();
        }
        let result = command.output().unwrap();
        assert!(result.status.success());
        let result: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(
            result[0]["state"],
            if missing_session { "skipped" } else { "failed" }
        );
        let history = h.ok("operation.list", json!({}));
        assert_eq!(history["operations"][0]["id"], result[0]["operation_id"]);
        assert_eq!(history["operations"][0]["error"].is_null(), missing_session);
        assert_eq!(fs::read(h.path(".config/kdeglobals")).unwrap(), before);
    }
}

#[test]
fn import_rejects_packaged_client_instructions_and_hooks_before_publication() {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let h = Home::new();
    h.ok(
        "agent.prepare",
        json!({"name":"safe-export","values":{"widget_style":"Breeze"}}),
    );
    let archive = h.path("safe.zip");
    assert!(h
        .run(&[
            "export",
            "safe-export",
            &format!("--out={}", archive.display()),
            "--json"
        ])
        .status
        .success());
    let mut reader = zip::ZipArchive::new(fs::File::open(&archive).unwrap()).unwrap();
    let mut entries = vec![];
    for n in 0..reader.len() {
        let mut entry = reader.by_index(n).unwrap();
        let name = entry.name().to_string();
        let mut bytes = vec![];
        entry.read_to_end(&mut bytes).unwrap();
        entries.push((name, bytes));
    }
    for (n, path) in [
        "AGENTS.md",
        "AGENTS.override.md",
        "CLAUDE.md",
        ".claude/settings.json",
        ".codex/config.toml",
        ".git/config",
        ".local/share/plasma/look-and-feel/example/AGENTS.md",
    ]
    .iter()
    .enumerate()
    {
        let bad = h.path("bad.zip");
        let mut writer = zip::ZipWriter::new(fs::File::create(&bad).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (key, bytes) in &entries {
            let bytes = if key == "export.json" {
                let mut manifest: Value = serde_json::from_slice(bytes).unwrap();
                manifest["files"][path.to_string()] =
                    json!(format!("{:x}", Sha256::digest(b"untrusted instructions")));
                serde_json::to_vec(&manifest).unwrap()
            } else {
                bytes.clone()
            };
            writer.start_file(key, options).unwrap();
            writer.write_all(&bytes).unwrap();
        }
        writer.start_file(*path, options).unwrap();
        writer.write_all(b"untrusted instructions").unwrap();
        writer.finish().unwrap();
        let name = format!("blocked-{n}");
        let result = h.run(&[
            "import",
            bad.to_str().unwrap(),
            &format!("--name={name}"),
            "--json",
        ]);
        assert!(!result.status.success(), "{path}");
        assert!(String::from_utf8_lossy(&result.stderr).contains("not theme content"));
        assert!(!h.path(&format!(".config/kolira/themes/{name}")).exists());
    }
}

#[test]
fn mcp_apply_requires_preview_token_before_starting_transaction() {
    let h = Home::new();
    let before = fs::read(h.path(".config/kdeglobals")).unwrap();
    let result = h.rpc("profile.apply", json!({"name":"probe","source":"mcp"}));
    assert_eq!(result["ok"], false);
    assert!(result["error"].as_str().unwrap().contains("preview_token"));
    assert_eq!(fs::read(h.path(".config/kdeglobals")).unwrap(), before);
    assert!(h.ok("operation.list", json!({}))["operations"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn workspace_readme_install_preserves_edits_and_permissions() {
    let h = Home::new();
    let installed = h.ok("agent.install", json!({}));
    assert_eq!(installed["version"], 2);
    let path = h.path(".config/kolira/themes/README.md");
    let canonical = fs::read_to_string(&path).unwrap();
    assert!(canonical.contains("kolira core agent.prepare"));
    assert!(canonical.contains("previewToken"));
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    h.ok("agent.install", json!({}));
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    fs::write(&path, "Personal workspace documentation\n").unwrap();
    h.ok("agent.install", json!({}));
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "Personal workspace documentation\n"
    );
    assert_eq!(
        fs::read_to_string(path.with_file_name("README.md.kit-new")).unwrap(),
        canonical
    );
}
#[test]
fn bundled_sync_follows_a_theme_that_matches_the_kit() {
    use sha2::{Digest, Sha256};
    let h = Home::new();
    h.ok(
        "agent.prepare",
        json!({"name":"kitme","values":{"widget_style":"Breeze"}}),
    );
    let kit = h.path("kit");
    let kit_theme = kit.join("kitme");
    let out = h.run(&["export", "kitme", &format!("--dir={}", kit_theme.display())]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let sync = || {
        let out = h.run(&["bundled", "sync", kit.to_str().unwrap()]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    // Installed copy is exactly the kit (as after `import --force`): not "kept".
    assert_eq!(sync(), "");

    // A new kit version replaces an unchanged copy.
    let bump = |text: &str| {
        let rel = ".config/kdeglobals";
        fs::write(kit_theme.join(rel), text).unwrap();
        let manifest_path = kit_theme.join("export.json");
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        manifest["files"][rel] = json!(format!("{:x}", Sha256::digest(text.as_bytes())));
        fs::write(&manifest_path, manifest.to_string()).unwrap();
    };
    bump("[KDE]\nwidgetStyle=Fusion\n");
    assert_eq!(sync().trim(), "Updated theme: kitme");
    let installed = h.path(".config/kolira/themes/kitme/.config/kdeglobals");
    assert!(fs::read_to_string(&installed).unwrap().contains("Fusion"));

    // A user's edit is still kept when the kit moves on.
    fs::write(&installed, "[KDE]\nwidgetStyle=Mine\n").unwrap();
    bump("[KDE]\nwidgetStyle=Oxygen\n");
    assert!(sync().starts_with("Kept theme kitme"));
    assert!(fs::read_to_string(&installed).unwrap().contains("Mine"));
}
