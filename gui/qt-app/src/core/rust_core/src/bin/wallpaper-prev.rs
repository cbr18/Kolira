//! `wallpaper-prev` — Rust CLI (thin wrapper over the shared core).
//!
//! Same contract as `wallpaper-next`, stepping backwards with wrap-around.
//! All logic lives in `wallpaper.rs` — the binary just wires the environment.

use std::path::PathBuf;
use std::process::ExitCode;

use kolira_core::paths::{cache_home, config_home};
use kolira_core::wallpaper::{list_wallpapers_in, step_index, wallpaper_dir};

fn env_or(key: &str, default: PathBuf) -> PathBuf {
    std::env::var_os(key).map(PathBuf::from).unwrap_or(default)
}

fn main() -> ExitCode {
    let dir = wallpaper_dir(); // $WALLPAPER_DIR → GUI setting → XDG Pictures
    let state = env_or("WALLPAPER_STATE", cache_home().join("wallpaper_index"));
    let plasma = env_or(
        "PLASMA_APPLETSRC",
        config_home().join("plasma-org.kde.plasma.desktop-appletsrc"),
    );

    let files = match list_wallpapers_in(&dir) {
        Ok(files) => files,
        Err(e) => {
            eprintln!("wallpaper-prev: {e}");
            return ExitCode::from(1);
        }
    };
    if files.is_empty() {
        return ExitCode::from(1);
    }

    let index = match step_index(&files, &state, Some(&plasma), -1) {
        Ok(index) => index,
        Err(e) => {
            eprintln!("wallpaper-prev: {e}");
            return ExitCode::from(1);
        }
    };

    if std::env::var_os("WALLPAPER_DRY_RUN").is_some()
        || std::env::var("KTM_THEME_NO_APPLY").as_deref() == Ok("1")
    {
        println!("{}", files[index]);
        ExitCode::SUCCESS
    } else {
        match kolira_core::wallpaper::apply_image(&files[index]) {
            Ok(()) => ExitCode::SUCCESS,
            _ => ExitCode::from(1),
        }
    }
}
