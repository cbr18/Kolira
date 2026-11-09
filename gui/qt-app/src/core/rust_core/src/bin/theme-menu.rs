//! `theme-menu` — wofi launcher (replaces the bash script removed in 0.5.0).
//!
//! Same menu text and actions: apply / save-as / open the profiles folder /
//! restart Plasma Shell / status. Uses the library core directly instead of
//! shelling out to `kolira`.

use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};

use kolira_core::paths::{config_home, profiles_dir};
use kolira_core::profile::{apply_profile_cli, list_profiles, save_profile_cli};
use kolira_core::status::status;

fn style_path() -> PathBuf {
    config_home().join("wofi/style.css")
}

/// wofi dmenu; returns the selected line (None when the user cancelled).
fn wofi(prompt: &str, width: &str, height: &str) -> Option<String> {
    let style = style_path().to_string_lossy().to_string();
    let output = Command::new("wofi")
        .args([
            "--dmenu",
            "--insensitive",
            "--prompt",
            prompt,
            "--width",
            width,
            "--height",
            height,
            "--style",
            &style,
        ])
        .stdin(Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn active_profile() -> String {
    status()
        .lines()
        .find_map(|line| line.strip_prefix("active_profile=").map(|v| v.to_string()))
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn profile_menu(prompt: &str) -> Option<String> {
    let active = active_profile();
    let mut options = String::new();
    let profiles = list_profiles().unwrap_or_default();
    for profile in profiles {
        if profile.name == active {
            options.push_str(&format!("{}  [active]\n", profile.name));
        } else {
            options.push_str(&format!("{}\n", profile.name));
        }
    }
    if options.is_empty() {
        return None;
    }
    wofi_with_input(prompt, "360", "260", &options).map(|choice| {
        choice
            .split_whitespace()
            .next()
            .unwrap_or(&choice)
            .to_string()
    })
}

fn wofi_with_input(prompt: &str, width: &str, height: &str, input: &str) -> Option<String> {
    let style = style_path().to_string_lossy().to_string();
    let mut child = Command::new("wofi")
        .args([
            "--dmenu",
            "--insensitive",
            "--prompt",
            prompt,
            "--width",
            width,
            "--height",
            height,
            "--style",
            &style,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(input.as_bytes());
    }
    let output = child.wait_with_output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn main() -> ExitCode {
    let active = active_profile();
    let choices =
        "Apply profile\nSave as\nOpen profile folder\nRestart Plasma Shell\nStatus\nCancel\n";

    let action = match wofi_with_input(&format!("Theme: {active}"), "380", "260", choices) {
        Some(a) => a,
        None => return ExitCode::SUCCESS,
    };

    match action.as_str() {
        "Apply profile" => {
            if let Some(profile) = profile_menu("Apply profile") {
                match apply_profile_cli(&profile, None) {
                    Ok(backup) => {
                        println!("Applied profile: {profile}");
                        println!("Pre-apply backup: {}", backup.display());
                    }
                    Err(e) => eprintln!("{e}"),
                }
            }
        }
        "Save as" => {
            let default = active_profile();
            if let Some(name) = wofi(
                &format!("Save as profile name (default {default})"),
                "420",
                "120",
            ) {
                let name = name.split_whitespace().next().unwrap_or(&name).to_string();
                match save_profile_cli(&name, None) {
                    Ok(()) => println!("Saved profile: {name}"),
                    Err(e) => eprintln!("{e}"),
                }
            }
        }
        "Open profile folder" => {
            let dir = profiles_dir();
            if let Err(e) = std::fs::create_dir_all(&dir) {
                eprintln!("{e}");
                return ExitCode::from(1);
            }
            let dir_str = dir.to_string_lossy().to_string();
            if Command::new("dolphin")
                .arg(&dir_str)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .is_err()
            {
                let _ = Command::new("xdg-open")
                    .arg(&dir_str)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn();
            }
        }
        "Restart Plasma Shell" => {
            let _ = Command::new("systemctl")
                .args(["--user", "restart", "plasma-plasmashell.service"])
                .status();
        }
        "Status" => {
            print!("{}", status());
        }
        _ => {}
    }
    ExitCode::SUCCESS
}
