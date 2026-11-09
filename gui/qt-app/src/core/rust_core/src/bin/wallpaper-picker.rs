//! `wallpaper-picker` — Rust CLI (thin wrapper over the shared core).
//!
//! wofi grid with generated thumbnails; the whole picker flow (thumbnails,
//! wofi, index caching) lives in `wallpaper.rs` and is shared with the library
//! (`kde_theme_core_wallpaper_pick`). The binary only wires the environment and
//! decides between a dry run and actually applying the wallpaper.

use std::path::PathBuf;
use std::process::ExitCode;

use kde_theme_core::paths::{cache_home, config_home};
use kde_theme_core::wallpaper::{pick_wallpaper_flow, wallpaper_dir, PickOutcome};

fn env_or(key: &str, default: PathBuf) -> PathBuf {
    std::env::var_os(key).map(PathBuf::from).unwrap_or(default)
}

fn main() -> ExitCode {
    let dir = wallpaper_dir(); // $WALLPAPER_DIR → GUI setting → XDG Pictures
    let state = env_or("WALLPAPER_STATE", cache_home().join("wallpaper_index"));
    let thumb_dir = env_or("WALLPAPER_THUMB_DIR", cache_home().join("wallpaper-picker"));
    let style = config_home().join("wofi/wallpaper.css");

    let outcome = match pick_wallpaper_flow(&dir, &state, &thumb_dir, &style) {
        Ok(outcome) => outcome,
        Err(e) => {
            eprintln!("wallpaper-picker: {e}");
            return ExitCode::from(1);
        }
    };

    let selected = match outcome {
        // Cancelled (empty selection) — exit 0 without touching anything.
        PickOutcome::Cancelled => return ExitCode::SUCCESS,
        PickOutcome::Selected(path) => path,
    };

    if std::env::var_os("WALLPAPER_DRY_RUN").is_some()
        || std::env::var("KTM_THEME_NO_APPLY").as_deref() == Ok("1")
    {
        println!("{selected}");
        ExitCode::SUCCESS
    } else {
        match kde_theme_core::wallpaper::apply_image(&selected) {
            Ok(()) => ExitCode::SUCCESS,
            _ => ExitCode::from(1),
        }
    }
}
