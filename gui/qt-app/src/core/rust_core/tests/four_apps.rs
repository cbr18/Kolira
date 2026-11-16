//! Third batch: native palette selection and safe return, including missing resources.
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};
struct Home(tempfile::TempDir);
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
        let p = self.rpc("profile.preview", json!({"name":name}));
        assert_eq!(p["ok"], true, "{p}");
        let a = self.rpc(
            "profile.apply",
            json!({"name":name,"preview_token":p["data"]["token"],"skip_dependencies":true}),
        );
        assert_eq!(a["ok"], true, "{a}");
    }
}

#[test]
fn four_hosts_return_exactly_and_never_import_commands_or_lua() {
    let h = Home::new();
    let originals=[
        (".config/starship.toml","# personal\npalette='mine'\nformat='$directory$character'\n[palettes.mine]\ncustom_bg='#203040'\n[directory]\nstyle='fg:custom_bg'\n[custom.mine]\ncommand='printf kept'\n"),
        (".config/foot/foot.ini","font=Monospace:size=17\n[colors]\nbackground=123456\nalpha=0.72\n[key-bindings]\nclipboard-copy=Control+Shift+c\n"),
        (".config/wezterm/wezterm.lua","local config = {font_size=17, default_prog={'fish'}, colors={background='#112233'}}\nreturn config\n"),
        (".tmux.conf","# personal\nset -g mouse on\nset -g status-style 'bg=blue'\n"),
    ];
    for (p, t) in originals {
        h.write(p, t);
    }
    h.palette(json!(["starship", "foot", "wezterm", "tmux"]));
    h.write(
        ".config/kolira/themes/next/.config/wezterm/wezterm.lua",
        "error('theme Lua must never run')\n",
    );
    h.write(
        ".config/kolira/themes/next/.tmux.conf",
        "run-shell 'never-execute-theme-commands'\n",
    );
    for _ in 0..3 {
        h.apply("next");
        let star = h.text(".config/starship.toml");
        assert!(star.contains("printf kept") && star.contains("custom_bg"));
        assert!(h.text(".config/foot/foot.ini").contains("alpha=0.72"));
        assert!(h
            .text(".config/wezterm/wezterm.lua")
            .contains("font_size=17"));
        assert!(!h.text(".config/wezterm/wezterm.lua").contains("theme Lua"));
        assert!(!h.text(".tmux.conf").contains("never-execute"));
        assert!(h.text(".tmux.conf").contains("set -g mouse on"));
        h.apply("neutral");
        for (p, t) in originals {
            assert_eq!(h.text(p), t, "{p}");
        }
        for p in [
            ".config/tmux/kolira.json",
            ".config/tmux/kolira.conf",
            ".config/wezterm/kolira.toml",
        ] {
            assert!(!h.path(p).exists(), "{p}");
        }
    }
}
#[test]
fn late_personal_edits_survive_and_original_colours_return() {
    let h = Home::new();
    h.write(
        ".config/starship.toml",
        "palette='mine'\n[palettes.mine]\nblue='#112233'\n[directory]\ntruncation_length=9\n",
    );
    h.write(
        ".config/foot/foot.ini",
        "font=Mono:size=17\n[colors-dark]\nforeground=112233\n",
    );
    h.write(".config/wezterm/wezterm.lua", "return {font_size=17}\n");
    h.write(".tmux.conf", "set -g mouse on");
    h.palette(json!(["starship", "foot", "wezterm", "tmux"]));
    h.apply("next");
    h.write(
        ".config/starship.toml",
        &h.text(".config/starship.toml")
            .replace("truncation_length=9", "truncation_length=12"),
    );
    h.write(
        ".config/foot/foot.ini",
        &h.text(".config/foot/foot.ini")
            .replace("size=17", "size=21"),
    );
    h.write(
        ".config/wezterm/wezterm.lua",
        &h.text(".config/wezterm/wezterm.lua")
            .replace("font_size=17", "font_size=21"),
    );
    h.write(
        ".tmux.conf",
        &(h.text(".tmux.conf") + "set -g history-limit 9000\n"),
    );
    h.apply("neutral");
    assert!(h
        .text(".config/starship.toml")
        .contains("truncation_length=12"));
    let star: toml::Value = h.text(".config/starship.toml").parse().unwrap();
    assert_eq!(star["palette"].as_str(), Some("mine"));
    assert!(h
        .text(".config/foot/foot.ini")
        .contains("foreground=112233"));
    assert!(h.text(".config/foot/foot.ini").contains("size=21"));
    assert_eq!(
        h.text(".config/wezterm/wezterm.lua"),
        "return {font_size=21}\n"
    );
    assert_eq!(
        h.text(".tmux.conf"),
        "set -g mouse on\nset -g history-limit 9000\n"
    );
}
#[test]
fn absent_hosts_and_resources_disappear_on_return() {
    let h = Home::new();
    h.palette(json!(["starship", "foot", "wezterm", "tmux"]));
    // Fixtures represent installed binaries without running them.
    use std::os::unix::fs::PermissionsExt;
    for name in ["starship", "foot", "wezterm", "tmux"] {
        h.write(&format!("bin/{name}"), "#!/bin/sh\nexit 0\n");
        fs::set_permissions(
            h.path(&format!("bin/{name}")),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    let preview = h
        .command(&["apply", "next", "--without-dependencies"])
        .env("PATH", h.path("bin"))
        .output()
        .unwrap();
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    h.apply("neutral");
    for name in ["starship.toml", "foot", "wezterm", "tmux"] {
        assert!(!h.path(&format!(".config/{name}")).exists(), "{name}");
    }
}
#[test]
fn invalid_data_and_redirected_config_abort_before_writes() {
    let h = Home::new();
    h.write(".config/starship.toml", "format='kept'\n");
    h.write(".config/wezterm/wezterm.lua", "return {}\n");
    h.write(".tmux.conf", "set -g mouse on\n");
    h.palette(json!(["starship", "wezterm", "tmux"]));
    h.write(
        ".config/kolira/themes/next/.config/wezterm/kolira.toml",
        "[colors]\nbackground='not a colour'\n[metadata]\nname='Kolira'\n",
    );
    let p = h.rpc("profile.preview", json!({"name":"next"}));
    assert_eq!(p["ok"], false, "{p}");
    assert_eq!(h.text(".config/starship.toml"), "format='kept'\n");
    assert_eq!(h.text(".config/wezterm/wezterm.lua"), "return {}\n");
    fs::remove_file(h.path(".config/kolira/themes/next/.config/wezterm/kolira.toml")).unwrap();
    h.write(
        ".config/kolira/themes/next/.config/starship.toml",
        "palette='kolira'\n[palettes.kolira]\ngreen='##112233'\n",
    );
    assert_eq!(
        h.rpc("profile.preview", json!({"name":"next"}))["ok"],
        false
    );
    assert_eq!(h.text(".config/starship.toml"), "format='kept'\n");
    fs::remove_file(h.path(".config/kolira/themes/next/.config/starship.toml")).unwrap();
    let p = h
        .command(&["apply", "next", "--without-dependencies"])
        .env("STARSHIP_CONFIG", h.path("personal-starship.toml"))
        .output()
        .unwrap();
    assert!(!p.status.success());
    assert_eq!(h.text(".config/starship.toml"), "format='kept'\n");
}
#[test]
fn raw_tmux_code_in_a_theme_is_ignored_and_json_commands_are_rejected() {
    let h = Home::new();
    h.write(".tmux.conf", "# personal\n");
    h.palette(json!(["tmux"]));
    h.write(
        ".config/kolira/themes/next/.config/tmux/kolira.conf",
        "run-shell 'must-not-run'\n",
    );
    h.apply("next");
    assert!(!h.text(".config/tmux/kolira.conf").contains("run-shell"));
    h.apply("neutral");
    let mut values = kolira_core::tmux_runtime::styles(
        &serde_json::from_str(&h.text(".config/kolira/palette.json")).unwrap(),
    );
    values.insert("status-style".into(), "fg=#112233; run-shell bad".into());
    h.write(
        ".config/kolira/themes/next/.config/tmux/kolira.json",
        &serde_json::to_string(&values).unwrap(),
    );
    assert_eq!(
        h.rpc("profile.preview", json!({"name":"next"}))["ok"],
        false
    );
    assert_eq!(h.text(".tmux.conf"), "# personal\n");
}

#[test]
fn wezterm_search_order_and_explicit_native_file_are_respected() {
    for explicit in [false, true] {
        let h = Home::new();
        h.palette(json!(["wezterm"]));
        h.write(".config/wezterm/wezterm.lua", "return {font_size=13}\n");
        h.write(".wezterm.lua", "return {font_size=19}\n");
        let mut cmd = h.command(&["apply", "next", "--without-dependencies"]);
        if explicit {
            cmd.env("WEZTERM_CONFIG_FILE", h.path(".wezterm.lua"));
        }
        let result = cmd.output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(h.text(".wezterm.lua").contains("BEGIN kolira"), explicit);
        assert_eq!(
            h.text(".config/wezterm/wezterm.lua")
                .contains("BEGIN kolira"),
            !explicit
        );
        h.apply("neutral");
        assert_eq!(h.text(".wezterm.lua"), "return {font_size=19}\n");
        assert_eq!(
            h.text(".config/wezterm/wezterm.lua"),
            "return {font_size=13}\n"
        );
    }
}
#[test]
fn relocated_xdg_is_used_for_all_four_resources_and_local_glue() {
    let h = Home::new();
    h.palette(json!(["starship", "foot", "wezterm", "tmux"]));
    let package = h.text(".config/kolira/themes/next/theme.json");
    h.write("settings/kolira/themes/next/theme.json", &package);
    h.write(
        "settings/kolira/themes/neutral/theme.json",
        "{\"schema\":\"kolira/theme@2\",\"name\":\"neutral\"}",
    );
    for (p, t) in [
        ("starship.toml", "format='kept'\n"),
        ("foot/foot.ini", "font=Mono:size=13\n"),
        ("wezterm/wezterm.lua", "return {font_size=13}\n"),
        ("tmux/tmux.conf", "set -g mouse on\n"),
    ] {
        h.write(&format!("settings/{p}"), t);
    }
    for name in ["next", "neutral"] {
        let result = h
            .command(&["apply", name, "--without-dependencies"])
            .env("XDG_CONFIG_HOME", h.path("settings"))
            .env("XDG_DATA_HOME", h.path("data"))
            .env("XDG_CACHE_HOME", h.path("cache"))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        if name == "next" {
            assert!(h
                .text("settings/wezterm/wezterm.lua")
                .contains(h.path("settings/wezterm/kolira.toml").to_str().unwrap()));
            assert!(h
                .text("settings/tmux/tmux.conf")
                .contains(h.path("settings/tmux/kolira.conf").to_str().unwrap()));
        }
    }
    assert_eq!(h.text("settings/starship.toml"), "format='kept'\n");
    assert_eq!(h.text("settings/foot/foot.ini"), "font=Mono:size=13\n");
    assert!(!h.path(".config/wezterm").exists());
    assert!(!h.path(".config/tmux").exists());
}
#[test]
fn an_existing_personal_tmux_script_is_never_replaced() {
    let h = Home::new();
    h.write(".tmux.conf", "set -g mouse on\n");
    h.palette(json!(["starship", "tmux"]));
    h.write(".config/starship.toml", "format='personal'\n");
    h.write(".config/tmux/kolira.conf", "run-shell 'personal command'\n");
    assert_eq!(
        h.rpc("profile.preview", json!({"name":"next"}))["ok"],
        false
    );
    assert_eq!(h.text(".config/starship.toml"), "format='personal'\n");
    assert_eq!(
        h.text(".config/tmux/kolira.conf"),
        "run-shell 'personal command'\n"
    );
}
#[test]
fn old_native_parsers_are_refused_before_personal_configs_change() {
    use std::os::unix::fs::PermissionsExt;
    for (id, version, rel, text) in [
        (
            "foot",
            "foot version: 1.16.2",
            ".config/foot/foot.ini",
            "font=Mono:size=17\n",
        ),
        (
            "wezterm",
            "wezterm 20200202-0000",
            ".config/wezterm/wezterm.lua",
            "return {font_size=17}\n",
        ),
        (
            "starship",
            "starship 1.0.0",
            ".config/starship.toml",
            "format='personal'\n",
        ),
        ("tmux", "tmux 2.8", ".tmux.conf", "set -g mouse on\n"),
    ] {
        let h = Home::new();
        h.palette(json!([id]));
        h.write(rel, text);
        h.write(
            &format!("bin/{id}"),
            &format!("#!/bin/sh\nprintf '%s\\n' '{version}'\n"),
        );
        fs::set_permissions(
            h.path(&format!("bin/{id}")),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        let result = h
            .command(&["apply", "next", "--without-dependencies"])
            .env("KTM_THEME_NO_APPLY", "0")
            .env("PATH", h.path("bin"))
            .output()
            .unwrap();
        assert!(!result.status.success(), "{id}");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("Unsupported"),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(h.text(rel), text, "{id}");
    }
}
#[test]
fn generated_selected_text_is_readable_in_all_eight_palettes() {
    use kolira_core::{
        four_apps as apps,
        palette::{contrast, Palette, Rgb},
    };
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../..");
    for theme in fs::read_dir(repo.join("themes")).unwrap().flatten() {
        let file = theme.path().join("theme.json");
        if !file.is_file() {
            continue;
        }
        let v: Value = serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
        let p: Palette = serde_json::from_value(v["palette"].clone()).unwrap();
        let wez: toml::Value = apps::wezterm(&p).parse().unwrap();
        for (fg, bg) in [
            ("foreground", "background"),
            ("selection_fg", "selection_bg"),
            ("cursor_fg", "cursor_bg"),
        ] {
            let a = Rgb::parse(wez["colors"][fg].as_str().unwrap()).unwrap();
            let b = Rgb::parse(wez["colors"][bg].as_str().unwrap()).unwrap();
            assert!(contrast(a, b) >= 4.5, "{} {fg}/{bg}", p.name);
        }
        let foot = apps::foot(&p);
        for section in ["colors-dark", "colors-light"] {
            let rgb = |key: &str| {
                Rgb::parse(&format!(
                    "#{}",
                    kolira_core::field::ini_get(&foot, section, key).unwrap()
                ))
                .unwrap()
            };
            assert!(
                contrast(rgb("selection-foreground"), rgb("selection-background")) >= 4.5,
                "{} Foot selection",
                p.name
            );
        }
        for (key, style) in kolira_core::tmux_runtime::styles(&p) {
            if let Some((fg, bg)) = style.strip_prefix("fg=").and_then(|s| s.split_once(",bg=")) {
                assert!(
                    contrast(Rgb::parse(fg).unwrap(), Rgb::parse(bg).unwrap()) >= 4.5,
                    "{} tmux {key}",
                    p.name
                );
            }
        }
    }
}
