//! End-to-end regressions for cached extensions, preview, follow-ups and recovery.
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    os::unix::fs::{symlink, PermissionsExt},
    path::PathBuf,
    process::{Command, Output, Stdio},
};
use tempfile::TempDir;

struct Home(TempDir);
impl Home {
    fn new() -> Self {
        let h = Self(tempfile::tempdir().unwrap());
        h.write(".config/kdeglobals", "[General]\nColorScheme=Breeze\n");
        h.write(
            ".config/kolira/themes/next/.config/kdeglobals",
            "[General]\nColorScheme=Breeze\n",
        );
        h.manifest(json!({}));
        h
    }
    fn path(&self, rel: &str) -> PathBuf {
        self.0.path().join(rel)
    }
    fn write(&self, rel: &str, text: &str) {
        let path = self.path(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn manifest(&self, mut value: Value) {
        value["schema"] = "kolira/theme@2".into();
        value["name"] = "next".into();
        self.write(".config/kolira/themes/next/theme.json", &value.to_string());
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_kolira"));
        c.args(args)
            .env("HOME", self.0.path())
            .env("XDG_CONFIG_HOME", self.path(".config"))
            .env("XDG_DATA_HOME", self.path(".local/share"))
            .env("XDG_CACHE_HOME", self.path(".cache"))
            .env("KTM_OFFLINE", "1")
            .env("KTM_THEME_NO_APPLY", "1")
            .env_remove("WALLPAPER_DIR");
        c
    }
    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
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
        let out = child.wait_with_output().unwrap();
        serde_json::from_slice(&out.stdout).unwrap()
    }
    fn palette(&self) -> Value {
        let result = self.run(&["palette", "import", "gallery:nord"]);
        assert!(result.status.success());
        serde_json::from_slice(&fs::read(self.path(".config/kolira/palette.json")).unwrap())
            .unwrap()
    }
    fn editor(&self, local: &str) -> Value {
        json!({"editor":"vscodium", "store":"local", "id":"example.night", "local_path":local, "native_watch":true})
    }
    fn cache(&self) {
        fs::create_dir_all(self.path(".vscode-oss/extensions")).unwrap();
        self.write(".config/kolira/themes/next/assets/editor-themes/vscodium/example.night/package.json",
            &json!({"publisher":"example", "name":"night", "version":"1.0.0", "main":"./extension.js", "activationEvents":["*"], "contributes":{"commands":[{"command":"doAnything"}],"themes":[{"label":"Night","path":"themes/night.json"}]}}).to_string());
        self.write(".config/kolira/themes/next/assets/editor-themes/vscodium/example.night/themes/night.json", "{\"colors\":{}}");
    }
    fn applied(&self, without: bool) -> Value {
        let preview = self.rpc(
            "profile.preview",
            json!({"name":"next", "without_sync":without}),
        );
        assert_eq!(preview["ok"], true, "{preview}");
        let result = self.rpc("profile.apply", json!({"name":"next", "without_sync":without,"preview_token":preview["data"]["token"], "skip_dependencies":true}));
        assert_eq!(result["ok"], true, "{result}");
        result["data"].clone()
    }
}

#[test]
fn cache_installs_complete_extension_and_roundtrips_zip() {
    let h = Home::new();
    h.cache();
    h.manifest(json!({"editor_themes":[h.editor("assets/editor-themes/vscodium/example.night")]}));
    let original = h.path(".vscode-oss/extensions/example.night-1.0.0");
    fs::create_dir_all(&original).unwrap();
    fs::write(original.join("keep"), "original").unwrap();
    let src = h.path(
        ".config/kolira/themes/next/assets/editor-themes/vscodium/example.night/package.json",
    );
    let before = fs::read(&src).unwrap();
    let preview = h.rpc("profile.preview", json!({"name":"next"}));
    assert!(preview["data"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["path"]
            .as_str()
            .unwrap()
            .contains("example.night-kolira/package.json")));
    let report = h.applied(false);
    let installed = h.path(".vscode-oss/extensions/example.night-kolira");
    let pkg: Value =
        serde_json::from_slice(&fs::read(installed.join("package.json")).unwrap()).unwrap();
    assert_eq!(pkg["_watch"], true);
    assert!(pkg.get("main").is_none());
    assert!(pkg.get("activationEvents").is_none());
    assert!(pkg["contributes"].get("commands").is_none());
    assert_eq!(pkg["contributes"]["themes"][0]["label"], "Night");
    assert!(installed.join("themes/night.json").is_file());
    assert_eq!(fs::read(src).unwrap(), before);
    assert_eq!(
        fs::read_to_string(original.join("keep")).unwrap(),
        "original"
    );
    assert!(!h.path(".vscode/extensions").exists());
    let zip = h.path("theme.zip");
    let export = h.run(&["export", "next", &format!("--out={}", zip.display())]);
    assert!(
        export.status.success(),
        "{}",
        String::from_utf8_lossy(&export.stderr)
    );
    let import = h.run(&["import", zip.to_str().unwrap(), "--name=roundtrip"]);
    assert!(
        import.status.success(),
        "{}",
        String::from_utf8_lossy(&import.stderr)
    );
    assert!(h.path(".config/kolira/themes/roundtrip/assets/editor-themes/vscodium/example.night/themes/night.json").is_file());
    let restored = h.rpc(
        "operation.restore",
        json!({"id":report["operation_id"], "confirmed":true}),
    );
    assert_eq!(restored["ok"], true, "{restored}");
    assert!(!installed.exists());
    assert!(original.join("keep").is_file());
}

#[test]
fn editor_paths_and_links_cannot_escape_theme_or_destination() {
    for local in [
        "../outside",
        "/tmp/outside",
        "assets/editor-themes/../../outside",
    ] {
        let h = Home::new();
        h.manifest(json!({"editor_themes":[h.editor(local)]}));
        let out = h.run(&["apply", "next", "--without-dependencies"]);
        assert!(!out.status.success());
        assert!(!h.path(".config/kolira/backups").exists());
    }
    let h = Home::new();
    h.cache();
    h.write("secret", "private");
    symlink(
        h.path("secret"),
        h.path(
            ".config/kolira/themes/next/assets/editor-themes/vscodium/example.night/themes/secret",
        ),
    )
    .unwrap();
    h.manifest(json!({"editor_themes":[h.editor("assets/editor-themes/vscodium/example.night")]}));
    assert!(!h
        .run(&["apply", "next", "--without-dependencies"])
        .status
        .success());
    assert_eq!(fs::read_to_string(h.path("secret")).unwrap(), "private");
    assert!(!h.path(".config/kolira/backups").exists());
}

#[test]
fn app_followups_are_previewed_preserve_personal_settings_and_recover() {
    let h = Home::new();
    let palette = h.palette();
    h.manifest(json!({"palette":palette,"app_targets":["firefox","discord","konsole","ghostty","cava","telegram"]}));
    let user_css = "/* personal Firefox CSS */\n#sidebar { width: 20em; }\n";
    h.write(
        ".mozilla/firefox/profiles.ini",
        "[Profile0]\nPath=probe.default\nDefault=1\n",
    );
    h.write(
        ".mozilla/firefox/probe.default/chrome/userChrome.css",
        user_css,
    );
    h.write(
        ".mozilla/firefox/probe.default/user.js",
        "// privacy\nuser_pref(\"privacy.donottrackheader.enabled\", true);\n",
    );
    h.write(
        ".config/vesktop/settings/settings.json",
        "{\"transparent\":true,\"personal\":42}",
    );
    h.write(
        ".config/chromium/Default/Preferences",
        "{\"profile\":{\"name\":\"Personal\",\"password_manager_enabled\":false}}",
    );
    let prefs = fs::read(h.path(".config/chromium/Default/Preferences")).unwrap();
    let css = h.path(".mozilla/firefox/probe.default/chrome/userChrome.css");
    fs::set_permissions(&css, fs::Permissions::from_mode(0o640)).unwrap();
    let preview = h.rpc("profile.preview", json!({"name":"next"}));
    assert_eq!(preview["ok"], true, "{preview}");
    assert!(preview["data"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["path"].as_str().unwrap().ends_with("/chrome/kolira.css")));
    assert_eq!(fs::read_to_string(&css).unwrap(), user_css);
    let skipped = h.rpc(
        "profile.preview",
        json!({"name":"next","without_sync":true}),
    );
    assert_ne!(preview["data"]["token"], skipped["data"]["token"]);
    h.applied(true);
    assert_eq!(fs::read_to_string(&css).unwrap(), user_css);
    let report = h.applied(false);
    assert!(fs::read_to_string(&css).unwrap().contains(user_css));
    assert_eq!(
        fs::metadata(&css).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert_eq!(
        fs::read(h.path(".config/chromium/Default/Preferences")).unwrap(),
        prefs
    );
    let settings: Value = serde_json::from_slice(
        &fs::read(h.path(".config/vesktop/settings/settings.json")).unwrap(),
    )
    .unwrap();
    assert_ne!(settings["transparent"], false);
    assert_eq!(settings["personal"], 42);
    let user = fs::read_to_string(h.path(".mozilla/firefox/probe.default/user.js")).unwrap();
    assert!(user.contains("privacy.donottrackheader.enabled\", true"));
    assert!(user.contains("legacyUserProfileCustomizations.stylesheets\", true"));
    let restored = h.rpc(
        "operation.restore",
        json!({"id":report["operation_id"],"confirmed":true}),
    );
    assert_eq!(restored["ok"], true, "{restored}");
    assert_eq!(fs::read_to_string(css).unwrap(), user_css);
    assert!(!h
        .path(".mozilla/firefox/probe.default/chrome/kolira.css")
        .exists());
}

#[test]
fn native_extension_metadata_patch_never_truncates_theme_files() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("themes")).unwrap();
    fs::write(temp.path().join("package.json"), "{\"version\":\"1.2.3\"}").unwrap();
    fs::write(temp.path().join("themes/night.json"), "theme payload").unwrap();
    kolira_core::stores::create_native_extension(temp.path(), temp.path(), "publisher", "night")
        .unwrap();
    assert_eq!(
        fs::read_to_string(temp.path().join("themes/night.json")).unwrap(),
        "theme payload"
    );
    let pkg: Value =
        serde_json::from_slice(&fs::read(temp.path().join("package.json")).unwrap()).unwrap();
    assert_eq!(pkg["version"], "1.2.3");
    assert_eq!(pkg["_watch"], true);
}

#[test]
fn vsix_keeps_nested_package_names_and_rejects_traversal() {
    use zip::{write::SimpleFileOptions, ZipWriter};
    for bad in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("extension.vsix");
        let mut zip = ZipWriter::new(fs::File::create(&archive).unwrap());
        zip.start_file(
            if bad {
                "../escape"
            } else {
                "extension/package.json"
            },
            SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(b"{}").unwrap();
        if !bad {
            zip.start_file(
                "extension/themes/package/night.json",
                SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(b"theme").unwrap();
        }
        zip.finish().unwrap();
        let out = temp.path().join("out");
        let result = kolira_core::stores::extract_vsix(&archive, &out);
        if bad {
            assert!(result.is_err());
            assert!(!temp.path().join("escape").exists());
        } else {
            assert_eq!(
                fs::read_to_string(result.unwrap().join("themes/package/night.json")).unwrap(),
                "theme"
            );
        }
    }
}

#[test]
fn saved_app_resources_win_over_generated_palettes() {
    let h = Home::new();
    let palette = h.palette();
    h.manifest(json!({"palette":palette,"app_targets":["firefox","discord","konsole","ghostty","cava","telegram"]}));
    h.write(
        ".config/ghostty/config",
        "font-size = 13\nbackground = #112233\n",
    );
    h.write(
        ".config/kolira/themes/next/.config/ghostty/kolira",
        "background = #abcdef\n",
    );
    h.write(
        ".config/kolira/themes/next/.config/ghostty/config",
        "config-file = ?kolira\n",
    );
    h.applied(false);
    let text = fs::read_to_string(h.path(".config/ghostty/kolira")).unwrap();
    assert!(text.contains("#abcdef"), "{text}");
    assert!(fs::read_to_string(h.path(".config/ghostty/config"))
        .unwrap()
        .contains("13"));
}

#[test]
fn palette_save_validates_live_colors_and_rejects_stale_gui_state() {
    let h = Home::new();
    let palette = h.palette();
    let rejected = h.rpc(
        "palette.save_theme",
        json!({"name":"saved", "palette":palette}),
    );
    assert_eq!(rejected["ok"], false);
    assert!(!h.path(".config/kolira/themes/saved").exists());
    let p: kolira_core::palette::Palette = serde_json::from_value(palette.clone()).unwrap();
    h.write(
        ".config/kdeglobals",
        &kolira_core::palette::kde_color_scheme(&p),
    );
    let saved = h.rpc(
        "palette.save_theme",
        json!({"name":"saved", "palette":palette}),
    );
    assert_eq!(saved["ok"], true, "{saved}");
    let manifest: Value = serde_json::from_slice(
        &fs::read(h.path(".config/kolira/themes/saved/theme.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["palette"], palette);
    let different = kolira_core::palette::kde_color_scheme(&p)
        .replace("BackgroundNormal=46,52,64", "BackgroundNormal=1,2,3");
    assert_ne!(different, kolira_core::palette::kde_color_scheme(&p));
    h.write(".config/kdeglobals", &different);
    let rejected = h.rpc(
        "palette.save_theme",
        json!({"name":"stale", "palette":palette}),
    );
    assert_eq!(rejected["ok"], false);
    assert!(!h.path(".config/kolira/themes/stale").exists());
}

#[test]
fn followup_write_failure_rolls_back_desktop_and_extension_files() {
    let h = Home::new();
    h.cache();
    h.manifest(json!({"editor_themes":[h.editor("assets/editor-themes/vscodium/example.night")]}));
    h.write(
        ".config/kolira/themes/next/.config/kdeglobals",
        "[General]\nColorScheme=Changed\n",
    );
    h.write(".vscode-oss/extensions/personal", "keep this");
    let extensions = h.path(".vscode-oss/extensions");
    fs::set_permissions(&extensions, fs::Permissions::from_mode(0o555)).unwrap();
    let old = fs::read(h.path(".config/kdeglobals")).unwrap();
    let out = h.run(&["apply", "next", "--without-dependencies", "--json"]);
    fs::set_permissions(&extensions, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("rolled_back"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(fs::read(h.path(".config/kdeglobals")).unwrap(), old);
    assert_eq!(
        fs::read_to_string(extensions.join("personal")).unwrap(),
        "keep this"
    );
    assert!(!extensions.join("example.night-kolira").exists());
}

#[test]
fn palette_without_app_targets_does_not_opt_in_installed_apps() {
    let h = Home::new();
    let palette = h.palette();
    h.manifest(json!({"palette":palette}));
    h.write(
        ".config/vesktop/settings/settings.json",
        "{\"personal\":42}",
    );
    h.applied(false);
    assert!(!h.path(".config/vesktop/themes/kolira.theme.css").exists());
    assert!(!h
        .path(".local/share/konsole/Theme Profile.profile")
        .exists());
}
