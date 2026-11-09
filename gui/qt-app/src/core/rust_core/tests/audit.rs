//! Isolated CLI regressions: package boundaries, safe resources and timeouts.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{Duration, Instant},
};
use tempfile::TempDir;

struct Home(TempDir);
impl Home {
    fn new() -> Self {
        Self(tempfile::tempdir().unwrap())
    }
    fn path(&self, path: &str) -> PathBuf {
        self.0.path().join(path)
    }
    fn write(&self, path: &str, content: &str) {
        let path = self.path(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    fn cmd(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_kolira"));
        c.args(args)
            .env("HOME", self.0.path())
            .env("XDG_CONFIG_HOME", self.path(".config"))
            .env("XDG_DATA_HOME", self.path(".local/share"))
            .env("XDG_CACHE_HOME", self.path(".cache"))
            .env("KTM_THEME_NO_APPLY", "1")
            .env("KTM_OFFLINE", "1");
        c
    }
    fn run(&self, args: &[&str]) -> Output {
        self.cmd(args).output().unwrap()
    }
    fn package(&self, kind: &str, path: &str) -> Value {
        let package = json!({"schema":"kolira/theme@2","name":"probe","assets":[{"kind":kind,"path":path,"name":"Probe","origin":"user","redistributable":true}]});
        self.write(
            ".config/kolira/themes/probe/theme.json",
            &package.to_string(),
        );
        package
    }
    fn imported(&self, package: &Value) -> Output {
        let theme = package.to_string();
        let payload = "theme payload";
        self.write("export/theme.json", &theme);
        let rel = package["assets"][0]["path"].as_str().unwrap();
        self.write(&format!("export/{rel}"), payload);
        let manifest = json!({"format":"kolira/export@1","theme":"probe","exported":"audit","generator":"audit","home_token":"$HOME","files":{
            "theme.json":format!("{:x}",Sha256::digest(theme.as_bytes())),rel:format!("{:x}",Sha256::digest(payload.as_bytes()))}});
        self.write("export/export.json", &manifest.to_string());
        self.run(&["import", self.path("export").to_str().unwrap()])
    }
}

#[test]
fn arbitrary_asset_import_and_direct_apply_are_rejected_without_writes() {
    for (kind, path) in [
        ("custom", ".ssh/config"),
        ("color-scheme", ".ssh/config"),
        ("kvantum", ".config"),
        ("kvantum", ".config/Kvantum"),
        ("kvantum", ".config/Kvantum/../other"),
        ("icons", ".local/share/icons"),
        ("icons", ".local/share/icons/../other"),
        ("wallpaper", "../outside"),
    ] {
        let h = Home::new();
        h.write(".ssh/config", "original");
        let package = h.package(kind, path);
        // Avoid constructing an on-disk traversal fixture; direct loading still exercises it.
        if !path.contains("..") && path != ".config" {
            let out = h.imported(&package);
            assert!(!out.status.success(), "import accepted {kind}:{path}");
            assert!(String::from_utf8_lossy(&out.stderr).contains("invalid"));
        }
        let out = h.run(&["apply", "probe", "--without-dependencies", "--json"]);
        assert!(!out.status.success(), "apply accepted {kind}:{path}");
        assert_eq!(
            fs::read_to_string(h.path(".ssh/config")).unwrap(),
            "original"
        );
        assert!(!h.path(".config/kolira/backups").exists());
    }
}

#[test]
fn supported_resource_still_imports_applies_and_backs_up() {
    let h = Home::new();
    let rel = ".local/share/color-schemes/Probe.colors";
    let package = h.package("color-scheme", rel);
    fs::remove_dir_all(h.path(".config/kolira/themes/probe")).unwrap();
    let out = h.imported(&package);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    h.write(rel, "old resource");
    h.write(".config/unrelated.conf", "keep me");
    let out = h.run(&["apply", "probe", "--without-dependencies", "--json"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(fs::read_to_string(h.path(rel)).unwrap(), "theme payload");
    assert_eq!(
        fs::read_to_string(h.path(".config/unrelated.conf")).unwrap(),
        "keep me"
    );
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        fs::read_to_string(Path::new(report["backup"].as_str().unwrap()).join(rel)).unwrap(),
        "old resource"
    );
}

#[test]
fn asset_links_are_rejected_before_backup_or_live_writes() {
    for link_source in [false, true] {
        let h = Home::new();
        let rel = ".local/share/color-schemes/Probe.colors";
        h.package("color-scheme", rel);
        h.write(".ssh/config", "original");
        h.write(&format!(".config/kolira/themes/probe/{rel}"), "payload");
        let link = if link_source {
            h.path(&format!(".config/kolira/themes/probe/{rel}"))
        } else {
            h.path(rel)
        };
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        let _ = fs::remove_file(&link);
        symlink(h.path(".ssh/config"), link).unwrap();
        let out = h.run(&["apply", "probe", "--without-dependencies", "--json"]);
        assert!(!out.status.success());
        assert_eq!(
            fs::read_to_string(h.path(".ssh/config")).unwrap(),
            "original"
        );
        assert!(!h.path(".config/kolira/backups").exists());
    }
}

#[test]
fn all_bundled_themes_still_validate_and_import() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../..");
    let h = Home::new();
    for entry in fs::read_dir(repo.join("themes")).unwrap() {
        let path = entry.unwrap().path();
        if !path.join("theme.json").is_file() {
            continue;
        }
        let out = h.run(&["import", path.to_str().unwrap()]);
        assert!(
            out.status.success(),
            "{}: {}",
            path.display(),
            String::from_utf8_lossy(&out.stderr)
        );
        h.write(".config/unrelated.conf", "keep me");
        let name = path.file_name().unwrap().to_str().unwrap();
        let out = h.run(&["apply", name, "--without-dependencies", "--json"]);
        assert!(
            out.status.success(),
            "apply {name}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            fs::read_to_string(h.path(".config/unrelated.conf")).unwrap(),
            "keep me"
        );
    }
}

#[test]
fn bat_timeout_reports_error_and_rolls_back_apply() {
    let h = Home::new();
    h.write(
        ".config/kolira/themes/probe/theme.json",
        r#"{"schema":"kolira/theme@2","name":"probe"}"#,
    );
    h.write(
        ".config/kolira/themes/probe/.config/bat/themes/Probe.tmTheme",
        "probe",
    );
    h.write("bin/bat", "#!/bin/sh\nexec /bin/sleep 15\n");
    fs::set_permissions(h.path("bin/bat"), fs::Permissions::from_mode(0o755)).unwrap();
    let start = Instant::now();
    let out = h
        .cmd(&["apply", "probe", "--without-dependencies", "--json"])
        .env_remove("KTM_THEME_NO_APPLY")
        .env("KTM_COMMAND_TIMEOUT", "1")
        .env("PATH", h.path("bin"))
        .output()
        .unwrap();
    assert!(start.elapsed() < Duration::from_secs(8));
    assert!(!out.status.success());
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(
        error.contains("bat") && error.contains("rolled_back"),
        "{error}"
    );
    assert!(!h.path(".config/kolira/active-profile").exists());
    let records = h.run(&["operations", "list"]);
    let value: Value = serde_json::from_slice(&records.stdout).unwrap();
    assert_eq!(value["operations"][0]["state"], "rolled_back");
    assert!(Path::new(value["operations"][0]["backup"].as_str().unwrap()).is_dir());
}

#[test]
fn resource_restore_honors_xdg_and_relocated_container() {
    for relocated in [false, true] {
        let h = Home::new();
        let rel = ".local/share/color-schemes/Probe.colors";
        h.package("color-scheme", rel);
        h.write(&format!(".config/kolira/themes/probe/{rel}"), "payload");
        h.write("data/color-schemes/Probe.colors", "old");
        let destination = if relocated {
            h.write("relocated/Probe.colors", "old");
            fs::remove_dir_all(h.path("data/color-schemes")).unwrap();
            symlink(h.path("relocated"), h.path("data/color-schemes")).unwrap();
            h.path("relocated/Probe.colors")
        } else {
            h.path("data/color-schemes/Probe.colors")
        };
        let out = h
            .cmd(&["apply", "probe", "--without-dependencies", "--json"])
            .env("XDG_DATA_HOME", h.path("data"))
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(fs::read_to_string(destination).unwrap(), "payload");
    }
}

#[test]
fn branding_keeps_legacy_theme_and_export_formats_readable() {
    let h = Home::new();
    let mut package = h.package("color-scheme", ".local/share/color-schemes/Probe.colors");
    package["schema"] = json!("theme-profile/theme@2");
    h.write(
        ".config/kolira/themes/probe/theme.json",
        &package.to_string(),
    );
    assert!(kolira_core::theme::load(&h.path(".config/kolira/themes/probe")).is_ok());
    // Construct a real legacy export, including its original checksum contract.
    fs::remove_dir_all(h.path(".config/kolira/themes/probe")).unwrap();
    let result = h.imported(&package);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let manifest_path = h.path("export/export.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["format"] = json!("theme-profile/export@1");
    fs::write(&manifest_path, manifest.to_string()).unwrap();
    let result = h.run(&[
        "import",
        h.path("export").to_str().unwrap(),
        "--name=legacy-import",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn branded_cli_migrates_both_historical_data_roots_without_overwriting() {
    for old in ["kde-theme-profile", "theme-profile"] {
        let h = Home::new();
        h.write(
            &format!(".config/{old}/profiles/mine/.config/{old}/palette.json"),
            "{}",
        );
        h.write(&format!(".config/{old}/active-profile"), "mine");
        h.write(&format!(".config/{old}/backups/kept/marker"), "backup");
        h.write(".config/kolira/app-preferences.json", "{}");
        let old_backup = h.path(&format!(".config/{old}/backups/kept"));
        h.write(
            &format!(".config/{old}/operations/apply-legacy.json"),
            &json!({
                "id":"apply-legacy", "profile":"mine", "state":"succeeded",
                "started":"2026-11-09T00:00:00Z", "updated":"2026-11-09T00:00:00Z",
                "backup":old_backup, "error":null, "recovery_errors":[]
            })
            .to_string(),
        );
        let result = h.run(&["list"]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).contains("mine"));
        let result = h.run(&["operations", "list"]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).contains("/kolira/backups/kept"));

        assert!(h
            .path(".config/kolira/themes/mine/.config/kolira/palette.json")
            .is_file());
        assert_eq!(
            fs::read_to_string(h.path(".config/kolira/backups/kept/marker")).unwrap(),
            "backup"
        );
        assert_eq!(
            fs::read_to_string(h.path(".config/kolira/app-preferences.json")).unwrap(),
            "{}"
        );
    }
}
