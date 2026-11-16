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

#[test]
fn launchers_return_exactly_and_keep_personal_edits() {
    let h = Home::new();
    for name in ["rofi", "fuzzel", "walker"] {
        h.write(
            &format!("bin/{name}"),
            "#!/bin/sh\nexit 0\n",
        );
        fs::set_permissions(
            h.path(&format!("bin/{name}")),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    // Personal configs exist before apply.
    h.write(
        ".config/rofi/config.rasi",
        "* { font: \"mono 10\"; }\n#personal { border: 2px red; }\n",
    );
    assert!(
        !h.text(".config/rofi/config.rasi").is_empty(),
        "rofi config was not written"
    );
    h.write(
        ".config/fuzzel/fuzzel.ini",
        "[launch]\n# personal launcher settings\nshell = fish\n",
    );
    h.write(
        ".config/walker/style.css",
        "/* personal */\nwindow { border: 1px solid blue; }\n",
    );
    let out = h
        .command(&["palette", "import", "gallery:nord"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let p: Value = serde_json::from_str(&h.text(".config/kolira/palette.json")).unwrap();
    h.write(
        ".config/kolira/themes/next/theme.json",
        &json!({"schema":"kolira/theme@2","name":"next","palette":p,"app_targets":json!(["rofi", "fuzzel", "walker"])})
            .to_string(),
    );
    for _ in 0..3 {
        h.apply("next");
        // Late personal edits after apply.
        h.write(
            ".config/rofi/config.rasi",
            &h.text(".config/rofi/config.rasi").replace("mono 10", "mono 12"),
        );
        h.write(
            ".config/fuzzel/fuzzel.ini",
            &h.text(".config/fuzzel/fuzzel.ini").replace("shell = fish", "shell = bash"),
        );
        h.write(
            ".config/walker/style.css",
            &h.text(".config/walker/style.css").replace("solid blue", "solid green"),
        );
        h.apply("neutral");
        // Personal edits survive the return.
        let rofi = h.text(".config/rofi/config.rasi");
        assert!(rofi.contains("font: \"mono 12\""), "rofi personal edit survives");
        assert!(rofi.contains("#personal { border: 2px red; }"));
        assert!(h.text(".config/fuzzel/fuzzel.ini").contains("shell = bash"));
        assert!(h.text(".config/walker/style.css").contains("solid green"));
    }
}

#[test]
fn launchers_absent_when_not_installed() {
    let h = Home::new();
    h.palette(json!(["rofi", "fuzzel", "walker"]));
    let p = h
        .command(&["apply", "next", "--without-dependencies"])
        .output()
        .unwrap();
    assert!(
        p.status.success(),
        "{}",
        String::from_utf8_lossy(&p.stderr)
    );
    h.apply("neutral");
    for p in [
        ".config/rofi/config.rasi",
        ".config/rofi/themes/kolira.rasi",
        ".config/fuzzel/fuzzel.ini",
        ".config/walker/style.css",
    ] {
        let path = h.path(p);
        if path.exists() {
            let content = h.text(p);
            assert!(
                !content.contains("@theme") && !content.contains("BEGIN kolira colours"),
                "{p} must not carry managed launcher colours"
            );
        }
    }
}

#[test]
fn rofi_theme_line_added_once_and_removed_on_return() {
    let h = Home::new();
    h.write(
        "bin/rofi",
        "#!/bin/sh\nexit 0\n",
    );
    fs::set_permissions(
        h.path("bin/rofi"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    h.write(".config/rofi/config.rasi", "* { font: \"mono 10\"; }\n");
    h.palette(json!(["rofi"]));
    h.apply("next");
    let theme_line = format!(
        "@theme \"{}\"",
        h.path(".config/rofi/themes/kolira.rasi").display()
    );
    assert_eq!(
        h.text(".config/rofi/config.rasi")
            .lines()
            .filter(|l| l.trim().starts_with("@theme"))
            .count(),
        1
    );
    assert!(h.text(".config/rofi/config.rasi").contains(&theme_line));
    h.apply("neutral");
    assert!(!h.text(".config/rofi/config.rasi").contains("@theme"));
    assert_eq!(h.text(".config/rofi/config.rasi"), "* { font: \"mono 10\"; }\n");
}

#[test]
fn invalid_launcher_resource_aborts_before_writes() {
    let h = Home::new();
    h.write(
        "bin/rofi",
        "#!/bin/sh\nexit 0\n",
    );
    fs::set_permissions(
        h.path("bin/rofi"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    h.write(".config/rofi/config.rasi", "* { font: \"mono 10\"; }\n");
    h.palette(json!(["rofi"]));
    h.write(
        ".config/kolira/themes/next/.config/rofi/themes/kolira.rasi",
        "bad color = #notacolour\n",
    );
    let p = h.rpc("profile.preview", json!({"name":"next"}));
    assert_eq!(p["ok"], false, "{p}");
    assert_eq!(
        h.text(".config/rofi/config.rasi"),
        "* { font: \"mono 10\"; }\n"
    );
}
