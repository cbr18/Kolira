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
    h.rpc(
        "inspector.stage",
        json!({"session":id,"id":"window_bg","value":"#566778"}),
    );
    let discarded = h.rpc("inspector.discard", json!({"session":id}));
    assert_eq!(discarded["draft"], json!([]));
    assert_eq!(discarded["undoDepth"], 1);
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
fn failed_live_apply_restores_files_and_preserves_the_draft_for_retry() {
    let h = Home::new();
    h.seed();
    let kdeglobals = h.path(".config/kdeglobals");
    fs::set_permissions(&kdeglobals, fs::Permissions::from_mode(0o640)).unwrap();
    let original = fs::read(&kdeglobals).unwrap();
    let settings = ".config/Code/User/settings.json";
    let initial = "{\n  // Personal formatting\n  \"nested\": { \"key\": \"original\" }\n}\n";
    h.write(settings, initial);
    let id = h.init();
    assert_eq!(
        h.rpc(
            "inspector.stage",
            json!({"session":id,"id":"icon_theme","value":"Papirus"})
        )["ok"],
        true
    );
    for (key, value) in [("nested", "flat"), ("nested/key", "replacement")] {
        assert_eq!(
            h.rpc(
                "inspector.config",
                json!({"session":id,"path":settings,"key":key,"value":value})
            )["ok"],
            true
        );
    }
    let before = h.rpc("inspector.state", json!({"session":id}));

    let result = h.rpc(
        "inspector.apply",
        json!({"session":id,"check_live":true,"source":"gui"}),
    );
    assert_eq!(result["ok"], false, "{result}");
    assert!(result["message"]
        .as_str()
        .unwrap()
        .contains("JSON parent must be an object"));
    assert_eq!(result["draft"], before["draft"]);
    assert_eq!(result["undoDepth"], before["undoDepth"]);
    assert_eq!(fs::read(&kdeglobals).unwrap(), original);
    assert_eq!(
        fs::metadata(&kdeglobals).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert_eq!(fs::read_to_string(h.path(settings)).unwrap(), initial);
    assert!(!h.path(".config/gtk-3.0").exists());
    assert!(!h.path(".config/gtk-4.0").exists());
    let operations = h.rpc("operation.list", json!({}));
    let record = &operations["operations"][0];
    assert_eq!(record["state"], "rolled_back", "{record}");
    assert_eq!(record["source"], "gui");
    assert_eq!(record["report"]["kind"], "live");
    let changes = record["report"]["changed_paths"].to_string();
    assert!(
        changes.contains("kdeglobals") && changes.contains("settings.json"),
        "{record}"
    );

    let fixed = h.rpc(
        "inspector.config",
        json!({"session":id,"path":settings,"key":"nested/key","value":"original"}),
    );
    assert_eq!(fixed["draft"].as_array().unwrap().len(), 2);
    let retried = h.rpc(
        "inspector.apply",
        json!({"session":id,"check_live":true,"source":"gui"}),
    );
    assert_eq!(retried["ok"], true, "{retried}");
    assert_eq!(retried["draft"], json!([]));
    assert_ne!(fs::read(&kdeglobals).unwrap(), original);
    let operations = h.rpc("operation.list", json!({}));
    assert_eq!(operations["operations"][0]["state"], "succeeded");
    assert!(operations["last_profile_apply"].is_null());
    let operation = operations["operations"][0]["id"].as_str().unwrap();
    let added_later = ".config/gtk-3.0/gtk.css";
    h.write(added_later, "/* personal style added after Apply */");
    h.rpc(
        "operation.restore",
        json!({"id":operation,"confirmed":true,"source":"gui"}),
    );
    assert_eq!(fs::read(&kdeglobals).unwrap(), original);
    assert_eq!(fs::read_to_string(h.path(settings)).unwrap(), initial);
    assert!(!h.path(".config/gtk-3.0/settings.ini").exists());
    assert_eq!(
        fs::read_to_string(h.path(added_later)).unwrap(),
        "/* personal style added after Apply */"
    );
    assert!(!h.path(".config/gtk-4.0").exists());
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
fn discarding_one_live_category_keeps_other_draft_changes_and_live() {
    let h = Home::new();
    h.seed();
    let id = h.init();
    let kdeglobals = h.path(".config/kdeglobals");
    let original = fs::read(&kdeglobals).unwrap();
    for (field, value) in [("window_bg", json!("#112233")), ("cursor_size", json!(32))] {
        assert_eq!(
            h.rpc(
                "inspector.stage",
                json!({"session":id,"id":field,"value":value})
            )["ok"],
            true
        );
    }
    let result = h.rpc(
        "inspector.discard",
        json!({"session":id,"section":"colors"}),
    );
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["staged"], json!({"cursor_size":32}));
    assert_eq!(fs::read(&kdeglobals).unwrap(), original);
    assert!(!h.path(".config/kcminputrc").exists());
}

#[test]
fn saved_theme_edit_preserves_live_and_exclusion_restores_the_previous_cursor() {
    let h = Home::new();
    h.seed();
    let config = ".config/kcminputrc";
    let original = "[Mouse]\ncursorTheme=OriginalCursor\ncursorSize=25\n";
    h.write(config, original);
    let theme = ".config/kolira/themes/first";
    let manifest = format!("{theme}/theme.json");
    h.write(&manifest, r#"{"schema":"kolira/theme@2","name":"first","description":"Keep my description","appearance_only":true,"values":{"animation_factor":1.0},"unknown_future_value":{"keep":true}}"#);
    h.write(
        &format!("{theme}/.config/kdeglobals"),
        "[KDE]\nAnimationDurationFactor=1\n[Icons]\nTheme=breeze\n",
    );
    h.write(&format!("{theme}/personal-note"), "Keep this file");
    h.write(".local/share/icons/TestCursor/cursors/left_ptr", "test");
    let opened = h.rpc("bundle.editor", json!({"name":"first"}));
    assert!(opened["values"]["cursor_theme"].is_null());

    let result = h.rpc("bundle.save_edits", json!({"name":"first","revision":opened["revision"],"changes":{"cursor_theme":"TestCursor","animations":0.0}}));
    assert_eq!(result["values"]["cursor_theme"], "TestCursor");
    assert_eq!(result["values"]["animations"], 0.0);
    assert_eq!(fs::read_to_string(h.path(config)).unwrap(), original);
    assert_eq!(h.rpc("profile.active", json!({})), "");
    assert_eq!(
        fs::read_to_string(h.path(format!("{theme}/personal-note").as_str())).unwrap(),
        "Keep this file"
    );
    let stored: Value =
        serde_json::from_str(&fs::read_to_string(h.path(&manifest)).unwrap()).unwrap();
    assert_eq!(stored["unknown_future_value"]["keep"], true);
    assert_eq!(stored["description"], "Keep my description");
    assert_eq!(stored["dependencies"][0]["name"], "TestCursor");

    let stale = h.raw(
        "bundle.save_edits",
        json!({"name":"first","revision":opened["revision"],"changes":{"cursor_theme":null}}),
    );
    assert_eq!(stale["ok"], false);
    assert!(stale["error"]
        .as_str()
        .unwrap()
        .contains("Theme changed on disk"));

    h.rpc(
        "profile.apply",
        json!({"name":"first","skip_dependencies":true}),
    );
    assert_eq!(
        h.rpc("field.read", json!({"id":"cursor_theme"})),
        "TestCursor"
    );
    let removed = h.rpc(
        "bundle.save_edits",
        json!({"name":"first","revision":result["revision"],"changes":{"cursor_theme":null}}),
    );
    assert!(removed["values"]["cursor_theme"].is_null());
    assert_eq!(
        h.rpc("field.read", json!({"id":"cursor_theme"})),
        "TestCursor"
    );
    assert!(!h.path(&format!("{theme}/{config}")).exists());
    assert!(!h
        .path(&format!("{theme}/.config/gtk-3.0/settings.ini"))
        .exists());
    h.rpc(
        "profile.apply",
        json!({"name":"first","skip_dependencies":true}),
    );
    assert_eq!(fs::read_to_string(h.path(config)).unwrap(), original);
}

#[test]
fn saved_theme_resource_edits_preserve_bundled_files_and_live_font_size() {
    let h = Home::new();
    let desktop = ".config/kdeglobals";
    let initial = "[General]\nfont=Desktop Font,13,-1,5,50,0,0,0,0,0\n";
    h.write(desktop, initial);
    let profile = ".config/kolira/themes/edit-me";
    let manifest = format!("{profile}/theme.json");
    let asset_path = ".local/share/icons/LocalIcons";
    h.write(
        &manifest,
        &json!({"schema":"kolira/theme@2","name":"edit-me","appearance_only":true,
            "values":{"icon_theme":"LocalIcons","gtk_theme":"SavedGtk","ui_font":"Old Font,19,-1,5,75,0,0,0,0,0"},
            "gsettings_gtk_theme":"EffectiveGtk",
            "assets":[{"kind":"icons","name":"LocalIcons","path":asset_path,"origin":"user","redistributable":false,"license":"MIT"}],
            "dependencies":[{"kind":"gtk","name":"SavedGtk","source":"none"},{"kind":"plasmoid","name":"unrelated","source":"none"}]}).to_string(),
    );
    h.write(
        &format!("{profile}/{desktop}"),
        "[General]\nfont=Old Font,19,-1,5,75,0,0,0,0,0\n[Icons]\nTheme=LocalIcons\n",
    );
    h.write(
        &format!("{profile}/{asset_path}/index.theme"),
        "[Icon Theme]\nName=LocalIcons\n",
    );
    h.write(
        ".local/share/icons/PublicIcons/index.theme",
        "[Icon Theme]\nName=PublicIcons\n",
    );
    h.write(
        ".local/share/themes/NewGtk/gtk-3.0/gtk.css",
        "/* fixture */",
    );
    let opened = h.rpc("bundle.editor", json!({"name":"edit-me"}));
    assert_eq!(opened["values"]["gtk_theme"], "EffectiveGtk");
    assert_eq!(opened["values"]["ui_font"], "Old Font");
    assert_eq!(opened["bundled"]["icon_theme"], json!(["LocalIcons"]));
    let saved = h.rpc("bundle.save_edits", json!({"name":"edit-me","revision":opened["revision"],
        "changes":{"icon_theme":"PublicIcons","gtk_theme":"NewGtk","ui_font":"New Font","plasma_style":"breeze-dark","widget_style":"kvantum"}}));
    assert_eq!(fs::read_to_string(h.path(desktop)).unwrap(), initial);
    let edited: Value = serde_json::from_slice(&fs::read(h.path(&manifest)).unwrap()).unwrap();
    assert_eq!(edited["assets"], json!([]));
    assert_eq!(edited["inactive_assets"][0]["license"], "MIT");
    assert!(h
        .path(&format!("{profile}/{asset_path}/index.theme"))
        .is_file());
    assert!(edited.get("gsettings_gtk_theme").is_none());
    assert_eq!(edited["values"]["ui_font"], "New Font,19,-1,5,75,0,0,0,0,0");
    let deps = edited["dependencies"].as_array().unwrap();
    for name in ["unrelated", "PublicIcons", "NewGtk", "kvantum"] {
        assert!(deps.iter().any(|d| d["name"] == name), "{deps:?}");
    }
    assert!(deps.iter().all(|d| d["name"] != "SavedGtk"));
    let export_path = h.path("exported");
    let export = Command::new(env!("CARGO_BIN_EXE_kolira"))
        .args([
            "export",
            "edit-me",
            &format!("--dir={}", export_path.display()),
        ])
        .envs(
            h.command("unused")
                .get_envs()
                .filter_map(|(k, v)| v.map(|v| (k, v))),
        )
        .output()
        .unwrap();
    assert!(
        export.status.success(),
        "{}",
        String::from_utf8_lossy(&export.stderr)
    );
    let less: Value =
        serde_json::from_slice(&fs::read(export_path.join("theme.json")).unwrap()).unwrap();
    assert!(less.get("inactive_assets").is_none());
    assert!(!export_path.join(asset_path).exists());

    h.rpc(
        "profile.apply",
        json!({"name":"edit-me","skip_dependencies":true}),
    );
    let live_font = h.rpc("field.read", json!({"id":"ui_font"}));
    assert!(
        live_font.as_str().unwrap().starts_with("New Font,13,"),
        "{live_font}"
    );
    assert!(!h.path(&format!("{asset_path}/index.theme")).exists());

    h.rpc(
        "bundle.save_edits",
        json!({"name":"edit-me","revision":saved["revision"],
        "changes":{"icon_theme":"LocalIcons","gtk_theme":null}}),
    );
    let restored: Value = serde_json::from_slice(&fs::read(h.path(&manifest)).unwrap()).unwrap();
    assert_eq!(restored["assets"][0]["license"], "MIT");
    assert!(restored.get("inactive_assets").is_none());
    assert!(restored["values"].get("gtk_theme").is_none());
    assert!(restored["dependencies"]
        .as_array()
        .unwrap()
        .iter()
        .all(|d| d["name"] != "LocalIcons" && d["name"] != "NewGtk"));
}

#[test]
fn decoration_edit_releases_the_selection_without_losing_other_kwin_fields() {
    let h = Home::new();
    let config = ".config/kwinrc";
    let initial = "[org.kde.kdecoration2]\nlibrary=org.kde.oxygen\ntheme=Oxygen\nNoPlugin=false\nBorderSize=Large\nButtonsOnRight=IAX\n";
    h.write(config, initial);
    let first = ".config/kolira/themes/first";
    h.write(
        &format!("{first}/theme.json"),
        r#"{"schema":"kolira/theme@2","name":"first","appearance_only":true}"#,
    );
    h.write(
        &format!("{first}/{config}"),
        "[org.kde.kdecoration2]\nBorderSize=Normal\n",
    );
    let opened = h.rpc("bundle.editor", json!({"name":"first"}));
    assert!(opened["values"]["decoration_theme"].is_null());
    assert!(opened["choices"]["decoration_theme"]
        .as_array()
        .unwrap()
        .iter()
        .any(|choice| choice["label"] == "Breeze"));

    let disabled = h.rpc(
        "bundle.save_edits",
        json!({"name":"first","revision":opened["revision"],
        "changes":{"decoration_theme":{"disabled":true}}}),
    );
    assert_eq!(fs::read_to_string(h.path(config)).unwrap(), initial);
    assert_eq!(h.rpc("profile.active", json!({})), "");
    h.rpc(
        "profile.apply",
        json!({"name":"first","skip_dependencies":true}),
    );
    assert_eq!(
        h.rpc("field.read", json!({"id":"decoration_disabled"})),
        "true"
    );
    assert_eq!(
        h.rpc("field.read", json!({"id":"decoration_library"})),
        "org.kde.oxygen"
    );

    let omitted = h.rpc(
        "bundle.save_edits",
        json!({"name":"first","revision":disabled["revision"],
        "changes":{"decoration_theme":null}}),
    );
    assert!(omitted["values"]["decoration_theme"].is_null());
    assert_eq!(
        h.rpc("field.read", json!({"id":"decoration_disabled"})),
        "true"
    );
    let on_disk = fs::read_to_string(h.path(&format!("{first}/{config}"))).unwrap();
    assert_eq!(on_disk, "[org.kde.kdecoration2]\nBorderSize=Normal\n");

    let next = ".config/kolira/themes/next";
    h.write(
        &format!("{next}/theme.json"),
        r#"{"schema":"kolira/theme@2","name":"next","appearance_only":true}"#,
    );
    let next_editor = h.rpc("bundle.editor", json!({"name":"next"}));
    let invalid = h.raw("bundle.save_edits", json!({"name":"next","revision":next_editor["revision"],
        "changes":{"decoration_theme":{"library":"org.kde.breeze","theme":"__aurorae__svg__Other"}}}));
    assert_eq!(invalid["ok"], false);
    assert_eq!(
        h.rpc("bundle.editor", json!({"name":"next"}))["revision"],
        next_editor["revision"]
    );
    let breeze = h.rpc(
        "bundle.save_edits",
        json!({"name":"next","revision":next_editor["revision"],
        "changes":{"decoration_theme":{"library":"org.kde.breeze","theme":"Breeze"}}}),
    );
    h.rpc(
        "profile.apply",
        json!({"name":"next","skip_dependencies":true}),
    );
    assert_eq!(
        h.rpc("field.read", json!({"id":"decoration_library"})),
        "org.kde.breeze"
    );

    let preview = h.rpc("profile.preview", json!({"name":"first"}));
    h.rpc(
        "profile.apply",
        json!({"name":"first","preview_token":preview["token"]}),
    );
    assert_eq!(
        h.rpc("field.read", json!({"id":"decoration_library"})),
        "org.kde.oxygen"
    );
    assert_eq!(
        h.rpc("field.read", json!({"id":"decoration_theme"})),
        "Oxygen"
    );
    assert_eq!(
        h.rpc("field.read", json!({"id":"decoration_disabled"})),
        "false"
    );
    assert!(fs::read_to_string(h.path(config))
        .unwrap()
        .contains("BorderSize=Normal"));
    assert!(fs::read_to_string(h.path(config))
        .unwrap()
        .contains("ButtonsOnRight=IAX"));

    h.rpc("bundle.save_edits", json!({"name":"next","revision":breeze["revision"],
        "changes":{"decoration_theme":{"library":"org.kde.uninstalled-kolira-test","theme":"Other"}}}));
    let preview = h.rpc("profile.preview", json!({"name":"next"}));
    assert!(preview["kept_fields"]
        .as_array()
        .unwrap()
        .contains(&json!("decoration_library")));
    h.rpc(
        "profile.apply",
        json!({"name":"next","preview_token":preview["token"]}),
    );
    assert_eq!(
        h.rpc("field.read", json!({"id":"decoration_library"})),
        "org.kde.oxygen"
    );
    assert_eq!(
        h.rpc("field.read", json!({"id":"decoration_theme"})),
        "Oxygen"
    );
}

#[test]
fn switching_decoration_parks_and_restores_the_bundled_aurorae_theme() {
    let h = Home::new();
    let profile = ".config/kolira/themes/aurorae";
    let manifest = format!("{profile}/theme.json");
    let asset_path = ".local/share/aurorae/themes/Example";
    h.write(&manifest, &json!({"schema":"kolira/theme@2","name":"aurorae","appearance_only":true,
        "values":{"decoration_library":"org.kde.kwin.aurorae","decoration_theme":"__aurorae__svg__Example"},
        "assets":[{"kind":"aurorae","name":"Example","path":asset_path,"origin":"user","redistributable":false,"license":"MIT"}]}).to_string());
    h.write(&format!("{profile}/{asset_path}/Examplerc"), "[General]\n");
    h.write(
        &format!("{profile}/.config/kwinrc"),
        "[org.kde.kdecoration2]\nlibrary=org.kde.kwin.aurorae\ntheme=__aurorae__svg__Example\n",
    );
    let opened = h.rpc("bundle.editor", json!({"name":"aurorae"}));
    let original = &opened["values"]["decoration_theme"];
    assert_eq!(original["theme"], "__aurorae__svg__Example");
    let breeze = h.rpc(
        "bundle.save_edits",
        json!({"name":"aurorae","revision":opened["revision"],
        "changes":{"decoration_theme":{"library":"org.kde.breeze","theme":"Breeze"}}}),
    );
    let data: Value = serde_json::from_slice(&fs::read(h.path(&manifest)).unwrap()).unwrap();
    assert_eq!(data["assets"], json!([]));
    assert_eq!(data["inactive_assets"][0]["license"], "MIT");
    assert!(h
        .path(&format!("{profile}/{asset_path}/Examplerc"))
        .is_file());
    let reopened = h.rpc(
        "bundle.save_edits",
        json!({"name":"aurorae","revision":breeze["revision"],
        "changes":{"decoration_theme":original}}),
    );
    assert_eq!(
        reopened["values"]["decoration_theme"]["theme"],
        "__aurorae__svg__Example"
    );
    let data: Value = serde_json::from_slice(&fs::read(h.path(&manifest)).unwrap()).unwrap();
    assert_eq!(data["assets"][0]["license"], "MIT");
    assert!(data.get("inactive_assets").is_none());
    assert!(!h.path(".config/kwinrc").exists());
}

#[test]
fn saved_kde_palette_updates_preview_and_partial_switches_without_touching_base16() {
    let h = Home::new();
    let globals = ".config/kdeglobals";
    let baseline = "[General]\nColorScheme=Baseline\nColorSchemeHash=baseline-hash\nAccentColor=1,2,3\naccentColorFromWallpaper=true\nfont=Noto Sans,13\n\n[Colors:Window]\nBackgroundNormal=40,41,42\n\n[Icons]\nTheme=baseline-icons\n";
    h.write(globals, baseline);
    let scheme_path = ".local/share/color-schemes/Old.colors";
    let first = ".config/kolira/themes/first";
    h.write(
        &format!("{first}/theme.json"),
        &json!({"schema":"kolira/theme@2","name":"first","appearance_only":true,
            "values":{"color_scheme":"Old","window_bg":"2,3,4"},
            "assets":[{"kind":"color-scheme","name":"Old","path":scheme_path,"origin":"user","redistributable":false,"license":"MIT"}],
            "palette":{"name":"App colors","variant":"dark","base":vec!["#112233";16],"accent":"#beda55"},
            "app_targets":["kitty"]}).to_string(),
    );
    let old_scheme = "[Colors:Window]\nBackgroundNormal=2,3,4\n";
    h.write(&format!("{first}/{scheme_path}"), old_scheme);
    h.write(
        &format!("{first}/{globals}"),
        &format!("[General]\nColorScheme=Old\nColorSchemeHash=old-hash\n\n{old_scheme}"),
    );
    h.write(
        ".local/share/color-schemes/Fresh.colors",
        "[General]\nName=Fresh palette\nfont=Must Not Copy,4\n\n[Colors:Window]\nBackgroundNormal=45,53,59\nForegroundNormal=240,240,242\n\n[Colors:Selection]\nBackgroundNormal=177,189,207\n",
    );
    let opened = h.rpc("bundle.editor", json!({"name":"first"}));
    let unavailable = h.raw("bundle.save_edits", json!({"name":"first","revision":opened["revision"],"changes":{"color_scheme":"Missing","accent_color":"#778899"}}));
    assert_eq!(unavailable["ok"], false);
    let invalid_color = h.raw(
        "bundle.save_edits",
        json!({"name":"first","revision":opened["revision"],"changes":{"accent_color":"#яяя"}}),
    );
    assert_eq!(invalid_color["ok"], false);
    assert_eq!(h.rpc("bundle.editor", json!({"name":"first"})), opened);
    let edited = h.rpc(
        "bundle.save_edits",
        json!({"name":"first","revision":opened["revision"],"changes":{
        "color_scheme":"Fresh","accent_color":"#778899","accent_from_wallpaper":false}}),
    );
    assert_eq!(edited["values"]["accent_color"], "#778899");
    let summary = h.rpc("profile.summaries", json!({"names":["first"]}));
    assert_eq!(summary["first"]["previewColors"]["window"], "#2d353b");
    assert_eq!(summary["first"]["previewColors"]["accent"], "#778899");
    let manifest: Value =
        serde_json::from_slice(&fs::read(h.path(&format!("{first}/theme.json"))).unwrap()).unwrap();
    assert_eq!(manifest["palette"]["accent"], "#beda55");
    assert_eq!(manifest["app_targets"], json!(["kitty"]));
    assert_eq!(manifest["inactive_assets"][0]["license"], "MIT");
    assert_eq!(fs::read_to_string(h.path(globals)).unwrap(), baseline);
    assert_eq!(h.rpc("profile.active", json!({})), "");

    let no_scheme = h.rpc(
        "bundle.save_edits",
        json!({"name":"first","revision":edited["revision"],"changes":{
        "color_scheme":null,"accent_color":null,"accent_from_wallpaper":null,"ui_font":"Inter"}}),
    );
    let saved_globals = fs::read_to_string(h.path(&format!("{first}/{globals}"))).unwrap();
    assert!(!saved_globals.contains("ColorScheme"));
    assert!(!saved_globals.contains("Colors:"));
    let other = ".config/kolira/themes/other";
    h.write(
        &format!("{other}/theme.json"),
        r#"{"schema":"kolira/theme@2","name":"other","appearance_only":true}"#,
    );
    let other_edit = h.rpc("bundle.editor", json!({"name":"other"}));
    h.rpc(
        "bundle.save_edits",
        json!({"name":"other","revision":other_edit["revision"],"changes":{
        "color_scheme":"Fresh","accent_color":"#778899","accent_from_wallpaper":false}}),
    );
    let apply = |name| {
        let preview = h.rpc("profile.preview", json!({"name":name}));
        h.rpc(
            "profile.apply",
            json!({"name":name,"preview_token":preview["token"]}),
        );
        fs::read_to_string(h.path(globals)).unwrap()
    };
    let applied_first = apply("first");
    assert!(applied_first.contains("ColorScheme=Baseline\n"));
    assert!(applied_first.contains("ColorSchemeHash=baseline-hash\n"));
    assert!(applied_first.contains("BackgroundNormal=40,41,42\n"));
    assert!(applied_first.contains("accentColorFromWallpaper=true\n"));
    assert!(applied_first.contains("AccentColor=1,2,3\n"));
    let applied_other = apply("other");
    assert!(applied_other.contains("ColorScheme=Fresh\n"));
    assert!(applied_other.contains("BackgroundNormal=45,53,59\n"));
    assert!(applied_other.contains("AccentColor=119,136,153\n"));
    assert!(applied_other.contains("accentColorFromWallpaper=false\n"));
    assert!(applied_other.contains("Theme=baseline-icons\n"));
    assert!(!applied_other.contains("ColorSchemeHash="));
    assert_eq!(apply("first"), applied_first);

    let restored = h.rpc(
        "bundle.save_edits",
        json!({"name":"first","revision":no_scheme["revision"],"changes":{
        "color_scheme":"Old","accent_color":"#778899","accent_from_wallpaper":true}}),
    );
    assert_eq!(restored["bundled"]["color_scheme"], json!(["Old"]));
    let summary = h.rpc("profile.summaries", json!({"names":["first"]}));
    assert_eq!(summary["first"]["previewColors"]["window"], "#020304");
    assert_eq!(summary["first"]["previewColors"]["accent"], "");
    assert_eq!(summary["first"]["accentFromWallpaper"], true);
    assert_eq!(fs::read_to_string(h.path(globals)).unwrap(), applied_first);
}

#[test]
fn saved_base16_edits_refresh_matching_colors_and_preserve_custom_components() {
    use kolira_core::{konsole, palette, terminal_apps};

    let h = Home::new();
    h.seed();
    let profile = ".config/kolira/themes/base16";
    let manifest = format!("{profile}/theme.json");
    h.write(
        &manifest,
        r#"{"schema":"kolira/theme@2","name":"base16","appearance_only":true}"#,
    );
    let opened = h.rpc("bundle.editor", json!({"name":"base16"}));
    assert!(opened["values"]["base16_palette"].is_null());
    let preset = opened["palettePresets"]
        .as_array()
        .unwrap()
        .first()
        .unwrap();
    let added = h.rpc(
        "bundle.save_edits",
        json!({"name":"base16","revision":opened["revision"],"changes":{"base16_palette":preset}}),
    );
    assert_eq!(&added["values"]["base16_palette"], preset);
    assert!(!h.path(&format!("{profile}/.config")).exists());
    let old: palette::Palette = serde_json::from_value(preset.clone()).unwrap();
    let id = palette::kde_scheme_id(&old);
    let scheme = format!(".local/share/color-schemes/{id}.colors");
    let konsole_scheme = format!(".local/share/konsole/{id}.colorscheme");
    h.write(&manifest, &json!({"schema":"kolira/theme@2","name":"base16","appearance_only":true,
        "values":{"color_scheme":id,"accent_from_wallpaper":true},"palette":old,"app_targets":["kitty","foot","konsole"],
        "assets":[{"kind":"color-scheme","name":id,"path":scheme,"origin":"generated","redistributable":true},
                  {"kind":"konsole-scheme","name":id,"path":konsole_scheme,"origin":"generated","redistributable":true}]}).to_string());
    let generated = palette::kde_color_scheme(&old);
    h.write(&format!("{profile}/{scheme}"), &generated);
    let globals = format!("{profile}/.config/kdeglobals");
    let kde =
        kolira_core::field::set_ini_text(&generated, "Colors:Window", "ForegroundNormal", "9,8,7");
    h.write(
        &globals,
        &kolira_core::field::set_ini_text(&kde, "General", "accentColorFromWallpaper", "true"),
    );
    let first_konsole =
        kolira_core::field::set_ini_text(&konsole::colorscheme(&old), "General", "Opacity", "0.62");
    h.write(&format!("{profile}/{konsole_scheme}"), &first_konsole);
    let kitty = kolira_core::appearance::KITTY_THEME;
    let kitty_config = palette::kitty_conf_with_palette("background_opacity 0.65\n", &old);
    h.write(&format!("{profile}/{kitty}"), &kitty_config);
    let ghostty = kolira_core::extras::GHOSTTY_THEME;
    let custom_ghostty = "background = #001234\nbackground-opacity = 0.6\n";
    h.write(&format!("{profile}/{ghostty}"), custom_ghostty);
    let resources = terminal_apps::bundle_resources(&old);
    let fzf = terminal_apps::FZF_DATA;
    h.write(&format!("{profile}/{fzf}"), &resources[fzf]);
    let saved_palette = format!("{profile}/{}", palette::LIVE_PALETTE_REL);
    h.write(&saved_palette, &preset.to_string());
    let nvim = format!("{profile}/{}", kolira_core::neovim::DATA);
    h.write(&nvim, &preset.to_string());
    let live = fs::read(h.path(".config/kdeglobals")).unwrap();
    h.write(palette::LIVE_PALETTE_REL, "separate Live palette");

    let opened = h.rpc("bundle.editor", json!({"name":"base16"}));
    assert_eq!(h.raw("bundle.save_edits", json!({"name":"base16","revision":opened["revision"],"changes":{"base16_palette":{"name":"Incomplete"}}}))["ok"], false);
    assert_eq!(
        h.rpc("bundle.editor", json!({"name":"base16"}))["revision"],
        opened["revision"]
    );
    let mut new = old.clone();
    new.set("base00", "#102030").unwrap();
    new.set("accent", "#778899").unwrap();
    let changed = h.rpc(
        "bundle.save_edits",
        json!({"name":"base16","revision":opened["revision"],"changes":{"base16_palette":new}}),
    );
    assert_eq!(changed["values"]["base16_palette"], json!(new));
    assert_eq!(
        fs::read_to_string(h.path(format!("{profile}/{scheme}").as_str())).unwrap(),
        palette::kde_color_scheme(&new)
    );
    let new_kde = fs::read_to_string(h.path(&globals)).unwrap();
    assert_eq!(
        kolira_core::field::ini_get(&new_kde, "Colors:Window", "ForegroundNormal").as_deref(),
        Some("9,8,7")
    );
    assert_eq!(
        kolira_core::field::ini_get(&new_kde, "General", "accentColorFromWallpaper").as_deref(),
        Some("true")
    );
    let summary = h.rpc("profile.summaries", json!({"names":["base16"]}));
    assert_eq!(summary["base16"]["previewColors"]["window"], "#102030");
    assert_eq!(summary["base16"]["base16Name"], new.name);
    let new_konsole = fs::read_to_string(h.path(&format!("{profile}/{konsole_scheme}"))).unwrap();
    assert!(new_konsole.contains("Opacity=0.62\n"));
    assert!(new_konsole.contains("Color=16,32,48\n"));
    assert_eq!(
        fs::read_to_string(h.path(&format!("{profile}/{kitty}"))).unwrap(),
        palette::kitty_conf_with_palette(&kitty_config, &new)
    );
    assert_eq!(
        fs::read_to_string(h.path(&format!("{profile}/{ghostty}"))).unwrap(),
        custom_ghostty
    );
    assert_eq!(
        fs::read_to_string(h.path(&format!("{profile}/{fzf}"))).unwrap(),
        terminal_apps::bundle_resources(&new)[fzf]
    );
    assert_eq!(palette::load(&h.path(&nvim)).unwrap(), new);

    let stored: Value = serde_json::from_slice(&fs::read(h.path(&manifest)).unwrap()).unwrap();
    assert_eq!(stored["app_targets"], json!(["kitty", "foot", "konsole"]));
    let excluded = h.rpc(
        "bundle.save_edits",
        json!({"name":"base16","revision":changed["revision"],"changes":{"base16_palette":null}}),
    );
    assert!(excluded["values"]["base16_palette"].is_null());
    assert!(
        h.rpc("profile.summaries", json!({"names":["base16"]}))["base16"]["base16Name"].is_null()
    );
    let stored: Value = serde_json::from_slice(&fs::read(h.path(&manifest)).unwrap()).unwrap();
    assert_eq!(stored["app_targets"], json!([]));
    assert!(!h.path(&saved_palette).exists());
    assert_eq!(palette::load(&h.path(&nvim)).unwrap(), new);
    assert_eq!(fs::read(h.path(".config/kdeglobals")).unwrap(), live);
    assert_eq!(
        fs::read_to_string(h.path(palette::LIVE_PALETTE_REL)).unwrap(),
        "separate Live palette"
    );
    assert_eq!(h.rpc("profile.active", json!({})), "");
}

#[test]
fn base16_changes_flow_through_profile_preview_apply_and_return() {
    let h = Home::new();
    let kitty_host = kolira_core::appearance::KITTY_HOST;
    let kitty_theme = kolira_core::appearance::KITTY_THEME;
    let personal = "font_size 14\nmap ctrl+c copy_to_clipboard\n";
    h.write(kitty_host, personal);
    for name in ["one", "two", "partial"] {
        h.write(
            &format!(".config/kolira/themes/{name}/theme.json"),
            &json!({"schema":"kolira/theme@2","name":name,"appearance_only":true}).to_string(),
        );
    }
    let opened = h.rpc("bundle.editor", json!({"name":"one"}));
    let make = |name: &str, preset: &Value| {
        h.write(&format!(".config/kolira/themes/{name}/theme.json"), &json!({"schema":"kolira/theme@2","name":name,"appearance_only":true,"palette":preset,"app_targets":["kitty"]}).to_string());
    };
    make("one", &opened["palettePresets"][0]);
    make("two", &opened["palettePresets"][1]);
    let stale = h.rpc("profile.preview", json!({"name":"one"}));
    let opened = h.rpc("bundle.editor", json!({"name":"one"}));
    let mut edited = opened["values"]["base16_palette"].clone();
    edited["base"][0] = json!("#102030");
    h.rpc(
        "bundle.save_edits",
        json!({"name":"one","revision":opened["revision"],"changes":{"base16_palette":edited}}),
    );
    assert_eq!(
        h.raw(
            "profile.apply",
            json!({"name":"one","preview_token":stale["token"]})
        )["ok"],
        false
    );
    assert!(!h.path(kitty_theme).exists());
    let apply = |name| {
        let preview = h.rpc("profile.preview", json!({"name":name}));
        h.rpc(
            "profile.apply",
            json!({"name":name,"preview_token":preview["token"]}),
        );
    };
    apply("one");
    let first = fs::read_to_string(h.path(kitty_theme)).unwrap();
    assert!(first.contains("background #102030\n"));
    apply("two");
    assert_ne!(fs::read_to_string(h.path(kitty_theme)).unwrap(), first);
    apply("one");
    assert_eq!(fs::read_to_string(h.path(kitty_theme)).unwrap(), first);
    apply("partial");
    assert!(!h.path(kitty_theme).exists());
    assert_eq!(fs::read_to_string(h.path(kitty_host)).unwrap(), personal);
}

#[test]
fn saved_terminal_settings_apply_as_partial_profiles_and_restore_their_baseline() {
    use kolira_core::appearance::{ALACRITTY_HOST, ALACRITTY_THEME, KITTY_HOST, KITTY_THEME};
    use kolira_core::field;

    let h = Home::new();
    let kitty = "# My terminal\nbackground #223344\nbackground_opacity 0.65\nfont_size 18\n";
    let alacritty = "# My terminal\n[window]\nopacity=0.7\nblur=true\n[colors.primary]\nbackground='#334455'\n[font]\nsize=17\n";
    h.write(KITTY_HOST, kitty);
    h.write(ALACRITTY_HOST, alacritty);
    let path = |name: &str| format!(".config/kolira/themes/{name}");
    for name in ["one", "two", "empty"] {
        h.write(
            &format!("{}/theme.json", path(name)),
            &json!({"schema":"kolira/theme@2","name":name,"appearance_only":true}).to_string(),
        );
    }
    let opened = h.rpc("bundle.editor", json!({"name":"one"}));
    assert!(opened["values"]["kitty_opacity"].is_null());
    assert_eq!(
        h.raw(
            "bundle.save_edits",
            json!({"name":"one","revision":opened["revision"],"changes":{"kitty_opacity":1.5}})
        )["ok"],
        false
    );
    h.rpc("bundle.save_edits", json!({"name":"one","revision":opened["revision"],"changes":{"kitty_blur":5,"alacritty_blur":false}}));
    h.write(
        &format!("{}/{}", path("two"), KITTY_THEME),
        "background #112233\nbackground_opacity 0.3\n",
    );
    h.write(
        &format!("{}/{}", path("two"), ALACRITTY_THEME),
        "[window]\nopacity=0.4\n[colors.primary]\nbackground='#122334'\n",
    );
    assert_eq!(fs::read_to_string(h.path(KITTY_HOST)).unwrap(), kitty);
    assert_eq!(
        fs::read_to_string(h.path(ALACRITTY_HOST)).unwrap(),
        alacritty
    );

    let apply = |name| {
        let preview = h.rpc("profile.preview", json!({"name":name}));
        h.rpc(
            "profile.apply",
            json!({"name":name,"preview_token":preview["token"]}),
        );
        (
            fs::read_to_string(h.path(KITTY_THEME)).unwrap(),
            fs::read_to_string(h.path(ALACRITTY_THEME)).unwrap(),
        )
    };
    let first = apply("one");
    assert!(first.0.contains("background #223344\n"));
    assert!(first.0.contains("background_opacity 0.65\n"));
    assert!(first.0.contains("background_blur 5\n"));
    let first_toml: toml::Value = first.1.parse().unwrap();
    assert_eq!(
        first_toml["colors"]["primary"]["background"].as_str(),
        Some("#334455")
    );
    assert_eq!(first_toml["window"]["blur"].as_bool(), Some(false));
    assert_eq!(first_toml["window"]["opacity"].as_float(), Some(0.7));
    let second = apply("two");
    assert!(second.0.contains("background_opacity 0.3\n"));
    assert_ne!(first, second);
    assert_eq!(apply("one"), first);

    let opened = h.rpc("bundle.editor", json!({"name":"two"}));
    assert_eq!(opened["values"]["kitty_opacity"], 0.3);
    let updated = h.rpc("bundle.save_edits", json!({"name":"two","revision":opened["revision"],"changes":{"kitty_opacity":null,"alacritty_opacity":null}}));
    assert!(updated["values"]["kitty_opacity"].is_null());
    let restored = apply("two");
    assert!(restored.0.contains("background #112233\n"));
    assert_eq!(
        field::kv_get(&restored.0, "background_opacity").as_deref(),
        Some("0.65")
    );
    let restored_toml: toml::Value = restored.1.parse().unwrap();
    assert_eq!(restored_toml["window"]["opacity"].as_float(), Some(0.7));
    h.rpc(
        "profile.apply",
        json!({"name":"empty","skip_dependencies":true}),
    );
    assert_eq!(fs::read_to_string(h.path(KITTY_HOST)).unwrap(), kitty);
    assert_eq!(
        fs::read_to_string(h.path(ALACRITTY_HOST)).unwrap(),
        alacritty
    );
    assert!(!h.path(KITTY_THEME).exists());
    assert!(!h.path(ALACRITTY_THEME).exists());

    let with_include = "background_opacity 0.35\ninclude kolira.conf\nfont_size 18\n";
    h.write(KITTY_HOST, with_include);
    h.write(KITTY_THEME, "background_opacity 0.72\n");
    apply("one");
    assert_eq!(h.rpc("field.read", json!({"id":"kitty_opacity"})), "0.72");
    h.rpc(
        "profile.apply",
        json!({"name":"empty","skip_dependencies":true}),
    );
    assert_eq!(
        fs::read_to_string(h.path(KITTY_HOST)).unwrap(),
        with_include
    );
    assert_eq!(
        fs::read_to_string(h.path(KITTY_THEME)).unwrap(),
        "background_opacity 0.72\n"
    );
}

#[test]
fn saved_application_selectors_keep_personal_settings_and_respect_base16_targets() {
    let h = Home::new();
    let code = ".config/Code/User/settings.json";
    let cursor = ".config/Cursor/User/settings.json";
    let btop = ".config/btop/btop.conf";
    let base_code = "{\n  // Personal editor settings\n  \"workbench.colorTheme\": \"Visual Studio Dark\",\n  \"editor.fontFamily\": \"Fira Code\",\n  \"editor.fontSize\": 19 // keep this comment\n}\n";
    let base_btop = "color_theme = \"Personal\"\nrounded_corners = True\nupdate_ms = 1000\n";
    h.write(code, base_code);
    h.write(cursor, "{\n  \"editor.fontSize\": 18 // personal\n}\n");
    h.write(btop, base_btop);
    h.write(".vscode/extensions/author.local-theme/package.json", r#"{"name":"local-theme","publisher":"author","contributes":{"themes":[{"label":"Night/Blue","id":"Night/Blue"}]}}"#);
    let one = ".config/kolira/themes/one";
    let two = ".config/kolira/themes/two";
    for (dir, name) in [(one, "one"), (two, "two")] {
        h.write(
            &format!("{dir}/theme.json"),
            &json!({"schema":"kolira/theme@2","name":name,"appearance_only":true}).to_string(),
        );
    }
    let preset = h.rpc("bundle.editor", json!({"name":"one"}))["palettePresets"][0].clone();
    h.write(&format!("{one}/theme.json"), &json!({"schema":"kolira/theme@2","name":"one","appearance_only":true,
        "palette":preset,"app_targets":["btop"],
        "values":{"vscode_theme":"Removed theme"},
        "sources":{"vscode_theme":{"id":"author.removed","store":"vscode-marketplace"}},
        "dependencies":[{"name":"Removed theme","kind":"vscode","source":"vscode-marketplace","id":"author.removed"}]}).to_string());
    h.write(&format!("{one}/{code}"), "{\n  \"workbench.iconTheme\": \"Project icons\", // Keep after removing selector\n  \"workbench.colorTheme\": \"Removed theme\"\n}\n");
    h.write(&format!("{one}/{btop}"), "rounded_corners = True\n");
    h.write(
        &format!("{two}/{code}"),
        "{\n  \"workbench.colorTheme\": \"Abyss\"\n}\n",
    );
    let opened = h.rpc("bundle.editor", json!({"name":"one"}));
    let choices = opened["choices"]["vscode_theme"].as_array().unwrap();
    assert!(choices
        .iter()
        .any(|c| c["value"] == "Removed theme" && c["available"] == false));
    assert!(choices
        .iter()
        .any(|c| c["value"] == "Night/Blue" && c["available"] == true));
    let edited = h.rpc("bundle.save_edits", json!({"name":"one","revision":opened["revision"],"changes":{
        "vscode_theme":"Night/Blue","cursor_ide_theme":"Cursor Dark Midnight","btop_theme":"Default","btop_rounded":false}}));
    let saved: Value =
        serde_json::from_slice(&fs::read(h.path(&format!("{one}/theme.json"))).unwrap()).unwrap();
    assert_eq!(saved["sources"]["vscode_theme"]["id"], "author.local-theme");
    assert_eq!(saved["dependencies"].as_array().unwrap().len(), 1);
    assert_eq!(saved["dependencies"][0]["name"], "Night/Blue");
    assert_eq!(fs::read_to_string(h.path(code)).unwrap(), base_code);
    assert_eq!(fs::read_to_string(h.path(btop)).unwrap(), base_btop);
    assert_eq!(h.rpc("profile.active", json!({})), "");
    let apply = |name| {
        let preview = h.rpc("profile.preview", json!({"name":name}));
        h.rpc(
            "profile.apply",
            json!({"name":name,"preview_token":preview["token"]}),
        );
        fs::read_to_string(h.path(code)).unwrap()
    };
    let first = apply("one");
    assert!(first.contains("Night/Blue"));
    assert!(first.contains("Fira Code"));
    assert!(first.contains("// keep this comment"));
    let cursor = fs::read_to_string(h.path(cursor)).unwrap();
    assert!(cursor.contains("Cursor Dark Midnight"));
    assert!(cursor.contains("// personal"));
    let monitor = fs::read_to_string(h.path(btop)).unwrap();
    assert!(monitor.contains("color_theme=\"Default\""));
    assert!(monitor.contains("rounded_corners=False"));
    assert!(monitor.contains("update_ms = 1000"));
    assert!(!h.path(".config/btop/themes/kolira.theme").exists());
    assert!(apply("two").contains("Abyss"));
    assert_eq!(apply("one"), first);

    let excluded = h.rpc(
        "bundle.save_edits",
        json!({"name":"one","revision":edited["revision"],"changes":{
        "vscode_theme":null,"btop_theme":null,"app_targets":[]}}),
    );
    assert!(excluded["values"]["vscode_theme"].is_null());
    let snapshot = fs::read_to_string(h.path(&format!("{one}/{code}"))).unwrap();
    assert!(snapshot.contains("// Keep after removing selector"));
    assert!(apply("one").contains("Visual Studio Dark"));
    assert!(fs::read_to_string(h.path(btop))
        .unwrap()
        .contains("Personal"));
}

#[test]
fn saved_obsidian_choices_preserve_slots_and_switch_bundled_themes_transactionally() {
    let h = Home::new();
    let vault_path = |name: &str| format!("Notes/{name}/.obsidian/appearance.json");
    let originals = [
        r#"{"baseTheme":"dark","theme":"obsidian","cssTheme":"Personal","baseFontSize":23,"enabledCssSnippets":["local"]}"#,
        r#"{"theme":"moonstone","cssTheme":"","baseFontSize":27,"enabledCssSnippets":["other"]}"#,
        r#"{"theme":"moonstone","baseFontSize":21}"#,
    ];
    for (name, content) in ["first", "second", "private"].into_iter().zip(originals) {
        h.write(&vault_path(name), content);
        h.write(&format!("Notes/{name}/private.md"), "note contents\n");
    }
    h.write(
        "Notes/first/.obsidian/themes/Personal/theme.css",
        "body{}\n",
    );
    h.write(".config/obsidian/obsidian.json", &json!({"vaults":{
        "a":{"path":h.path("Notes/first")},"b":{"path":h.path("Notes/second")},"c":{"path":h.path("Notes/private")}
    }}).to_string());
    h.rpc(
        "app.select_vaults",
        json!({"vaults":["Notes/first","Notes/second"]}),
    );

    let one = ".config/kolira/themes/one";
    let slots = format!("{one}/.config/kolira/app-snapshots/obsidian");
    let slot0 = format!("{slots}/0/appearance.json");
    let slot1 = format!("{slots}/1/appearance.json");
    let raw0 = r#"{"baseTheme":"light","cssTheme":"Bundled"}"#;
    let raw1 = r#"{"theme":"obsidian","cssTheme":""}"#;
    h.write(&slot0, raw0);
    h.write(&slot1, raw1);
    h.write(
        &format!("{slots}/0/themes/Bundled/theme.css"),
        "body{--color:#113355}\n",
    );
    h.write(
        &format!("{slots}/0/themes/Bundled/manifest.json"),
        r#"{"name":"Bundled Colors"}"#,
    );
    for name in ["one", "two"] {
        let file = format!(".config/kolira/themes/{name}/theme.json");
        h.write(
            &file,
            &json!({"name":name,"schema":"kolira/theme@2","appearance_only":true}).to_string(),
        );
    }
    let editor = h.rpc("bundle.editor", json!({"name":"one"}));
    assert_eq!(editor["values"]["obsidian_mode"], json!({"mixed":true}));
    assert_eq!(editor["values"]["obsidian_theme"], json!({"mixed":true}));
    assert!(editor["choices"]["obsidian_theme"]
        .as_array()
        .unwrap()
        .iter()
        .any(|option| {
            option["value"] == "Bundled"
                && option["label"] == "Bundled Colors"
                && option["available"] == true
        }));
    let updated = h.rpc("bundle.save_edits", json!({"name":"one","revision":editor["revision"],"changes":{"obsidian_mode":"light","obsidian_theme":"Bundled"}}));
    assert_eq!(updated["values"]["obsidian_mode"], "light");
    assert_eq!(fs::read_to_string(h.path(&slot0)).unwrap(), raw0);
    assert_eq!(fs::read_to_string(h.path(&slot1)).unwrap(), raw1);
    assert_eq!(
        fs::read_to_string(h.path(&vault_path("first"))).unwrap(),
        originals[0]
    );
    assert_eq!(h.rpc("profile.active", json!({})), "");
    let apply = |name| {
        let options = json!({"name":name,"without_sync":true});
        let preview = h.rpc("profile.preview", options);
        h.rpc(
            "profile.apply",
            json!({"name":name,"without_sync":true,"preview_token":preview["token"]}),
        )
    };
    let resource = |name| h.path(&format!("Notes/{name}/.obsidian/themes/Bundled/theme.css"));
    let read = |name| {
        serde_json::from_slice::<Value>(&fs::read(h.path(&vault_path(name))).unwrap()).unwrap()
    };
    for _ in 0..2 {
        apply("one");
        for (name, size) in [("first", 23), ("second", 27)] {
            assert_eq!(read(name)["theme"], "moonstone");
            assert_eq!(read(name)["baseTheme"], "light");
            assert_eq!(read(name)["cssTheme"], "Bundled");
            assert_eq!(read(name)["baseFontSize"], size);
            assert!(resource(name).is_file());
        }
        assert_eq!(read("first")["enabledCssSnippets"], json!(["local"]));
        assert_eq!(read("second")["enabledCssSnippets"], json!(["other"]));
        apply("two");
        assert!(!resource("first").exists() && !resource("second").exists());
        assert_eq!(
            fs::read_to_string(h.path(&vault_path("first"))).unwrap(),
            originals[0]
        );
        assert_eq!(
            fs::read_to_string(h.path(&vault_path("second"))).unwrap(),
            originals[1]
        );
    }
    let excluded = h.rpc("bundle.save_edits", json!({"name":"one","revision":updated["revision"],"changes":{"obsidian_mode":null,"obsidian_theme":null}}));
    assert!(
        excluded["values"]["obsidian_mode"].is_null()
            && excluded["values"]["obsidian_theme"].is_null()
    );
    apply("one");
    assert_eq!(read("first")["cssTheme"], "Personal");
    assert_eq!(read("first")["theme"], "obsidian");
    assert_eq!(read("first")["baseTheme"], "dark");
    assert_eq!(
        fs::read_to_string(h.path(&vault_path("private"))).unwrap(),
        originals[2]
    );
    for name in ["first", "second", "private"] {
        assert_eq!(
            fs::read_to_string(h.path(&format!("Notes/{name}/private.md"))).unwrap(),
            "note contents\n"
        );
    }
}

#[test]
fn saved_obsidian_missing_theme_keeps_live_selector_and_explicit_mode_beats_base16() {
    let h = Home::new();
    let user = "Notes/vault/.obsidian/appearance.json";
    h.write(
        user,
        r#"{"theme":"moonstone","cssTheme":"Personal","baseFontSize":19}"#,
    );
    h.write(
        "Notes/vault/.obsidian/themes/Personal/theme.css",
        "body {}\n",
    );
    h.write(
        ".config/obsidian/obsidian.json",
        &json!({"vaults":{"a":{"path":h.path("Notes/vault")}}}).to_string(),
    );
    h.write(
        ".config/kolira/themes/one/theme.json",
        r#"{"schema":"kolira/theme@2","name":"one","appearance_only":true}"#,
    );
    let editor = h.rpc("bundle.editor", json!({"name":"one"}));
    let preset = editor["palettePresets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["variant"] == "dark")
        .unwrap();
    assert_eq!(h.raw("bundle.save_edits", json!({"name":"one","revision":editor["revision"],"changes":{"obsidian_theme":"../outside"}}))["ok"], false);
    let saved = h.rpc("bundle.save_edits", json!({"name":"one","revision":editor["revision"],"changes":{
        "base16_palette":preset,"app_targets":["obsidian"],"obsidian_mode":"light","obsidian_theme":"Missing Colors"
    }}));
    assert_eq!(saved["values"]["obsidian_theme"], "Missing Colors");
    assert!(saved["choices"]["obsidian_theme"]
        .as_array()
        .unwrap()
        .iter()
        .any(|option| option["value"] == "Missing Colors" && option["available"] == false));
    let apply = || {
        let preview = h.rpc("profile.preview", json!({"name":"one"}));
        h.rpc(
            "profile.apply",
            json!({"name":"one","preview_token":preview["token"]}),
        );
        preview
    };
    let preview = apply();
    assert!(preview["missing"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["field"] == "obsidian_theme" && item["name"] == "Missing Colors"));
    let read = || serde_json::from_slice::<Value>(&fs::read(h.path(user)).unwrap()).unwrap();
    assert_eq!(read()["theme"], "moonstone");
    assert_eq!(read()["baseTheme"], "light");
    assert_eq!(read()["cssTheme"], "Personal");
    assert!(h.path("Notes/vault/.obsidian/snippets/kolira.css").exists());
    h.rpc(
        "bundle.save_edits",
        json!({"name":"one","revision":saved["revision"],"changes":{"obsidian_theme":""}}),
    );
    assert!(apply()["missing"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["field"] != "obsidian_theme"));
    assert_eq!(read()["cssTheme"], "");
    assert_eq!(read()["baseFontSize"], 19);
}

#[test]
fn saved_zen_appearance_is_portable_and_preserves_other_browser_settings() {
    let h = Home::new();
    let user = ".zen/receiver.default/user.js";
    let original = "// personal browser preferences\nuser_pref(\"privacy.resistFingerprinting\", true);\nuser_pref(\"zen.view.window.scheme\", 1);\nuser_pref(\"zen.theme.accent-color\", \"#abcdef\");\nuser_pref(\"ui.accentcolor\", \"#abcdef\");\nuser_pref(\"ui.accentcolortext\", \"#000000\");\n";
    h.write(
        ".zen/profiles.ini",
        "[Profile0]\nPath=receiver.default\nDefault=1\n",
    );
    h.write(user, original);
    h.write(
        ".zen/receiver.default/prefs.js",
        "// Browser-managed preferences\n",
    );
    h.write(
        ".zen/receiver.default/chrome/userChrome.css",
        "/* personal CSS */\n",
    );
    let author = ".config/kolira/themes/one/.config/zen/author.default/user.js";
    h.write(author, "// user_pref(\"zen.view.window.scheme\", 2);\nuser_pref(\"zen.view.window.scheme\", 1);\nuser_pref(\"zen.theme.accent-color\", \"#334455\");\n");
    for name in ["one", "two"] {
        h.write(
            &format!(".config/kolira/themes/{name}/theme.json"),
            &json!({"schema":"kolira/theme@2","name":name,"appearance_only":true}).to_string(),
        );
    }
    h.write(
        ".config/kolira/themes/two/.zen/other.default/user.js",
        "user_pref(\"zen.view.window.scheme\", 1);\n",
    );
    let editor = h.rpc("bundle.editor", json!({"name":"one"}));
    assert_eq!(editor["values"]["zen_accent_color"], "#334455");
    assert_eq!(editor["values"]["zen_mode"], "light");
    assert_eq!(editor["choices"]["zen_mode"].as_array().unwrap().len(), 2);
    assert!(editor["values"].get("zen_sites_scheme").is_none());
    assert_eq!(
        h.raw(
            "bundle.save_edits",
            json!({"name":"one","revision":editor["revision"],"changes":{"zen_mode":"system"}})
        )["ok"],
        false
    );
    h.write(
        author,
        &format!(
            "{}// externally edited\n",
            fs::read_to_string(h.path(author)).unwrap()
        ),
    );
    assert_eq!(
        h.raw(
            "bundle.save_edits",
            json!({"name":"one","revision":editor["revision"],"changes":{"zen_mode":"dark"}})
        )["ok"],
        false
    );
    let editor = h.rpc("bundle.editor", json!({"name":"one"}));
    let edited = h.rpc("bundle.save_edits", json!({"name":"one","revision":editor["revision"],"changes":{"zen_mode":"dark","zen_accent_color":"#113355"}}));
    assert_eq!(edited["values"]["zen_mode"], "dark");
    assert_eq!(edited["values"]["zen_accent_color"], "#113355");
    assert_eq!(fs::read_to_string(h.path(user)).unwrap(), original);
    assert_eq!(h.rpc("profile.active", json!({})), "");
    let snapshot = fs::read_to_string(h.path(author)).unwrap();
    assert!(snapshot.contains("// user_pref(\"zen.view.window.scheme\", 2);"));
    assert!(snapshot.contains("// externally edited"));
    let apply = |name| {
        let preview = h.rpc("profile.preview", json!({"name":name}));
        h.rpc(
            "profile.apply",
            json!({"name":name,"preview_token":preview["token"]}),
        );
        fs::read_to_string(h.path(user)).unwrap()
    };
    let first = apply("one");
    assert!(first.contains("user_pref(\"zen.view.window.scheme\", 0);"));
    assert!(first.contains("user_pref(\"zen.theme.accent-color\", \"#113355\");"));
    assert!(first.contains("user_pref(\"ui.accentcolortext\", \"#ffffff\");"));
    assert!(first.contains("user_pref(\"privacy.resistFingerprinting\", true);"));
    let light = apply("two");
    assert!(light.contains("user_pref(\"zen.view.window.scheme\", 1);"));
    assert!(light.contains("user_pref(\"zen.theme.accent-color\", \"#abcdef\");"));
    assert_eq!(apply("one"), first);
    let excluded = h.rpc(
        "bundle.save_edits",
        json!({"name":"one","revision":edited["revision"],"changes":{"zen_mode":null}}),
    );
    assert!(excluded["values"]["zen_mode"].is_null());
    let accent_only = apply("one");
    assert!(accent_only.contains("user_pref(\"zen.view.window.scheme\", 1);"));
    assert!(accent_only.contains("user_pref(\"zen.theme.accent-color\", \"#113355\");"));
    assert_eq!(
        fs::read_to_string(h.path(".zen/receiver.default/prefs.js")).unwrap(),
        "// Browser-managed preferences\n"
    );
    assert_eq!(
        fs::read_to_string(h.path(".zen/receiver.default/chrome/userChrome.css")).unwrap(),
        "/* personal CSS */\n"
    );
}

#[test]
fn saved_zen_fields_override_or_exclude_base16_independently() {
    let h = Home::new();
    let user = ".config/zen/receiver.default/user.js";
    h.write(
        ".config/zen/profiles.ini",
        "[Profile0]\nDefault=1\nPath=receiver.default\n",
    );
    let original = "user_pref(\"zen.theme.accent-color\", \"#223344\");\nuser_pref(\"zen.view.window.scheme\", 1);\n";
    h.write(user, original);
    let manifest = ".config/kolira/themes/generated/theme.json";
    h.write(
        manifest,
        r#"{"schema":"kolira/theme@2","name":"generated","appearance_only":true}"#,
    );
    let editor = h.rpc("bundle.editor", json!({"name":"generated"}));
    let preset = editor["palettePresets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["variant"] == "dark")
        .unwrap();
    let generated = h.rpc("bundle.save_edits", json!({"name":"generated","revision":editor["revision"],"changes":{"base16_palette":preset,"app_targets":["zen"]}}));
    assert_eq!(generated["values"]["zen_mode"], "dark");
    assert_eq!(generated["values"]["zen_accent_color"], preset["accent"]);
    let edited = h.rpc("bundle.save_edits", json!({"name":"generated","revision":generated["revision"],"changes":{"zen_accent_color":"#ccddff"}}));
    assert_eq!(edited["values"]["zen_mode"], "dark");
    assert_eq!(edited["values"]["zen_accent_color"], "#ccddff");
    assert_eq!(fs::read_to_string(h.path(user)).unwrap(), original);
    let apply = || {
        let preview = h.rpc("profile.preview", json!({"name":"generated"}));
        h.rpc(
            "profile.apply",
            json!({"name":"generated","preview_token":preview["token"]}),
        );
        fs::read_to_string(h.path(user)).unwrap()
    };
    let live = apply();
    assert!(live.contains("user_pref(\"zen.theme.accent-color\", \"#ccddff\");"));
    assert!(live.contains("user_pref(\"zen.view.window.scheme\", 0);"));
    let edited = h.rpc("bundle.save_edits", json!({"name":"generated","revision":edited["revision"],"changes":{"zen_accent_color":null,"zen_mode":"light"}}));
    assert_eq!(edited["values"]["app_targets"], json!(["zen"]));
    assert!(edited["values"]["zen_accent_color"].is_null());
    let live = apply();
    assert!(live.contains("user_pref(\"zen.theme.accent-color\", \"#223344\");"));
    assert!(live.contains("user_pref(\"zen.view.window.scheme\", 1);"));
    h.rpc(
        "bundle.save_edits",
        json!({"name":"generated","revision":edited["revision"],"changes":{"zen_mode":null}}),
    );
    assert_eq!(apply(), original);
}

#[test]
fn saved_zed_theme_edits_merge_with_live_and_keep_legacy_fixed_selection() {
    let h = Home::new();
    h.executable("zeditor", "#!/bin/sh\nexit 0\n");
    let settings = ".config/zed/settings.json";
    let live = "{\n  \"theme\": { \"mode\": \"system\", \"light\": \"One Light\", \"dark\": \"One Dark\" },\n  \"ui_font_family\": \"Personal\",\n  \"agent\": { \"enabled\": false } // personal\n}\n";
    h.write(settings, live);
    for name in ["partial", "other", "legacy"] {
        let root = format!(".config/kolira/themes/{name}");
        h.write(
            &format!("{root}/theme.json"),
            &json!({"schema":"kolira/theme@2","name":name,"appearance_only":true}).to_string(),
        );
    }
    let partial = h.rpc("bundle.editor", json!({"name":"partial"}));
    assert!(partial["values"]["zed_mode"].is_null());
    assert!(partial["choices"]["zed_dark"]
        .as_array()
        .unwrap()
        .iter()
        .any(|option| option["value"] == "Ayu Dark" && option["available"] == true));
    let edited = h.rpc(
        "bundle.save_edits",
        json!({"name":"partial","revision":partial["revision"],"changes":{"zed_dark":"Ayu Dark"}}),
    );
    assert!(edited["values"]["zed_light"].is_null());
    assert!(edited["values"]["zed_mode"].is_null());
    assert_eq!(
        h.raw(
            "bundle.save_edits",
            json!({"name":"partial","revision":edited["revision"],"changes":{"zed_mode":"random"}})
        )["ok"],
        false
    );
    h.write(
        &format!(".config/kolira/themes/other/{settings}"),
        r#"{"theme":{"mode":"light","dark":"One Dark","light":"Ayu Light"}}"#,
    );
    let legacy_file = format!(".config/kolira/themes/legacy/{settings}");
    let legacy = "{\n  \"theme\": \"One Dark\",\n  \"icon_theme\": \"Zed (Default)\" // keep this comment\n}\n";
    h.write(&legacy_file, legacy);
    let opened = h.rpc("bundle.editor", json!({"name":"legacy"}));
    assert_eq!(opened["values"]["zed_light"], "One Dark");
    assert_eq!(opened["values"]["zed_dark"], "One Dark");
    assert!(opened["values"]["zed_mode"].is_null());
    assert_eq!(fs::read_to_string(h.path(&legacy_file)).unwrap(), legacy);
    let changed = h.rpc("bundle.save_edits", json!({"name":"legacy","revision":opened["revision"],"changes":{"zed_mode":"system","zed_light":null}}));
    assert_eq!(changed["values"]["zed_mode"], "system");
    assert_eq!(changed["values"]["zed_dark"], "One Dark");
    assert!(changed["values"]["zed_light"].is_null());
    let snapshot = fs::read_to_string(h.path(&legacy_file)).unwrap();
    assert!(snapshot.contains("// keep this comment"));
    assert_eq!(fs::read_to_string(h.path(settings)).unwrap(), live);
    assert_eq!(h.rpc("profile.active", json!({})), "");

    let apply = |name| {
        let preview = h.rpc("profile.preview", json!({"name":name}));
        h.rpc(
            "profile.apply",
            json!({"name":name,"preview_token":preview["token"]}),
        );
        fs::read_to_string(h.path(settings)).unwrap()
    };
    let first = apply("partial");
    assert!(first.contains(r#""dark": "Ayu Dark""#));
    assert!(first.contains(r#""mode": "system""#));
    assert!(first.contains(r#""light": "One Light""#));
    assert!(first.contains(r#""ui_font_family": "Personal""#));
    assert!(first.contains("// personal"));
    assert!(apply("other").contains("Ayu Light"));
    assert_eq!(apply("partial"), first);
    h.rpc(
        "bundle.save_edits",
        json!({"name":"partial","revision":edited["revision"],"changes":{"zed_dark":null}}),
    );
    assert!(!h
        .path(&format!(".config/kolira/themes/partial/{settings}"))
        .exists());
    assert_eq!(apply("partial"), live);
}

#[test]
fn saved_zed_assets_follow_both_theme_slots_and_keep_inactive_files() {
    let h = Home::new();
    h.executable("zeditor", "#!/bin/sh\nexit 0\n");
    h.write(".config/zed/settings.json", r#"{"theme":"One Light"}"#);
    let root = ".config/kolira/themes/zed";
    let family = ".config/zed/themes/local.json";
    h.write(
        &format!("{root}/{family}"),
        r#"{"themes":[{"name":"Bundled Colors","appearance":"dark","style":{}}]}"#,
    );
    h.write(
        &format!("{root}/.config/zed/settings.json"),
        r#"{"theme":"Bundled Colors"}"#,
    );
    let manifest = format!("{root}/theme.json");
    h.write(&manifest, &json!({"schema":"kolira/theme@2","name":"zed","appearance_only":true,"assets":[
        {"kind":"zed","name":"Bundled Colors","path":family,"origin":"user","redistributable":false,"license":"MIT"}
    ]}).to_string());
    let open = h.rpc("bundle.editor", json!({"name":"zed"}));
    let read_manifest =
        || serde_json::from_slice::<Value>(&fs::read(h.path(&manifest)).unwrap()).unwrap();
    let edited = h.rpc(
        "bundle.save_edits",
        json!({"name":"zed","revision":open["revision"],"changes":{"zed_light":null}}),
    );
    assert_eq!(read_manifest()["assets"].as_array().unwrap().len(), 1);
    let edited = h.rpc(
        "bundle.save_edits",
        json!({"name":"zed","revision":edited["revision"],"changes":{"zed_dark":null}}),
    );
    assert_eq!(read_manifest()["assets"], json!([]));
    assert_eq!(read_manifest()["inactive_assets"][0]["license"], "MIT");
    assert!(h.path(&format!("{root}/{family}")).exists());
    assert!(edited["choices"]["zed_light"]
        .as_array()
        .unwrap()
        .iter()
        .any(|choice| choice["value"] == "Bundled Colors" && choice["available"] == true));
    h.rpc(
        "bundle.save_edits",
        json!({"name":"zed","revision":edited["revision"],"changes":{"zed_dark":"Bundled Colors"}}),
    );
    assert_eq!(read_manifest()["assets"][0]["license"], "MIT");
    assert!(read_manifest()["inactive_assets"].is_null());
    let preview = h.rpc("profile.preview", json!({"name":"zed"}));
    h.rpc(
        "profile.apply",
        json!({"name":"zed","preview_token":preview["token"]}),
    );
    let live = fs::read_to_string(h.path(".config/zed/settings.json")).unwrap();
    assert!(live.contains(r#""dark": "Bundled Colors""#));
    assert!(live.contains(r#""light": "One Light""#));
    assert!(h.path(family).exists());
    let edited_live = live.replace(r#""light": "One Light""#, r#""light": "Ayu Light""#);
    h.write(".config/zed/settings.json", &edited_live);
    h.write(
        ".config/kolira/themes/empty/theme.json",
        r#"{"schema":"kolira/theme@2","name":"empty","appearance_only":true}"#,
    );
    let preview = h.rpc("profile.preview", json!({"name":"empty"}));
    h.rpc(
        "profile.apply",
        json!({"name":"empty","preview_token":preview["token"]}),
    );
    let live = fs::read_to_string(h.path(".config/zed/settings.json")).unwrap();
    assert!(live.contains(r#""dark": "One Light""#));
    assert!(live.contains(r#""light": "Ayu Light""#));
}

#[test]
fn cached_editor_choices_can_be_selected_without_an_installed_extension() {
    let h = Home::new();
    h.executable("code", "#!/bin/sh\nexit 0\n");
    let root = ".config/kolira/themes/cached";
    let cache = "assets/editor-themes/vscode/example.local";
    h.write(&format!("{root}/theme.json"), &json!({"schema":"kolira/theme@2","name":"cached","appearance_only":true,
        "editor_themes":[{"editor":"vscode","store":"local","id":"example.local","local_path":cache}]}).to_string());
    h.write(&format!("{root}/{cache}/package.json"), r#"{"publisher":"example","name":"local","contributes":{"themes":[{"label":"Cached Colors","id":"cache-colors"}]}}"#);
    let opened = h.rpc("bundle.editor", json!({"name":"cached"}));
    let cached_choices: Vec<_> = opened["choices"]["vscode_theme"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|choice| choice["label"] == "Cached Colors")
        .collect();
    assert_eq!(cached_choices.len(), 1);
    assert_eq!(cached_choices[0]["value"], "cache-colors");
    assert_eq!(cached_choices[0]["available"], true);
    let edited = h.rpc("bundle.save_edits", json!({"name":"cached","revision":opened["revision"],"changes":{"vscode_theme":"cache-colors"}}));
    let legacy = h.rpc("bundle.save_edits", json!({"name":"cached","revision":edited["revision"],"changes":{"vscode_theme":"Cached Colors"}}));
    assert_eq!(legacy["values"]["vscode_theme"], "Cached Colors");
    let cached_choices: Vec<_> = legacy["choices"]["vscode_theme"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|choice| choice["label"] == "Cached Colors")
        .collect();
    assert_eq!(cached_choices.len(), 1);
    assert_eq!(cached_choices[0]["value"], "Cached Colors");
    assert_eq!(cached_choices[0]["available"], true);
    let summary = h.rpc("profile.summaries", json!({"names":["cached"]}));
    assert_eq!(summary["cached"]["unavailableResources"], json!([]));
    assert!(h.path(&format!("{root}/{cache}/package.json")).exists());
    assert!(!h.path(".config/Code/User/settings.json").exists());
}

#[test]
fn saved_app_targets_require_base16_and_kitty_generates_colors_alongside_sparse_settings() {
    use kolira_core::appearance::{KITTY_HOST, KITTY_THEME};
    let h = Home::new();
    let original = "background #334455\nfont_size 18\n";
    h.write(KITTY_HOST, original);
    let root = ".config/kolira/themes/apps";
    h.write(
        &format!("{root}/theme.json"),
        r#"{"schema":"kolira/theme@2","name":"apps","appearance_only":true}"#,
    );
    let editor = h.rpc("bundle.editor", json!({"name":"apps"}));
    assert!(editor["appTargets"]
        .as_array()
        .unwrap()
        .iter()
        .any(|app| app["id"] == "kitty"));
    let preset = &editor["palettePresets"][0];
    for changes in [
        json!({"app_targets":["kitty"]}),
        json!({"base16_palette":preset,"app_targets":["untracked"]}),
        json!({"base16_palette":preset,"app_targets":["kitty","kitty"]}),
    ] {
        assert_eq!(
            h.raw(
                "bundle.save_edits",
                json!({"name":"apps","revision":editor["revision"],"changes":changes})
            )["ok"],
            false
        );
    }
    let edited = h.rpc("bundle.save_edits", json!({"name":"apps","revision":editor["revision"],"changes":{"base16_palette":preset,"app_targets":["kitty"],"kitty_opacity":0.8}}));
    assert_eq!(edited["values"]["app_targets"], json!(["kitty"]));
    assert_eq!(fs::read_to_string(h.path(KITTY_HOST)).unwrap(), original);
    let apply = || {
        let preview = h.rpc("profile.preview", json!({"name":"apps"}));
        h.rpc(
            "profile.apply",
            json!({"name":"apps","preview_token":preview["token"]}),
        );
        fs::read_to_string(h.path(KITTY_THEME)).unwrap()
    };
    let kitty = apply();
    assert!(kitty.contains("background_opacity 0.8\n"));
    assert!(kitty.contains(kolira_core::palette::KITTY_BEGIN));
    h.rpc(
        "bundle.save_edits",
        json!({"name":"apps","revision":edited["revision"],"changes":{"app_targets":[]}}),
    );
    let kitty = apply();
    assert!(kitty.contains("background #334455\n"));
    assert!(kitty.contains("background_opacity 0.8\n"));
    assert!(!kitty.contains(kolira_core::palette::KITTY_BEGIN));
}

#[test]
fn accent_only_profile_still_updates_plasma_with_an_implicit_scheme() {
    let h = Home::new();
    let profile = ".config/kolira/themes/accent";
    h.write(
        &format!("{profile}/theme.json"),
        r#"{"schema":"kolira/theme@2","name":"accent","appearance_only":true}"#,
    );
    h.write(".config/kdeglobals", "[KDE]\nwidgetStyle=Breeze\n");
    h.executable(
        "plasma-apply-colorscheme",
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$HOME/scheme-args\"\n",
    );
    let opened = h.rpc("bundle.editor", json!({"name":"accent"}));
    h.rpc(
        "bundle.save_edits",
        json!({"name":"accent","revision":opened["revision"],"changes":{"accent_color":"#123abc"}}),
    );
    let mut apply = h.command("profile.apply");
    apply.env_remove("KTM_THEME_NO_APPLY");
    let result = Home::send(apply, json!({"name":"accent","skip_dependencies":true}));
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(
        fs::read_to_string(h.path("scheme-args")).unwrap(),
        "--accent-color #123abc\n"
    );
    let globals = fs::read_to_string(h.path(".config/kdeglobals")).unwrap();
    assert!(!globals.contains("ColorScheme="));
    assert!(globals.contains("widgetStyle=Breeze"));

    h.executable(
        "kwriteconfig6",
        "#!/bin/sh\n[ \"$1\" = --notify ] || exit 1\nprintf '%s\\n' \"$*\" >> \"$HOME/config-args\"\n",
    );
    let session = h.init();
    let staged = h.rpc(
        "inspector.component",
        json!({"session":session,"id":"accent_from_wallpaper","value":true}),
    );
    assert_eq!(staged["ok"], true);
    assert!(!h.path("config-args").exists());
    let mut apply = h.command("inspector.apply");
    apply.env_remove("KTM_THEME_NO_APPLY");
    let result = Home::send(apply, json!({"session":session}));
    assert_eq!(result["data"]["ok"], true, "{result}");
    assert_eq!(
        fs::read_to_string(h.path("config-args")).unwrap(),
        "--notify --file kdeglobals --group General --key accentColorFromWallpaper --type bool true\n"
    );
    assert_eq!(
        h.rpc("field.read", json!({"id":"accent_from_wallpaper"})),
        "true"
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
        "[%General]\nblurring=true\n",
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
    assert_eq!(
        h.rpc("inspector.apply", json!({"session":id,"check_live":true}))["ok"],
        true
    );
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
    assert_eq!(data["saved"]["previewColors"]["window"], "#20241f");
    assert_eq!(data["package"]["previewColors"]["window"], "");
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
    assert_eq!(
        home.rpc("profile.live_summary", json!({}))["color_scheme"],
        "LiveMustNotLeak"
    );
}

#[test]
fn profile_artwork_uses_only_saved_values_and_local_assets() {
    let h = Home::new();
    let root = ".config/kolira/themes/Sample";
    let asset = format!("{root}/.local/share/icons/PreviewIcons/48x48/places/folder.svg");
    h.write(
        &format!("{root}/theme.json"),
        &json!({"schema":"kolira/theme@2","name":"Sample","values":{
            "decoration_library":"org.kde.klassy", "klassy_radius":0,
            "translucency_enabled":true, "translucency_inactive":45,
            "ui_font":"Serif,12", "title_font":"Sans Serif,14", "icon_theme":"PreviewIcons"
        }})
        .to_string(),
    );
    h.write(&asset, "<svg xmlns=\"http://www.w3.org/2000/svg\"/>");
    let live = ".config/kdeglobals";
    h.write(live, "[General]\nfont=Unrelated,25\n");
    let summary = || h.rpc("profile.summaries", json!({"names":["Sample"]}))["Sample"].clone();
    let initial = summary();
    let style = &initial["previewStyle"];
    assert_eq!(style["windowOpacity"], 0.45);
    assert_eq!(style["radius"], 0.0);
    assert_eq!(style["font"], json!({"family":"Serif","points":12.0}));
    assert_eq!(
        style["titleFont"],
        json!({"family":"Sans Serif","points":14.0})
    );
    assert!(style["terminalOpacity"].is_null());
    assert_eq!(initial["iconPreview"]["path"], json!(h.path(&asset)));
    let assets = || {
        h.rpc(
            "bundle.resource_previews",
            json!({
                "id":"icon_theme","options":["PreviewIcons","MissingIcons"],"profile":"Sample"
            }),
        )
    };
    assert_eq!(assets()["PreviewIcons"], initial["iconPreview"]);
    assert!(assets()["MissingIcons"].is_null());
    h.write(
        &asset,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"48\"/>",
    );
    assert_ne!(
        summary()["iconPreview"]["revision"],
        initial["iconPreview"]["revision"]
    );
    fs::remove_file(h.path(&asset)).unwrap();
    assert!(summary()["iconPreview"].is_null());
    assert_eq!(
        fs::read_to_string(h.path(live)).unwrap(),
        "[General]\nfont=Unrelated,25\n"
    );
    assert_eq!(h.rpc("profile.active", json!({})), "");
}

#[test]
fn live_matches_only_confirm_readable_saved_appearance() {
    let h = Home::new();
    let globals = ".config/kdeglobals";
    let matching = "[General]\nAccentColor=1,2,3\n";
    for name in ["Partial", "Another"] {
        h.write(
            &format!(".config/kolira/themes/{name}/theme.json"),
            &json!({"schema":"kolira/theme@2","name":name,"values":{"accent_color":"1,2,3"}})
                .to_string(),
        );
        h.write(&format!(".config/kolira/themes/{name}/{globals}"), matching);
    }
    h.write(
        ".config/kolira/themes/Empty/theme.json",
        r#"{"schema":"kolira/theme@2","name":"Empty"}"#,
    );
    h.write(globals, &format!("{matching}PersonalCommand=ignored\n"));
    h.write(".config/kcminputrc", "[Mouse]\ncursorTheme=OutsideTheme\n");
    let names = json!({"names":["Partial","Another","Empty","Missing"]});
    let read = || h.rpc("profile.live_matches", names.clone());
    assert_eq!(read()["matches"], json!(["Another", "Partial"]));
    assert_eq!(read()["profiles"]["Empty"], "unknown");
    assert_eq!(read()["profiles"]["Missing"], "unknown");
    h.write(globals, "[General]\nAccentColor=8,9,10\n");
    assert_eq!(read()["profiles"]["Partial"], "different");
    assert!(h
        .rpc("profile.active", json!({}))
        .as_str()
        .unwrap()
        .is_empty());
    fs::remove_file(h.path(globals)).unwrap();
    assert_eq!(read()["profiles"]["Partial"], "unknown");
}

#[test]
fn managed_points_restore_appearance_and_keep_history_independent() {
    let h = Home::new();
    let globals = ".config/kdeglobals";
    let cursor = ".config/kcminputrc";
    h.write(
        globals,
        "[General]\nAccentColor=1,2,3\nOtherCommand=before\n",
    );
    h.write(cursor, "[Mouse]\ncursorTheme=breeze_cursors\n");
    let manual = h.rpc("restore_point.create", json!({"name":"Before experiments"}));
    let manual_id = manual["id"].as_str().unwrap();
    assert_eq!(h.rpc("operation.list", json!({}))["operations"], json!([]));
    h.rpc(
        "restore_point.rename",
        json!({"id":manual_id,"name":"First desktop"}),
    );
    h.rpc("restore_point.pin", json!({"id":manual_id,"pinned":true}));
    let target = ".config/kolira/themes/Tint";
    h.write(
        &format!("{target}/theme.json"),
        r#"{"schema":"kolira/theme@2","name":"Tint","values":{"accent_color":"40,50,60"}}"#,
    );
    h.write(
        &format!("{target}/{globals}"),
        "[General]\nAccentColor=40,50,60\n",
    );
    let applied = h.rpc(
        "profile.apply",
        json!({"name":"Tint","source":"gui","skip_dependencies":true}),
    );
    let automatic = applied["operation_id"].as_str().unwrap();
    let items = h.rpc("restore_point.list", json!({}));
    let points = items["points"].as_array().unwrap();
    assert!(points
        .iter()
        .any(|point| point["id"] == automatic && point["kind"] == "automatic"));
    assert!(points.iter().any(|point| point["id"] == manual_id
        && point["name"] == "First desktop"
        && point["pinned"] == true));
    assert_eq!(items["max_count"], 20);
    assert_eq!(items["max_bytes"], 2_u64 * 1024 * 1024 * 1024);
    h.write(cursor, "[Mouse]\ncursorTheme=ChangedOutside\n");
    assert_eq!(
        h.raw("restore_point.restore", json!({"id":automatic}))["ok"],
        false
    );
    h.rpc(
        "restore_point.restore",
        json!({"id":automatic,"confirmed":true,"source":"gui"}),
    );
    assert!(fs::read_to_string(h.path(globals))
        .unwrap()
        .contains("AccentColor=1,2,3"));
    assert!(fs::read_to_string(h.path(cursor))
        .unwrap()
        .contains("ChangedOutside"));
    h.write(
        globals,
        "[General]\nAccentColor=80,90,100\nOtherCommand=after\n",
    );
    h.rpc(
        "restore_point.restore",
        json!({"id":manual_id,"confirmed":true,"source":"gui"}),
    );
    let result = fs::read_to_string(h.path(globals)).unwrap();
    assert!(
        result.contains("AccentColor=1,2,3") && result.contains("OtherCommand=after"),
        "{result}"
    );
    assert!(fs::read_to_string(h.path(cursor))
        .unwrap()
        .contains("breeze_cursors"));
    h.rpc(
        "restore_point.delete",
        json!({"id":automatic,"confirmed":true}),
    );
    let history = h.rpc("operation.list", json!({"limit":1,"offset":2}));
    assert_eq!(history["operations"][0]["id"], automatic);
    assert!(history["next_offset"].is_null());
}

#[test]
fn legacy_migration_preserves_overrides_and_reviews_the_result_as_a_package() {
    let h = Home::new();
    let old = ".config/kolira/themes/Old";
    let globals = "[General]\nAccentColor=4,5,6\nAnimationDurationFactor=1\n";
    let live = "[General]\nAccentColor=9,10,11\nAnimationDurationFactor=1\n";
    let editor = r#"{"editor.fontSize":19,"workbench.colorTheme":"Current"}"#;
    let wall = h.path("Pictures/legacy.png");
    let bundle = json!({
        "schema":"theme-bundle/1",
        "components":{"accent_color":"#112233","animations":1.5,"gtk_dark":true},
        "applications":{"vscode":"Saved Editor"},
        "createdAt":"2023-01-01T12:00:00Z","wallpaper":wall
    });
    h.write(".config/kdeglobals", live);
    h.write(".config/Code/User/settings.json", editor);
    h.write("Pictures/legacy.png", "legacy image");
    h.write(&format!("{old}/.config/kdeglobals"), globals);
    h.write(
        &format!("{old}/.config/Code/User/settings.json"),
        r#"{"editor.fontSize":8,"workbench.colorTheme":"Snapshot"}"#,
    );
    h.write(&format!("{old}/theme-bundle.json"), &bundle.to_string());
    h.write(
        &format!("{old}/manifest.json"),
        r#"{"description":"Original description"}"#,
    );
    h.write(
        ".config/kolira/app-preferences.json",
        r#"{"default_components_mode":"custom","custom_components":["kwin"]}"#,
    );
    assert_eq!(
        h.rpc("bundle.metadata", json!({"name":"Old"}))["legacy"],
        true
    );
    let legacy = h.rpc("profile.preview", json!({"name":"Old"}));
    assert!(!h.path(&format!("{old}/theme.json")).exists());
    let request = json!({"name":"Old","keep_wallpaper":true,"without_sync":true});
    let review = h.rpc("profile.prepare_apply", request.clone());
    assert_ne!(review["token"], legacy["token"]);
    assert!(review["components"].is_null());
    assert_eq!(review["keep_wallpaper"], true);
    assert_eq!(review["without_sync"], true);
    let backup = PathBuf::from(review["migration_backup"].as_str().unwrap());
    assert_eq!(
        fs::read_to_string(backup.join(".config/kdeglobals")).unwrap(),
        globals
    );
    assert_eq!(
        fs::read_to_string(backup.join("theme-bundle.json")).unwrap(),
        bundle.to_string()
    );
    assert_eq!(
        fs::read_to_string(h.path(".config/kdeglobals")).unwrap(),
        live
    );
    assert_eq!(
        fs::read_to_string(h.path(".config/Code/User/settings.json")).unwrap(),
        editor
    );
    assert_eq!(h.rpc("profile.active", json!({})), "");
    let package: Value =
        serde_json::from_reader(fs::File::open(h.path(&format!("{old}/theme.json"))).unwrap())
            .unwrap();
    assert_eq!(package["schema"], "kolira/theme@2");
    assert_eq!(package["description"], "Original description");
    assert_eq!(package["created"], "2023-01-01T12:00:00Z");
    assert_eq!(package["wallpaper"], "wallpaper/legacy.png");
    assert!(!h.path(&format!("{old}/theme-bundle.json")).exists());
    assert_eq!(package["values"]["accent_color"], "17,34,51");
    assert_eq!(package["values"]["animation_factor"], 1.5);
    assert_eq!(package["values"]["gtk_dark"], true);
    assert_eq!(package["values"]["vscode_theme"], "Saved Editor");
    assert!(package["values"].get("color_scheme").is_none());
    assert!(package["values"].get("cursor_theme").is_none());
    assert_eq!(
        h.rpc("profile.migrate", json!({"name":"Old"}))["migrated"],
        false
    );
    let repeat = h.rpc("profile.prepare_apply", request);
    assert_eq!(repeat["token"], review["token"]);
    assert!(repeat.get("migration_backup").is_none());
    h.rpc(
        "profile.apply",
        json!({
            "name":"Old","keep_wallpaper":true,"without_sync":true,
            "preview_token":repeat["token"],"source":"gui"
        }),
    );
    let applied = current_globals(&h);
    assert!(
        applied.contains("AccentColor=17,34,51") && applied.contains("AnimationDurationFactor=1.5")
    );
    let current: Value =
        serde_json::from_reader(fs::File::open(h.path(".config/Code/User/settings.json")).unwrap())
            .unwrap();
    assert_eq!(current["workbench.colorTheme"], "Saved Editor");
    assert_eq!(current["editor.fontSize"], 19);
    h.write(
        ".config/kolira/themes/Other/.config/kdeglobals",
        "[General]\nAccentColor=30,40,50\n",
    );
    h.write("Pictures/sidecar%20wall.png", "wallpaper from sidecar");
    h.write(
        ".config/kolira/themes/Other/wallpaper/source",
        h.path("Pictures/sidecar%20wall.png").to_str().unwrap(),
    );
    h.rpc("profile.apply", json!({"name":"Other","source":"gui"}));
    let other = fs::read_to_string(h.path(".config/kolira/themes/Other/theme.json")).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&other).unwrap()["wallpaper"],
        "wallpaper/sidecar%20wall.png"
    );
    h.rpc(
        "profile.apply",
        json!({"name":"Old","source":"gui","keep_wallpaper":true}),
    );
    assert_eq!(current_globals(&h), applied);
}

fn current_globals(h: &Home) -> String {
    fs::read_to_string(h.path(".config/kdeglobals")).unwrap()
}

#[test]
fn failed_legacy_migration_preserves_original_and_blocks_gui_apply() {
    let h = Home::new();
    let old = ".config/kolira/themes/Old";
    let art = ".config/kolira/themes/Linked";
    h.seed();
    let live = current_globals(&h);
    let original = "[General]\nAccentColor=4,5,6\n";
    let bundle = r##"{"schema":"theme-bundle/1","components":{"accent_color":"#AABBCC","unsupported-old-setting":"unknown"}}"##;
    h.write(&format!("{old}/.config/kdeglobals"), original);
    h.write(&format!("{old}/theme-bundle.json"), bundle);
    for method in ["profile.migrate", "profile.prepare_apply", "profile.apply"] {
        let result = h.raw(method, json!({"name":"Old","source":"gui"}));
        assert_eq!(result["ok"], false, "{result}");
        assert!(
            result["error"]
                .as_str()
                .unwrap()
                .contains("unsupported-old-setting"),
            "{result}"
        );
    }
    assert_eq!(current_globals(&h), live);
    assert_eq!(
        fs::read_to_string(h.path(&format!("{old}/.config/kdeglobals"))).unwrap(),
        original
    );
    assert_eq!(
        fs::read_to_string(h.path(&format!("{old}/theme-bundle.json"))).unwrap(),
        bundle
    );
    assert!(!h.path(&format!("{old}/theme.json")).exists());
    assert!(!h.path(".config/kolira/backups").exists());
    assert_eq!(h.rpc("profile.active", json!({})), "");

    h.write(
        &format!("{old}/theme-bundle.json"),
        r#"{"components":{"plasma_global":"org.example.NotCaptured"}}"#,
    );
    let error = h.raw("profile.migrate", json!({"name":"Old"}));
    assert_eq!(error["ok"], false);
    assert!(error["error"]
        .as_str()
        .unwrap()
        .contains("global theme was not captured"));
    assert!(!h.path(&format!("{old}/theme.json")).exists());

    h.write(&format!("{art}/manifest.json"), "{}");
    std::os::unix::fs::symlink(h.path(".config"), h.path(&format!("{art}/.config"))).unwrap();
    let error = h.raw("profile.migrate", json!({"name":"Linked"}));
    assert_eq!(error["ok"], false);
    assert!(error["error"].as_str().unwrap().contains("outside theme"));
    assert!(!h.path(&format!("{art}/theme.json")).exists());
    assert_eq!(current_globals(&h), live);
}

#[test]
fn live_wallpaper_draft_captures_and_applies_all_desktops_without_changing_panels() {
    let h = Home::new();
    let layout = kolira_core::wallpaper::APPLETSRC_REL;
    h.write("Pictures/old-one.png", "old first image");
    h.write("Pictures/old-two.png", "old second image");
    h.write("Pictures/outside.png", "selected outside Kolira");
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"/>"#;
    h.write("Pictures/new #%.svg", svg);
    let picture = h.path("Pictures/new #%.svg");
    let old_one = h.path("Pictures/old-one.png");
    let old_two = h.path("Pictures/old-two.png");
    let external = h.path("Pictures/outside.png");
    let original = format!("[Containments][1]\nplugin=org.kde.plasma.folder\nlastScreen=0\nwallpaperplugin=org.kde.image\n\n[Containments][1][Wallpaper][org.kde.image][General]\nImage=file://{}\nFillMode=2\n\n[Containments][2]\nplugin=org.kde.plasma.folder\nlastScreen=1\nwallpaperplugin=org.kde.image\n\n[Containments][2][Wallpaper][org.kde.image][General]\nImage=file://{}\n\n[Containments][3]\nplugin=org.kde.panel\nlocation=4\n", old_one.display(), old_two.display());
    h.write(layout, &original);
    let session = h.init();
    let stage = |image: &std::path::Path| {
        h.rpc(
            "inspector.wallpaper",
            json!({"session":session,"value":image,"stage":true}),
        )
    };
    assert_eq!(stage(&h.path("Pictures/missing.png"))["ok"], false);
    let pending = stage(&picture);
    assert_eq!(pending["wallpaperDraft"], json!(picture));
    assert_eq!(pending["draft"][0]["after"], "new #%.svg");
    assert_eq!(fs::read_to_string(h.path(layout)).unwrap(), original);
    assert!(h.rpc(
        "inspector.discard",
        json!({"session":session,"section":"wallpaper"})
    )["wallpaperDraft"]
        .is_null());
    stage(&picture);
    h.rpc("profile.capture_live", json!({"name":"Actual"}));
    h.rpc(
        "profile.capture_live",
        json!({"name":"Draft","draft_session":session}),
    );
    let package: Value = serde_json::from_slice(
        &fs::read(h.path(".config/kolira/themes/Draft/theme.json")).unwrap(),
    )
    .unwrap();
    let media = package["wallpaper"].as_str().unwrap();
    assert_eq!(media, "wallpaper/new #%.svg");
    assert!(h
        .path(".config/kolira/themes/Actual/wallpaper/old-one.png")
        .is_file());
    assert_eq!(
        fs::read_to_string(h.path(&format!(".config/kolira/themes/Draft/{media}"))).unwrap(),
        svg
    );
    assert!(!h
        .path(".config/kolira/themes/Draft/wallpaper/old-one.png")
        .exists());
    h.rpc(
        "profile.capture_live",
        json!({"name":"Without","draft_session":session,"include_wallpaper":false}),
    );
    let filtered: Value = serde_json::from_slice(
        &fs::read(h.path(".config/kolira/themes/Without/theme.json")).unwrap(),
    )
    .unwrap();
    assert!(filtered["wallpaper"].is_null());
    assert_eq!(fs::read_to_string(h.path(layout)).unwrap(), original);
    assert_eq!(h.rpc("profile.active", json!({})), "");

    let refresh = || h.rpc("inspector.refresh", json!({"session":session}));
    let panel_changed = original.replace("location=4", "location=3");
    h.write(layout, &panel_changed);
    assert_eq!(refresh()["data"]["conflicts"], json!([]));
    let current = panel_changed.replace(old_two.to_str().unwrap(), external.to_str().unwrap());
    h.write(layout, &current);
    let apply = || {
        h.rpc(
            "inspector.apply",
            json!({"session":session,"check_live":true,"source":"gui"}),
        )
    };
    let blocked = apply();
    assert_eq!(blocked["ok"], false);
    let conflict = &blocked["data"]["conflicts"][0];
    assert_eq!(conflict["id"], "wallpaper");
    assert!(conflict["current"]["display"]
        .as_str()
        .unwrap()
        .contains("outside.png"));
    h.rpc(
        "inspector.resolve",
        json!({"session":session,"token":conflict["token"],"choice":"draft"}),
    );
    let committed = apply();
    assert_eq!(committed["ok"], true);
    assert_eq!(committed["undoDepth"], 1);
    assert!(committed["wallpaperDraft"].is_null());
    let changed = fs::read_to_string(h.path(layout)).unwrap();
    assert_eq!(changed.matches("new%20%23%25.svg").count(), 2);
    assert!(changed.contains("FillMode=2") && changed.contains("location=3"));
    let expanded = format!(
        "{}\n[Containments][4]\nplugin=org.kde.plasma.folder\nlastScreen=2\nwallpaperplugin=org.kde.image\n\n[Containments][4][Wallpaper][org.kde.image][General]\nImage=file://{}\n",
        changed.replace("location=3", "location=2").replace("FillMode=2", "FillMode=4"),
        old_two.display()
    );
    h.write(layout, &expanded);
    assert_eq!(
        h.rpc("inspector.undo", json!({"session":session}))["ok"],
        true
    );
    let undone = fs::read_to_string(h.path(layout)).unwrap();
    assert!(undone.contains("old-one.png") && undone.contains("outside.png"));
    assert!(undone.contains("location=2") && !undone.contains("new%20"));
    assert!(undone.contains("FillMode=4"));
    assert!(undone.contains("[Containments][4][Wallpaper][org.kde.image][General]"));
    assert!(undone.contains("old-two.png"));
}

#[test]
fn live_wallpaper_native_failure_rolls_back_app_changes_and_preserves_draft() {
    let h = Home::new();
    let layout = kolira_core::wallpaper::APPLETSRC_REL;
    let code = ".config/Code/User/settings.json";
    h.executable("code", "#!/bin/sh\nexit 0\n");
    let editor = r#"{"workbench.colorTheme":"Default Dark Modern"}"#;
    h.write(code, editor);
    h.write("old.png", "old image");
    h.write("new.png", "new image");
    let old = format!("[Containments][1]\nlastScreen=0\nwallpaperplugin=org.kde.image\n[Containments][1][Wallpaper][org.kde.image][General]\nImage=file://{}\n", h.path("old.png").display());
    h.write(layout, &old);
    h.executable("qdbus6", r#"#!/usr/bin/python3
import json, os, re, sys
from pathlib import Path
if len(sys.argv) < 5 or not sys.argv[3].endswith("evaluateScript"):
    sys.exit(0)
layout = Path(os.environ["XDG_CONFIG_HOME"]) / "plasma-org.kde.plasma.desktop-appletsrc"
script = sys.argv[4]
text = layout.read_text()
if "const result" in script:
    print(json.dumps({"1": {"plugin": "org.kde.image", "image": re.search(r"(?m)^Image=(.*)$", text)[1]}}))
elif "const selection" in script:
    selection = json.loads(re.search(r"const selection = (.*);", script)[1])
    image = selection["1"]["image"]
    layout.write_text(re.sub(r"(?m)^Image=.*$", "Image=" + image, text))
    if image.endswith("/new.png"):
        sys.exit(1)
    print('["1"]')
"#);
    let session = h.init();
    assert_eq!(
        h.rpc(
            "inspector.wallpaper",
            json!({"session":session,"stage":true,"value":h.path("new.png")})
        )["ok"],
        true
    );
    assert_eq!(
        h.rpc(
            "inspector.app",
            json!({"session":session,"stage":true,"id":"vscode","value":"Monokai"})
        )["ok"],
        true
    );
    let mut command = h.command("inspector.apply");
    command.env_remove("KTM_THEME_NO_APPLY");
    let result = Home::send(
        command,
        json!({"session":session,"check_live":true,"source":"gui"}),
    );
    let failed = &result["data"];
    assert_eq!(failed["ok"], false);
    assert!(
        failed["message"].as_str().unwrap().contains("rolled_back"),
        "{failed}"
    );
    assert_eq!(failed["wallpaperDraft"], json!(h.path("new.png")));
    assert_eq!(failed["apps"]["vscode"], "Monokai");
    assert_eq!(fs::read_to_string(h.path(layout)).unwrap(), old);
    assert_eq!(fs::read_to_string(h.path(code)).unwrap(), editor);
}

#[test]
fn live_app_drafts_review_conflicts_apply_and_capture_without_mutating_live() {
    let h = Home::new();
    h.executable("code", "#!/bin/sh\nexit 0\n");
    h.executable("zeditor", "#!/bin/sh\nexit 0\n");
    let code = ".config/Code/User/settings.json";
    let zed = ".config/zed/settings.json";
    let original = r#"{"workbench.colorTheme":"Default Dark Modern","editor.fontSize":17}"#;
    let zed_original = r#"{"theme":{"mode":"system","light":"One Light","dark":"One Dark"}}"#;
    h.write(code, original);
    h.write(zed, zed_original);
    let session = h.init();
    let wallpaper = h.rpc(
        "inspector.wallpaper",
        json!({"session":session,"value":"/tmp/example.png"}),
    );
    assert_eq!(wallpaper["appDraftCount"], 0);
    assert_eq!(wallpaper["draft"], json!([]));
    assert_eq!(
        h.rpc("inspector.apply", json!({"session":session}))["ok"],
        true
    );
    let stage = |id, value| {
        h.rpc(
            "inspector.app",
            json!({"session":session,"stage":true,"id":id,"value":value}),
        )
    };
    let first = stage("vscode", "Monokai");
    assert_eq!(first["apps"]["vscode"], "Monokai");
    assert_eq!(first["appDraftCount"], 1);
    assert_eq!(first["draft"][0]["before"], "Default Dark Modern");
    assert_eq!(first["draft"][0]["after"], "Monokai");
    assert_eq!(fs::read_to_string(h.path(code)).unwrap(), original);
    assert_eq!(stage("vscode", "Default Dark Modern")["draft"], json!([]));
    stage("zed", "Ayu Dark");
    stage("vscode", "Monokai");

    let code_live = original.replace(":17", ":21");
    let zed_live = zed_original.replace("One Light", "Ayu Light");
    h.write(code, &code_live);
    h.write(zed, &zed_live);
    let refresh = || h.rpc("inspector.refresh", json!({"session":session}));
    assert_eq!(refresh()["data"]["conflicts"], json!([]));
    h.rpc(
        "profile.capture_live",
        json!({"name":"Actual","include_wallpaper":false}),
    );
    h.rpc(
        "profile.capture_live",
        json!({"name":"Draft","draft_session":session,"include_wallpaper":false}),
    );
    let values = |profile: &str| -> Value {
        let file = h.path(&format!(".config/kolira/themes/{profile}/theme.json"));
        serde_json::from_slice::<Value>(&fs::read(file).unwrap()).unwrap()["values"].clone()
    };
    assert_eq!(values("Actual")["vscode_theme"], "Default Dark Modern");
    assert_eq!(values("Draft")["vscode_theme"], "Monokai");
    assert_eq!(values("Draft")["zed_dark"], "Ayu Dark");
    assert_eq!(values("Draft")["zed_light"], "Ayu Light");
    assert_eq!(fs::read_to_string(h.path(code)).unwrap(), code_live);
    assert_eq!(fs::read_to_string(h.path(zed)).unwrap(), zed_live);
    let code_live = code_live.replace("Default Dark Modern", "Solarized Light");
    h.write(code, &code_live);
    let apply = || {
        h.rpc(
            "inspector.apply",
            json!({"session":session,"check_live":true,"source":"gui"}),
        )
    };
    let blocked = apply();
    assert_eq!(blocked["ok"], false);
    assert_eq!(blocked["data"]["conflicts"].as_array().unwrap().len(), 1);
    let conflict = &blocked["data"]["conflicts"][0];
    assert_eq!(conflict["before"], "Default Dark Modern");
    assert_eq!(conflict["current"], "Solarized Light");
    assert_eq!(conflict["after"], "Monokai");
    h.rpc(
        "inspector.resolve",
        json!({"session":session,"token":conflict["token"],"choice":"draft"}),
    );
    let applied = apply();
    assert_eq!(applied["ok"], true);
    assert_eq!(applied["draft"], json!([]));
    assert_eq!(applied["undoDepth"], 2);
    let code_applied = fs::read_to_string(h.path(code)).unwrap();
    assert!(code_applied.contains("Monokai") && code_applied.contains("\"editor.fontSize\":21"));
    let zed_applied = fs::read_to_string(h.path(zed)).unwrap();
    assert!(zed_applied.contains("Ayu Dark") && zed_applied.contains("Ayu Light"));
    for _ in 0..2 {
        assert_eq!(
            h.rpc("inspector.undo", json!({"session":session}))["ok"],
            true
        );
    }
    assert_eq!(fs::read_to_string(h.path(code)).unwrap(), code_live);
    assert_eq!(fs::read_to_string(h.path(zed)).unwrap(), zed_live);
}

#[test]
fn unavailable_live_app_theme_keeps_all_draft_changes_and_category_discard_is_scoped() {
    let h = Home::new();
    h.seed();
    h.executable("code", "#!/bin/sh\nexit 0\n");
    let code = ".config/Code/User/settings.json";
    let original = r#"{"workbench.colorTheme":"Monokai"}"#;
    let extension = ".vscode/extensions/example.colors/package.json";
    h.write(code, original);
    h.write(extension, r#"{"name":"colors","publisher":"example","contributes":{"themes":[{"id":"Package Theme","label":"Package Theme"}]}}"#);
    let kde = fs::read(h.path(".config/kdeglobals")).unwrap();
    let session = h.init();
    assert_eq!(
        h.rpc(
            "inspector.app",
            json!({"session":session,"stage":true,"id":"vscode","value":"Package Theme"})
        )["ok"],
        true
    );
    assert_eq!(
        h.rpc(
            "inspector.stage",
            json!({"session":session,"id":"window_bg","value":"#112233"})
        )["ok"],
        true
    );
    fs::remove_file(h.path(extension)).unwrap();
    let failed = h.rpc(
        "inspector.apply",
        json!({"session":session,"check_live":true}),
    );
    assert_eq!(failed["ok"], false);
    assert!(failed["message"]
        .as_str()
        .unwrap()
        .contains("Package Theme"));
    assert_eq!(failed["draft"].as_array().unwrap().len(), 2);
    assert_eq!(fs::read_to_string(h.path(code)).unwrap(), original);
    assert_eq!(fs::read(h.path(".config/kdeglobals")).unwrap(), kde);
    let discarded = h.rpc(
        "inspector.discard",
        json!({"session":session,"section":"apps"}),
    );
    assert_eq!(discarded["draft"].as_array().unwrap().len(), 1);
    assert_eq!(discarded["apps"], json!({}));
    assert_eq!(discarded["staged"]["window_bg"], "17,34,51");
}

#[test]
fn capture_live_with_draft_keeps_live_and_staging_untouched() {
    let h = Home::new();
    h.seed();
    let editor = ".config/Code/User/settings.json";
    h.write(
        editor,
        r#"{"workbench.colorTheme":"Original","editor.fontSize":17}"#,
    );
    let original = fs::read(h.path(".config/kdeglobals")).unwrap();
    let editor_original = fs::read(h.path(editor)).unwrap();
    let session = h.init();
    assert_eq!(
        h.rpc(
            "inspector.stage",
            json!({"session":session,"id":"window_bg","value":"#112233"})
        )["ok"],
        true
    );
    assert_eq!(
        h.rpc(
            "inspector.config",
            json!({"session":session,"path":editor,"key":"workbench.colorTheme","value":"Draft"})
        )["ok"],
        true
    );
    h.rpc(
        "profile.capture_live",
        json!({"name":"Actual","include_wallpaper":false}),
    );
    h.rpc(
        "profile.capture_live",
        json!({"name":"Drafted","include_wallpaper":false,"draft_session":session}),
    );
    let read = |profile: &str| {
        let root = h.path(&format!(".config/kolira/themes/{profile}"));
        let package: Value =
            serde_json::from_slice(&fs::read(root.join("theme.json")).unwrap()).unwrap();
        let settings: Value =
            serde_json::from_slice(&fs::read(root.join(editor)).unwrap()).unwrap();
        (
            package["values"]["window_bg"].clone(),
            settings["workbench.colorTheme"].clone(),
        )
    };
    assert_eq!(read("Actual"), (json!("32,36,31"), json!("Original")));
    assert_eq!(read("Drafted"), (json!("17,34,51"), json!("Draft")));
    assert_eq!(fs::read(h.path(".config/kdeglobals")).unwrap(), original);
    assert_eq!(fs::read(h.path(editor)).unwrap(), editor_original);
    assert_eq!(
        h.rpc("inspector.state", json!({"session":session}))["staged"]["window_bg"],
        "17,34,51"
    );
    assert!(!h.path(".config/kolira/active-profile").exists());

    assert_eq!(
        h.rpc(
            "inspector.config",
            json!({"session":session,"path":editor,"key":"editor.fontSize","value":"33"})
        )["ok"],
        true
    );
    assert_eq!(
        h.raw(
            "profile.capture_live",
            json!({"name":"Excluded","draft_session":session})
        )["ok"],
        false
    );
    assert!(!h.path(".config/kolira/themes/Excluded").exists());
    assert_eq!(fs::read(h.path(editor)).unwrap(), editor_original);
    let returned = h.rpc(
        "inspector.stage",
        json!({"session":session,"id":"window_bg","value":"#20241f"}),
    );
    assert!(returned["staged"].get("window_bg").is_none());
    assert_eq!(returned["draft"].as_array().unwrap().len(), 2);
}

#[test]
fn live_refresh_preserves_unrelated_edits_resolves_conflicts_and_rechecks_apply() {
    let h = Home::new();
    h.seed();
    let desktop = ".config/kdeglobals";
    let editor = ".config/Code/User/settings.json";
    h.write(
        editor,
        r#"{"workbench.colorTheme":"Original","editor.fontSize":17}"#,
    );
    let session = h.init();
    h.rpc(
        "inspector.stage",
        json!({"session":session,"id":"window_bg","value":"#112233"}),
    );
    h.rpc(
        "inspector.stage",
        json!({"session":session,"id":"text_color","value":"#445566"}),
    );
    h.rpc(
        "inspector.config",
        json!({"session":session,"path":editor,"key":"workbench.colorTheme","value":"Draft"}),
    );

    h.write(
        editor,
        r#"{"workbench.colorTheme":"Original","editor.fontSize":20}"#,
    );
    let refreshed = h.rpc("inspector.refresh", json!({"session":session}));
    assert_eq!(refreshed["data"]["conflicts"], json!([]));
    assert_eq!(refreshed["draft"].as_array().unwrap().len(), 3);

    let config = fs::read_to_string(h.path(desktop)).unwrap();
    let changed = kolira_core::field::set_ini_text(
        &kolira_core::field::set_ini_text(&config, "Colors:Window", "BackgroundNormal", "1,2,3"),
        "Colors:Window",
        "ForegroundNormal",
        "68,85,102",
    );
    h.write(desktop, &changed);
    h.write(
        editor,
        r#"{"workbench.colorTheme":"External","editor.fontSize":20}"#,
    );
    let checked = h.rpc(
        "inspector.apply",
        json!({"session":session,"check_live":true}),
    );
    assert_eq!(checked["ok"], false);
    assert!(checked["staged"].get("text_color").is_none());
    assert_eq!(checked["data"]["conflicts"].as_array().unwrap().len(), 2);
    assert_eq!(fs::read_to_string(h.path(desktop)).unwrap(), changed);
    let old_token = &checked["data"]["conflicts"][0]["token"];
    let changed_again = changed.replace("1,2,3", "3,4,5");
    h.write(desktop, &changed_again);
    let still_conflicted = h.rpc(
        "inspector.resolve",
        json!({"session":session,"token":old_token,"choice":"draft"}),
    );
    let entries = still_conflicted["data"]["conflicts"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_ne!(entries[0]["token"], *old_token);
    assert_eq!(entries[0]["current"], "3,4,5");

    let resolved = h.rpc(
        "inspector.resolve",
        json!({"session":session,"token":entries[0]["token"],"choice":"draft"}),
    );
    assert_eq!(resolved["data"]["conflicts"].as_array().unwrap().len(), 1);
    let resolved = h.rpc(
        "inspector.resolve",
        json!({"session":session,"token":entries[1]["token"],"choice":"live"}),
    );
    assert_eq!(resolved["data"]["conflicts"], json!([]));
    assert_eq!(fs::read_to_string(h.path(desktop)).unwrap(), changed_again);
    let applied = h.rpc(
        "inspector.apply",
        json!({"session":session,"check_live":true}),
    );
    assert_eq!(applied["ok"], true, "{applied}");
    assert_eq!(h.rpc("field.read", json!({"id":"window_bg"})), "17,34,51");
    assert_eq!(h.rpc("field.read", json!({"id":"text_color"})), "68,85,102");
    assert_eq!(
        fs::read_to_string(h.path(editor)).unwrap(),
        r#"{"workbench.colorTheme":"External","editor.fontSize":20}"#
    );
}

#[test]
fn unreadable_live_is_a_read_error_and_keeps_the_draft() {
    let h = Home::new();
    h.seed();
    let session = h.init();
    h.rpc(
        "inspector.stage",
        json!({"session":session,"id":"window_bg","value":"#112233"}),
    );
    let file = h.path(".config/kdeglobals");
    fs::write(&file, [0xff, 0xfe]).unwrap();
    let response = h.rpc("inspector.refresh", json!({"session":session}));
    assert_eq!(response["ok"], false);
    assert!(response["data"].is_null());
    assert_eq!(response["staged"]["window_bg"], "17,34,51");
    assert_eq!(fs::read(file).unwrap(), [0xff, 0xfe]);
}

#[test]
fn config_type_change_still_allows_using_the_external_value() {
    let h = Home::new();
    let editor = ".config/Code/User/settings.json";
    h.write(editor, r#"{"workbench.colorTheme":"Original"}"#);
    let session = h.init();
    let staged = h.rpc(
        "inspector.config",
        json!({"session":session,"path":editor,"key":"workbench.colorTheme","value":"Draft"}),
    );
    assert_eq!(staged["ok"], true);
    h.write(editor, r#"{"workbench.colorTheme":true}"#);
    let refreshed = h.rpc("inspector.refresh", json!({"session":session}));
    let entry = &refreshed["data"]["conflicts"][0];
    assert_eq!(entry["current"], true);
    assert_eq!(entry["after"], "Draft");
    assert_eq!(entry["can_keep"], false);
    let resolved = h.rpc(
        "inspector.resolve",
        json!({"session":session,"token":entry["token"],"choice":"live"}),
    );
    assert_eq!(resolved["draft"], json!([]));
    assert_eq!(
        fs::read_to_string(h.path(editor)).unwrap(),
        r#"{"workbench.colorTheme":true}"#
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

fn quick_switch_theme(h: &Home, name: &str, accent: &str, wallpaper: Option<&str>) {
    let package = json!({
        "schema": "kolira/theme@2",
        "name": name,
        "created": "2026-11-24T00:00:00Z",
        "palette": {
            "name": name,
            "variant": "dark",
            "base": [
                "#101014", "#17171d", "#2a2a32", "#3a3a42",
                "#4a4a52", "#5a5a62", "#6a6a72", "#7a7a82",
                "#da4453", "#f67400", "#f8a458", "#27ae60",
                "#22a4aa", "#1d99f3", "#9b59b6", "#e85c2a"
            ],
            "accent": accent
        },
        "values": {
            "accent_color": accent,
            "selection_bg": accent,
            "window_bg": "16,17,20",
            "view_bg": "20,21,24",
            "text_color": "240,240,240",
            "color_scheme": name
        },
        "wallpaper": wallpaper.unwrap_or("")
    });
    h.write(
        &format!(".config/kolira/themes/{name}/theme.json"),
        &package.to_string(),
    );
    if let Some(relative) = wallpaper {
        h.write(
            &format!(".config/kolira/themes/{name}/{relative}"),
            "not-a-real-image",
        );
    }
}

#[test]
fn quick_switch_defaults_merge_and_reject_bad_values() {
    let h = Home::new();
    let defaults = h.rpc("quick_switch.get", json!({}));
    assert_eq!(defaults["enabled"], false);
    assert_eq!(defaults["hotkey"], "Meta+K");
    assert_eq!(defaults["card_shape"], "rounded");
    assert_eq!(defaults["dim"], 0.72);

    h.rpc(
        "quick_switch.set",
        json!({"settings":{"enabled":true,"card_shape":"parallelogram","dim":0.5}}),
    );
    let saved = h.rpc("quick_switch.get", json!({}));
    assert_eq!(saved["enabled"], true);
    assert_eq!(saved["card_shape"], "parallelogram");
    assert_eq!(saved["hotkey"], "Meta+K");

    let before = fs::read(h.path(".config/kolira/app-preferences.json")).unwrap();
    for bad in [
        json!({"settings":{"card_shape":"hexagon"}}),
        json!({"settings":{"dim":2.0}}),
        json!({"settings":{"card_scale":"big"}}),
        json!({"settings":{"hotkey":"Meta+"}}),
        json!({"settings":{"enabled":"yes"}}),
    ] {
        assert_eq!(h.raw("quick_switch.set", bad)["ok"], false);
    }
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
fn quick_switch_cards_stay_cheap_and_keep_foreign_themes() {
    let h = Home::new();
    quick_switch_theme(&h, "zeta", "#bd93f9", Some("wallpaper/z.png"));
    quick_switch_theme(&h, "alpha", "#57d68a", Some("wallpaper/a.png"));
    h.write(
        ".config/kolira/themes/legacy/kdeglobals",
        "[General]\nColorScheme=Base\n",
    );

    let data = h.rpc("quick_switch.cards", json!({}));
    let themes = data["themes"].as_array().unwrap();
    let names: Vec<_> = themes
        .iter()
        .map(|theme| theme["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["alpha", "legacy", "zeta"]);
    assert_eq!(themes[0]["accent"], "#57d68a");
    assert_eq!(themes[0]["variant"], "dark");
    assert_eq!(themes[0]["hasPreview"], true);
    assert!(themes[0]["preview"]["path"]
        .as_str()
        .unwrap()
        .ends_with("wallpaper/a.png"));
    assert_eq!(themes[0]["active"], false);
    // A legacy snapshot has no palette; the card must still appear.
    assert_eq!(themes[1]["accent"], "");
    assert_eq!(themes[1]["hasPreview"], false);
}

#[test]
fn quick_switch_hotkey_moves_voxtype_aside_and_backs_up() {
    let h = Home::new();
    h.write(
        ".config/kglobalshortcutsrc",
        "[services][voxtype-toggle.desktop]\n_launch=Meta+K\n\n[services][net.local.theme-menu.desktop]\n_launch=Meta+Y\n",
    );
    let mut command = h.command("quick_switch.set_hotkey");
    // kwriteconfig6/kreadconfig6 must be reachable from the test.
    let path = format!(
        "{}:{}",
        h.path("bin").display(),
        std::env::var("PATH").unwrap_or_default()
    );
    command.env("PATH", &path);

    let result = Home::send(command, json!({"hotkey":"Meta+K"}));
    assert_eq!(result["ok"], true, "{result}");
    let saved = fs::read_to_string(h.path(".config/kglobalshortcutsrc")).unwrap();
    assert!(saved.contains("[services][kolira-quick-switch.desktop]\n_launch=Meta+K"));
    assert!(saved.contains("[services][voxtype-toggle.desktop]\n_launch=Meta+Shift+K"));
    assert!(saved.contains("[services][net.local.theme-menu.desktop]\n_launch=Meta+Y"));
    assert!(
        fs::read_dir(h.path(".config/kolira/backups/kglobalshortcutsrc"))
            .unwrap()
            .next()
            .is_some()
    );

    let mut unbind = h.command("quick_switch.set_hotkey");
    unbind.env("PATH", &path);
    assert_eq!(Home::send(unbind, json!({"hotkey":""}))["ok"], true);
    let saved = fs::read_to_string(h.path(".config/kglobalshortcutsrc")).unwrap();
    assert!(saved.contains("[services][kolira-quick-switch.desktop]\n_launch=none"));

    let mut bad = h.command("quick_switch.set_hotkey");
    bad.env("PATH", &path);
    assert_eq!(Home::send(bad, json!({"hotkey":"Hyper+X"}))["ok"], false);
}

#[test]
fn failed_core_and_cli_calls_are_written_to_a_bounded_error_log() {
    let h = Home::new();
    // A failing core call (the main GUI path) is recorded with its method name.
    assert_eq!(h.raw("no.such.method", json!({}))["ok"], false);
    // A failing CLI command (the Quick Switch `kolira apply` path) is recorded.
    let mut cli = Command::new(env!("CARGO_BIN_EXE_kolira"));
    cli.args(["apply", "does-not-exist"])
        .env("HOME", h.0.path())
        .env("XDG_CONFIG_HOME", h.path(".config"))
        .env("XDG_DATA_HOME", h.path(".local/share"))
        .env("XDG_CACHE_HOME", h.path(".cache"))
        .env("KTM_THEME_NO_APPLY", "1")
        .env("KTM_OFFLINE", "1");
    assert!(!cli.output().unwrap().status.success());
    let log = fs::read_to_string(h.path(".cache/kolira/error.log")).unwrap();
    assert!(log.contains("ERROR no.such.method:"), "{log}");
    assert!(log.contains("ERROR cli:"), "{log}");
    // Only errors land here: one line per failure, nothing else.
    assert!(log.lines().all(|line| line.contains(" ERROR ")), "{log}");
}
