//! Cross-process contracts of the single Rust GUI/MCP core.
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
};
use tempfile::TempDir;
struct Home(TempDir);
impl Home {
    fn new() -> Self {
        Self(tempfile::tempdir().unwrap())
    }
    fn path(&self, p: &str) -> PathBuf {
        self.0.path().join(p)
    }
    fn write(&self, p: &str, text: &str) {
        let path = self.path(p);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn command(&self, method: &str) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_kolira"));
        c.args(["core", method])
            .env("HOME", self.0.path())
            .env("XDG_CONFIG_HOME", self.path(".config"))
            .env("XDG_DATA_HOME", self.path(".local/share"))
            .env("XDG_CACHE_HOME", self.path(".cache"))
            .env("PATH", self.path("bin"))
            .env("KTM_THEME_NO_APPLY", "1")
            .env("KTM_OFFLINE", "1")
            .env_remove("WALLPAPER_DIR")
            .env_remove("WALLPAPER_DRY_RUN");
        c
    }
    fn send(mut c: Command, args: Value) -> Value {
        let mut p = c
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        p.stdin
            .take()
            .unwrap()
            .write_all(args.to_string().as_bytes())
            .unwrap();
        let out = p.wait_with_output().unwrap();
        let value: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
            panic!(
                "Invalid core output {e}: {} / {}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            )
        });
        assert_eq!(value["api"], 1);
        assert_eq!(out.status.success(), value["ok"] == true);
        value
    }
    fn raw(&self, m: &str, a: Value) -> Value {
        Self::send(self.command(m), a)
    }
    fn rpc(&self, m: &str, a: Value) -> Value {
        let v = self.raw(m, a);
        assert_eq!(v["ok"], true, "{m}: {v}");
        v["data"].clone()
    }
    fn init(&self) -> String {
        self.rpc("inspector.state", json!({}))["session"]
            .as_str()
            .unwrap()
            .into()
    }
    fn seed(&self) {
        self.write(".config/kdeglobals","[General]\nColorScheme=Base\nAccentColor=1,2,3\n\n[Colors:Window]\nBackgroundNormal=32,36,31\nForegroundNormal=250,250,250\n");
    }
    fn register(&self) {
        self.write(
            ".config/kolira/components.json",
            r#"{"components":[{"id":"mockapp","name":"Mock","config_paths":[".config/mockapp"]}]}"#,
        );
    }
    fn executable(&self, name: &str, script: &str) {
        self.write(&format!("bin/{name}"), script);
        fs::set_permissions(
            self.path(&format!("bin/{name}")),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
}

#[test]
fn settings_keep_foreign_keys_validate_types_and_private_mode() {
    let h = Home::new();
    h.write(
        ".config/kolira/app-preferences.json",
        r#"{"automation":{"store":"never"},"future":{"keep":true}}"#,
    );
    h.rpc(
        "settings.set",
        json!({"settings":{"theme":"paper","core_backend":"rust","automation":{"store":"always"}}}),
    );
    let saved = h.rpc("settings.get", json!({}));
    assert_eq!(saved["theme"], "paper");
    assert_eq!(saved["core_backend"], "core");
    assert_eq!(saved["automation"]["store"], "never");
    assert_eq!(saved["future"]["keep"], true);
    let before = fs::read(h.path(".config/kolira/app-preferences.json")).unwrap();
    assert_eq!(
        h.raw("settings.set", json!({"settings":{"theme":42}}))["ok"],
        false
    );
    assert_eq!(
        fs::read(h.path(".config/kolira/app-preferences.json")).unwrap(),
        before
    );
    assert_eq!(
        fs::metadata(h.path(".config/kolira/app-preferences.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn settings_updates_and_automation_share_one_lock() {
    let h = Home::new();
    let mut handles = vec![];
    for i in 0..8 {
        let c = h.command("settings.set");
        handles.push(thread::spawn(move || {
            Home::send(
                c,
                json!({"settings":{"theme":if i%2==0{"paper"}else{"dark"}}}),
            )
        }));
    }
    let mut auto = Command::new(env!("CARGO_BIN_EXE_kolira"));
    auto.args(["automation", "set", "store=never"])
        .env("HOME", h.0.path())
        .env("XDG_CONFIG_HOME", h.path(".config"));
    let mut child = auto.spawn().unwrap();
    for handle in handles {
        assert_eq!(handle.join().unwrap()["ok"], true);
    }
    assert!(child.wait().unwrap().success());
    assert_eq!(
        h.rpc("settings.get", json!({}))["automation"]["store"],
        "never"
    );
}

#[test]
fn session_stages_across_processes_applies_undoes_and_reverts() {
    let h = Home::new();
    h.seed();
    let original = fs::read(h.path(".config/kdeglobals")).unwrap();
    let id = h.init();
    let staged = h.rpc(
        "inspector.stage",
        json!({"session":id,"id":"window_bg","value":"#112233"}),
    );
    assert_eq!(staged["ok"], true);
    assert_eq!(staged["undoDepth"], 0);
    assert_eq!(fs::read(h.path(".config/kdeglobals")).unwrap(), original);
    let result = h.rpc("inspector.apply", json!({"session":id}));
    assert_eq!(result["ok"], true);
    assert_eq!(result["undoDepth"], 1);
    assert_eq!(h.rpc("field.read", json!({"id":"window_bg"})), "17,34,51");
    assert_eq!(h.rpc("inspector.undo", json!({"session":id}))["ok"], true);
    assert_eq!(h.rpc("field.read", json!({"id":"window_bg"})), "32,36,31");
    assert_eq!(h.rpc("inspector.revert", json!({"session":id}))["ok"], true);
    assert_eq!(fs::read(h.path(".config/kdeglobals")).unwrap(), original);
}

#[test]
fn staging_order_and_key_specific_undo_survive_a_batch() {
    let h = Home::new();
    h.seed();
    let id = h.init();
    h.rpc(
        "inspector.stage",
        json!({"session":id,"id":"text_color","value":"#445566"}),
    );
    h.rpc(
        "inspector.stage",
        json!({"session":id,"id":"window_bg","value":"#112233"}),
    );
    assert_eq!(h.rpc("inspector.apply", json!({"session":id}))["ok"], true);
    h.rpc("inspector.undo", json!({"session":id}));
    assert_eq!(h.rpc("field.read", json!({"id":"window_bg"})), "32,36,31");
    assert_eq!(h.rpc("field.read", json!({"id":"text_color"})), "68,85,102");
}

#[test]
fn concurrent_staging_has_monotonic_revisions_and_loses_no_changes() {
    let h = Home::new();
    h.seed();
    let id = h.init();
    let mut handles = vec![];
    for field in ["window_bg", "text_color", "selection_bg", "selection_text"] {
        let c = h.command("inspector.stage");
        let id = id.clone();
        handles.push(thread::spawn(move || {
            Home::send(c, json!({"session":id,"id":field,"value":"#112233"}))
        }));
    }
    let mut revisions = vec![];
    for handle in handles {
        let result = handle.join().unwrap();
        assert_eq!(result["data"]["ok"], true);
        revisions.push(result["data"]["revision"].as_u64().unwrap());
    }
    revisions.sort();
    revisions.dedup();
    assert_eq!(revisions.len(), 4);
    assert_eq!(
        h.rpc("inspector.state", json!({"session":id}))["staged"]
            .as_object()
            .unwrap()
            .len(),
        4
    );
}

#[test]
fn app_success_cannot_mask_a_failed_field_and_no_profile_is_saved() {
    let h = Home::new();
    h.seed();
    h.executable("code", "#!/bin/sh\nexit 0\n");
    h.write(
        ".config/Code/User/settings.json",
        r#"{"workbench.colorTheme":"old"}"#,
    );
    let id = h.init();
    assert_eq!(
        h.rpc(
            "inspector.component",
            json!({"session":id,"id":"vscode","value":"new"})
        )["ok"],
        true
    );
    h.rpc(
        "inspector.stage",
        json!({"session":id,"id":"accent_color","value":"#112233"}),
    );
    fs::remove_file(h.path(".config/kdeglobals")).unwrap();
    fs::create_dir(h.path(".config/kdeglobals")).unwrap();
    let result = h.rpc("inspector.apply", json!({"session":id}));
    assert_eq!(result["ok"], false);
    assert!(result["staged"].get("accent_color").is_some());
    let created = h.rpc(
        "inspector.create_profile",
        json!({"session":id,"name":"failed","components":{"color_scheme":"Base"}}),
    );
    assert_eq!(created["ok"], false);
    assert!(!h.path(".config/kolira/themes/failed").exists());
}

#[test]
fn invalid_composite_request_does_not_stage_or_change_apps() {
    let h = Home::new();
    h.seed();
    h.executable("code", "#!/bin/sh\nexit 0\n");
    let old = r#"{"workbench.colorTheme":"old"}"#;
    h.write(".config/Code/User/settings.json", old);
    let id = h.init();
    for args in [
        json!({"session":id,"name":"../bad","applications":{"vscode":"new"}}),
        json!({"session":id,"name":"valid","components":{"color_scheme":"Base","unknown":"x"},"applications":{"vscode":"new"}}),
    ] {
        let result = h.rpc("inspector.create_profile", args);
        assert_eq!(result["ok"], false);
        assert!(result["staged"].as_object().unwrap().is_empty());
        assert_eq!(
            fs::read_to_string(h.path(".config/Code/User/settings.json")).unwrap(),
            old
        );
    }
}

#[test]
fn toml_arrays_nested_tables_comments_and_json_order_are_preserved() {
    let h = Home::new();
    h.register();
    let id = h.init();
    h.write(
        ".config/mockapp/settings.toml",
        "items = [1, 2] # keep array\n[a.b]\nvalue = 1 # keep nested\n",
    );
    for (key, value) in [("items", "[3,4,5]"), ("a/b/value", "9")] {
        assert_eq!(
            h.rpc(
                "inspector.config",
                json!({"session":id,"path":".config/mockapp/settings.toml","key":key,"value":value})
            )["ok"],
            true
        );
    }
    assert_eq!(h.rpc("inspector.apply", json!({"session":id}))["ok"], true);
    let text = fs::read_to_string(h.path(".config/mockapp/settings.toml")).unwrap();
    let value: toml::Value = text.parse().unwrap();
    assert_eq!(value["items"].as_array().unwrap().len(), 3);
    assert_eq!(value["a"]["b"]["value"].as_integer(), Some(9));
    assert!(text.contains("# keep array") && text.contains("# keep nested"));
    h.write(
        ".config/mockapp/order.jsonc",
        "{\n// keep\n\"z\":1,\"a\":2,\"m\":3,\n}",
    );
    let params = h.rpc(
        "config.params",
        json!({"file":h.path(".config/mockapp/order.jsonc"),"limit":2}),
    );
    assert_eq!(params["params"][0]["key"], "z");
    assert_eq!(params["params"][1]["key"], "a");
    assert_eq!(params["truncated"], true);
}

#[test]
fn unregistered_paths_and_prefs_js_are_not_writable() {
    let h = Home::new();
    h.register();
    h.write(".ssh/config", "key value\n");
    h.write(".config/mockapp/prefs.js", "user_pref(\"a\", 1);\n");
    let id = h.init();
    for path in [
        ".ssh/config",
        ".config/mockapp/prefs.js",
        ".config/mockapp/../../.ssh/config",
    ] {
        assert_eq!(
            h.rpc(
                "inspector.config",
                json!({"session":id,"path":path,"key":"a","value":"2"})
            )["ok"],
            false
        );
    }
    assert_eq!(
        fs::read_to_string(h.path(".ssh/config")).unwrap(),
        "key value\n"
    );
}

#[test]
fn config_keys_and_filenames_with_newlines_can_be_unstaged() {
    let h = Home::new();
    h.register();
    let id = h.init();
    let path = ".config/mockapp/line\nbreak.json";
    h.write(path, "{\"a\\nb\":1}");
    let result = h.rpc(
        "inspector.config",
        json!({"session":id,"path":path,"key":"a\nb","value":"2"}),
    );
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(h.rpc("inspector.undo", json!({"session":id}))["ok"], true);
    assert!(h.rpc("inspector.state", json!({"session":id}))["configs"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn failed_undo_keeps_its_history_for_retry() {
    let h = Home::new();
    h.seed();
    let id = h.init();
    h.rpc(
        "inspector.stage",
        json!({"session":id,"id":"window_bg","value":"#112233"}),
    );
    h.rpc("inspector.apply", json!({"session":id}));
    fs::rename(h.path(".config/kdeglobals"), h.path(".config/saved")).unwrap();
    fs::create_dir(h.path(".config/kdeglobals")).unwrap();
    let result = h.rpc("inspector.undo", json!({"session":id}));
    assert_eq!(result["ok"], false);
    assert_eq!(result["undoDepth"], 1);
    fs::remove_dir(h.path(".config/kdeglobals")).unwrap();
    fs::rename(h.path(".config/saved"), h.path(".config/kdeglobals")).unwrap();
    assert_eq!(h.rpc("inspector.undo", json!({"session":id}))["ok"], true);
}

#[test]
fn kvantum_batch_revert_restores_the_base_actually_edited() {
    let h = Home::new();
    h.seed();
    h.write(".config/Kvantum/kvantum.kvconfig", "[General]\ntheme=Old\n");
    h.write(
        ".config/Kvantum/Old/Old.kvconfig",
        "[%General]\nblurring=false\n",
    );
    let other = "[%General]\nblurring=false\n# keep\n";
    h.write(".config/Kvantum/New/New.kvconfig", other);
    let id = h.init();
    h.rpc(
        "inspector.stage",
        json!({"session":id,"id":"kvantum_theme","value":"New"}),
    );
    h.rpc(
        "inspector.stage",
        json!({"session":id,"id":"kvantum_blurring","value":true}),
    );
    assert_eq!(h.rpc("inspector.apply", json!({"session":id}))["ok"], true);
    assert!(
        fs::read_to_string(h.path(".config/Kvantum/New/New.kvconfig"))
            .unwrap()
            .contains("blurring=true")
    );
    assert_eq!(h.rpc("inspector.revert", json!({"session":id}))["ok"], true);
    assert_eq!(
        fs::read_to_string(h.path(".config/Kvantum/New/New.kvconfig")).unwrap(),
        other
    );
    assert_eq!(h.rpc("field.read", json!({"id":"kvantum_theme"})), "Old");
}

#[test]
fn closing_session_removes_cache_and_keeps_private_backups() {
    let h = Home::new();
    h.seed();
    fs::set_permissions(
        h.path(".config/kdeglobals"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let id = h.init();
    h.rpc(
        "inspector.stage",
        json!({"session":id,"id":"window_bg","value":"#112233"}),
    );
    h.rpc("inspector.apply", json!({"session":id}));
    let backup = h.path(&format!(".config/kolira/backups/{id}"));
    assert_eq!(
        fs::metadata(&backup).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(backup.join(".config/kdeglobals"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        h.rpc("inspector.close", json!({"session":id}))["closed"],
        true
    );
    assert!(!h
        .path(&format!(".cache/kolira/inspector-sessions/{id}"))
        .exists());
    assert!(backup.is_dir());
    assert_eq!(h.raw("inspector.state", json!({"session":id}))["ok"], false);
}

#[test]
fn no_apply_disables_wallpaper_and_desktop_hooks() {
    let h = Home::new();
    let script = "#!/bin/sh\nprintf called >> \"$HOME/invoked\"\n";
    for name in [
        "plasma-apply-wallpaperimage",
        "qdbus6",
        "systemctl",
        "wofi",
        "magick",
    ] {
        h.executable(name, script);
    }
    h.rpc("wallpaper.set", json!({"path":"/not/a/real/image.png"}));
    h.rpc("desktop.load_blur", json!({}));
    h.rpc("desktop.recover", json!({}));
    assert!(!h.path("invoked").exists());
    let mut picker = Command::new(env!("CARGO_BIN_EXE_wallpaper-picker"));
    picker
        .env("HOME", h.0.path())
        .env("KTM_THEME_NO_APPLY", "1")
        .env("PATH", h.path("bin"));
    assert!(picker.output().unwrap().status.success());
    assert!(!h.path("invoked").exists());
}

#[test]
fn active_marker_clearing_does_not_delete_a_valid_new_selection() {
    let h = Home::new();
    h.write(".config/kolira/active-profile", "gone\n");
    assert_eq!(h.rpc("profile.clear_stale", json!({"name":"gone"})), true);
    h.write(".config/kolira/active-profile", "current\n");
    fs::create_dir_all(h.path(".config/kolira/themes/current")).unwrap();
    assert_eq!(h.rpc("profile.clear_stale", json!({"name":"gone"})), false);
    assert_eq!(h.rpc("profile.active", json!({})), "current");
}

#[test]
fn unknown_methods_and_wrong_envelopes_are_errors() {
    let h = Home::new();
    assert_eq!(h.raw("not-a-method", json!({}))["ok"], false);
    assert_eq!(h.raw("settings.get", json!([]))["ok"], false);
    assert_eq!(
        h.raw("profile.selection", json!({"components":[]}))["ok"],
        false
    );
}

#[test]
fn summary_batch_reads_snapshots_and_isolates_damaged_entries() {
    let home = Home::new();
    home.write(
        ".config/kdeglobals",
        "[General]\nColorScheme=LiveMustNotLeak\n",
    );
    home.write(".config/kolira/themes/saved/.config/kdeglobals", "[General]\nColorScheme=Saved\nAccentColor=10,20,30\n[Colors:Window]\nBackgroundNormal=32,36,31\n");
    home.write(".config/kolira/themes/package/theme.json", r##"{"schema":"kolira/theme@2","name":"package","values":{"color_scheme":"Package","accent_color":"#112233"}}"##);
    home.write(".config/kolira/themes/broken/theme.json", "{bad");
    std::os::unix::fs::symlink(home.0.path(), home.path(".config/kolira/themes/link")).unwrap();
    let data = home.rpc(
        "profile.summaries",
        json!({"names":["saved","package","broken","missing","link"]}),
    );
    assert_eq!(data["saved"]["scheme"], "Saved");
    assert_eq!(data["saved"]["accent"], "#0a141e");
    assert_eq!(data["saved"]["swatches"][0], "#20241f");
    assert_eq!(data["package"]["accent"], "#112233");
    assert_eq!(data["saved"]["fileCount"], 1);
    for name in ["broken", "missing", "link"] {
        assert!(data[name]["error"].is_string());
    }
    assert_eq!(
        home.rpc("profile.summaries", json!({"names":[]})),
        json!({})
    );
    assert_eq!(
        home.raw("profile.summaries", json!({"names":["../escape"]}))["ok"],
        false
    );
}
#[test]
fn app_errors_have_stable_templates_and_english_compatibility() {
    let home = Home::new();
    let data = home.rpc("app.current", json!({"id":"zed"}));
    assert_eq!(data["message_template"], "Settings file not found: %1");
    assert_eq!(data["message_args"][0], ".config/zed/settings.json");
    assert_eq!(
        data["message"],
        "Settings file not found: .config/zed/settings.json"
    );
}
