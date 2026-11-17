//! Existing-adapter regressions found during the final coverage audit.
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Stdio},
};
struct Home(tempfile::TempDir);
const CONFIG: &str = ".config/spicetify/config-xpui.ini";
const ORIGINAL:&str="[Setting]\ncurrent_theme=Personal\ncolor_scheme=Original\ninject_theme_js=0\nspotify_path=/personal/spotify\n[AdditionalOptions]\ncustom_apps=personal\n";
impl Home {
    fn new() -> Self {
        let h = Self(tempfile::tempdir().unwrap());
        h.write(".config/kdeglobals", "[General]\nColorScheme=Breeze\n");
        for name in ["next", "neutral"] {
            h.write(
                &format!(".config/kolira/themes/{name}/theme.json"),
                &json!({"schema":"kolira/theme@2","name":name}).to_string(),
            );
        }
        h
    }
    fn path(&self, rel: &str) -> PathBuf {
        self.0.path().join(rel)
    }
    fn write(&self, rel: &str, text: &str) {
        let p = self.path(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_kolira"));
        c.args(args)
            .env("HOME", self.0.path())
            .env("PATH", self.path("bin"))
            .env("XDG_CONFIG_HOME", self.path(".config"))
            .env("XDG_DATA_HOME", self.path(".local/share"))
            .env("XDG_CACHE_HOME", self.path(".cache"))
            .env("KTM_OFFLINE", "1")
            .env("KTM_THEME_NO_APPLY", "1");
        c
    }
    fn stub(&self, fail: bool) {
        self.write(
            "bin/spicetify",
            &format!(
                "#!/bin/sh\nprintf 'apply\\n' >> \"$HOME/calls\"\nexit {}\n",
                if fail { 7 } else { 0 }
            ),
        );
        fs::set_permissions(
            self.path("bin/spicetify"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    fn source(&self) {
        self.write(&format!(".config/kolira/themes/next/{CONFIG}"),"[Setting]\ncurrent_theme=Next\ncolor_scheme=New\ninject_theme_js=1\nspotify_path=/foreign/spotify\n");
    }
    fn apply(&self, name: &str, native: bool) -> std::process::Output {
        self.command(&["apply", name, "--without-dependencies"])
            .env("KTM_THEME_NO_APPLY", if native { "0" } else { "1" })
            .output()
            .unwrap()
    }
}
#[test]
fn spicetify_snapshot_never_imports_js_or_enables_execution() {
    let h = Home::new();
    h.write(CONFIG, ORIGINAL);
    h.source();
    h.stub(false);
    let script = ".config/spicetify/Themes/Next/theme.js";
    h.write(
        &format!(".config/kolira/themes/next/{script}"),
        "throw new Error('foreign theme code');\n",
    );
    let out = h.apply("next", false);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = fs::read_to_string(h.path(CONFIG)).unwrap();
    assert!(text.contains("current_theme=Next"));
    assert!(text.contains("inject_theme_js=0"));
    assert!(text.contains("spotify_path=/personal/spotify"));
    assert!(!h.path(script).exists());
    assert!(!h.path("calls").exists());
    assert!(kolira_core::appearance::for_theme(script, "foreign code").is_none());
    assert!(h.apply("neutral", false).status.success());
    assert_eq!(fs::read_to_string(h.path(CONFIG)).unwrap(), ORIGINAL);
}
#[test]
fn spicetify_native_failure_is_visible_and_profile_files_rollback() {
    let h = Home::new();
    h.write(CONFIG, ORIGINAL);
    h.source();
    h.stub(true);
    let out = h.apply("next", true);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("Spicetify apply failed") && err.contains("rolled_back"),
        "{err}"
    );
    assert_eq!(fs::read_to_string(h.path(CONFIG)).unwrap(), ORIGINAL);
}
#[test]
fn spicetify_return_to_missing_snapshot_refreshes_the_original_selection() {
    let h = Home::new();
    h.write(CONFIG, ORIGINAL);
    h.source();
    h.stub(false);
    for name in ["next", "neutral"] {
        let out = h.apply(name, true);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    assert_eq!(fs::read_to_string(h.path(CONFIG)).unwrap(), ORIGINAL);
    assert_eq!(
        fs::read_to_string(h.path("calls")).unwrap().lines().count(),
        2
    );
}
#[test]
fn spicetify_palette_failure_restores_bytes_permissions_and_absence() {
    let h = Home::new();
    h.write(CONFIG, ORIGINAL);
    h.stub(true);
    fs::set_permissions(h.path(CONFIG), fs::Permissions::from_mode(0o640)).unwrap();
    assert!(h
        .command(&["palette", "import", "gallery:nord"])
        .output()
        .unwrap()
        .status
        .success());
    let out = h
        .command(&["palette", "apply", "--targets=spicetify"])
        .env("KTM_THEME_NO_APPLY", "0")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("Spicetify apply failed"));
    assert_eq!(fs::read_to_string(h.path(CONFIG)).unwrap(), ORIGINAL);
    assert_eq!(
        fs::metadata(h.path(CONFIG)).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert!(!h.path(".config/spicetify/Themes/Kolira").exists());
}
#[test]
fn unconfigured_native_spicetify_is_refused_in_preview_without_creating_files() {
    let h = Home::new();
    h.source();
    h.stub(false);
    for without_sync in [false, true] {
        let mut child = h
            .command(&["core", "profile.preview"])
            .env("KTM_THEME_NO_APPLY", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(
                json!({"name":"next","without_sync":without_sync})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
        let result = child.wait_with_output().unwrap();
        let value: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(value["ok"], false, "{value}");
        assert!(!h.path(CONFIG).exists());
        assert!(!h.path("calls").exists());
    }
}
#[test]
fn btop_native_reformatting_restores_selector_without_losing_late_settings() {
    let h = Home::new();
    h.write(
        ".config/btop/btop.conf",
        "color_theme = \"Personal\"\ntruecolor = false\n",
    );
    h.write(
        ".config/kolira/themes/next/.config/btop/btop.conf",
        "color_theme = \"kolira\"\n",
    );
    assert!(h.apply("next", false).status.success());
    h.write(
        ".config/btop/btop.conf",
        "# native rewrite\ncolor_theme=\"kolira\"\ntruecolor=true\nupdate_ms=2000\n",
    );
    let out = h.apply("neutral", false);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = fs::read_to_string(h.path(".config/btop/btop.conf")).unwrap();
    assert!(text.contains("Personal"), "{text}");
    assert!(text.contains("truecolor=true"));
    assert!(text.contains("update_ms=2000"));
    assert!(text.contains("# native rewrite"));
}
#[test]
fn spicetify_theme_selector_cannot_escape_the_themes_directory() {
    for name in ["../../outside", "/absolute", "..", "folder\\theme"] {
        let h = Home::new();
        h.write(CONFIG, ORIGINAL);
        h.stub(false);
        h.write(
            &format!(".config/kolira/themes/next/{CONFIG}"),
            &format!("[Setting]\ncurrent_theme={name}\n"),
        );
        let mut child = h
            .command(&["core", "profile.preview"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"{\"name\":\"next\",\"without_sync\":true}")
            .unwrap();
        let out = child.wait_with_output().unwrap();
        let value: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(value["ok"], false, "{name}: {value}");
        assert_eq!(fs::read_to_string(h.path(CONFIG)).unwrap(), ORIGINAL);
        assert!(!h.path("calls").exists());
    }
}
