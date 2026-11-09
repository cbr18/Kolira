//! End-to-end CLI checks with isolated HOME/XDG and a controlled editor provider.
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

struct Home {
    dir: tempfile::TempDir,
}
impl Home {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt;
        let home = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        fs::create_dir_all(home.p("bin")).unwrap();
        let code = home.p("bin/code");
        fs::write(&code, r##"#!/usr/bin/python3
import json, os, pathlib, shutil, sys, time
root=pathlib.Path(os.environ['HOME'])/'.vscode/extensions'
dest=root/(sys.argv[2]+'-1.0.0')
mode=os.environ.get('PROVIDER_TEST_MODE','ok')
if mode=='hang' and sys.argv[1]=='--install-extension':
    (pathlib.Path(os.environ['HOME'])/'started').write_text(str(os.getpid()))
    time.sleep(30)
if mode=='empty' and sys.argv[1]=='--install-extension': sys.exit(0)
if sys.argv[1]=='--uninstall-extension':
    shutil.rmtree(dest)
    sys.exit(0)
dest.mkdir(parents=True,exist_ok=True)
(dest/'package.json').write_text(json.dumps({'publisher':'test','name':'theme','contributes':{'themes':[{'label':'Managed Dark'},{'label':'Managed Light'}]}}))
"##).unwrap();
        fs::set_permissions(code, fs::Permissions::from_mode(0o755)).unwrap();
        home.theme("one", "Managed Dark");
        home
    }
    fn p(&self, rel: &str) -> PathBuf {
        self.dir.path().join(rel)
    }
    fn theme(&self, name: &str, label: &str) {
        let dir = self.p(&format!("config/kde-theme-profile/themes/{name}"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("theme.json"), serde_json::to_vec(&json!({"schema":"theme-profile/theme@2","name":name,"dependencies":[{"kind":"vscode","name":label,"source":"open-vsx","id":"test.theme","exact":true}]})).unwrap()).unwrap();
    }
    fn cmd(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_theme-profile"));
        command
            .args(args)
            .env("HOME", self.dir.path())
            .env("XDG_CONFIG_HOME", self.p("config"))
            .env("XDG_DATA_HOME", self.p("data"))
            .env("XDG_CACHE_HOME", self.p("cache"))
            .env("PATH", format!("{}:/usr/bin:/bin", self.p("bin").display()))
            .env_remove("KTM_THEME_NO_APPLY")
            .env_remove("KTM_OFFLINE")
            .env_remove("KTM_DEP_OPERATION")
            .env_remove("KTM_DEP_EVENTS")
            .env_remove("KTM_DEP_RESULT");
        command
    }
    fn run(&self, args: &[&str]) -> Output {
        self.cmd(args).output().unwrap()
    }
    fn ok(&self, args: &[&str]) -> Output {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "{:?}: {}\n{}",
            args,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        out
    }
    fn rpc(&self, method: &str, args: Value, mode: &str) -> Value {
        let mut child = self
            .cmd(&["core", method])
            .env("PROVIDER_TEST_MODE", mode)
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
    fn rows(&self) -> Vec<Value> {
        serde_json::from_slice(&self.ok(&["dependencies", "list"]).stdout).unwrap()
    }
    fn id(&self) -> String {
        self.rows()
            .into_iter()
            .find(|r| r["dependency"]["id"] == "test.theme")
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .into()
    }
}

#[test]
fn shared_resource_full_install_remove_reinstall_cycle() {
    let h = Home::new();
    h.theme("two", "Managed Light");
    let rows = h.rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["themes"].as_array().unwrap().len(), 2);
    let id = h.id();
    h.ok(&["dependencies", "install", &id]);
    let row = &h.rows()[0];
    assert_eq!(row["installed"], true);
    assert_eq!(row["can_remove"], true);
    assert_eq!(row["instances"][0]["ownership"], "managed");
    assert!(!h.run(&["dependencies", "remove", &id]).status.success());
    h.ok(&["dependencies", "reinstall", &id, "--confirm-shared"]);
    assert_eq!(h.rows()[0]["installed"], true);
    h.ok(&["dependencies", "remove", &id, "--confirm-shared"]);
    assert_eq!(h.rows()[0]["installed"], false);
    h.ok(&["dependencies", "install", &id]);
    assert_eq!(h.rows().len(), 1);
    assert_eq!(h.rows()[0]["instances"].as_array().unwrap().len(), 1);
    h.ok(&["delete", "two"]);
    assert_eq!(h.rows()[0]["themes"], json!(["one"]));
}

#[test]
fn external_and_modified_resources_are_protected() {
    let h = Home::new();
    let id = h.id();
    h.ok(&["dependencies", "install", &id]);
    let package = h.p(".vscode/extensions/test.theme-1.0.0/package.json");
    fs::write(&package, "user changed the file").unwrap();
    assert_eq!(h.rows()[0]["can_remove"], false);
    assert!(!h
        .run(&["dependencies", "remove", &id, "--confirm-shared"])
        .status
        .success());
    assert_eq!(
        fs::read_to_string(package).unwrap(),
        "user changed the file"
    );
    let external = Home::new();
    external
        .cmd(&["apps", "store-install", "vscode", "test.theme"])
        .env("KTM_THEME_NO_APPLY", "1")
        .output()
        .unwrap();
    let dir = external.p(".vscode/extensions/test.theme-1.0.0");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("package.json"),
        r#"{"contributes":{"themes":[{"label":"Managed Dark"}]}}"#,
    )
    .unwrap();
    let id = external.id();
    external.ok(&["dependencies", "install", &id]);
    assert_eq!(external.rows()[0]["can_remove"], false);
    assert_eq!(external.rows()[0]["instances"][0]["ownership"], "external");
}

#[test]
fn successful_command_without_files_is_a_failed_install_and_retry_works() {
    let h = Home::new();
    let id = h.id();
    let out = h
        .cmd(&["dependencies", "install", &id])
        .env("PROVIDER_TEST_MODE", "empty")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(h.rows()[0]["installed"], false);
    assert!(h.rows()[0]["last_error"]
        .as_str()
        .unwrap()
        .contains("not found"));
    h.ok(&["dependencies", "install", &id]);
    assert_eq!(h.rows()[0]["installed"], true);
}

#[test]
fn cancellation_stops_the_current_provider_and_does_not_apply_a_theme() {
    let h = Home::new();
    let id = h.id();
    let mut child = h
        .cmd(&["dependencies", "install", &id])
        .env("PROVIDER_TEST_MODE", "hang")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let started: Value = serde_json::from_str(&line).unwrap();
    let operation = started["operation"].as_str().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !h.p("started").exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    let start = Instant::now();
    h.ok(&["dependencies", "cancel", operation]);
    let out = child.wait_with_output().unwrap();
    assert!(!out.status.success());
    assert!(start.elapsed() < Duration::from_secs(3));
    assert_eq!(h.rows()[0]["installed"], false);
    let state: Value =
        serde_json::from_slice(&h.ok(&["dependencies", "operation", operation]).stdout).unwrap();
    assert_eq!(state["status"], "cancelled");
    assert!(!h.p("config/kde-theme-profile/active-profile").exists());
}

#[test]
fn migration_keeps_sources_without_granting_ownership_and_corruption_is_visible() {
    let h = Home::new();
    let path = h.p("config/kde-theme-profile/installed-assets.json");
    let legacy = json!({"/some/external/theme":{"store":"kde-store","id":"123","title":"Old"}});
    fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    h.rows();
    let new: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(new["schema"], 2);
    assert_eq!(new["sources"], legacy);
    assert!(path.with_extension("json.pre-v2").exists());
    assert_eq!(h.rows()[0]["can_remove"], false);
    fs::write(&path, "broken registry").unwrap();
    assert!(!h.run(&["dependencies", "list"]).status.success());
    assert_eq!(fs::read_to_string(&path).unwrap(), "broken registry");
}

#[test]
fn registry_refreshes_are_serialized_and_policies_are_persistent() {
    let h = Home::new();
    let mut children = Vec::new();
    for _ in 0..6 {
        children.push(
            h.cmd(&["dependencies", "refresh"])
                .stdout(Stdio::null())
                .spawn()
                .unwrap(),
        );
    }
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }
    let id = h.id();
    assert!(!h
        .run(&[
            "dependencies",
            "remember",
            "--theme=one",
            "--policy=always",
            "--group=store"
        ])
        .status
        .success());
    h.ok(&[
        "dependencies",
        "remember",
        "--theme=one",
        "--policy=always",
        "--group=store",
        &format!("--selected={id}"),
    ]);
    let settings: Value = serde_json::from_slice(&h.ok(&["automation", "--json"]).stdout).unwrap();
    assert!(settings["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["key"] == "store" && i["policy"] == "always"));
    let pending: Value =
        serde_json::from_slice(&h.ok(&["deps", "one", "--pending", "--json"]).stdout).unwrap();
    assert!(pending.as_array().unwrap().is_empty());
    h.ok(&[
        "dependencies",
        "remember",
        "--theme=one",
        "--policy=never",
        "--group=store",
    ]);
    assert_eq!(h.rows().len(), 1);
}

#[test]
fn symlink_at_an_owned_location_is_not_removed() {
    let h = Home::new();
    let id = h.id();
    h.ok(&["dependencies", "install", &id]);
    let dir = h.p(".vscode/extensions/test.theme-1.0.0");
    let outside = h.p("outside");
    fs::rename(&dir, &outside).unwrap();
    std::os::unix::fs::symlink(&outside, &dir).unwrap();
    assert!(!h
        .run(&["dependencies", "remove", &id, "--confirm-shared"])
        .status
        .success());
    assert!(Path::new(&outside).join("package.json").exists());
}

#[test]
fn failed_reinstall_restores_the_working_resource() {
    let h = Home::new();
    let id = h.id();
    h.ok(&["dependencies", "install", &id]);
    let original = fs::read(h.p(".vscode/extensions/test.theme-1.0.0/package.json")).unwrap();
    let out = h
        .cmd(&["dependencies", "reinstall", &id, "--confirm-shared"])
        .env("PROVIDER_TEST_MODE", "empty")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(
        fs::read(h.p(".vscode/extensions/test.theme-1.0.0/package.json")).unwrap(),
        original
    );
    assert_eq!(h.rows()[0]["installed"], true);
    assert_eq!(h.rows()[0]["can_remove"], true);
}

#[test]
fn cancelling_reinstall_restores_previous_files() {
    let h = Home::new();
    let id = h.id();
    h.ok(&["dependencies", "install", &id]);
    let original = fs::read(h.p(".vscode/extensions/test.theme-1.0.0/package.json")).unwrap();
    let mut child = h
        .cmd(&["dependencies", "reinstall", &id, "--confirm-shared"])
        .env("PROVIDER_TEST_MODE", "hang")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let start: Value = serde_json::from_str(&line).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !h.p("started").exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    h.ok(&[
        "dependencies",
        "cancel",
        start["operation"].as_str().unwrap(),
    ]);
    assert!(!child.wait().unwrap().success());
    assert_eq!(
        fs::read(h.p(".vscode/extensions/test.theme-1.0.0/package.json")).unwrap(),
        original
    );
    assert_eq!(h.rows()[0]["installed"], true);
    assert_eq!(h.rows()[0]["can_remove"], true);
}

#[test]
fn asynchronous_api_returns_durable_id_and_keeps_verified_result_after_transport_exits() {
    let h = Home::new();
    let started = h.rpc(
        "dependency.start",
        json!({"action":"install","ids":[h.id()]}),
        "ok",
    );
    assert_eq!(started["ok"], true, "{started}");
    let id = started["data"]["id"].as_str().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let result: Value =
            serde_json::from_slice(&h.ok(&["dependencies", "operation", id]).stdout).unwrap();
        if result["status"] != "running" {
            assert_eq!(result["status"], "success", "{result}");
            assert_eq!(result["results"][0]["ok"], true);
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(h.rows()[0]["installed"], true);
    assert!(h
        .p(&format!(
            "config/kde-theme-profile/dependency-operations/{id}/events.jsonl"
        ))
        .is_file());
}
#[test]
fn asynchronous_job_can_be_cancelled_after_start_request_returns() {
    let h = Home::new();
    let started = h.rpc(
        "dependency.start",
        json!({"action":"install","ids":[h.id()]}),
        "hang",
    );
    assert_eq!(started["ok"], true, "{started}");
    let id = started["data"]["id"].as_str().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !h.p("started").is_file() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(25));
    }
    h.ok(&["dependencies", "cancel", id]);
    loop {
        let result: Value =
            serde_json::from_slice(&h.ok(&["dependencies", "operation", id]).stdout).unwrap();
        if result["status"] != "running" {
            assert_eq!(result["status"], "cancelled", "{result}");
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(h.rows()[0]["installed"], false);
}
