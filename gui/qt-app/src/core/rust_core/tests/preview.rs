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
        h.write(
            ".config/kdeglobals",
            "[General]\nColorScheme=Old\nTerminalApplication=my-terminal\n",
        );
        h.theme("next", true);
        h
    }
    fn path(&self, p: &str) -> PathBuf {
        self.0.path().join(p)
    }
    fn write(&self, p: &str, text: &str) {
        let p = self.path(p);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
    fn theme(&self, name: &str, package: bool) {
        self.write(
            &format!(".config/kde-theme-profile/themes/{name}/.config/kdeglobals"),
            "[General]\nColorScheme=New\nTerminalApplication=must-not-arrive\n",
        );
        if package {
            self.write(
                &format!(".config/kde-theme-profile/themes/{name}/theme.json"),
                &json!({"schema":"theme-profile/theme@2","name":name}).to_string(),
            );
        }
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_theme-profile"));
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
    fn rpc_raw(&self, method: &str, args: Value) -> Value {
        let mut child = self
            .command(&["core", method])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(args.to_string().as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        serde_json::from_slice(&out.stdout)
            .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&out.stderr)))
    }
    fn rpc(&self, method: &str, args: Value) -> Value {
        let out = self.rpc_raw(method, args);
        assert_eq!(out["ok"], true, "{method}: {out}");
        out["data"].clone()
    }
    fn preview(&self, name: &str, keep: bool) -> Value {
        self.rpc(
            "profile.preview",
            json!({"name":name,"keep_wallpaper":keep}),
        )
    }
    fn apply_reviewed(&self, report: &Value) {
        self.rpc("profile.apply",json!({"name":report["name"],"keep_wallpaper":report["keep_wallpaper"],"components":report["components"],"preview_token":report["token"],"skip_dependencies":true}));
    }
    fn assert_files(&self, report: &Value) {
        use sha2::{Digest, Sha256};
        for file in report["files"].as_array().unwrap() {
            let label = file["path"].as_str().unwrap();
            let p = if let Some(rel) = label.strip_prefix("~/") {
                self.path(rel)
            } else {
                PathBuf::from(label)
            };
            if file["operation"] == "delete" {
                assert!(!p.exists(), "{label}");
            } else if file["symlink"] != true {
                assert_eq!(
                    format!("{:x}", Sha256::digest(fs::read(&p).unwrap())),
                    file["after_sha256"].as_str().unwrap(),
                    "{label}"
                );
            }
        }
    }
}

#[test]
fn preview_is_readonly_and_confirmed_result_matches_every_file() {
    let h = Home::new();
    let before = fs::read(h.path(".config/kdeglobals")).unwrap();
    let report = h.preview("next", false);
    assert_eq!(fs::read(h.path(".config/kdeglobals")).unwrap(), before);
    assert!(!h.path(".config/kde-theme-profile/backups").exists());
    assert!(!h.path(".config/kde-theme-profile/operations").exists());
    assert_eq!(report["file_count"], 1);
    assert!(!report["files"][0]["diff"]
        .as_str()
        .unwrap()
        .contains("+TerminalApplication=must-not-arrive"));
    h.apply_reviewed(&report);
    h.assert_files(&report);
    assert!(fs::read_to_string(h.path(".config/kdeglobals"))
        .unwrap()
        .contains("my-terminal"));
}

#[test]
fn journal_resets_and_user_edits_match_actual_apply() {
    let h = Home::new();
    h.theme("first", true);
    h.write(
        ".config/kde-theme-profile/themes/first/.config/wofi/style.css",
        "window { color: red; }\n",
    );
    let first = h.preview("first", false);
    h.apply_reviewed(&first);
    let journal = h.path(".config/kde-theme-profile/apply-journal/journal.json");
    let original = fs::read(&journal).unwrap();
    let report = h.preview("next", false);
    assert_eq!(fs::read(&journal).unwrap(), original);
    assert!(report["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["operation"] == "delete" && f["path"].as_str().unwrap().contains("wofi")));
    h.apply_reviewed(&report);
    h.assert_files(&report);
    // A personal edit after the previous theme is not a previous-theme artifact.
    let first = h.preview("first", false);
    h.apply_reviewed(&first);
    h.write(".config/wofi/style.css", "my edited CSS\n");
    let report = h.preview("next", false);
    assert!(!report["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["path"].as_str().unwrap().contains("wofi/style.css")));
    h.apply_reviewed(&report);
    h.assert_files(&report);
    assert_eq!(
        fs::read_to_string(h.path(".config/wofi/style.css")).unwrap(),
        "my edited CSS\n"
    );
}

#[test]
fn stale_live_source_permissions_and_options_require_new_review() {
    for change in 0..4 {
        let h = Home::new();
        let report = h.preview("next", false);
        match change {
            0 => h.write(".config/kdeglobals", "[General]\nColorScheme=Edited\n"),
            1 => h.write(
                ".config/kde-theme-profile/themes/next/.config/kdeglobals",
                "[General]\nColorScheme=ChangedTheme\n",
            ),
            2 => fs::set_permissions(
                h.path(".config/kdeglobals"),
                fs::Permissions::from_mode(0o640),
            )
            .unwrap(),
            _ => {}
        }
        let before = fs::read(h.path(".config/kdeglobals")).unwrap();
        let result=h.rpc_raw("profile.apply",json!({"name":"next","preview_token":report["token"],"keep_wallpaper":change==3,"skip_dependencies":true}));
        assert_eq!(result["ok"], false, "{result}");
        assert!(result["error"].as_str().unwrap().contains("PREVIEW_STALE"));
        assert_eq!(fs::read(h.path(".config/kdeglobals")).unwrap(), before);
        assert!(!h.path(".config/kde-theme-profile/active-profile").exists());
    }
}

#[test]
fn partial_legacy_selection_and_cli_confirmation_share_the_contract() {
    let h = Home::new();
    h.theme("legacy", false);
    h.write(".config/kwinrc", "[Compositing]\nAnimationSpeed=3\n");
    h.write(
        ".config/kde-theme-profile/themes/legacy/.config/kwinrc",
        "[Compositing]\nAnimationSpeed=6\n",
    );
    let out = h.run(&["preview", "legacy", "--components=kwin", "--json"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(!report["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["path"].as_str().unwrap().contains("kdeglobals")));
    let token = format!("--preview-token={}", report["token"].as_str().unwrap());
    let out = h.run(&[
        "apply",
        "legacy",
        "--components=kwin",
        &token,
        "--without-dependencies",
        "--json",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    h.assert_files(&report);
}

#[test]
fn missing_assets_are_reported_and_preserved_in_the_plan() {
    let h = Home::new();
    h.write(".config/kdeglobals", "[Icons]\nTheme=breeze\n");
    h.write(
        ".config/kde-theme-profile/themes/next/.config/kdeglobals",
        "[Icons]\nTheme=ThisIconThemeDoesNotExist\n",
    );
    h.write(".config/kde-theme-profile/themes/next/theme.json",&json!({"schema":"theme-profile/theme@2","name":"next","values":{"icon_theme":"ThisIconThemeDoesNotExist"},"dependencies":[{"kind":"icons","name":"ThisIconThemeDoesNotExist","source":"aur","id":"not-installed-pkg","exact":true}]}).to_string());
    let report = h.preview("next", false);
    assert!(!report["missing"].as_array().unwrap().is_empty());
    assert!(!report["dependencies"].as_array().unwrap().is_empty());
    assert!(report["kept_fields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f == "icon_theme"));
    h.apply_reviewed(&report);
    h.assert_files(&report);
    assert!(fs::read_to_string(h.path(".config/kdeglobals"))
        .unwrap()
        .contains("Theme=breeze"));
}

#[test]
fn jsonc_diff_keeps_comments_and_personal_keys() {
    let h = Home::new();
    h.write(
        ".config/zed/settings.json",
        "{\n // personal\n \"theme\":\"Old\",\n \"ui_font_size\":19,\n}\n",
    );
    h.write(
        ".config/kde-theme-profile/themes/next/.config/zed/settings.json",
        "{\"theme\":\"New\",\"ui_font_size\":10}",
    );
    let report = h.preview("next", false);
    h.apply_reviewed(&report);
    h.assert_files(&report);
    let text = fs::read_to_string(h.path(".config/zed/settings.json")).unwrap();
    assert!(
        text.contains("// personal") && text.contains("\"ui_font_size\":19"),
        "{text}"
    );
}

#[test]
fn save_current_first_never_overwrites_an_existing_theme() {
    let h = Home::new();
    h.rpc("profile.save_before_apply", json!({"name":"before-next"}));
    let path = h.path(".config/kde-theme-profile/themes/before-next/theme.json");
    let saved = fs::read(&path).unwrap();
    for name in ["before-next", "BEFORE-NEXT", "../escape"] {
        assert_eq!(
            h.rpc_raw("profile.save_before_apply", json!({"name":name}))["ok"],
            false
        );
    }
    assert_eq!(fs::read(path).unwrap(), saved);
    assert_eq!(
        fs::read_to_string(h.path(".config/kde-theme-profile/active-profile"))
            .unwrap()
            .trim(),
        "before-next"
    );
}

#[test]
fn malicious_resource_symlink_is_rejected_by_preview_without_any_write() {
    let h = Home::new();
    h.write("private", "secret");
    let rel = ".local/share/color-schemes/Probe.colors";
    h.write(".config/kde-theme-profile/themes/next/theme.json",&json!({"schema":"theme-profile/theme@2","name":"next","assets":[{"kind":"color-scheme","name":"Probe","path":rel}]}).to_string());
    let src = h.path(&format!(".config/kde-theme-profile/themes/next/{rel}"));
    fs::create_dir_all(src.parent().unwrap()).unwrap();
    symlink(h.path("private"), src).unwrap();
    assert_eq!(
        h.rpc_raw("profile.preview", json!({"name":"next"}))["ok"],
        false
    );
    assert!(!h.path(rel).exists());
    assert!(!h.path(".config/kde-theme-profile/backups").exists());
}

#[test]
fn all_six_bundled_previews_match_the_applied_files() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../..");
    let mut count = 0;
    for entry in fs::read_dir(repo.join("themes")).unwrap() {
        let source = entry.unwrap().path();
        if !source.join("theme.json").is_file() {
            continue;
        }
        let h = Home::new();
        let out = h.run(&["import", source.to_str().unwrap()]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let name = source.file_name().unwrap().to_str().unwrap();
        let report = h.preview(name, false);
        h.apply_reviewed(&report);
        h.assert_files(&report);
        count += 1;
    }
    assert_eq!(count, 6);
}

#[test]
fn keep_wallpaper_and_saved_copy_use_the_same_plan() {
    for keep in [false, true] {
        let h = Home::new();
        h.theme("legacy", false);
        let apple = ".config/plasma-org.kde.plasma.desktop-appletsrc";
        h.write(apple,"[Containments][1]\nplugin=org.kde.plasma.folder\nwallpaperplugin=org.kde.image\n\n[Containments][1][Wallpaper][org.kde.image][General]\nImage=/old.jpg\n");
        let image_url =
            format!("file://{}", h.path("restored wall.jpg").display()).replace(' ', "%20");
        h.write(&format!(".config/kde-theme-profile/themes/legacy/{apple}"),&format!("[Containments][1]\nplugin=org.kde.plasma.folder\nwallpaperplugin=org.kde.image\n\n[Containments][1][Wallpaper][org.kde.image][General]\nImage={image_url}\n"));
        h.write(
            ".config/kde-theme-profile/themes/legacy/wallpaper/source",
            h.path("restored wall.jpg").to_str().unwrap(),
        );
        h.write(
            ".config/kde-theme-profile/themes/legacy/wallpaper/copy.jpg",
            "image bytes",
        );
        let report = h.preview("legacy", keep);
        assert!(!h.path("restored wall.jpg").exists());
        h.apply_reviewed(&report);
        h.assert_files(&report);
        assert_eq!(h.path("restored wall.jpg").exists(), !keep);
        if !keep {
            assert!(fs::read_to_string(h.path(apple))
                .unwrap()
                .contains(&image_url));
        }
        if keep {
            assert!(fs::read_to_string(h.path(apple))
                .unwrap()
                .contains("/old.jpg"));
        }
    }
}
