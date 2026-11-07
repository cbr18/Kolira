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
    set_profile_description, get_profile_description,
};
use kde_theme_core::status::status;
use kde_theme_core::wallpaper::{plasma_wallpaper_from, wallpaper_dir};
use kde_theme_core::palette;
use kde_theme_core::gallery;
use kde_theme_core::extract;
use kde_theme_core::stores;
use kde_theme_core::field;
use kde_theme_core::theme;
use kde_theme_core::share;

const USAGE: &str = "\
Usage:
  theme-profile status
  theme-profile list
  theme-profile save NAME [--description=...] [--no-wallpaper]
                              (full snapshot of the appearance files + theme.json)
  theme-profile apply NAME [--install-missing] [--keep-wallpaper] [--json]
                              (restores the theme; missing assets are reported,
                               --install-missing installs app themes from their stores)
  theme-profile check NAME [--json]           (what applying NAME would miss)
  theme-profile prune NAME|--all              (drop what is not part of the theme; backup first)
  theme-profile deps NAME [--refresh] [--install[=all|0,2]] [--json]
                              (what the theme needs and where from; install from official sources)
  theme-profile actions NAME [--json]         (what applying leaves to do: restarts, Telegram)
  theme-profile action ID                     (do one: vesktop, discord, zen, firefox)
  theme-profile automation [--json] | set KEY=ask|always|never … | ask
                              (what applying may do by itself: store, packages, restart_<app>)
  theme-profile license NAME ASSET --license=SPDX [--source=URL] [--no-redistribute]
                              (mark a theme asset — wallpaper, Look & Feel…, or kind:name — for publishing)
  theme-profile export NAME [--out=FILE.zip | --dir=DIR] [--no-wallpaper] [--public]
                              (portable copy: $HOME for the home dir, no desktop icons)
  theme-profile import FILE.zip|DIR [--name=NEW] [--force] [--apply] [--json]
  theme-profile migrate NAME|--all            (add theme.json to snapshot profiles)
  theme-profile new NAME [--from=THEME|neutral-light|neutral-dark] [--description=…]
                              (start a new theme from a base; replaces the live look, backup first)
  theme-profile look-and-feel NAME           (Look & Feel package of the live look, defaults only)
  theme-profile clone SOURCE TARGET
  theme-profile diff A B
  theme-profile delete NAME
  theme-profile describe NAME [--set=...]
  theme-profile components <list|add|edit|remove> [json_or_id]
  theme-profile wallpaper <dir|current>
  theme-profile field list
  theme-profile field get [ID...] [--profile=NAME] [--json]
  theme-profile field set ID=VALUE ...        (write keys + live command/hook)
  theme-profile apps store-search APP QUERY [--json]   (APP: zed, vscode, cursor)
  theme-profile apps store-install APP ID [--json]     (Zed extension id / publisher.name)
  theme-profile palette show [--json]
  theme-profile palette import FILE|gallery:ID   (base16 YAML, palette JSON or a gallery scheme)
  theme-profile palette gallery [QUERY] [--online] [--refresh] [--json]
                              (bundled + cached schemes; --online adds the tinted-theming catalogue)
  theme-profile palette set SLOT=#rrggbb ...  (base00..base0F, accent)
  theme-profile palette from-wallpaper [IMAGE] [--variant=dark|light] [--name=NAME]
                              (default IMAGE: the current Plasma wallpaper)
  theme-profile palette from-colors JSON|-    ({\"colors\":[{\"color\":\"#rrggbb\",\"ratio\":0.4},...]},
                              the shape of Kirigami.ImageColors.palette; - reads stdin)
  theme-profile palette generate <kde|kitty|alacritty>   (print the generated colours)
  theme-profile palette apply [--targets=kde,kitty,alacritty,gtk,kvantum] [--kvantum-base=THEME] [--opacity=0.3-1] [--gtk-opacity=…] [--discord-opacity=…]
                              (default: all but kvantum and gtk; kvantum switches the widget style, gtk recolours libadwaita apps)

Components: kwin, plasma_shell, kdeglobals, gtk3, gtk4, qt5ct,
            breeze, klassy, kvantum, alacritty, kitty, wofi, btop,
            fastfetch, zed, vscode, cursor, zen, obsidian,
            color_schemes, palette, look_and_feel

Use --components=all for all (default).

Themes live in $XDG_CONFIG_HOME/kde-theme-profile/themes
(default: ~/.config/kde-theme-profile/themes).
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
        } else if !arg.starts_with("--") {
            // Other --flags (--no-wallpaper, --install-missing, --json) are
            // read by the subcommands themselves.
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

fn field_cmd(args: &[String]) -> ExitCode {
    let json = args.iter().any(|a| a == "--json");
    let profile = args.iter().find_map(|a| a.strip_prefix("--profile="));
    let rest: Vec<&str> = args.iter().skip(1).map(String::as_str).filter(|a| !a.starts_with("--")).collect();
    match args.first().map(String::as_str) {
        Some("list") => {
            for f in field::registry() {
                println!("{}\t{}\t{}\t{}", f.id, f.kind, f.source, f.label);
            }
            ExitCode::SUCCESS
        }
        Some("get") => {
            let base = match profile {
                Some(name) => {
                    if validate_profile_name(name).is_err() {
                        return fail(format!("Invalid profile name: {name}"));
                    }
                    field::Base::Snapshot(kde_theme_core::paths::profiles_dir().join(name))
                }
                None => field::Base::Live,
            };
            let ids: Vec<&str> = if rest.is_empty() {
                field::registry().iter().map(|f| f.id.as_str()).collect()
            } else {
                rest
            };
            let mut map = serde_json::Map::new();
            for id in ids {
                let Some(f) = field::field(id) else { return fail(format!("Unknown field: {id}")) };
                let value = field::read(f, &base).unwrap_or(serde_json::Value::Null);
                if json {
                    map.insert(id.to_string(), value);
                } else {
                    println!("{id}={}", field::value_str(&value));
                }
            }
            if json {
                println!("{}", serde_json::Value::Object(map));
            }
            ExitCode::SUCCESS
        }
        Some("set") => {
            let mut values = Vec::new();
            for pair in rest {
                let Some((id, value)) = pair.split_once('=') else {
                    return fail(format!("Expected ID=VALUE, got {pair:?}"));
                };
                let parsed = serde_json::from_str(value).unwrap_or(serde_json::Value::String(value.to_string()));
                values.push((id.to_string(), parsed));
            }
            if values.is_empty() {
                return fail("Usage error: field set ID=VALUE ...");
            }
            let report = field::apply(&values);
            for id in &report.applied {
                println!("applied={id}");
            }
            for e in &report.errors {
                eprintln!("error={e}");
            }
            if report.errors.is_empty() { ExitCode::SUCCESS } else { ExitCode::from(1) }
        }
        _ => fail("Usage error: field <list|get|set>"),
    }
}

fn apps_cmd(args: &[String]) -> ExitCode {
    let json = args.iter().any(|a| a == "--json");
    let positional: Vec<&str> = args.iter().map(String::as_str).filter(|a| !a.starts_with("--")).collect();
    match positional.as_slice() {
        ["store-search", app, query @ ..] => match stores::search(app, &query.join(" ")) {
            Ok(items) if json => {
                println!("{}", serde_json::to_string(&items).unwrap_or_default());
                ExitCode::SUCCESS
            }
            Ok(items) => {
                for i in items {
                    println!("{}\t{}\t{}\t{}", i.id, i.downloads, i.version, i.name);
                }
                ExitCode::SUCCESS
            }
            Err(e) => fail(format!("{e:#}")),
        },
        ["store-install", app, id] => match stores::install(app, id) {
            Ok(report) if json => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                ExitCode::SUCCESS
            }
            Ok(report) => {
                println!("Installed {} for {} → {}", report.id, report.app, report.location);
                for theme in report.themes {
                    println!("theme={theme}");
                }
                ExitCode::SUCCESS
            }
            Err(e) => fail(format!("{e:#}")),
        },
        _ => {
            eprint!("{USAGE}");
            fail("Usage error: apps <store-search APP QUERY|store-install APP ID>")
        }
    }
}

fn palette_cmd(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("show") => match load_live_palette() {
            Ok(p) if args.iter().any(|a| a == "--json") => {
                // Palette + what the GUI needs to show it (warnings, scheme id).
                let mut value = serde_json::to_value(&p).unwrap_or_default();
                if let Some(map) = value.as_object_mut() {
                    map.insert("warnings".into(), serde_json::json!(p.contrast_warnings()));
                    map.insert("kdeScheme".into(), serde_json::json!(palette::kde_scheme_id(&p)));
                }
                println!("{}", serde_json::to_string_pretty(&value).unwrap_or_default());
                ExitCode::SUCCESS
            }
            Ok(p) => {
                print_palette(&p);
                ExitCode::SUCCESS
            }
            Err(code) => code,
        },
        Some("gallery") => {
            let query = args[1..].iter().find(|a| !a.starts_with("--")).map_or("", String::as_str);
            let online = args.iter().any(|a| a == "--online");
            let refresh = args.iter().any(|a| a == "--refresh");
            let entries = match gallery::list(query, online || refresh, refresh) {
                Ok(e) => e,
                Err(e) => return fail(format!("{e:#}")),
            };
            let variant = |v: Option<palette::Variant>| match v {
                Some(palette::Variant::Dark) => "dark",
                Some(palette::Variant::Light) => "light",
                None => "",
            };
            if args.iter().any(|a| a == "--json") {
                let items: Vec<serde_json::Value> = entries
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "id": e.id, "origin": e.origin.as_str(),
                            "name": e.name, "variant": variant(e.variant),
                        })
                    })
                    .collect();
                println!("{}", serde_json::Value::Array(items));
            } else {
                for e in &entries {
                    println!("{}\t{}\t{}\t{}", e.id, e.origin.as_str(), variant(e.variant), e.name.as_deref().unwrap_or(""));
                }
            }
            if online && !kde_theme_core::net::network_enabled() {
                eprintln!("Network is disabled (network_enabled=false or KTM_OFFLINE=1): cached catalogue only.");
            }
            ExitCode::SUCCESS
        }
        Some("import") => {
            let Some(file) = args.get(1) else {
                return fail("Usage error: palette import FILE|gallery:ID");
            };
            let loaded = match file.strip_prefix("gallery:") {
                Some(id) => gallery::fetch(id),
                None => palette::load(std::path::Path::new(file)),
            };
            let p = match loaded {
                Ok(p) => p,
                Err(e) => return fail(format!("{e:#}")),
            };
            if let Err(e) = palette::save(&p, &palette::live_palette_path()) {
                return fail(e);
            }
            print_palette(&p);
            ExitCode::SUCCESS
        }
        Some("from-wallpaper") | Some("from-colors") => {
            let value = |prefix: &str| args[1..].iter().find_map(|a| a.strip_prefix(prefix));
            let variant = match value("--variant=") {
                None => None,
                Some("dark") => Some(palette::Variant::Dark),
                Some("light") => Some(palette::Variant::Light),
                Some(other) => return fail(format!("Unknown variant {other:?} (dark|light)")),
            };
            let positional = args[1..].iter().find(|a| !a.starts_with("--")).cloned();
            let built = if args[0] == "from-wallpaper" {
                let image = positional.or_else(|| {
                    plasma_wallpaper_from(&kde_theme_core::paths::live_path(kde_theme_core::wallpaper::APPLETSRC_REL))
                });
                let Some(image) = image else {
                    return fail("No image given and no org.kde.image wallpaper found");
                };
                extract::palette_from_image(std::path::Path::new(&image), variant, value("--name="))
            } else {
                let json = match positional.as_deref() {
                    Some("-") | None => {
                        let mut text = String::new();
                        if let Err(e) = std::io::Read::read_to_string(&mut std::io::stdin(), &mut text) {
                            return fail(e);
                        }
                        text
                    }
                    Some(text) => text.to_string(),
                };
                extract::palette_from_colors_json(&json).map(|mut p| {
                    if let Some(v) = variant {
                        p.variant = v;
                    }
                    if let Some(name) = value("--name=") {
                        p.name = name.to_string();
                    }
                    p
                })
            };
            let p = match built {
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
                gtk_opacity: opacity_arg(&args[1..], "--gtk-opacity="),
                discord_opacity: opacity_arg(&args[1..], "--discord-opacity="),
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
            fail("Usage error: palette <show|import|gallery|set|from-wallpaper|from-colors|generate|apply>")
        }
    }
}

/// `--<app>-opacity=X`, else the shared `--opacity=X`.
fn opacity_arg(args: &[String], key: &str) -> Option<f64> {
    let find = |k: &str| args.iter().find_map(|a| a.strip_prefix(k)).and_then(|v| v.parse::<f64>().ok());
    find(key).or_else(|| find("--opacity="))
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
        "apps" => apps_cmd(&args[1..]),
        "field" => field_cmd(&args[1..]),
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
            if components.is_some() {
                eprintln!("Note: --components is ignored — a theme saves its theme values, not whole files.");
            }
            let options = theme::SaveOptions {
                description: desc.clone(),
                include_wallpaper: !args.iter().any(|a| a == "--no-wallpaper"),
            };
            match theme::save(name, &options).map(|_| ()) {
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
            let dir = kde_theme_core::paths::profiles_dir().join(name);
            if theme::is_package(&dir) {
                let options = theme::ApplyOptions {
                    install_missing: args.iter().any(|a| a == "--install-missing"),
                    keep_wallpaper: args.iter().any(|a| a == "--keep-wallpaper"),
                };
                return match theme::apply(name, &options) {
                    Ok(report) if args.iter().any(|a| a == "--json") => {
                        match serde_json::to_string(&report) {
                            Ok(json) => {
                                println!("{json}");
                                ExitCode::SUCCESS
                            }
                            Err(e) => fail(e),
                        }
                    }
                    Ok(report) => {
                        println!("Applied profile: {name}");
                        println!("Pre-apply backup: {}", report.backup);
                        for i in &report.installed {
                            println!("installed={i}");
                        }
                        if !report.skipped.is_empty() {
                            println!("skipped (app not set up here)={}", report.skipped.join(","));
                        }
                        for m in &report.missing {
                            println!("missing={}={}{}", m.field, m.name, if m.source.is_empty() { String::new() } else { format!(" ({})", m.source) });
                        }
                        for a in &report.actions {
                            println!("action={}: {}{}", a.id, a.title, if a.warning.is_empty() { String::new() } else { format!(" (warning: {})", a.warning) });
                        }
                        for e in &report.errors {
                            eprintln!("error={e}");
                        }
                        ExitCode::SUCCESS
                    }
                    Err(e) => fail(format!("{e:#}")),
                };
            }
            match apply_profile_cli(name, components) {
                Ok(backup) => {
                    println!("Applied profile: {name}{}", component_suffix(raw.as_deref()));
                    println!("Pre-apply backup: {}", backup.display());
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        "new" => {
            // A new theme from a base: a theme, neutral-light or neutral-dark.
            let Some(name) = args.iter().skip(1).find(|a| !a.starts_with("--")) else {
                return fail("Usage: theme-profile new NAME --from=THEME|neutral-light|neutral-dark [--description=…]");
            };
            let base = args.iter().find_map(|a| a.strip_prefix("--from=")).unwrap_or("neutral-light");
            let description = args.iter().find_map(|a| a.strip_prefix("--description=")).map(str::to_string);
            match kde_theme_core::constructor::new_theme(name, base, description) {
                Ok(t) => {
                    println!("New theme: {} (from {})", t.name, t.base);
                    for n in &t.notes {
                        eprintln!("error={n}");
                    }
                    println!("Edit the look, then save it again: theme-profile save {}", t.name);
                    ExitCode::SUCCESS
                }
                Err(e) => fail(format!("{e:#}")),
            }
        }
        "look-and-feel" => {
            // A Look & Feel package (defaults only) for the live look.
            let Some(name) = args.get(1) else {
                return fail("Usage: theme-profile look-and-feel NAME");
            };
            match kde_theme_core::constructor::look_and_feel(name) {
                Ok(dir) => {
                    println!("Look & Feel: {} (selected; save the theme to keep it)", dir.display());
                    ExitCode::SUCCESS
                }
                Err(e) => fail(format!("{e:#}")),
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
        "export" => {
            let Some(name) = args.get(1).filter(|a| !a.starts_with("--")) else {
                return fail("Usage error: export NAME [--out=FILE.zip | --dir=DIR] [--no-wallpaper]");
            };
            let options = share::ExportOptions {
                include_wallpaper: !args.iter().any(|a| a == "--no-wallpaper"),
                public: args.iter().any(|a| a == "--public"),
            };
            let dir = args.iter().find_map(|a| a.strip_prefix("--dir="));
            let result = match dir {
                Some(d) => share::export_dir(name, std::path::Path::new(d), &options).map(|n| (n, d.to_string())),
                None => {
                    let out = args
                        .iter()
                        .find_map(|a| a.strip_prefix("--out="))
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("{name}.ktmtheme.zip"));
                    share::export_zip(name, std::path::Path::new(&out), &options).map(|n| (n, out))
                }
            };
            match result {
                Ok((n, out)) => {
                    println!("Exported {name}: {n} files -> {out}");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(format!("{e:#}")),
            }
        }
        "import" => {
            let Some(src) = args.get(1).filter(|a| !a.starts_with("--")) else {
                return fail("Usage error: import FILE.zip|DIR [--name=NEW] [--force] [--apply] [--json]");
            };
            let options = share::ImportOptions {
                name: args.iter().find_map(|a| a.strip_prefix("--name=")).map(str::to_string),
                force: args.iter().any(|a| a == "--force"),
            };
            let report = match share::import(std::path::Path::new(src), &options) {
                Ok(r) => r,
                Err(e) => return fail(format!("{e:#}")),
            };
            if args.iter().any(|a| a == "--apply") {
                let apply = theme::ApplyOptions::default();
                if let Err(e) = theme::apply(&report.name, &apply) {
                    return fail(format!("imported {}, but apply failed: {e:#}", report.name));
                }
            }
            if args.iter().any(|a| a == "--json") {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
            } else {
                println!("Imported theme: {} ({} files)", report.name, report.files);
                if let Some(old) = &report.replaced {
                    println!("Replaced theme backed up to: {old}");
                }
                for m in &report.missing {
                    println!("missing={}={}{}", m.field, m.name, if m.source.is_empty() { String::new() } else { format!(" ({})", m.source) });
                }
            }
            ExitCode::SUCCESS
        }
        "actions" => {
            // What applying NAME leaves to do (restarts, Telegram).
            let Some(name) = args.iter().skip(1).find(|a| !a.starts_with("--")) else {
                return fail("Usage: theme-profile actions NAME [--json]");
            };
            if validate_profile_name(name).is_err() {
                return fail(format!("Invalid profile name: {name}"));
            }
            let list = kde_theme_core::actions::for_theme(&kde_theme_core::paths::profiles_dir().join(name));
            if args.iter().any(|a| a == "--json") {
                println!("{}", serde_json::to_string(&list).unwrap_or_default());
            } else if list.is_empty() {
                println!("Nothing to do.");
            } else {
                for a in &list {
                    println!("{}: {} — {}{}", a.id, a.title, a.reason, if a.warning.is_empty() { String::new() } else { format!(" (warning: {})", a.warning) });
                }
            }
            ExitCode::SUCCESS
        }
        "automation" => {
            // The user's choices: `automation` shows them, `automation set
            // KEY=ask|always|never …`, `automation ask` asks on the terminal.
            use kde_theme_core::automation::{self, Policy};
            match args.get(1).map(String::as_str) {
                None | Some("--json") => {
                    if args.iter().any(|a| a == "--json") {
                        let items: Vec<serde_json::Value> = automation::KEYS
                            .iter()
                            .map(|(k, what, note)| serde_json::json!({"key": k, "what": what, "note": note, "policy": automation::policy(k).as_str()}))
                            .collect();
                        println!("{}", serde_json::json!({"configured": automation::configured(), "items": items}));
                    } else {
                        for (k, what, _) in automation::KEYS {
                            println!("{k}={}  # {what}", automation::policy(k).as_str());
                        }
                    }
                    ExitCode::SUCCESS
                }
                Some("set") => {
                    let mut choices = Vec::new();
                    for pair in &args[2..] {
                        let Some((k, v)) = pair.split_once('=') else { return fail(format!("Expected KEY=ask|always|never, got {pair:?}")) };
                        let Some(p) = Policy::parse(v) else { return fail(format!("{k}: expected ask, always or never")) };
                        choices.push((k, p));
                    }
                    match automation::set(&choices) {
                        Ok(()) => ExitCode::SUCCESS,
                        Err(e) => fail(format!("{e:#}")),
                    }
                }
                Some("ask") => {
                    let stdin = std::io::stdin();
                    match automation::ask_interactively(&mut stdin.lock(), &mut std::io::stdout()) {
                        Ok(()) => ExitCode::SUCCESS,
                        Err(e) => fail(format!("{e:#}")),
                    }
                }
                Some(other) => fail(format!("Usage error: automation [--json] | set KEY=POLICY … | ask (got {other:?})")),
            }
        }
        "action" => {
            // Perform one action from `actions NAME` (the user agreed to it).
            let Some(id) = args.get(1) else {
                return fail("Usage: theme-profile action ID");
            };
            match kde_theme_core::actions::run(id) {
                Ok(msg) => {
                    println!("{msg}");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(format!("{e:#}")),
            }
        }
        "license" => {
            let pos: Vec<&String> = args.iter().skip(1).filter(|a| !a.starts_with("--")).collect();
            let licence = args.iter().find_map(|a| a.strip_prefix("--license="));
            let (Some(name), Some(asset), Some(licence)) = (pos.first(), pos.get(1), licence) else {
                return fail("Usage error: license NAME ASSET --license=SPDX [--source=URL] [--no-redistribute]");
            };
            let source = args.iter().find_map(|a| a.strip_prefix("--source=")).unwrap_or("");
            let redistribute = !args.iter().any(|a| a == "--no-redistribute");
            match theme::set_licence(name, asset, licence, source, redistribute) {
                Ok(a) => {
                    println!("{name}: {} {} — {} ({})", a.kind, a.name, a.license, if a.redistributable { "redistributable" } else { "not shipped by export --public" });
                    ExitCode::SUCCESS
                }
                Err(e) => fail(format!("{e:#}")),
            }
        }
        "prune" => {
            let names: Vec<String> = if args.iter().any(|a| a == "--all") {
                match list_profiles() {
                    Ok(list) => list.into_iter().map(|p| p.name).collect(),
                    Err(e) => return fail(e),
                }
            } else {
                args.iter().skip(1).filter(|a| !a.starts_with("--")).cloned().collect()
            };
            if names.is_empty() {
                return fail("Usage error: prune NAME|--all");
            }
            let mut failed = false;
            for name in names {
                match theme::prune(&name) {
                    Ok(n) => println!("Pruned {name}: {n} file(s) removed (backup in backups/pre-prune-{name}-…)"),
                    Err(e) => {
                        eprintln!("{name}: {e:#}");
                        failed = true;
                    }
                }
            }
            if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
        }
        "deps" => {
            let Some(name) = args.get(1).filter(|a| !a.starts_with("--")) else {
                return fail("Usage error: deps NAME [--refresh] [--install[=all|1,2]] [--json]");
            };
            let json = args.iter().any(|a| a == "--json");
            if args.iter().any(|a| a == "--refresh") {
                if let Err(e) = theme::refresh_dependencies(name) {
                    return fail(format!("{e:#}"));
                }
            }
            if let Some(spec) = args.iter().find_map(|a| a.strip_prefix("--install").map(|r| r.trim_start_matches('=').to_string())) {
                let indexes: Option<Vec<usize>> = if spec.is_empty() || spec == "all" {
                    None
                } else {
                    match spec.split(',').map(|x| x.trim().parse::<usize>()).collect() {
                        Ok(v) => Some(v),
                        Err(_) => return fail("--install expects all or indexes like 0,2"),
                    }
                };
                let results = match theme::install_dependencies(name, indexes.as_deref()) {
                    Ok(r) => r,
                    Err(e) => return fail(format!("{e:#}")),
                };
                let failed = results.iter().any(|(_, r)| r.is_err());
                if json {
                    let list: Vec<serde_json::Value> = results
                        .iter()
                        .map(|(i, r)| match r {
                            Ok(m) => serde_json::json!({"index": i, "ok": true, "message": m}),
                            Err(e) => serde_json::json!({"index": i, "ok": false, "message": format!("{e:#}")}),
                        })
                        .collect();
                    println!("{}", serde_json::to_string(&list).unwrap_or_default());
                } else {
                    for (i, r) in &results {
                        match r {
                            Ok(m) => println!("installed[{i}]: {m}"),
                            Err(e) => println!("failed[{i}]: {e:#}"),
                        }
                    }
                }
                return if failed { ExitCode::from(2) } else { ExitCode::SUCCESS };
            }
            match theme::needed(name) {
                Ok(list) if json => {
                    println!("{}", serde_json::to_string(&list).unwrap_or_default());
                    ExitCode::SUCCESS
                }
                Ok(list) => {
                    if list.is_empty() {
                        println!("All dependencies are installed.");
                    }
                    for n in list {
                        let d = &n.dependency;
                        let how = n.command.clone().unwrap_or_else(|| format!("{}{}", d.source, if d.exact { "" } else { " (search result — confirm)" }));
                        println!("[{}] {} {} — {} — {}", n.index, d.kind, d.name, how, d.url);
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => fail(format!("{e:#}")),
            }
        }
        "check" => {
            let Some(name) = args.get(1).filter(|a| !a.starts_with("--")) else {
                return fail("Usage error: check NAME");
            };
            match theme::check(name) {
                Ok(missing) if args.iter().any(|a| a == "--json") => {
                    println!("{}", serde_json::to_string(&missing).unwrap_or_default());
                    ExitCode::SUCCESS
                }
                Ok(missing) => {
                    if missing.is_empty() {
                        println!("Everything this theme needs is available.");
                    }
                    for m in missing {
                        println!("missing={}={}{}", m.field, m.name, if m.source.is_empty() { String::new() } else { format!(" ({})", m.source) });
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => fail(format!("{e:#}")),
            }
        }
        "migrate" => {
            let names: Vec<String> = if args.iter().any(|a| a == "--all") {
                match list_profiles() {
                    Ok(list) => list.into_iter().map(|p| p.name).collect(),
                    Err(e) => return fail(e),
                }
            } else {
                args.iter().skip(1).filter(|a| !a.starts_with("--")).cloned().collect()
            };
            if names.is_empty() {
                return fail("Usage error: migrate NAME|--all");
            }
            let mut failed = false;
            for name in names {
                match theme::migrate(&name) {
                    Ok(p) => println!("Migrated {name}: {} values, {} assets{}", p.values.len(), p.assets.len(),
                        if p.wallpaper.is_some() { ", wallpaper" } else { "" }),
                    Err(e) => {
                        failed = true;
                        eprintln!("{name}: {e:#}");
                    }
                }
            }
            if failed { ExitCode::from(1) } else { ExitCode::SUCCESS }
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
            println!("theme-profile {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            eprint!("{USAGE}");
            ExitCode::from(1)
        }
    }
}