//! Application appearance changes must be reversible across missing snapshots.
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
        for name in ["neutral", "next"] {
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
        fs::read_to_string(self.path(p))
            .unwrap_or_else(|e| panic!("{}: {e}", self.path(p).display()))
    }
    fn cmd(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_kolira"));
        c.args(args)
            .env("HOME", self.0.path())
            .env("XDG_CONFIG_HOME", self.path(".config"))
            .env("XDG_DATA_HOME", self.path(".local/share"))
            .env("XDG_CACHE_HOME", self.path(".cache"))
            .env("KTM_THEME_NO_APPLY", "1")
            .env("KTM_OFFLINE", "1")
            .env_remove("WALLPAPER_DIR")
            .env_remove("CODEX_HOME")
            .env_remove("CLAUDE_CONFIG_DIR")
            .env_remove("LG_CONFIG_FILE");
        c
    }
    fn rpc(&self, method: &str, args: Value) -> Value {
        let mut c = self
            .cmd(&["core", method])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        c.stdin
            .take()
            .unwrap()
            .write_all(args.to_string().as_bytes())
            .unwrap();
        let out = c.wait_with_output().unwrap();
        serde_json::from_slice(&out.stdout)
            .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&out.stderr)))
    }
    fn apply(&self, name: &str) {
        let p = self.rpc("profile.preview", json!({"name":name}));
        assert_eq!(p["ok"], true, "{p}");
        let r = self.rpc(
            "profile.apply",
            json!({"name":name,"preview_token":p["data"]["token"],"skip_dependencies":true}),
        );
        assert_eq!(r["ok"], true, "{r}");
    }
    fn package(&self, targets: Value) {
        let out = self
            .cmd(&["palette", "import", "gallery:nord"])
            .output()
            .unwrap();
        assert!(out.status.success());
        let palette: Value =
            serde_json::from_str(&self.text(".config/kolira/palette.json")).unwrap();
        self.write(".config/kolira/themes/next/theme.json",&json!({"schema":"kolira/theme@2","name":"next","palette":palette,"app_targets":targets}).to_string());
    }
}
#[test]
fn neovim_three_cycles_keep_init_and_remove_generated_files() {
    let h = Home::new();
    let init = "vim.g.personal = 'kept'\nvim.o.number = true\n";
    h.write(".config/nvim/init.lua", init);
    h.package(json!(["neovim"]));
    h.write(
        ".config/kolira/themes/next/.config/nvim/colors/kolira.lua",
        "error('theme-supplied Lua must never run')",
    );
    for _ in 0..3 {
        h.apply("next");
        assert_eq!(h.text(".config/nvim/init.lua"), init);
        if std::path::Path::new("/usr/bin/nvim").is_file() {
            let out=Command::new("/usr/bin/nvim").args(["--headless","-c","lua assert(vim.g.colors_name == 'kolira'); assert(vim.g.personal == 'kept'); assert(vim.o.number)","-c","qa!"]).env("HOME",h.0.path()).env("XDG_CONFIG_HOME",h.path(".config")).env("XDG_DATA_HOME",h.path(".local/share")).env("XDG_STATE_HOME",h.path(".local/state")).env("XDG_CACHE_HOME",h.path(".cache")).output().unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            assert!(
                !String::from_utf8_lossy(&out.stderr).contains("Error"),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        h.apply("neutral");
        for p in [
            ".config/nvim/kolira-palette.json",
            ".config/nvim/colors/kolira.lua",
            ".config/nvim/after/plugin/kolira.lua",
        ] {
            assert!(!h.path(p).exists(), "{p}");
        }
        assert_eq!(h.text(".config/nvim/init.lua"), init);
    }
}
#[test]
fn zen_changes_only_mode_not_ui_privacy_or_density() {
    let h = Home::new();
    h.write(
        ".zen/profiles.ini",
        "[Profile0]\nDefault=1\nPath=test.default\n",
    );
    let original="// personal\nuser_pref(\"privacy.resistFingerprinting\", true);\nuser_pref(\"browser.uidensity\", 1);\n";
    h.write(".zen/test.default/user.js", original);
    let css = "#sidebar { width: 20em; }\n";
    h.write(".zen/test.default/chrome/userChrome.css", css);
    h.write(".config/kolira/themes/next/.zen/author.default/user.js","user_pref(\"zen.view.window.scheme\", 1);\nuser_pref(\"browser.tabs.allow_transparent_browser\", true);\nuser_pref(\"privacy.resistFingerprinting\", false);\n");
    h.write(
        ".config/kolira/themes/next/.zen/author.default/chrome/userChrome.css",
        "* { fill: white !important; width: 1px; }\n",
    );
    h.apply("next");
    let js = h.text(".zen/test.default/user.js");
    assert!(js.contains("\"privacy.resistFingerprinting\", true"));
    assert!(js.contains("\"browser.uidensity\", 1"));
    assert!(!js.contains("allow_transparent"));
    assert!(js.contains("\"browser.theme.toolbar-theme\", 1"));
    assert!(js.contains("\"browser.theme.content-theme\", 1"));
    assert_eq!(h.text(".zen/test.default/chrome/userChrome.css"), css);
    h.apply("neutral");
    assert_eq!(h.text(".zen/test.default/user.js"), original);
    assert_eq!(h.text(".zen/test.default/chrome/userChrome.css"), css);
}
#[test]
fn profile_traversal_is_not_followed() {
    let h = Home::new();
    h.write(
        ".zen/profiles.ini",
        "[Profile0]\nDefault=1\nPath=../../private\n",
    );
    h.write("private/user.js", "secret\n");
    h.package(json!(["zen"]));
    h.apply("next");
    assert_eq!(h.text("private/user.js"), "secret\n");
    assert!(!h.path("private/chrome").exists());
}
#[test]
fn explicit_obsidian_vault_keeps_fonts_other_vaults_and_late_edits() {
    let h = Home::new();
    for name in ["one", "two"] {
        h.write(&format!("Documents/{name}/.obsidian/appearance.json"),"{\"cssTheme\":\"Mine\",\"theme\":\"moonstone\",\"baseFontSize\":17,\"enabledCssSnippets\":[\"personal\"]}");
    }
    h.write(".config/obsidian/obsidian.json",&json!({"vaults":{"a":{"path":h.path("Documents/one")},"b":{"path":h.path("Documents/two")}}}).to_string());
    h.package(json!(["obsidian"]));
    assert_eq!(
        h.rpc("profile.preview", json!({"name":"next"}))["ok"],
        false
    );
    let selected = h.rpc("app.select_vaults", json!({"vaults":["Documents/one"]}));
    assert_eq!(selected["ok"], true, "{selected}");
    let other = h.text("Documents/two/.obsidian/appearance.json");
    h.apply("next");
    let live = h.text("Documents/one/.obsidian/appearance.json");
    assert!(live.contains("\"baseFontSize\":17"));
    assert_eq!(h.text("Documents/two/.obsidian/appearance.json"), other);
    h.write(
        "Documents/one/.obsidian/appearance.json",
        &live.replace("\"baseFontSize\":17", "\"baseFontSize\":21"),
    );
    h.apply("neutral");
    let v: Value =
        serde_json::from_str(&h.text("Documents/one/.obsidian/appearance.json")).unwrap();
    assert_eq!(v["theme"], "moonstone");
    assert_eq!(v["baseFontSize"], 21);
    assert_eq!(v["enabledCssSnippets"], json!(["personal"]));
    assert!(!h
        .path("Documents/one/.obsidian/snippets/kolira.css")
        .exists());
}
#[test]
fn cursor_zed_kitty_konsole_return_exact_originals_three_times() {
    let h = Home::new();
    let files=[
 (".config/Cursor/User/settings.json","{ // personal\n\"workbench.colorTheme\":\"Mine\",\"editor.fontSize\":17}","{\"workbench.colorTheme\":\"Other\",\"editor.fontSize\":8}"),
 (".config/zed/settings.json","{\"theme\":\"Mine\",\"buffer_font_size\":17}","{\"theme\":\"Other\",\"buffer_font_size\":8}"),
 (".config/kitty/kitty.conf","font_size 17\nmap ctrl+a send_text all mine\n","background #123456\nfont_size 8\n"),
 (".local/share/konsole/Personal.profile","[General]\nName=Personal\nCommand=fish\n[Appearance]\nColorScheme=Mine\nFont=Hack,17\n","[Appearance]\nColorScheme=Other\nFont=Hack,8\n")];
    h.write(
        ".config/konsolerc",
        "[Desktop Entry]\nDefaultProfile=Personal.profile\n",
    );
    for (path, before, theme) in files {
        h.write(path, before);
        h.write(&format!(".config/kolira/themes/next/{path}"), theme);
    }
    for _ in 0..3 {
        h.apply("next");
        assert!(h.text(".config/kitty/kitty.conf").contains("font_size 17"));
        assert!(h
            .text(".local/share/konsole/Personal.profile")
            .contains("Command=fish"));
        h.apply("neutral");
        for (path, before, _) in files {
            assert_eq!(h.text(path), before, "{path}");
        }
    }
}
#[test]
fn flatpak_discord_preserves_unrelated_state() {
    let h = Home::new();
    let settings = ".var/app/dev.vencord.Vesktop/config/vesktop/settings/settings.json";
    let original = "{\"enabledThemes\":[\"mine.css\"],\"themeLinks\":[\"personal\"],\"zoom\":1.5}";
    h.write(settings, original);
    h.package(json!(["discord"]));
    h.apply("next");
    assert!(h.text(settings).contains("mine.css"));
    h.apply("neutral");
    assert_eq!(h.text(settings), original);
}

#[test]
fn cursor_cache_selection_registry_and_later_installs_roundtrip() {
    let h = Home::new();
    let original = "{ // personal\n\"workbench.colorTheme\":\"Mine\",\"editor.fontSize\":19}";
    h.write(".config/Cursor/User/settings.json", original);
    fs::create_dir_all(h.path(".cursor/extensions")).unwrap();
    let cache = "assets/editor-themes/cursor/example.night";
    h.write(&format!(".config/kolira/themes/next/{cache}/package.json"), &json!({"publisher":"example","name":"night","version":"1.0.0","contributes":{"themes":[{"id":"night-id","label":"Night","path":"themes/night.json"}]}}).to_string());
    h.write(
        &format!(".config/kolira/themes/next/{cache}/themes/night.json"),
        "{\"colors\":{}}",
    );
    h.write(".config/kolira/themes/next/theme.json", &json!({"schema":"kolira/theme@2","name":"next","values":{"cursor_ide_theme":"night-id"},"editor_themes":[{"editor":"cursor","id":"example.night","store":"local","local_path":cache,"native_watch":true}]}).to_string());
    h.apply("next");
    assert!(h
        .text(".config/Cursor/User/settings.json")
        .contains("// personal"));
    assert!(h
        .text(".config/Cursor/User/settings.json")
        .contains("\"night-id\""));
    let mut registry: Vec<Value> =
        serde_json::from_str(&h.text(".cursor/extensions/extensions.json")).unwrap();
    assert_eq!(
        registry[0]["identifier"]["id"],
        "kolira-cache.example-night"
    );
    // Saving our installed colour copy must not manufacture nested extension IDs
    // or marketplace dependencies on repeated save/apply cycles.
    for name in ["saved-one", "saved-two"] {
        let saved = h.cmd(&["save", name, "--no-wallpaper"]).output().unwrap();
        assert!(
            saved.status.success(),
            "{}",
            String::from_utf8_lossy(&saved.stderr)
        );
        let package: Value =
            serde_json::from_str(&h.text(&format!(".config/kolira/themes/{name}/theme.json")))
                .unwrap();
        assert_eq!(
            package["editor_themes"][0]["id"],
            "kolira-cache.example-night"
        );
        assert_eq!(package["editor_themes"][0]["store"], "local");
        assert_eq!(package["editor_themes"][0]["url"], "");
        h.apply(name);
        let entries: Vec<Value> =
            serde_json::from_str(&h.text(".cursor/extensions/extensions.json")).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["identifier"]["id"], "kolira-cache.example-night");
    }
    registry.push(json!({"identifier":{"id":"personal.extension"},"relativeLocation":"personal.extension-1.0"}));
    h.write(
        ".cursor/extensions/extensions.json",
        &serde_json::to_string(&registry).unwrap(),
    );
    h.apply("neutral");
    assert_eq!(h.text(".config/Cursor/User/settings.json"), original);
    let registry: Vec<Value> =
        serde_json::from_str(&h.text(".cursor/extensions/extensions.json")).unwrap();
    assert_eq!(registry.len(), 1);
    assert_eq!(registry[0]["identifier"]["id"], "personal.extension");
    assert!(!h
        .path(".cursor/extensions/kolira-cache.example-night-kolira")
        .exists());
}
#[test]
fn pywalfox_requires_baseline_and_returns_it_exactly() {
    let h = Home::new();
    h.package(json!(["pywalfox"]));
    assert_eq!(
        h.rpc("profile.preview", json!({"name":"next"}))["ok"],
        false
    );
    let original="{\"special\":{\"background\":\"#ffffff\",\"foreground\":\"#000000\"},\"colors\":{},\"personal\":42}";
    h.write(".cache/wal/colors.json", original);
    h.apply("next");
    assert!(h.text(".cache/wal/colors.json").contains("color15"));
    h.apply("neutral");
    assert_eq!(h.text(".cache/wal/colors.json"), original);
}
#[test]
fn obsidian_snapshot_uses_slots_and_maps_to_another_vault() {
    let h = Home::new();
    h.write(
        "Documents/Author/.obsidian/appearance.json",
        "{\"cssTheme\":\"\",\"theme\":\"obsidian\",\"baseFontSize\":17}",
    );
    h.write(
        ".config/obsidian/obsidian.json",
        &json!({"vaults":{"a":{"path":h.path("Documents/Author")}}}).to_string(),
    );
    let out = h.cmd(&["save", "portable"]).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let slot =
        ".config/kolira/themes/portable/.config/kolira/app-snapshots/obsidian/0/appearance.json";
    assert!(h.path(slot).is_file());
    assert!(!h.text(slot).contains("baseFontSize"));
    // The receiving machine selects a differently named vault.
    h.write(
        "Notes/Receiver/.obsidian/appearance.json",
        "{\"theme\":\"moonstone\",\"baseFontSize\":21}",
    );
    h.write(
        ".config/obsidian/obsidian.json",
        &json!({"vaults":{"a":{"path":h.path("Notes/Receiver")}}}).to_string(),
    );
    assert_eq!(
        h.rpc("app.select_vaults", json!({"vaults":["Notes/Receiver"]}))["ok"],
        true
    );
    h.apply("portable");
    let text = h.text("Notes/Receiver/.obsidian/appearance.json");
    assert!(text.contains("obsidian"));
    assert!(text.contains("21"));
    h.apply("neutral");
    assert_eq!(
        h.text("Notes/Receiver/.obsidian/appearance.json"),
        "{\"theme\":\"moonstone\",\"baseFontSize\":21}"
    );
}

#[test]
fn disabled_personal_browser_css_is_not_silently_activated() {
    let h = Home::new();
    h.write(
        ".zen/profiles.ini",
        "[Profile0]\nPath=test.default\nDefault=1\n",
    );
    h.write(
        ".zen/test.default/user.js",
        "user_pref(\"toolkit.legacyUserProfileCustomizations.stylesheets\", false);\n",
    );
    h.write(
        ".zen/test.default/chrome/userChrome.css",
        "* { display:none; }\n",
    );
    h.package(json!(["zen"]));
    let preview = h.rpc("profile.preview", json!({"name":"next"}));
    assert_eq!(preview["ok"], true, "{preview}");
    h.apply("next");
    assert_eq!(
        h.text(".zen/test.default/chrome/userChrome.css"),
        "* { display:none; }\n"
    );
    assert!(!h
        .path(".zen/test.default/chrome/kolira-colors.css")
        .exists());
    assert!(h.text(".zen/test.default/user.js").contains("false"));
}

#[test]
fn explicit_kitty_palette_keeps_font_and_bindings_then_returns_inline_colours() {
    let h = Home::new();
    let old = "font_size 19\nfont_family Hack\nbackground #eeeeee\nmap ctrl+a send_text all mine\n";
    h.write(".config/kitty/kitty.conf", old);
    h.package(json!(["kitty"]));
    h.apply("next");
    let host = h.text(".config/kitty/kitty.conf");
    assert!(host.contains("font_size 19") && host.contains("map ctrl+a"));
    assert!(h
        .text(".config/kitty/kolira.conf")
        .contains("font_family Hack"));
    h.apply("neutral");
    assert_eq!(h.text(".config/kitty/kitty.conf"), old);
    assert!(!h.path(".config/kitty/kolira.conf").exists());
}
#[test]
fn cached_zed_dark_light_and_mode_are_selected_without_reserializing_settings() {
    let h = Home::new();
    let old="{ // personal\n\"theme\":{\"mode\":\"system\",\"dark\":\"Mine\",\"light\":\"MyLight\"},\"buffer_font_size\":19}";
    h.write(".config/zed/settings.json", old);
    let cache = "assets/editor-themes/zed/example";
    h.write(&format!(".config/kolira/themes/next/{cache}/themes/night.json"),&json!({"name":"Example","themes":[{"name":"Night","appearance":"dark","style":{}},{"name":"Day","appearance":"light","style":{}}]}).to_string());
    h.write(".config/kolira/themes/next/theme.json",&json!({"schema":"kolira/theme@2","name":"next","values":{"zed_dark":"Night","zed_light":"Day","zed_mode":"dark"},"editor_themes":[{"editor":"zed","id":"example","store":"local","local_path":cache}]}).to_string());
    h.apply("next");
    let text = h.text(".config/zed/settings.json");
    assert!(text.contains("// personal") && text.contains("\"buffer_font_size\":19"));
    assert!(text.contains("Night") && text.contains("Day"));
    h.apply("neutral");
    assert_eq!(h.text(".config/zed/settings.json"), old);
    assert!(!h.path(".config/zed/themes/night.json").exists());
}

#[test]
fn portable_obsidian_zip_works_in_another_home_without_author_paths() {
    let author = Home::new();
    author.write(
        "Documents/Author/.obsidian/appearance.json",
        "{\"theme\":\"obsidian\",\"baseFontSize\":17}",
    );
    author.write(
        ".config/obsidian/obsidian.json",
        &json!({"vaults":{"a":{"path":author.path("Documents/Author")}}}).to_string(),
    );
    assert!(author
        .cmd(&["save", "portable"])
        .output()
        .unwrap()
        .status
        .success());
    let zip = author.path("portable.zip");
    let out = author
        .cmd(&["export", "portable", &format!("--out={}", zip.display())])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let receiver = Home::new();
    receiver.write(
        "Notes/Receiver/.obsidian/appearance.json",
        "{\"theme\":\"moonstone\",\"baseFontSize\":21}",
    );
    receiver.write(
        ".config/obsidian/obsidian.json",
        &json!({"vaults":{"b":{"path":receiver.path("Notes/Receiver")}}}).to_string(),
    );
    let out = receiver
        .cmd(&["import", zip.to_str().unwrap(), "--name=received"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    receiver.apply("received");
    let v: Value =
        serde_json::from_str(&receiver.text("Notes/Receiver/.obsidian/appearance.json")).unwrap();
    assert_eq!(v["theme"], "obsidian");
    assert_eq!(v["baseFontSize"], 21);
    assert!(!receiver.path("Documents/Author").exists());
    receiver.apply("neutral");
    assert_eq!(
        receiver.text("Notes/Receiver/.obsidian/appearance.json"),
        "{\"theme\":\"moonstone\",\"baseFontSize\":21}"
    );
}
#[test]
fn native_browser_snapshot_maps_into_flatpak_default_without_importing_ui() {
    let h = Home::new();
    let root = ".var/app/org.mozilla.firefox/.mozilla/firefox";
    h.write(
        &format!("{root}/profiles.ini"),
        "[Profile0]\nDefault=1\nPath=receiver.default\n",
    );
    h.write(
        &format!("{root}/receiver.default/user.js"),
        "user_pref(\"privacy.resistFingerprinting\", true);\n",
    );
    h.write(".config/kolira/themes/next/.mozilla/firefox/author.default/user.js","user_pref(\"browser.theme.toolbar-theme\", 0);\nuser_pref(\"privacy.resistFingerprinting\", false);\n");
    h.apply("next");
    let live = h.text(&format!("{root}/receiver.default/user.js"));
    assert!(live.contains("\"browser.theme.toolbar-theme\", 0"));
    assert!(live.contains("\"privacy.resistFingerprinting\", true"));
    h.apply("neutral");
    assert_eq!(
        h.text(&format!("{root}/receiver.default/user.js")),
        "user_pref(\"privacy.resistFingerprinting\", true);\n"
    );
}

#[test]
fn bundled_first_ten_packages_export_apply_and_return_without_network() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../..");
    let mut names = Vec::new();
    for entry in fs::read_dir(repo.join("themes")).unwrap().flatten() {
        if entry.path().join("theme.json").is_file() {
            names.push(entry.file_name().into_string().unwrap());
        }
    }
    names.sort();
    assert!(names.iter().any(|n| n == "material-you"));
    for name in names {
        let h = Home::new();
        let original = [
            (
                ".config/Code/User/settings.json",
                "{\"editor.fontSize\":18,\"workbench.colorTheme\":\"Mine\"}",
            ),
            (
                ".claude/settings.json",
                "{\"permissions\":{\"deny\":[\"Bash(*)\"]}}",
            ),
            (".codex/config.toml", "model = \"personal\"\n"),
            (".config/opencode/cli.json", "{\"model\":\"personal\"}"),
            (".config/oh-my-posh/kolira.omp.json", "{\"blocks\":[]}"),
            (".config/btop/btop.conf", "update_ms = 1900\n"),
            (".config/cava/config", "[input]\nmethod = pipewire\n"),
            (".config/yazi/theme.toml", "[icon]\nprepend_rules = []\n"),
            (".config/lazygit/config.yml", "gui:\n  mouseEvents: false\n"),
            (".config/zellij/config.kdl", "default_shell \"fish\"\n"),
            (".config/alacritty/alacritty.toml", "[font]\nsize = 17\n"),
            (
                ".config/ghostty/config",
                "font-size = 17\nkeybind = ctrl+a=new_tab\n",
            ),
            (
                ".config/Cursor/User/settings.json",
                "{ // personal\n\"editor.fontSize\":19,\"workbench.colorTheme\":\"Mine\"}",
            ),
            (
                ".config/zed/settings.json",
                "{\"buffer_font_size\":18,\"theme\":{\"mode\":\"dark\",\"dark\":\"Mine\"}}",
            ),
            (
                ".config/kitty/kitty.conf",
                "font_size 17\nmap ctrl+shift+x launch fish\n",
            ),
            (
                ".config/vesktop/settings/settings.json",
                "{\"zoom\":1.5,\"themeLinks\":[\"personal\"]}",
            ),
            (
                ".config/konsolerc",
                "[Desktop Entry]\nDefaultProfile=Personal.profile\n",
            ),
            (
                ".local/share/konsole/Personal.profile",
                "[General]\nName=Personal\nCommand=fish\n[Appearance]\nColorScheme=Mine\n",
            ),
            (
                ".mozilla/firefox/receiver.default/user.js",
                "user_pref(\"privacy.resistFingerprinting\", true);\n",
            ),
            (
                ".zen/receiver.default/user.js",
                "user_pref(\"browser.uidensity\", 1);\n",
            ),
            (
                "Documents/Notes/.obsidian/appearance.json",
                "{\"theme\":\"obsidian\",\"baseFontSize\":19}",
            ),
            (
                ".local/share/KotatogramDesktop/tdata/keep",
                "private encrypted state",
            ),
        ];
        for (path, text) in original {
            h.write(path, text);
        }
        h.write(
            ".mozilla/firefox/receiver.default/chrome/userChrome.css",
            "/* disabled personal skin */ #nav-bar { height: 80px; }\n",
        );
        h.write(
            ".config/obsidian/obsidian.json",
            &json!({"vaults":{"mine":{"path":h.path("Documents/Notes")}}}).to_string(),
        );
        for root in [".zen", ".mozilla/firefox"] {
            h.write(
                &format!("{root}/profiles.ini"),
                "[Profile0]\nDefault=1\nPath=receiver.default\n",
            );
        }
        let source = repo.join("themes").join(&name);
        let imported = h
            .cmd(&["import", source.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(
            imported.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&imported.stderr)
        );
        let package: Value =
            serde_json::from_str(&h.text(&format!(".config/kolira/themes/{name}/theme.json")))
                .unwrap();
        for id in kolira_core::third_apps::TARGETS {
            assert!(package["app_targets"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v == id));
        }
        assert!(!package["app_targets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "libreoffice"));
        assert!(!package["app_targets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "pywalfox"));
        // Export while the receiver's profile name differs from the snapshot.
        let zip = h.path("package.zip");
        let exported = h
            .cmd(&["export", &name, &format!("--out={}", zip.display())])
            .output()
            .unwrap();
        assert!(
            exported.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&exported.stderr)
        );
        let other = Home::new();
        let imported = other
            .cmd(&["import", zip.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(imported.status.success());
        assert!(other
            .path(&format!(
                ".config/kolira/themes/{name}/.zen/kolira.default/user.js"
            ))
            .is_file());
        assert!(other
            .path(&format!(
                ".config/kolira/themes/{name}/.mozilla/firefox/kolira.default/user.js"
            ))
            .is_file());
        h.apply(&name);
        assert_palette_resources(&h, &name, &package);
        assert!(
            h.text(".config/Cursor/User/settings.json")
                .contains("\"editor.fontSize\":19"),
            "{name}"
        );
        assert!(
            h.text(".config/zed/settings.json")
                .contains("\"buffer_font_size\":18"),
            "{name}"
        );
        assert!(
            h.text(".config/kitty/kitty.conf").contains("font_size 17"),
            "{name}"
        );
        assert!(
            h.text(".local/share/konsole/Personal.profile")
                .contains("Command=fish"),
            "{name}"
        );
        assert!(
            !h.path(".zen/receiver.default/chrome/kolira-colors.css")
                .exists(),
            "{name}"
        );
        assert!(
            !h.path(".mozilla/firefox/receiver.default/chrome/kolira-colors.css")
                .exists(),
            "{name}"
        );
        let firefox = h.text(".mozilla/firefox/receiver.default/user.js");
        assert!(firefox.contains("browser.theme.toolbar-theme"), "{name}");
        assert!(
            !firefox.contains("legacyUserProfileCustomizations.stylesheets"),
            "{name}"
        );
        assert_eq!(
            h.text(".mozilla/firefox/receiver.default/chrome/userChrome.css"),
            "/* disabled personal skin */ #nav-bar { height: 80px; }\n"
        );
        assert!(
            h.text("Documents/Notes/.obsidian/appearance.json")
                .contains("\"baseFontSize\":19"),
            "{name}"
        );
        assert!(h.path(".config/nvim/colors/kolira.lua").is_file(), "{name}");
        h.apply("neutral");
        for (path, text) in original {
            assert_eq!(h.text(path), text, "{name}: {path}");
        }
        assert!(
            !h.path(".zen/receiver.default/chrome/kolira-colors.css")
                .exists(),
            "{name}"
        );
        assert!(!h.path(".config/nvim/colors/kolira.lua").exists(), "{name}");
    }
}

fn pref(text: &str, key: &str) -> Option<Value> {
    let prefix = format!("user_pref(\"{key}\", ");
    text.lines().find_map(|line| {
        line.trim()
            .strip_prefix(&prefix)
            .and_then(|s| s.strip_suffix(");"))
            .and_then(|s| serde_json::from_str(s).ok())
    })
}

fn assert_palette_resources(h: &Home, name: &str, package: &Value) {
    use kolira_core::{
        field,
        palette::{kde_roles, Palette},
    };
    let p: Palette = serde_json::from_value(package["palette"].clone()).unwrap();
    let r = kde_roles(&p);
    for root in [".zen", ".mozilla/firefox"] {
        let js = h.text(&format!("{root}/receiver.default/user.js"));
        assert_eq!(
            pref(&js, "ui.accentcolor"),
            Some(json!(p.accent)),
            "{name} {root}"
        );
        assert_eq!(
            pref(&js, "ui.accentcolortext"),
            Some(json!(r.selection_text.hex())),
            "{name} {root}"
        );
        assert!(!js.contains("legacyUserProfileCustomizations"), "{name}");
        if root == ".zen" {
            assert_eq!(
                pref(&js, "zen.theme.accent-color"),
                Some(json!(p.accent)),
                "{name}"
            );
        }
    }
    for gtk in ["gtk-3.0", "gtk-4.0"] {
        let path = h.path(&format!(".config/{gtk}/colors.css"));
        if path.is_file() {
            let text = fs::read_to_string(path).unwrap();
            let accent = if text.contains("theme_selected_bg_color_breeze") {
                "theme_selected_bg_color_breeze"
            } else {
                "theme_selected_bg_color"
            };
            assert!(
                text.contains(&format!("@define-color {accent} {};", p.accent)),
                "{name} {gtk}"
            );
            assert!(!text.contains("#943147"), "{name} stale red GTK selection");
        }
    }
    let global = h.text(".config/kdeglobals");
    assert_eq!(
        field::ini_get(&global, "General", "AccentColor"),
        Some(p.accent_rgb().kde()),
        "{name}"
    );
    assert_eq!(
        field::ini_get(&global, "Colors:Selection", "BackgroundNormal"),
        Some(p.accent_rgb().kde()),
        "{name}"
    );
    assert!(
        h.text(".config/kitty/kolira.conf")
            .contains(&format!("active_border_color {}", p.accent)),
        "{name} kitty"
    );
    let alacritty: toml_edit::DocumentMut =
        h.text(".config/alacritty/kolira.toml").parse().unwrap();
    assert_eq!(
        alacritty["colors"]["selection"]["background"].as_str(),
        Some(p.accent.as_str()),
        "{name} alacritty"
    );
    assert!(
        h.text(".config/fish/fish_variables").contains(&format!(
            "fish_color_command:{}",
            p.base[13].trim_start_matches('#')
        )),
        "{name} fish"
    );
    assert!(
        h.text(".config/fzf/kolira.opts")
            .contains(&format!("pointer:{}", p.accent)),
        "{name} fzf"
    );
    assert!(
        h.text(".config/lazygit/config.yml")
            .contains(&format!("activeBorderColor: [\"{}\"]", p.accent)),
        "{name} lazygit"
    );
    let yazi: toml_edit::DocumentMut = h.text(".config/yazi/theme.toml").parse().unwrap();
    assert_eq!(
        yazi["mgr"]["cwd"]["fg"].as_str(),
        Some(p.base[13].as_str()),
        "{name} yazi"
    );
    assert!(
        h.text(".config/btop/themes/kolira.theme")
            .contains(&format!("theme[main_bg]=\"{}\"", p.base[0])),
        "{name} btop"
    );
    let cava = h.text(".config/cava/config");
    assert_eq!(
        field::ini_get(&cava, "color", "foreground"),
        Some(format!("'{}'", p.accent)),
        "{name} cava"
    );
    for rel in [
        ".config/bat/themes/kolira.tmTheme",
        ".codex/themes/kolira.tmTheme",
    ] {
        let xml = h.text(rel);
        assert!(
            xml.contains(&format!(
                "<key>background</key><string>{}</string>",
                p.base[0]
            )),
            "{name} {rel}"
        );
    }
    let zellij = h.text(".config/zellij/themes/kolira.kdl");
    assert!(
        zellij.contains(&format!("background \"{}\"", p.base[0])),
        "{name} zellij"
    );
    let ai: Value = serde_json::from_str(&h.text(".config/opencode/cli.json")).unwrap();
    assert_eq!(ai["theme"]["name"], "system", "{name} opencode");
    let ai: Value = serde_json::from_str(&h.text(".claude/settings.json")).unwrap();
    assert_eq!(
        ai["theme"],
        if p.variant == kolira_core::palette::Variant::Dark {
            "dark-ansi"
        } else {
            "light-ansi"
        },
        "{name} Claude"
    );
    let codex: toml_edit::DocumentMut = h.text(".codex/config.toml").parse().unwrap();
    assert_eq!(
        codex["tui"]["theme"].as_str(),
        Some("kolira"),
        "{name} Codex selector"
    );
    assert!(
        h.text(".config/wofi/kolira.css").contains(&p.base[0]),
        "{name} Wofi"
    );
    let vscode = h.text(".config/Code/User/settings.json");
    assert_eq!(
        field::json_get(
            &vscode,
            &["workbench.colorCustomizations".into(), "focusBorder".into()]
        ),
        Some(json!(p.accent)),
        "{name} VS Code"
    );
    let cursor = h.text(".config/Cursor/User/settings.json");
    assert_eq!(
        field::json_get(
            &cursor,
            &["workbench.colorCustomizations".into(), "focusBorder".into()]
        ),
        Some(json!(p.accent)),
        "{name} Cursor"
    );
    let zed: Value =
        serde_json::from_str(&h.text(&format!(".config/zed/themes/kolira-{name}.json"))).unwrap();
    assert_eq!(
        zed["themes"][0]["style"]["text.accent"], p.accent,
        "{name} Zed"
    );
    let obsidian = h.text("Documents/Notes/.obsidian/snippets/kolira.css");
    assert!(obsidian.contains(&p.accent), "{name} Obsidian");
    let selected: Value =
        serde_json::from_str(&h.text(".config/vesktop/settings/settings.json")).unwrap();
    assert!(
        selected["enabledThemes"]
            .as_array()
            .is_some_and(|v| v.contains(&json!("kolira.theme.css"))),
        "{name} Discord selection"
    );
    assert!(
        h.text(".config/wofi/style.css")
            .contains("@import url(\"kolira.css\");"),
        "{name} Wofi selection"
    );
    assert!(
        h.text(".config/btop/btop.conf")
            .contains("color_theme = \"kolira\""),
        "{name} btop selection"
    );
    assert!(
        h.text(".config/bat/config").contains("--theme=kolira"),
        "{name} bat selection"
    );
    let zed_selection: Value = serde_json::from_str(&h.text(".config/zed/settings.json")).unwrap();
    assert_eq!(
        zed_selection["theme"]["dark"],
        format!("Kolira {name}"),
        "{name} Zed selection"
    );
    let discord = h.text(".config/vesktop/themes/kolira.theme.css");
    assert!(
        discord.contains(&format!("--accent-2: {};", p.accent)),
        "{name} Discord"
    );
    let ghostty = h.text(".config/ghostty/kolira");
    assert!(ghostty.contains(&p.accent), "{name} Ghostty");
    let rc = h.text(".config/konsolerc");
    let selected = field::ini_get(&rc, "Desktop Entry", "DefaultProfile").unwrap();
    let profile = h.text(&format!(".local/share/konsole/{selected}"));
    let scheme = field::ini_get(&profile, "Appearance", "ColorScheme").unwrap();
    let text = h.text(&format!(".local/share/konsole/{scheme}.colorscheme"));
    assert_eq!(
        field::ini_get(&text, "Background", "Color"),
        Some(r.window.kde()),
        "{name} Konsole"
    );
    let nvim: Value = serde_json::from_str(&h.text(".config/nvim/kolira-palette.json")).unwrap();
    assert_eq!(nvim["accent"], p.accent, "{name} Neovim");
    let omp = h.text(".config/oh-my-posh/kolira.omp.json");
    let allowed: Vec<_> = p
        .base
        .iter()
        .map(|s| s.to_lowercase())
        .chain([p.accent.to_lowercase(), "#000000".into(), "#ffffff".into()])
        .collect();
    for (i, byte) in omp.bytes().enumerate() {
        if byte == b'#'
            && i + 7 <= omp.len()
            && omp.as_bytes()[i + 1..i + 7]
                .iter()
                .all(u8::is_ascii_hexdigit)
        {
            assert!(
                allowed.contains(&omp[i..i + 7].to_lowercase()),
                "{name} OMP stale {}",
                &omp[i..i + 7]
            );
        }
    }
}

#[test]
fn gtk_stale_red_is_replaced_without_changing_alpha_or_widget_rules() {
    let p =
        kolira_core::palette::parse_base16(include_str!("../data/base16/everforest.yaml"), "test")
            .unwrap();
    let old = "@define-color theme_button_foreground_active_breeze #ffffff;\n@define-color theme_selected_bg_color_breeze #943147;\n@define-color window_bg_color alpha(#1f1115, 0.70);\nbutton { border-radius: 8px; }\n";
    let new = kolira_core::extras::gtk_colors_with_palette(old, &p);
    assert!(new.contains(&format!(
        "@define-color theme_selected_bg_color_breeze {};",
        p.accent
    )));
    assert!(new.contains(&format!("alpha({}, 0.70)", p.base[0])));
    assert!(new.contains("button { border-radius: 8px; }"));
    assert!(!new.contains("#943147"));
    assert!(new.contains(&format!(
        "@define-color theme_button_foreground_active_breeze {};",
        kolira_core::palette::kde_roles(&p).selection_text.hex()
    )));
    assert_eq!(kolira_core::extras::gtk_colors_with_palette(&new, &p), new);
}

#[test]
fn retired_apps_are_ignored_in_legacy_themes_and_undo_journals() {
    use sha2::{Digest, Sha256};
    let h = Home::new();
    let rels = [
        ".config/chromium/Default/Preferences",
        ".config/google-chrome/kolira-theme/manifest.json",
        ".var/app/org.chromium.Chromium/config/chromium/Default/Preferences",
        ".var/app/com.google.Chrome/config/google-chrome/Default/Preferences",
        ".config/telegram-desktop/settings.json",
        ".config/kotatogram-desktop/settings.json",
        ".local/share/TelegramDesktop/kolira.tdesktop-palette",
        ".local/share/KotatogramDesktop/kolira.tdesktop-palette",
        ".var/app/org.telegram.desktop/data/TelegramDesktop/tdata/personal",
    ];
    h.package(json!([
        "telegram",
        "kotatogram",
        "chromium",
        "chrome",
        "google-chrome"
    ]));
    for rel in rels {
        h.write(rel, "personal client data\n");
        h.write(
            &format!(".config/kolira/themes/next/{rel}"),
            "stale theme\n",
        );
    }
    // Import a checksummed pre-retirement package, not just an already-loaded theme.
    let source = h.path(".config/kolira/themes/next");
    let mut files = serde_json::Map::new();
    for rel in std::iter::once("theme.json").chain(rels) {
        files.insert(
            rel.into(),
            json!(format!(
                "{:x}",
                Sha256::digest(fs::read(source.join(rel)).unwrap())
            )),
        );
    }
    h.write(
        ".config/kolira/themes/next/export.json",
        &json!({
            "format":"kolira/export@1", "theme":"next", "exported":"2026-11-16",
            "generator":"legacy", "home_token":"$HOME", "files":files
        })
        .to_string(),
    );
    let imported = h
        .cmd(&["import", source.to_str().unwrap(), "--name=legacy-import"])
        .output()
        .unwrap();
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    for rel in rels {
        assert!(!h
            .path(&format!(".config/kolira/themes/legacy-import/{rel}"))
            .exists());
    }
    let outdir = h.path("retired-export");
    let exported = h
        .cmd(&["export", "next", &format!("--dir={}", outdir.display())])
        .output()
        .unwrap();
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    for rel in rels {
        assert!(!outdir.join(rel).exists());
    }
    let exported_package: Value =
        serde_json::from_slice(&fs::read(outdir.join("theme.json")).unwrap()).unwrap();
    assert_eq!(exported_package["app_targets"], json!([]));
    let hash = format!("{:x}", Sha256::digest(b"personal client data\n"));
    let mut created = serde_json::Map::new();
    let mut replaced = serde_json::Map::new();
    let mut changes = serde_json::Map::new();
    // Exercise all historical journal formats. Missing blobs must not be read
    // for excluded clients: undo no longer owns these files or their parents.
    for rel in rels {
        created.insert(rel.into(), json!(hash));
        replaced.insert(rel.into(), json!([hash, "missing-old-blob"]));
        changes.insert(
            rel.into(),
            json!({
                "before":{"kind":"missing"},
                "after":{"kind":"file","blob":hash,"mode":384}
            }),
        );
    }
    h.write(
        ".config/kolira/apply-journal/journal.json",
        &json!({
            "created":created, "replaced":replaced, "changes":changes
        })
        .to_string(),
    );
    for name in ["next", "neutral", "legacy-import"] {
        let preview = h.rpc("profile.preview", json!({"name":name}));
        assert_eq!(preview["ok"], true, "{preview}");
        h.apply(name);
        for rel in rels {
            assert_eq!(h.text(rel), "personal client data\n", "{name}: {rel}");
        }
    }
    assert!(kolira_core::tracked_apps::get_tracked_apps()
        .iter()
        .all(|app| !matches!(
            app.id.as_str(),
            "telegram" | "kotatogram" | "chromium" | "chrome" | "google-chrome"
        )));
    assert!(!kolira_core::palette::TARGETS.contains(&"telegram"));
    assert!(!kolira_core::palette::DEFAULT_TARGETS.contains(&"telegram"));
    let result = h
        .cmd(&["palette", "apply", "--targets=telegram"])
        .output()
        .unwrap();
    assert!(!result.status.success());
}
