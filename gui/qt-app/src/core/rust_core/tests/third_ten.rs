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
fn all_third_batch_hosts_return_exactly_three_cycles() {
    let h = Home::new();
    let originals = [
      (".config/micro/settings.json", "{\"colorscheme\":\"default\",\"tabsize\":7}"),
      (".config/lsd/config.yaml", "# personal\ncolor:\n  when: always\n  theme: default\nsorting:\n  reverse: true\n"),
      (".config/glow/glow.yml", "# personal\nstyle: auto\nwidth: 93\n"),
      (".config/GIMP/3.0/gimprc", "# personal\n(theme \"Default\")\n(theme-color-scheme dark)\n(num-processors 7)\n"),
      (".config/heroic/config.json", "{\"defaultSettings\":{\"customCSS\":\"body { width: 900px; }\",\"language\":\"ru\"},\"games\":[\"private\"]}"),
      (".local/share/PrismLauncher/prismlauncher.cfg", "[General]\nApplicationTheme=system\nJavaPath=/my/java\n"),
      (".config/mpv/mpv.conf", "volume=57\nosd-color=#123456\n[private]\nvo=null\nosd-color=#abcdef\n"),
      (".config/cliamp/config.toml", "theme = \"default\"\nvolume = 57\n"),
      (".config/VSCodium/User/settings.json", "{// personal\n\"editor.fontSize\":19,\"editor.fontFamily\":\"PersonalFont\",\"workbench.iconTheme\":\"PersonalIcons\",\"workbench.colorTheme\":\"Mine\"}"),
      (".vimrc", "set number\nlet g:private = 57\n"),
      (".config/libreoffice/4/user/registrymodifications.xcu", "private-office-settings"),
    ];
    for (path, text) in originals {
        h.write(path, text);
    }
    h.palette(json!(kolira_core::third_apps::TARGETS));
    for _ in 0..3 {
        h.apply("next");
        assert!(h.text(".config/lsd/config.yaml").contains("reverse: true"));
        assert!(h.text(".config/glow/glow.yml").contains("width: 93"));
        assert!(h
            .text(".config/GIMP/3.0/gimprc")
            .contains("(num-processors 7)"));
        assert!(h.text(".config/heroic/config.json").contains("private"));
        assert!(h
            .text(".config/heroic/config.json")
            .contains("width: 900px"));
        assert!(h
            .text(".local/share/PrismLauncher/prismlauncher.cfg")
            .contains("JavaPath=/my/java"));
        assert!(h
            .text(".config/mpv/mpv.conf")
            .contains("[private]\nvo=null\nosd-color=#abcdef"));
        let editor = h.text(".config/VSCodium/User/settings.json");
        assert!(
            editor.contains("PersonalFont") && editor.contains("PersonalIcons"),
            "{editor}"
        );
        assert!(h.path(".vim/colors/kolira.vim").is_file());
        h.apply("neutral");
        for (path, text) in originals {
            assert_eq!(h.text(path), text, "{path}");
        }
        for path in [
            ".vim/colors/kolira.vim",
            ".vim/after/plugin/kolira.vim",
            ".vim/kolira-palette.json",
            ".config/glow/kolira.json",
            ".config/cliamp/themes/kolira.toml",
            ".config/micro/colorschemes/kolira.micro",
            ".config/lsd/themes/kolira.yaml",
            ".config/GIMP/3.0/themes/Kolira/gimp.css",
            ".local/share/PrismLauncher/themes/kolira/theme.json",
        ] {
            assert!(!h.path(path).exists(), "{path}");
        }
    }
}
#[test]
fn late_personal_edits_survive_and_absent_selectors_disappear() {
    let h = Home::new();
    let originals=[(".config/lsd/config.yaml","sorting:\n  reverse: false\n"),(".config/glow/glow.yml","width: 80\n"),(".config/GIMP/3.0/gimprc","(num-processors 3)\n"),(".config/mpv/mpv.conf","volume=50\n[private]\nvo=null\n"),(".config/heroic/config.json","{\"defaultSettings\":{\"customCSS\":\"body { width: 900px; }\"},\"games\":[\"secret\"]}")];
    for (p, t) in originals {
        h.write(p, t);
    }
    h.palette(json!(["lsd", "glow", "gimp", "mpv", "heroic"]));
    h.apply("next");
    for (p, old, new) in [
        (originals[0].0, "reverse: false", "reverse: true"),
        (originals[1].0, "width: 80", "width: 99"),
        (originals[2].0, "num-processors 3", "num-processors 8"),
        (originals[3].0, "volume=50", "volume=62"),
    ] {
        h.write(p, &h.text(p).replace(old, new));
    }
    let p = originals[4].0;
    let mut v: Value = serde_json::from_str(&h.text(p)).unwrap();
    let css = v["defaultSettings"]["customCSS"]
        .as_str()
        .unwrap()
        .to_owned()
        + "\n/* personal late edit */";
    v["defaultSettings"]["customCSS"] = css.into();
    v["games"] = json!(["later"]);
    h.write(p, &v.to_string());
    h.apply("neutral");
    assert_eq!(h.text(originals[0].0), "sorting:\n  reverse: true\n");
    assert_eq!(h.text(originals[1].0), "width: 99\n");
    assert!(h.text(originals[2].0).contains("num-processors 8"));
    assert!(!h.text(originals[2].0).contains("theme-color"));
    assert_eq!(h.text(originals[3].0), "volume=62\n[private]\nvo=null\n");
    let v: Value = serde_json::from_str(&h.text(p)).unwrap();
    assert_eq!(
        v["defaultSettings"]["customCSS"],
        "body { width: 900px; }\n/* personal late edit */"
    );
    assert_eq!(v["games"], json!(["later"]));
}
#[test]
fn conflicting_vim_loader_aborts_before_any_writes() {
    let h = Home::new();
    h.write(".vim/colors/kolira.vim", "let g:personal=1\n");
    h.palette(json!(["vim", "micro"]));
    let p = h.rpc("profile.preview", json!({"name":"next"}));
    assert_eq!(p["ok"], false, "{p}");
    assert!(!h.path(".config/micro/settings.json").exists());
    assert_eq!(h.text(".vim/colors/kolira.vim"), "let g:personal=1\n");
}
#[test]
fn snapshot_never_imports_foreign_vimscript() {
    let h = Home::new();
    h.write(
        ".config/kolira/themes/next/.vim/colors/kolira.vim",
        "call writefile(['bad'], '/tmp/never-kolira-vim')\n",
    );
    h.palette(json!(["vim"]));
    h.apply("next");
    let code = h.text(".vim/colors/kolira.vim");
    assert!(!code.contains("writefile"));
    assert!(code.contains("json_decode"));
}
#[test]
fn heroic_snapshot_contains_only_own_colours() {
    let p:kolira_core::palette::Palette=serde_json::from_value(json!({"name":"test","variant":"dark","base":["#111111","#222222","#333333","#777777","#bbbbbb","#eeeeee","#ffffff","#ffffff","#aa0000","#aa5500","#aaaa00","#00aa00","#00aaaa","#0000aa","#aa00aa","#555555"],"accent":"#00aaaa"})).unwrap();
    let own = kolira_core::third_apps::heroic(&p);
    let live="{\"defaultSettings\":{\"customCSS\":\"body { background: url(private); }\",\"private\":true},\"auth\":\"secret\"}";
    let merged = kolira_core::third_apps::heroic_merge(&own, live).unwrap();
    let capture = kolira_core::third_apps::heroic_capture(&merged).unwrap();
    assert!(!capture.contains("private"));
    assert!(!capture.contains("secret"));
    assert!(!capture.contains("url("));
    assert!(capture.contains("--accent"));
}

#[test]
fn malformed_or_executable_resources_fail_in_preview() {
    for (path, payload) in [
        (
            ".local/share/PrismLauncher/themes/kolira/theme.json",
            r##"{"name":"bad","widgets":"Fusion","qssFile":"other.qss","colors":{"Window":"#112233"}}"##,
        ),
        (
            ".config/GIMP/3.0/themes/Kolira/gimp.css",
            "@import url(https://example.org/external.css);\n* { font-size: 2px; }\n",
        ),
        (
            ".vim/kolira-palette.json",
            r##"{"name":"bad","variant":"dark","base":[],"accent":"#112233|call system('bad')"}"##,
        ),
    ] {
        let h = Home::new();
        h.write(&format!(".config/kolira/themes/next/{path}"), payload);
        // Ensure each app is configured even on machines without its binary.
        fs::create_dir_all(h.path(path).parent().unwrap()).unwrap();
        let p = h.rpc("profile.preview", json!({"name":"next"}));
        assert_eq!(p["ok"], false, "{path}: {p}");
        assert!(!h.path(path).exists(), "{path}");
    }
}
#[test]
fn libreoffice_is_preserved_even_in_an_old_snapshot() {
    let h = Home::new();
    let path = ".config/libreoffice/4/user/registrymodifications.xcu";
    h.write(path, "my standard Office settings");
    h.write(
        &format!(".config/kolira/themes/next/{path}"),
        "foreign appearance or preferences",
    );
    h.apply("next");
    h.apply("neutral");
    assert_eq!(h.text(path), "my standard Office settings");
    assert!(kolira_core::appearance::normalize(path, "foreign", false)
        .unwrap()
        .is_empty());
}
#[test]
fn empty_heroic_host_keeps_late_settings_without_leaving_own_block() {
    let h = Home::new();
    h.write(".config/heroic/config.json", "{}");
    h.palette(json!(["heroic"]));
    h.apply("next");
    let text = h.text(".config/heroic/config.json");
    let mut v: Value = serde_json::from_str(&text).unwrap();
    v["defaultSettings"]["language"] = json!("ru");
    v["games"] = json!(["personal"]);
    h.write(".config/heroic/config.json", &v.to_string());
    h.apply("neutral");
    let v: Value = serde_json::from_str(&h.text(".config/heroic/config.json")).unwrap();
    assert_eq!(v["defaultSettings"]["language"], "ru");
    assert!(v["defaultSettings"].get("customCSS").is_none());
    assert_eq!(v["games"], json!(["personal"]));
}
#[test]
fn yaml_indentation_and_nested_jsonc_insertion_preserve_personal_text() {
    let rel = ".config/lsd/config.yaml";
    let before = "color:\n    when: always\nsorting:\n    reverse: true\n";
    let after =
        kolira_core::appearance::applied(rel, "color:\n  theme: kolira\n", Some(before)).unwrap();
    assert!(after.contains("color:\n    theme: kolira\n    when: always\n"));
    assert_eq!(
        kolira_core::structured_appearance::restore(rel, before, &after, &after).unwrap(),
        before
    );
    let text =
        "{\n\"defaultSettings\":{ /* keep comment */ \"language\": \"ru\" },\"private\":true\n}";
    let patched = kolira_core::field::json_set(
        text,
        &["defaultSettings".into(), "customCSS".into()],
        &json!("palette"),
    )
    .unwrap();
    assert!(patched.contains("/* keep comment */ \"language\": \"ru\""));
    assert!(patched.contains("\"private\":true"));
}

#[test]
fn bundled_wallpaper_accent_metadata_stays_consistent() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../..");
    for name in [
        "Paper",
        "dracula",
        "everforest",
        "frutiger-aero",
        "frutiger-aero-light",
        "glass",
        "material-you",
        "minecraft-overworld-dark",
    ] {
        let root = repo.join("themes").join(name);
        let expected = matches!(name, "glass" | "material-you");
        let package: Value =
            serde_json::from_slice(&fs::read(root.join("theme.json")).unwrap()).unwrap();
        let manifest: Value =
            serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
        // The package author may choose Breeze, Kolira or MaterialBright.
        // Maintenance must preserve that choice and agree with kdeglobals;
        // the refresh example separately verifies preserving arbitrary Breeze choices.
        let scheme = package["values"]["color_scheme"].as_str().unwrap();
        assert!(!scheme.is_empty(), "{name}");
        assert_eq!(
            package["values"]["accent_from_wallpaper"],
            json!(expected),
            "{name}"
        );
        assert_eq!(manifest["accent_from_wallpaper"], json!(expected), "{name}");
        let text = fs::read_to_string(root.join(".config/kdeglobals")).unwrap();
        assert_eq!(
            kolira_core::field::ini_get(&text, "General", "ColorScheme").as_deref(),
            Some(scheme),
            "{name}"
        );
        let keys: Vec<_> = text
            .lines()
            .filter(|l| {
                l.split_once('=')
                    .is_some_and(|(k, _)| k.trim().eq_ignore_ascii_case("accentColorFromWallpaper"))
            })
            .collect();
        assert_eq!(
            keys,
            vec![format!("accentColorFromWallpaper={expected}").as_str()],
            "{name}"
        );
    }
}

#[test]
fn all_bundled_native_button_text_and_hover_pairs_keep_contrast() {
    use kolira_core::palette::{contrast, Palette, Rgb};
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../..");
    for dir in fs::read_dir(repo.join("themes"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("theme.json").is_file())
    {
        let v: Value = serde_json::from_slice(&fs::read(dir.join("theme.json")).unwrap()).unwrap();
        let p: Palette = serde_json::from_value(v["palette"].clone()).unwrap();
        let heroic: Value =
            serde_json::from_slice(&fs::read(dir.join(".config/heroic/config.json")).unwrap())
                .unwrap();
        let css = heroic["defaultSettings"]["customCSS"].as_str().unwrap();
        let color = |name: &str| {
            let key = format!("{name}: ");
            let text = css
                .split_once(&key)
                .unwrap()
                .1
                .split_whitespace()
                .next()
                .unwrap()
                .trim_end_matches(';');
            Rgb::parse(text).unwrap()
        };
        for (text, bg) in [
            ("--text-default", "--background"),
            ("--primary-button-text", "--primary-button"),
            ("--primary-button-text", "--primary-button-overlay"),
            ("--secondary-button-text", "--secondary-button"),
            ("--secondary-button-text", "--secondary-button-overlay"),
            ("--tertiary-button-text", "--tertiary-button"),
            ("--tertiary-button-text", "--tertiary-button-overlay"),
        ] {
            assert!(
                contrast(color(text), color(bg)) >= 4.5,
                "{} {text} on {bg}: {}",
                p.name,
                contrast(color(text), color(bg))
            );
        }
        let prism: Value = serde_json::from_slice(
            &fs::read(dir.join(".local/share/PrismLauncher/themes/kolira/theme.json")).unwrap(),
        )
        .unwrap();
        for (text, bg) in [
            ("WindowText", "Window"),
            ("Text", "Base"),
            ("ButtonText", "Button"),
            ("HighlightedText", "Highlight"),
        ] {
            assert!(
                contrast(
                    Rgb::parse(prism["colors"][text].as_str().unwrap()).unwrap(),
                    Rgb::parse(prism["colors"][bg].as_str().unwrap()).unwrap()
                ) >= 4.5,
                "{} Prism {text}/{bg}",
                p.name
            );
        }
    }
}
