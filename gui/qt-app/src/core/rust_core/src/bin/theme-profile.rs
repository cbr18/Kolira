//! `theme-profile` — Rust CLI (replaces the bash script removed in 0.5.0).
//!
//! Keeps the historical CLI contract: same commands, same exit-code semantics
//! (`--help` → 0, invalid names / missing profiles → 1, unknown command → 1)
//! and the same stdout shapes (`status` key=value lines, `Saved profile:` /
//! `Applied profile:` messages, unified diff text).

use std::process::ExitCode;

use kde_theme_core::paths::validate_profile_name;
use kde_theme_core::profile::{
    apply_profile_cli, clone_profile, delete_profile, diff_profiles, list_profiles,
    save_profile_cli, set_profile_description, get_profile_description,
};
use kde_theme_core::status::status;
use kde_theme_core::wallpaper::{plasma_wallpaper_from, wallpaper_dir};
use kde_theme_core::palette;

const USAGE: &str = "\
Usage:
  theme-profile status
  theme-profile list
  theme-profile save NAME [--components=comp1,comp2] [--description=...]
  theme-profile apply NAME [--components=comp1,comp2]
  theme-profile clone SOURCE TARGET
  theme-profile diff A B
  theme-profile delete NAME
  theme-profile describe NAME [--set=...]
  theme-profile components <list|add|edit|remove> [json_or_id]
  theme-profile wallpaper <dir|current>
  theme-profile palette show [--json]
  theme-profile palette import FILE           (base16 YAML or palette JSON)
  theme-profile palette set SLOT=#rrggbb ...  (base00..base0F, accent)
  theme-profile palette generate <kde|kitty|alacritty>   (print the generated colours)
  theme-profile palette apply [--targets=kde,kitty,alacritty,kvantum] [--kvantum-base=THEME]
                              (default targets: kde,kitty,alacritty; kvantum switches the widget style)

Components: kwin, plasma_shell, kdeglobals, gtk3, gtk4, qt5ct,
            klassy, kvantum, alacritty, kitty, wofi, btop,
            fastfetch, zed, vscode, cursor, zen, obsidian,
            color_schemes, palette, look_and_feel

Use --components=all for all (default).

Profiles live in $XDG_CONFIG_HOME/theme-profile/profiles
(default: ~/.config/theme-profile/profiles).
";

/// Pull `--components=...` and optional `--description=...` out of the
/// arguments, returning (positional args, raw component spec, description).
fn parse_args(args: &[String]) -> (Vec<&str>, Option<String>, Option<String>) {
    let mut positional = Vec::new();
    let mut components: Option<String> = None;
    let mut description: Option<String> = None;
    for arg in args {
        if let Some(rest) = arg.strip_prefix("--components=") {
            components = Some(rest.to_string());
        } else if let Some(rest) = arg.strip_prefix("--description=") {
            description = Some(rest.to_string());
        } else {
            positional.push(arg.as_str());
        }
    }
    (positional, components, description)
}

/// Split the raw component spec the way the bash CLI did:
/// `all`/absent → everything, otherwise comma-separated ids (unknown → warn).
fn components_for_cli(raw: Option<&str>) -> Result<(Option<Vec<String>>, Option<String>), String> {
    match raw {
        None => Ok((None, None)),
        Some("all") => Ok((None, Some("all".to_string()))),
        Some(spec) => {
            if spec.is_empty() {
                return Err("No components selected".to_string());
            }
            let ids: Vec<String> = spec
                .split(',')
                .map(|c| c.trim().to_string())
                .filter(|c| !c.is_empty())
                .collect();
            if ids.is_empty() {
                return Err("No components selected".to_string());
            }
            Ok((Some(ids), Some(spec.to_string())))
        }
    }
}

fn component_suffix(raw: Option<&str>) -> String {
    match raw {
        Some(spec) => format!(" (components: {spec})"),
        None => String::new(),
    }
}

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("{msg}");
    ExitCode::from(1)
}

fn print_palette(p: &palette::Palette) {
    println!("name={}", p.name);
    println!("variant={}", match p.variant {
        palette::Variant::Dark => "dark",
        palette::Variant::Light => "light",
    });
    for (slot, value) in palette::SLOTS.iter().zip(&p.base) {
        println!("{slot}={value}");
    }
    println!("accent={}", p.accent);
    println!("source={}", p.source);
    for warning in p.contrast_warnings() {
        println!("warning={warning}");
    }
}

fn load_live_palette() -> Result<palette::Palette, ExitCode> {
    let path = palette::live_palette_path();
    palette::load(&path).map_err(|e| {
        fail(format!("No palette yet ({}): {e}. Use `theme-profile palette import FILE`.", path.display()))
    })
}

fn palette_cmd(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("show") => match load_live_palette() {
            Ok(p) if args.iter().any(|a| a == "--json") => {
                println!("{}", serde_json::to_string_pretty(&p).unwrap_or_default());
                ExitCode::SUCCESS
            }
            Ok(p) => {
                print_palette(&p);
                ExitCode::SUCCESS
            }
            Err(code) => code,
        },
        Some("import") => {
            let Some(file) = args.get(1) else {
                return fail("Usage error: palette import FILE");
            };
            let p = match palette::load(std::path::Path::new(file)) {
                Ok(p) => p,
                Err(e) => return fail(format!("{e:#}")),
            };
            if let Err(e) = palette::save(&p, &palette::live_palette_path()) {
                return fail(e);
            }
            print_palette(&p);
            ExitCode::SUCCESS
        }
        Some("set") => {
            let mut p = match load_live_palette() {
                Ok(p) => p,
                Err(code) => return code,
            };
            if args.len() < 2 {
                return fail("Usage error: palette set SLOT=#rrggbb ...");
            }
            for pair in &args[1..] {
                let Some((slot, value)) = pair.split_once('=') else {
                    return fail(format!("Expected SLOT=#rrggbb, got {pair:?}"));
                };
                if let Err(e) = p.set(slot, value) {
                    return fail(e);
                }
            }
            p.source = "manual".to_string();
            if let Err(e) = palette::save(&p, &palette::live_palette_path()) {
                return fail(e);
            }
            print_palette(&p);
            ExitCode::SUCCESS
        }
        Some("generate") => {
            let p = match load_live_palette() {
                Ok(p) => p,
                Err(code) => return code,
            };
            match args.get(1).map(String::as_str) {
                Some("kde") => {
                    print!("{}", palette::kde_color_scheme(&p));
                    ExitCode::SUCCESS
                }
                Some("kitty") => {
                    print!("{}", palette::kitty_colors(&p));
                    ExitCode::SUCCESS
                }
                Some("alacritty") => match palette::alacritty_with_palette("", &p) {
                    Ok(text) => {
                        print!("{text}");
                        ExitCode::SUCCESS
                    }
                    Err(e) => fail(e),
                },
                other => fail(format!("Unknown generator {other:?} (known: kde, kitty, alacritty)")),
            }
        }
        Some("apply") => {
            let p = match load_live_palette() {
                Ok(p) => p,
                Err(code) => return code,
            };
            let targets: Vec<String> = args[1..]
                .iter()
                .find_map(|a| a.strip_prefix("--targets="))
                .map(|t| t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
                .unwrap_or_else(|| palette::DEFAULT_TARGETS.iter().map(|t| t.to_string()).collect());
            let options = palette::ApplyOptions {
                kvantum_base: args[1..]
                    .iter()
                    .find_map(|a| a.strip_prefix("--kvantum-base="))
                    .map(str::to_string),
            };
            match palette::apply(&p, &targets, &options) {
                Ok(lines) => {
                    for line in lines {
                        println!("{line}");
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        _ => {
            eprint!("{USAGE}");
            fail("Usage error: palette <show|import|set|generate|apply>")
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("");

    match cmd {
        "status" => {
            print!("{}", status());
            ExitCode::SUCCESS
        }
        "list" => match list_profiles() {
            Ok(profiles) => {
                for p in profiles {
                    println!("{}", p.name);
                }
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        },
        "components" => {
            let sub = args.get(1).map(|s| s.as_str()).unwrap_or("");
            match sub {
                "list" => {
                    use kde_theme_core::custom_components::load_custom_components;
                    match load_custom_components() {
                        Ok(components) => {
                            for c in components {
                                println!("{}", c.id);
                            }
                            ExitCode::SUCCESS
                        }
                        Err(e) => fail(e),
                    }
                }
                "add" => {
                    if args.len() < 3 {
                        eprint!("{USAGE}");
                        return fail("Usage: theme-profile components add <json>");
                    }
                    let json = &args[2];
                    use kde_theme_core::custom_components::add_custom_component;
                    match serde_json::from_str::<kde_theme_core::custom_components::CustomComponent>(json) {
                        Ok(mut c) => {
                            c.is_custom = true;
                            match add_custom_component(c.clone()) {
                                Ok(()) => {
                                    println!("Added component: {}", c.id);
                                    ExitCode::SUCCESS
                                }
                                Err(e) => fail(e),
                            }
                        }
                        Err(e) => fail(format!("Invalid JSON: {e}")),
                    }
                }
                "edit" => {
                    if args.len() < 3 {
                        eprint!("{USAGE}");
                        return fail("Usage: theme-profile components edit <json>");
                    }
                    let json = &args[2];
                    use kde_theme_core::custom_components::update_custom_component;
                    match serde_json::from_str::<kde_theme_core::custom_components::CustomComponent>(json) {
                        Ok(mut c) => {
                            c.is_custom = true;
                            match update_custom_component(c.clone()) {
                                Ok(()) => {
                                    println!("Updated component: {}", c.id);
                                    ExitCode::SUCCESS
                                }
                                Err(e) => fail(e),
                            }
                        }
                        Err(e) => fail(format!("Invalid JSON: {e}")),
                    }
                }
                "remove" => {
                    if args.len() < 3 {
                        eprint!("{USAGE}");
                        return fail("Usage: theme-profile components remove <id>");
                    }
                    let id = &args[2];
                    use kde_theme_core::custom_components::remove_custom_component;
                    match remove_custom_component(id) {
                        Ok(()) => {
                            println!("Removed component: {}", id);
                            ExitCode::SUCCESS
                        }
                        Err(e) => fail(e),
                    }
                }
                "" => {
                    eprint!("{USAGE}");
                    fail("Usage: theme-profile components <list|add|edit|remove>")
                }
                _ => {
                    eprint!("{USAGE}");
                    fail(format!("Unknown subcommand: {}", sub))
                }
            }
        }
        "palette" => palette_cmd(&args[1..]),
        "wallpaper" => match args.get(1).map(String::as_str) {
            // Single source of truth for the GUI and scripts.
            Some("dir") => {
                println!("{}", wallpaper_dir().display());
                ExitCode::SUCCESS
            }
            Some("current") => {
                let appletsrc = std::env::var_os("PLASMA_APPLETSRC")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| {
                        kde_theme_core::paths::live_path(kde_theme_core::wallpaper::APPLETSRC_REL)
                    });
                match plasma_wallpaper_from(&appletsrc) {
                    Some(path) => {
                        println!("{path}");
                        ExitCode::SUCCESS
                    }
                    None => fail("No org.kde.image wallpaper found"),
                }
            }
            _ => {
                eprint!("{USAGE}");
                fail("Usage error: wallpaper <dir|current>")
            }
        },
        "save" => {
            let (positional, raw, desc) = parse_args(&args[1..]);
            let Some(name) = positional.first().copied() else {
                eprint!("{USAGE}");
                return fail("Usage error: save requires a profile name");
            };
            if positional.len() != 1 {
                return fail("Expected exactly one profile name");
            }
            if validate_profile_name(name).is_err() {
                return fail(format!(
                    "Invalid profile name: {name} (must match [A-Za-z0-9][A-Za-z0-9._-]{{0,63}})"
                ));
            }
            let (components, raw) = match components_for_cli(raw.as_deref()) {
                Ok(v) => v,
                Err(e) => return fail(e),
            };
            match save_profile_cli(name, components) {
                Ok(()) => {
                    println!("Saved profile: {name}{}", component_suffix(raw.as_deref()));
                    if let Some(desc) = desc.as_deref() {
                        if let Err(e) = set_profile_description(name, desc) {
                            eprintln!("Warning: could not save description: {e}");
                        }
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        "apply" => {
            let (positional, raw, _desc) = parse_args(&args[1..]);
            let Some(name) = positional.first().copied() else {
                eprint!("{USAGE}");
                return fail("Usage error: apply requires a profile name");
            };
            if positional.len() != 1 {
                return fail("Expected exactly one profile name");
            }
            if validate_profile_name(name).is_err() {
                return fail(format!(
                    "Invalid profile name: {name} (must match [A-Za-z0-9][A-Za-z0-9._-]{{0,63}})"
                ));
            }
            let (components, raw) = match components_for_cli(raw.as_deref()) {
                Ok(v) => v,
                Err(e) => return fail(e),
            };
            match apply_profile_cli(name, components) {
                Ok(backup) => {
                    println!("Applied profile: {name}{}", component_suffix(raw.as_deref()));
                    println!("Pre-apply backup: {}", backup.display());
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        "clone" => {
            let source = args.get(1).map(String::as_str).unwrap_or("");
            let target = args.get(2).map(String::as_str).unwrap_or("");
            if source.is_empty() || target.is_empty() {
                eprint!("{USAGE}");
                return fail("Usage error: clone requires SOURCE and TARGET names");
            }
            if validate_profile_name(source).is_err() {
                return fail(format!(
                    "Invalid profile name: {source} (must match [A-Za-z0-9][A-Za-z0-9._-]{{0,63}})"
                ));
            }
            if validate_profile_name(target).is_err() {
                return fail(format!(
                    "Invalid profile name: {target} (must match [A-Za-z0-9][A-Za-z0-9._-]{{0,63}})"
                ));
            }
            match clone_profile(source, target) {
                Ok(()) => {
                    println!("Cloned profile: {source} -> {target}");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        "diff" => {
            let a = args.get(1).map(String::as_str).unwrap_or("");
            let b = args.get(2).map(String::as_str).unwrap_or("");
            if a.is_empty() || b.is_empty() {
                eprint!("{USAGE}");
                return fail("Usage error: diff requires two profile names");
            }
            match diff_profiles(a, b) {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        "delete" => {
            let Some(name) = args.get(1).map(String::as_str) else {
                eprint!("{USAGE}");
                return fail("Usage error: delete requires a profile name");
            };
            match delete_profile(name) {
                Ok(()) => {
                    println!("Deleted profile: {name}");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        "describe" => {
            let name = args.get(1).map(String::as_str).unwrap_or("");
            if name.is_empty() {
                eprint!("{USAGE}");
                return fail("Usage error: describe requires a profile name");
            }
            // Extract --set from the remaining args.
            let set = args.iter().find_map(|a| a.strip_prefix("--set="));
            if let Some(value) = set {
                match set_profile_description(name, value) {
                    Ok(()) => println!("Description updated."),
                    Err(e) => return fail(e),
                }
            } else {
                match get_profile_description(name) {
                    Ok(d) => {
                        if d.is_empty() {
                            println!("(no description)");
                        } else {
                            println!("{d}");
                        }
                    }
                    Err(e) => return fail(e),
                }
            }
            ExitCode::SUCCESS
        }
        "--help" | "-h" => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        "--version" => {
            println!("theme-profile 0.5.0");
            ExitCode::SUCCESS
        }
        _ => {
            eprint!("{USAGE}");
            ExitCode::from(1)
        }
    }
}