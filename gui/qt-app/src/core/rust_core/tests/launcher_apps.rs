//! Launcher adapters: Rofi, Fuzzel, Walker — safe return when a theme
//! omits the config, late personal edits survive.

use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Stdio},
};
struct Home(tempfile::TempDir);
impl Home {
    fn new() -> Self {
        let h = Self(tempfile::tempdir().unwrap());
        fs::create_dir_all(h.path("bin")).unwrap();
        h.write(".config/kdeglobals", "[General]\nColorScheme=Breeze\n");
        for name in ["next", "neutral"] {
            h.write(
                &format!(".config/kolira/themes/{name}/theme.json"),
                &json!({"schema":"kolira/theme@2","name":name}).to_string(),
            );
        }
        h
    }
    fn path(&self, p: &str) -> PathBuf {
        self.0.path().join(p)
    }
    fn write(&self, p: &str, t: &str) {
        let path = self.path(p);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, t).unwrap();
    }
    fn text(&self, p: &str) -> String {
        fs::read_to_string(self.path(p)).unwrap()
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_kolira"));
        c.args(args)
            .env("HOME", self.0.path())
            .env("PATH", self.path("bin"))
            .env("XDG_CONFIG_HOME", self.path(".config"))
            .env("XDG_DATA_HOME", self.path(".local/share"))
            .env("XDG_CACHE_HOME", self.path(".cache"))
            .env("KTM_THEME_NO_APPLY", "1")
            .env("KTM_OFFLINE", "1");
        for k in [
            "STARSHIP_CONFIG",
            "WEZTERM_CONFIG_FILE",
            "WEZTERM_CONFIG_DIR",
            "CODEX_HOME",
            "CLAUDE_CONFIG_DIR",
            "LG_CONFIG_FILE",
            "CONFIG_DIR",
            "WALLPAPER_DIR",
        ] {
            c.env_remove(k);
        }
        c
    }
    fn rpc(&self, method: &str, value: Value) -> Value {
        let mut p = self
            .command(&["core", method])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        p.stdin
            .take()
            .unwrap()
            .write_all(value.to_string().as_bytes())
            .unwrap();
        let out = p.wait_with_output().unwrap();
        serde_json::from_slice(&out.stdout)
            .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&out.stderr)))
    }
    fn palette(&self, targets: Value) {
        let out = self
            .command(&["palette", "import", "gallery:nord"])
            .output()
            .unwrap();
        assert!(out.status.success());
        let p: Value = serde_json::from_str(&self.text(".config/kolira/palette.json")).unwrap();
        self.write(
            ".config/kolira/themes/next/theme.json",
            &json!({"schema":"kolira/theme@2","name":"next","palette":p,"app_targets":targets})
                .to_string(),
        );
    }
    fn apply(&self, name: &str) {
        let out = self
            .command(&["apply", name, "--without-dependencies"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "apply {name}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

fn installed(h: &Home) {
    for name in ["rofi", "fuzzel", "walker"] {
        h.write(&format!("bin/{name}"), "#!/bin/sh\nexit 0\n");
        fs::set_permissions(
            h.path(&format!("bin/{name}")),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
}
const ROFI: &str = ".config/rofi/config.rasi";
const FUZZEL: &str = ".config/fuzzel/fuzzel.ini";
const WALKER: &str = ".config/walker/themes/personal/style.css";
fn configured(h: &Home) {
    installed(h);
    h.write(
        ROFI,
        "configuration { modi: \"drun\"; }\n@theme \"personal\"\n* { font: \"mono 13\"; }\n",
    );
    h.write(FUZZEL, "[main]\nfont=monospace:size=13\nlaunch-prefix=personal-command\n[colors]\nbackground=12345680\ntext=abcdefee\n");
    h.write(
        ".config/walker/config.toml",
        "theme = \"personal\"\n[keybinds]\nclose = [\"Escape\"]\n",
    );
    h.write(
        WALKER,
        ".box-wrapper { padding: 23px; border-radius: 9px; }\n",
    );
    h.write(
        ".config/walker/themes/personal/layout.xml",
        "<personal-layout/>\n",
    );
    h.palette(json!(["rofi", "fuzzel", "walker"]));
}
#[test]
fn three_cycles_restore_exact_bytes_permissions_and_absent_files() {
    let h = Home::new();
    configured(&h);
    let before: Vec<_> = [ROFI, FUZZEL, WALKER]
        .iter()
        .map(|r| (*r, h.text(r)))
        .collect();
    fs::set_permissions(h.path(ROFI), fs::Permissions::from_mode(0o640)).unwrap();
    for _ in 0..3 {
        h.apply("next");
        assert!(h.text(ROFI).contains("BEGIN kolira colours"));
        assert_eq!(h.text(ROFI).matches("@theme \"personal\"").count(), 1);
        assert!(!h.text(ROFI).contains("@theme \"kolira"));
        assert!(h.text(FUZZEL).contains("launch-prefix=personal-command"));
        assert!(h.text(WALKER).contains("padding: 23px"));
        h.apply("neutral");
        for (rel, text) in &before {
            assert_eq!(&h.text(rel), text, "{rel}");
        }
        assert_eq!(
            fs::metadata(h.path(ROFI)).unwrap().permissions().mode() & 0o777,
            0o640
        );
        for rel in [
            kolira_core::launcher_apps::ROFI_THEME,
            kolira_core::launcher_apps::WALKER,
        ] {
            assert!(!h.path(rel).exists(), "{rel}");
        }
    }
    assert_eq!(
        h.text(".config/walker/themes/personal/layout.xml"),
        "<personal-layout/>\n"
    );
}
#[test]
fn late_personal_changes_do_not_leave_stale_colours_or_dangling_references() {
    let h = Home::new();
    configured(&h);
    h.apply("next");
    h.write(
        ROFI,
        &format!(
            "{}// late personal comment\n",
            h.text(ROFI).replace("mono 13", "mono 17")
        ),
    );
    h.write(
        FUZZEL,
        &format!(
            "{}# late personal comment\n",
            h.text(FUZZEL).replace("size=13", "size=17")
        ),
    );
    h.write(
        WALKER,
        &format!(
            "{}/* late personal comment */\n",
            h.text(WALKER).replace("23px", "29px")
        ),
    );
    h.apply("neutral");
    assert!(h.text(ROFI).contains("mono 17"));
    assert!(h.text(ROFI).contains("@theme \"personal\""));
    assert!(!h.text(ROFI).contains("kolira"));
    assert!(h.text(FUZZEL).contains("background=12345680"));
    assert!(h.text(FUZZEL).contains("text=abcdefee"));
    assert!(h.text(FUZZEL).contains("size=17"));
    assert!(h.text(WALKER).contains("29px"));
    assert!(!h.text(WALKER).contains("kolira"));
    for r in [ROFI, FUZZEL, WALKER] {
        assert!(h.text(r).contains("late personal comment"));
    }
}
#[test]
fn late_colour_edits_inside_managed_css_survive_without_other_managed_colours() {
    let h = Home::new();
    configured(&h);
    h.apply("next");
    for (rel, selector) in [
        (ROFI, "window { background-color:"),
        (WALKER, ".box-wrapper { background-color:"),
    ] {
        let text = h
            .text(rel)
            .lines()
            .map(|line| {
                if line.starts_with(selector) {
                    format!("{selector} #123abc; }}\n")
                } else {
                    format!("{line}\n")
                }
            })
            .collect::<String>();
        h.write(rel, &text);
    }
    let source = h.text(FUZZEL);
    h.write(
        FUZZEL,
        &source
            .lines()
            .map(|l| {
                if l.starts_with("text=") {
                    "text=112233ff\n".to_string()
                } else {
                    format!("{l}\n")
                }
            })
            .collect::<String>(),
    );
    h.apply("neutral");
    for rel in [ROFI, WALKER] {
        let text = h.text(rel);
        assert!(text.contains("#123abc"));
        assert!(!text.contains("BEGIN kolira"));
        assert!(!text.contains("selection"));
        assert!(!text.contains("#88c0d0"));
    }
    assert!(h.text(FUZZEL).contains("text=112233ff"));
    assert!(h.text(FUZZEL).contains("background=12345680"));
}
#[test]
fn repeated_apply_does_not_duplicate_overrides_or_reset_layout() {
    let h = Home::new();
    configured(&h);
    h.apply("next");
    let before = [h.text(ROFI), h.text(FUZZEL), h.text(WALKER)];
    h.apply("next");
    for (r, text) in [ROFI, FUZZEL, WALKER].iter().zip(before) {
        assert_eq!(h.text(r), text, "{r}");
    }
    assert_eq!(h.text(ROFI).matches("@theme").count(), 1);
}
#[test]
fn foreign_hosts_commands_and_layout_never_enter_live_files_or_exports() {
    let h = Home::new();
    configured(&h);
    h.write(
        ".config/kolira/themes/next/.config/rofi/config.rasi",
        "configuration { modi: \"run\"; kb-accept-entry: \"Control+X\"; }\n",
    );
    h.write(
        ".config/kolira/themes/next/.config/walker/config.toml",
        "theme=\"foreign\"\n",
    );
    h.write(
        ".config/kolira/themes/next/.config/walker/themes/personal/layout.xml",
        "<foreign-secret/>\n",
    );
    h.write(
        ".config/kolira/themes/next/.config/walker/style.css",
        "@import url('/tmp/foreign');\n",
    );
    h.write(
        ".config/kolira/themes/next/.config/fuzzel/fuzzel.ini",
        "[main]\nfont=foreign-font\nlaunch-prefix=foreign-command\n[colors]\nbackground=112233ff\n",
    );
    h.apply("next");
    assert!(!h.text(ROFI).contains("Control+X"));
    assert!(h.text(ROFI).contains("drun"));
    assert!(!h.text(FUZZEL).contains("foreign"));
    assert_eq!(
        h.text(".config/walker/themes/personal/layout.xml"),
        "<personal-layout/>\n"
    );
    assert!(!h.path(".config/walker/style.css").exists());
    assert!(h.text(".config/walker/config.toml").contains("personal"));
    let out = h
        .command(&[
            "export",
            "next",
            &format!("--out={}", h.path("out.zip").display()),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let bytes = fs::read(h.path("out.zip")).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).unwrap();
        let name = entry.name().to_string();
        assert!(!name.ends_with("config.rasi"));
        assert!(!name.ends_with("layout.xml"));
        assert!(!name.ends_with("walker/config.toml"));
        if name.ends_with("fuzzel.ini") {
            let mut text = String::new();
            std::io::Read::read_to_string(&mut entry, &mut text).unwrap();
            assert!(!text.contains("foreign"));
        }
    }
}
#[test]
fn malicious_or_malformed_resource_fails_preview_before_writes() {
    for (resource, text) in [
        (
            kolira_core::launcher_apps::ROFI_THEME,
            "@import \"/tmp/foreign.rasi\"\nwindow { background-color: #123456; }",
        ),
        (
            kolira_core::launcher_apps::ROFI_THEME,
            "window { background-color: #123456; padding: 7px; }",
        ),
        (
            kolira_core::launcher_apps::ROFI_THEME,
            "window { background-color: #123456789; }",
        ),
        (
            kolira_core::launcher_apps::WALKER,
            "@import url('/tmp/foreign'); .box-wrapper { color: #123456; }",
        ),
        (
            kolira_core::launcher_apps::WALKER,
            ".box-wrapper { color: url('/tmp/foreign'); }",
        ),
        (FUZZEL, "[colors]\ntext=123456gg\n"),
    ] {
        let h = Home::new();
        configured(&h);
        let before = h.text(ROFI);
        h.write(&format!(".config/kolira/themes/next/{resource}"), text);
        let preview = h.rpc("profile.preview", json!({"name":"next"}));
        assert_eq!(preview["ok"], false, "{resource}: {preview}");
        assert_eq!(h.text(ROFI), before);
    }
}
#[test]
fn snapshot_only_resources_work_without_a_palette() {
    let h = Home::new();
    configured(&h);
    let data = kolira_core::launcher_apps::bundle_resources(
        &serde_json::from_str(&h.text(".config/kolira/palette.json")).unwrap(),
    );
    for (rel, text) in data {
        h.write(&format!(".config/kolira/themes/next/{rel}"), &text);
    }
    h.write(
        ".config/kolira/themes/next/theme.json",
        &json!({"schema":"kolira/theme@2","name":"next"}).to_string(),
    );
    h.apply("next");
    assert!(h.text(ROFI).contains("BEGIN kolira colours"));
    assert!(h.text(WALKER).contains("BEGIN kolira colours"));
    h.apply("neutral");
    assert!(!h.text(ROFI).contains("kolira"));
}
#[test]
fn malformed_markers_are_never_truncated_and_absent_hosts_return_to_absent() {
    let h = Home::new();
    installed(&h);
    h.palette(json!(["rofi", "fuzzel"]));
    h.apply("next");
    assert!(h.path(ROFI).exists());
    assert!(h.path(FUZZEL).exists());
    h.apply("neutral");
    assert!(!h.path(ROFI).exists());
    assert!(!h.path(FUZZEL).exists());
    let original = "/* BEGIN kolira colours */\n// personal text after incomplete marker\n";
    h.write(ROFI, original);
    let preview = h.rpc("profile.preview", json!({"name":"next"}));
    assert_eq!(preview["ok"], false);
    assert_eq!(h.text(ROFI), original);
}
#[test]
fn walker_embedded_scss_external_and_legacy_configs_are_left_untouched() {
    for (config, extra) in [
        ("", None),
        (
            "theme=\"personal\"\n",
            Some(".config/walker/themes/personal/style.scss"),
        ),
        (
            "theme=\"personal\"\nadditional_theme_location=\"/tmp/elsewhere\"\n",
            Some(WALKER),
        ),
    ] {
        let h = Home::new();
        installed(&h);
        h.palette(json!(["walker"]));
        h.write(".config/walker/config.toml", config);
        if let Some(r) = extra {
            h.write(r, "/* personal */\n");
        }
        h.write(".config/walker/style.css", "/* legacy personal skin */\n");
        h.apply("next");
        assert_eq!(h.text(".config/walker/config.toml"), config);
        assert_eq!(
            h.text(".config/walker/style.css"),
            "/* legacy personal skin */\n"
        );
        if let Some(r) = extra {
            assert_eq!(h.text(r), "/* personal */\n");
        }
        assert!(!h.path(kolira_core::launcher_apps::WALKER).exists());
    }
}
#[test]
fn fuzzel_keeps_opacity_and_restores_rgb_after_a_late_opacity_edit() {
    let h = Home::new();
    configured(&h);
    h.apply("next");
    let text = h.text(FUZZEL);
    let value = kolira_core::field::ini_get(&text, "colors", "background").unwrap();
    assert_eq!(&value[6..], "80");
    h.write(
        FUZZEL,
        &kolira_core::field::set_ini_text(
            &text,
            "colors",
            "background",
            &format!("{}40", &value[..6]),
        ),
    );
    h.apply("neutral");
    assert_eq!(
        kolira_core::field::ini_get(&h.text(FUZZEL), "colors", "background").unwrap(),
        "12345640"
    );
}
#[test]
fn relocated_xdg_and_unterminated_personal_files_roundtrip() {
    let h = Home::new();
    configured(&h);
    h.write(ROFI, "@theme \"personal\"");
    let before = [
        (ROFI, h.text(ROFI)),
        (FUZZEL, h.text(FUZZEL)),
        (WALKER, h.text(WALKER)),
    ];
    fs::rename(h.path(".config"), h.path("xdg")).unwrap();
    for name in ["next", "neutral"] {
        let out = h
            .command(&["apply", name, "--without-dependencies"])
            .env("XDG_CONFIG_HOME", h.path("xdg"))
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    assert!(!h.path(".config").exists());
    for (rel, text) in before {
        assert_eq!(h.text(&rel.replacen(".config/", "xdg/", 1)), text);
    }
}
#[test]
fn personal_resource_collisions_and_linked_hosts_abort_before_writes() {
    for rel in [kolira_core::launcher_apps::ROFI_THEME, ROFI, FUZZEL, WALKER] {
        let h = Home::new();
        configured(&h);
        let original = h.text(ROFI);
        if rel == kolira_core::launcher_apps::ROFI_THEME {
            h.write(rel, "* { font: \"my private font\"; }\n");
        } else {
            let target = h.path(rel).with_extension("personal");
            fs::rename(h.path(rel), &target).unwrap();
            std::os::unix::fs::symlink(&target, h.path(rel)).unwrap();
        }
        let preview = h.rpc("profile.preview", json!({"name":"next"}));
        assert_eq!(preview["ok"], false, "{rel}: {preview}");
        assert_eq!(h.text(ROFI), original);
    }
}
#[test]
fn no_apply_does_not_execute_launcher_probes_and_unknown_walker_is_skipped() {
    let h = Home::new();
    configured(&h);
    for name in ["rofi", "fuzzel", "walker"] {
        h.write(
            &format!("bin/{name}"),
            "#!/bin/sh\n: > \"$HOME/probe-executed\"\necho 0.1\n",
        );
    }
    h.apply("next");
    assert!(!h.path("probe-executed").exists());
    h.apply("neutral");
    // On native preview, refuse old Rofi/Fuzzel before touching any file.
    let mut cmd = h.command(&["core", "profile.preview"]);
    cmd.env("KTM_THEME_NO_APPLY", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let mut proc = cmd.spawn().unwrap();
    proc.stdin
        .take()
        .unwrap()
        .write_all(b"{\"name\":\"next\"}")
        .unwrap();
    let out = proc.wait_with_output().unwrap();
    let response: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(response["ok"], false);
    assert!(!h.text(ROFI).contains("kolira"));
}
#[test]
fn formatted_managed_blocks_and_private_properties_return_by_colour_value() {
    let h = Home::new();
    configured(&h);
    h.apply("next");
    for rel in [ROFI, WALKER] {
        let text = h.text(rel);
        let begin = text.find("/* BEGIN kolira colours */").unwrap();
        let (prefix, block) = text.split_at(begin);
        let block = block.replace(" { ", " {\n  ").replace("; }", ";\n}");
        let block = block.replacen(
            "background-color:",
            "/* personal inline */ background-color:",
            1,
        );
        let block = block.replace(
            "/* END kolira colours */",
            "/* personal comment */\nwindow { padding: 37px; }\n/* END kolira colours */",
        );
        h.write(rel, &format!("{prefix}{block}"));
    }
    h.apply("neutral");
    for rel in [ROFI, WALKER] {
        let text = h.text(rel);
        assert!(text.contains("padding: 37px"));
        assert!(text.contains("personal inline"));
        assert!(text.contains("personal comment"));
        assert!(!text.contains("background-color"), "{rel}: {text}");
        assert!(!text.contains("BEGIN kolira"));
    }
}
#[test]
fn failed_launcher_write_rolls_back_all_earlier_files() {
    let h = Home::new();
    configured(&h);
    let before: Vec<_> = [ROFI, FUZZEL, WALKER]
        .iter()
        .map(|r| (*r, h.text(r)))
        .collect();
    let parent = h.path(WALKER).parent().unwrap().to_path_buf();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o555)).unwrap();
    let out = h
        .command(&["apply", "next", "--without-dependencies"])
        .output()
        .unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(!out.status.success(), "write failure must abort apply");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("rolled_back"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    for (rel, text) in before {
        assert_eq!(h.text(rel), text, "{rel}");
    }
    for rel in [
        kolira_core::launcher_apps::ROFI_THEME,
        kolira_core::launcher_apps::WALKER,
    ] {
        assert!(!h.path(rel).exists(), "{rel}");
    }
}
