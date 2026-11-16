//! Disposable-container tests of native tmux runtime transactions. Never run on host.
use anyhow::{ensure, Result};
use kolira_core::{four_apps, operation, palette::Palette, paths, tmux_runtime as runtime, undo};
use std::{
    fs,
    path::Path,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
fn backup() -> Result<std::path::PathBuf> {
    static N: AtomicUsize = AtomicUsize::new(0);
    let p = paths::backups_dir().join(format!("tmux-native-{}", N.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir_all(&p)?;
    Ok(p)
}
fn apply(p: &Palette, fail: bool) -> Result<()> {
    operation::run("tmux-native-apply", || {
        let data = paths::live_path(four_apps::TMUX_DATA);
        let backup = backup()?;
        operation::prepare(
            &backup,
            vec![data.clone(), paths::theme_root().join("apply-journal")],
        )?;
        let j = undo::revert()?;
        let baseline = undo::baseline(&[four_apps::TMUX_DATA.into()])?;
        kolira_core::gui_api::write_bytes(
            &data,
            &serde_json::to_vec(&runtime::styles(p))?,
            Some(0o600),
        )?;
        runtime::apply_live()?;
        if fail {
            anyhow::bail!("Injected post-tmux failure")
        }
        undo::record(j, &baseline)?;
        Ok(())
    })
}
fn neutral() -> Result<()> {
    operation::run("tmux-native-return", || {
        let backup = backup()?;
        operation::prepare(
            &backup,
            vec![
                paths::live_path(four_apps::TMUX_DATA),
                paths::theme_root().join("apply-journal"),
            ],
        )?;
        undo::revert()?;
        Ok(())
    })
}
fn main() -> Result<()> {
    ensure!(
        Path::new("/.dockerenv").is_file()
            && std::env::var("KOLIRA_FOUR_NATIVE_CONTAINER").as_deref() == Ok("1"),
        "Disposable Docker only"
    );
    let root = tempfile::tempdir()?;
    std::env::set_var("HOME", root.path());
    for (name, dir) in [
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_CACHE_HOME", "cache"),
        ("TMUX_TMPDIR", "sockets"),
    ] {
        let path = root.path().join(dir);
        fs::create_dir_all(&path)?;
        std::env::set_var(name, path);
    }
    // Only desktop recovery is stubbed: this image intentionally has no Plasma.
    // All tmux queries, writes and rollback below use the real native server.
    use std::os::unix::fs::PermissionsExt;
    let bin = root.path().join("desktop-stubs");
    fs::create_dir(&bin)?;
    fs::write(bin.join("systemctl"), "#!/bin/sh\nexit 0\n")?;
    fs::set_permissions(bin.join("systemctl"), fs::Permissions::from_mode(0o755))?;
    let path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{}:{path}", bin.display()));
    std::env::remove_var("KTM_THEME_NO_APPLY");
    std::env::remove_var("TMUX");
    ensure!(
        Command::new("tmux")
            .args([
                "-L",
                "kolira-four",
                "-f",
                "/dev/null",
                "new-session",
                "-d",
                "-s",
                "test",
                "sleep 600"
            ])
            .status()?
            .success(),
        "Cannot start isolated tmux"
    );
    ensure!(
        Command::new("tmux")
            .args([
                "-L",
                "kolira-four",
                "set-option",
                "-g",
                "status-style",
                "fg=yellow,bg=green,bold"
            ])
            .status()?
            .success(),
        "Cannot set original attributes"
    );
    let before = runtime::capture()?;
    ensure!(before.len() == 1, "Missing tmux server");
    let result = (|| {
        for theme in fs::read_dir("/src/themes")? {
            let theme = theme?.path();
            if !theme.join("theme.json").is_file() {
                continue;
            }
            let v: serde_json::Value =
                serde_json::from_slice(&fs::read(theme.join("theme.json"))?)?;
            let p: Palette = serde_json::from_value(v["palette"].clone())?;
            apply(&p, false)?;
            for s in runtime::capture()?.values() {
                for (key, value) in runtime::styles(&p) {
                    for field in value.split(',') {
                        ensure!(
                            s.values[&key].split(',').any(|v| v == field),
                            "Native tmux colour mismatch: {key}"
                        );
                    }
                }
                ensure!(
                    s.values["status-style"].contains("bold"),
                    "Personal bold attribute lost"
                );
            }
            neutral()?;
            ensure!(runtime::capture()? == before, "tmux baseline not restored");
            ensure!(
                !paths::live_path(four_apps::TMUX_DATA).exists(),
                "Missing-file baseline not restored"
            );
            let error = apply(&p, true).expect_err("Injected failure did not fail");
            ensure!(error.to_string().contains("rolled_back"), "{error:#}");
            ensure!(runtime::capture()? == before, "Runtime rollback failed");
            println!("{}: apply/return + failure rollback PASS", p.name);
        }
        let v: serde_json::Value =
            serde_json::from_slice(&fs::read("/src/themes/everforest/theme.json")?)?;
        let p: Palette = serde_json::from_value(v["palette"].clone())?;
        apply(&p, false)?;
        ensure!(
            Command::new("tmux")
                .args([
                    "-L",
                    "kolira-four",
                    "set-option",
                    "-g",
                    "status-style",
                    "fg=red,bg=blue"
                ])
                .status()?
                .success(),
            "Native personal edit failed"
        );
        let late = runtime::capture()?.values().next().unwrap().values["status-style"].clone();
        neutral()?;
        let restored = runtime::capture()?;
        let values = &restored.values().next().unwrap().values;
        ensure!(
            values["status-style"] == late,
            "Late personal style was overwritten"
        );
        for (key, value) in &before.values().next().unwrap().values {
            if key != "status-style" {
                ensure!(values[key] == *value, "Unchanged style did not return");
            }
        }
        println!("tmux late personal edit preserved PASS");
        let attrs_baseline = runtime::capture()?;
        apply(&p, false)?;
        ensure!(
            Command::new("tmux")
                .args([
                    "-L",
                    "kolira-four",
                    "set-option",
                    "-ag",
                    "status-style",
                    "underscore"
                ])
                .status()?
                .success(),
            "Native late attribute edit failed"
        );
        let colours_before = attrs_baseline.values().next().unwrap().values["status-style"].clone();
        neutral()?;
        let attr_state = runtime::capture()?;
        let attr = &attr_state.values().next().unwrap().values["status-style"];
        for field in colours_before
            .split(',')
            .filter(|s| s.starts_with("fg=") || s.starts_with("bg="))
        {
            ensure!(
                attr.split(',').any(|s| s == field),
                "Colours did not return after attribute edit"
            );
        }
        ensure!(attr.contains("underscore"), "Late underline was lost");
        println!("tmux restores colours while keeping late style attributes PASS");
        apply(&p, false)?;
        ensure!(
            Command::new("tmux")
                .args(["-L", "kolira-four", "kill-server"])
                .status()?
                .success(),
            "Cannot stop test server"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
        ensure!(
            Command::new("tmux")
                .args([
                    "-L",
                    "kolira-four",
                    "-f",
                    "/dev/null",
                    "new-session",
                    "-d",
                    "-s",
                    "new-test",
                    "sleep 600"
                ])
                .status()?
                .success(),
            "Cannot restart test server"
        );
        let restarted = runtime::capture()?;
        neutral()?;
        ensure!(
            runtime::capture()? == restarted,
            "Old journal touched a replacement server"
        );
        println!("tmux replacement server is preserved PASS");

        Ok(())
    })();
    let _ = Command::new("tmux")
        .args(["-L", "kolira-four", "kill-server"])
        .status();
    result
}
