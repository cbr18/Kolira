//! P0 rows 11–20: shared preview/apply/restore, no host configuration mutations.
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
fn all_second_batch_hosts_return_exactly_three_cycles() {
    let h = Home::new();
    let originals=[
 (".config/fish/fish_variables","# personal\nSETUVAR fish_user_paths:/my/bin\nSETUVAR fish_color_command:123456\nSETUVAR pure_color_primary:abcdef\nSETUVAR pure_show_jobs:true\n"),
 (".config/btop/btop.conf","# personal\ncolor_theme = \"Mine\"\nupdate_ms = 1900\n"),
 (".config/cava/config","[general]\nframerate=30\n[input]\nmethod=pipewire\n[color]\nforeground='#abcdef'\n"),
 (".config/yazi/theme.toml","# my filetypes\n[mgr]\ncwd = { fg = \"#123456\" }\n[icon]\nprepend_rules = [{ name=\"*.private\", text=\"P\" }]\n"),
 (".config/bat/config","--paging=never\n--theme=Mine\n"),
 (".config/wofi/style.css","/* personal geometry */\nwindow { width: 500px; }\n"),
 (".config/lazygit/config.yml","# keep\ngui:\n  mouseEvents: false\n  theme:\n    activeBorderColor: [\"red\"]\ngit:\n  paging:\n    pager: delta\ncustomCommands: []\n"),
 (".config/zellij/config.kdl","// personal\nmouse_mode false\ntheme \"mine\" // note\nkeybinds { normal { bind \"Alt n\" { NewPane; }; }; }\n"),
 (".config/opencode/cli.json","{\"theme\":{\"name\":\"mine\",\"mode\":\"system\"},\"keybinds\":{\"leader\":\"ctrl+x\"}}"),
 (".claude/settings.json","{\"theme\":\"light\",\"permissions\":{\"allow\":[\"Read\"]},\"model\":\"personal\"}"),
 (".codex/config.toml","# personal\nmodel = \"personal\"\napproval_policy = \"on-request\"\n[tui]\ntheme = \"mine\" # keep\nnotifications = false\n[mcp_servers.personal]\ncommand = \"my-server\"\n"),
 ];
    for (path, text) in originals {
        h.write(path, text);
    }
    h.write(
        ".config/opencode/opencode.json",
        "{\"provider\":\"private\"}",
    );
    h.write(".codex/auth.json", "private-credentials");
    h.palette(json!([
        "fish",
        "pure",
        "btop",
        "cava",
        "yazi",
        "bat",
        "wofi",
        "fzf",
        "lazygit",
        "zellij",
        "opencode",
        "claude-code",
        "codex"
    ]));
    for _ in 0..3 {
        h.apply("next");
        assert!(h
            .text(".config/fish/fish_variables")
            .contains("fish_user_paths:/my/bin"));
        assert!(h
            .text(".config/fish/fish_variables")
            .contains("pure_show_jobs:true"));
        assert!(h
            .text(".config/btop/btop.conf")
            .contains("update_ms = 1900"));
        assert!(h.text(".config/cava/config").contains("method=pipewire"));
        assert!(h.text(".config/yazi/theme.toml").contains("*.private"));
        assert!(h.text(".config/bat/config").contains("--paging=never"));
        assert!(h.text(".config/wofi/style.css").contains("width: 500px"));
        assert!(h
            .text(".config/lazygit/config.yml")
            .contains("pager: delta"));
        assert!(h.text(".config/zellij/config.kdl").contains("NewPane"));
        assert_eq!(
            h.text(".config/opencode/opencode.json"),
            "{\"provider\":\"private\"}"
        );
        assert_eq!(h.text(".codex/auth.json"), "private-credentials");
        assert!(h.text(".claude/settings.json").contains("personal"));
        assert!(h.text(".codex/config.toml").contains("my-server"));
        assert!(h.path(".config/fish/conf.d/kolira-fzf.fish").is_file());
        h.apply("neutral");
        for (path, text) in originals {
            assert_eq!(h.text(path), text, "{path}");
        }
        for path in [
            ".config/fish/conf.d/kolira-fzf.fish",
            ".config/fzf/kolira.opts",
            ".config/wofi/kolira.css",
            ".config/btop/themes/kolira.theme",
            ".codex/themes/kolira.tmTheme",
        ] {
            assert!(!h.path(path).exists(), "{path}");
        }
    }
}
#[test]
fn unsupported_yaml_fails_before_any_theme_writes() {
    let h = Home::new();
    h.write(
        ".config/lazygit/config.yml",
        "gui: { theme: { activeBorderColor: [red] } }\n",
    );
    h.palette(json!(["lazygit", "fish"]));
    let p = h.rpc("profile.preview", json!({"name":"next"}));
    assert_eq!(p["ok"], false, "{p}");
    assert!(!h.path(".config/fish/fish_variables").exists());
    assert!(h.text(".config/lazygit/config.yml").contains("gui: {"));
}
#[test]
fn late_ai_settings_and_yaml_kdl_toml_edits_survive_return() {
    let h = Home::new();
    h.write(
        ".claude/settings.json",
        "{\"theme\":\"dark-daltonized\",\"model\":\"first\"}",
    );
    h.write(".codex/config.toml", "model = \"first\"\n");
    h.write(".config/lazygit/config.yml", "gui:\n  mouseEvents: false\n");
    h.write(".config/zellij/config.kdl", "mouse_mode false\n");
    h.palette(json!(["claude-code", "codex", "lazygit", "zellij"]));
    h.apply("next");
    assert!(h.text(".claude/settings.json").contains("dark-daltonized"));
    h.write(
        ".claude/settings.json",
        &h.text(".claude/settings.json").replace("first", "later"),
    );
    h.write(
        ".codex/config.toml",
        &h.text(".codex/config.toml").replace("first", "later"),
    );
    h.write(
        ".config/lazygit/config.yml",
        &h.text(".config/lazygit/config.yml")
            .replace("mouseEvents: false", "mouseEvents: true"),
    );
    h.write(
        ".config/zellij/config.kdl",
        &h.text(".config/zellij/config.kdl")
            .replace("mouse_mode false", "mouse_mode true"),
    );
    h.apply("neutral");
    assert!(h.text(".claude/settings.json").contains("later"));
    assert!(h.text(".codex/config.toml").contains("later"));
    assert!(!h.text(".codex/config.toml").contains("kolira"));
    assert!(h
        .text(".config/lazygit/config.yml")
        .contains("mouseEvents: true"));
    assert!(!h
        .text(".config/lazygit/config.yml")
        .contains("activeBorderColor"));
    assert!(h
        .text(".config/zellij/config.kdl")
        .contains("mouse_mode true"));
    assert!(!h.text(".config/zellij/config.kdl").contains("kolira"));
}
#[test]
fn fzf_snapshot_cannot_install_bind_execute_or_theme_supplied_shell_code() {
    let h = Home::new();
    h.write(
        ".config/kolira/themes/next/.config/fzf/kolira.opts",
        "--color=fg:#ffffff --bind=enter:execute(bad)\n",
    );
    h.write(
        ".config/kolira/themes/next/.config/fish/conf.d/kolira-fzf.fish",
        "touch /tmp/should-not-run\n",
    );
    let p = h.rpc("profile.preview", json!({"name":"next"}));
    assert_eq!(p["ok"], false, "{p}");
    assert!(!h.path(".config/fish/conf.d/kolira-fzf.fish").exists());
}

fn native(h: &Home, binary: &str, args: &[&str]) -> Command {
    let mut c = Command::new(binary);
    c.args(args)
        .current_dir(h.0.path())
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("HOME", h.0.path())
        .env("XDG_CONFIG_HOME", h.path(".config"))
        .env("XDG_DATA_HOME", h.path(".local/share"))
        .env("XDG_CACHE_HOME", h.path(".cache"))
        .env("TERM", "xterm-256color")
        .env("COLORTERM", "truecolor");
    c
}
#[test]
fn real_fish_and_fzf_use_colours_without_losing_personal_options() {
    if which::which("fish").is_err() || which::which("fzf").is_err() {
        return;
    }
    let h = Home::new();
    h.palette(json!(["fish", "pure", "fzf"]));
    h.apply("next");
    let out = native(
        &h,
        "fish",
        &[
            "-c",
            "printf '%s|%s' $fish_color_command $pure_color_primary",
        ],
    )
    .output()
    .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("81a1c1|81a1c1"),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let source = h.path(".config/fish/conf.d/kolira-fzf.fish");
    let script=format!("source '{}'; set -l first \"$FZF_DEFAULT_OPTS\"; source '{}'; test \"$first\" = \"$FZF_DEFAULT_OPTS\"; or exit 7; printf '%s' \"$FZF_DEFAULT_OPTS\"",source.display(),source.display());
    let out = native(&h, "fish", &["--no-config", "--interactive", "-c", &script])
        .env("FZF_DEFAULT_OPTS", "--height=40% --bind=ctrl-g:abort")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let opts = String::from_utf8(out.stdout).unwrap();
    assert!(opts.contains("--height=40% --bind=ctrl-g:abort"));
    assert_eq!(opts.matches("--color=").count(), 1);
    let mut child = native(&h, "fzf", &["--filter=needle"])
        .env("FZF_DEFAULT_OPTS", opts)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"needle\nother\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"needle\n");
}
#[test]
fn real_bat_accepts_the_same_tmtheme_format_used_for_codex() {
    if which::which("bat").is_err() {
        return;
    }
    let h = Home::new();
    h.palette(json!(["bat", "codex"]));
    h.apply("next");
    let out = native(&h, "bat", &["cache", "--build"]).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = native(&h, "bat", &["--list-themes", "--color=never"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("kolira"));
    assert_eq!(
        h.text(".config/bat/themes/kolira.tmTheme"),
        h.text(".codex/themes/kolira.tmTheme")
    );
}
#[test]
fn omp_changes_only_existing_activation_and_preserves_prompt_structure() {
    let h = Home::new();
    let theme = json!({"version":3,"blocks":[{"type":"prompt","alignment":"left","segments":[{"type":"text","style":"plain","foreground":"#ffffff","background":"#000000","template":"hello"}]}]});
    h.write(".config/oh-my-posh/personal.json", &theme.to_string());
    let host = format!(
        "# keep personal shell\noh-my-posh init fish --config '{}' | source\nset -g personal yes\n",
        h.path(".config/oh-my-posh/personal.json").display()
    );
    h.write(".config/fish/config.fish", &host);
    h.palette(json!(["oh-my-posh"]));
    h.apply("next");
    let after = h.text(".config/fish/config.fish");
    assert!(after.contains("personal yes"));
    assert!(after.contains("kolira.omp.json"));
    let generated: Value =
        serde_json::from_str(&h.text(".config/oh-my-posh/kolira.omp.json")).unwrap();
    assert_eq!(generated["blocks"][0]["segments"][0]["template"], "hello");
    assert_eq!(
        h.text(".config/oh-my-posh/personal.json"),
        theme.to_string()
    );
    h.apply("neutral");
    assert_eq!(h.text(".config/fish/config.fish"), host);
    assert!(!h.path(".config/oh-my-posh/kolira.omp.json").exists());
    let h = Home::new();
    h.write(".config/fish/config.fish", "set -g personal yes\n");
    h.palette(json!(["oh-my-posh"]));
    h.apply("next");
    assert_eq!(h.text(".config/fish/config.fish"), "set -g personal yes\n");
}
#[test]
fn save_export_import_never_carries_ai_auth_models_permissions_or_hooks() {
    let h = Home::new();
    h.write(".claude/settings.json","{\"theme\":\"dark-ansi\",\"model\":\"never-export-model\",\"permissions\":{\"allow\":[\"never-export-permission\"]},\"hooks\":{\"secret\":\"never-export-hook\"}}");
    h.write(".codex/config.toml","model = 'never-export-model'\napi_key = 'never-export-token'\n[tui]\ntheme='kolira'\n[mcp_servers.personal]\ncommand='never-export-command'\n");
    h.write(".codex/auth.json", "never-export-auth");
    let saved = h
        .command(&["save", "clean", "--no-wallpaper"])
        .output()
        .unwrap();
    assert!(
        saved.status.success(),
        "{}",
        String::from_utf8_lossy(&saved.stderr)
    );
    for rel in [".claude/settings.json", ".codex/config.toml"] {
        let data = h.text(&format!(".config/kolira/themes/clean/{rel}"));
        assert!(!data.contains("never-export"), "{data}");
        assert!(data.contains("theme"));
    }
    assert!(!h
        .path(".config/kolira/themes/clean/.codex/auth.json")
        .exists());
    let outdir = h.path("export");
    let out = h
        .command(&["export", "clean", &format!("--dir={}", outdir.display())])
        .output()
        .unwrap();
    assert!(out.status.success());
    let other = Home::new();
    let out = other
        .command(&["import", outdir.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(!other
        .path(".config/kolira/themes/clean/.codex/auth.json")
        .exists());
}
#[test]
fn owned_fzf_hook_collision_and_symlink_hosts_fail_without_writes() {
    use std::os::unix::fs::symlink;
    let h = Home::new();
    h.write(".config/fish/conf.d/kolira-fzf.fish", "# personal hook\n");
    h.palette(json!(["fzf"]));
    assert_eq!(
        h.rpc("profile.preview", json!({"name":"next"}))["ok"],
        false
    );
    assert!(!h.path(".config/fzf/kolira.opts").exists());
    let h = Home::new();
    h.write("outside/config.yml", "gui:\n  mouseEvents: false\n");
    fs::create_dir_all(h.path(".config")).unwrap();
    symlink(h.path("outside"), h.path(".config/lazygit")).unwrap();
    h.palette(json!(["lazygit"]));
    assert_eq!(
        h.rpc("profile.preview", json!({"name":"next"}))["ok"],
        false
    );
    assert_eq!(h.text("outside/config.yml"), "gui:\n  mouseEvents: false\n");
}

#[test]
fn custom_ai_roots_and_cached_bat_baseline_return_without_secrets() {
    let h = Home::new();
    h.palette(json!(["codex", "claude-code"]));
    h.write("ai/codex/config.toml", "model = \"personal\"\n");
    h.write(
        "ai/claude/settings.json",
        "{\"theme\":\"dark-daltonized\",\"permissions\":{\"deny\":[\"Bash(*)\"]}}",
    );
    h.write(".cache/bat/themes.bin", "cache-only-personal-theme");
    for name in ["next", "neutral"] {
        let out = h
            .command(&["apply", name, "--without-dependencies", "--json"])
            .env("CODEX_HOME", h.path("ai/codex"))
            .env("CLAUDE_CONFIG_DIR", h.path("ai/claude"))
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        if name == "next" {
            assert!(h
                .text("ai/claude/settings.json")
                .contains("dark-daltonized"));
            // Model unrelated runtime cache regeneration between theme switches.
            h.write(".cache/bat/themes.bin", "rebuilt-cache");
        }
    }
    assert_eq!(h.text("ai/codex/config.toml"), "model = \"personal\"\n");
    assert!(!h.path(".codex/config.toml").exists());
    // No bat changes were owned by this apply, so unrelated later cache edits survive.
    assert_eq!(h.text(".cache/bat/themes.bin"), "rebuilt-cache");
}

#[test]
fn snapshot_claude_keeps_accessibility_and_zellij_native_accepts_theme() {
    let h = Home::new();
    h.write(".claude/settings.json", "{\"theme\":\"light-daltonized\"}");
    h.write(
        ".config/kolira/themes/next/.claude/settings.json",
        "{\"theme\":\"dark-ansi\"}",
    );
    h.apply("next");
    assert!(h.text(".claude/settings.json").contains("dark-daltonized"));
    h.apply("neutral");
    assert_eq!(
        h.text(".claude/settings.json"),
        "{\"theme\":\"light-daltonized\"}"
    );
    if which::which("zellij").is_ok() {
        h.palette(json!(["zellij"]));
        h.apply("next");
        let out = native(&h, "zellij", &["setup", "--check"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(!String::from_utf8_lossy(&out.stdout).contains("Failed to parse"));
    }
}

#[test]
fn bat_binary_cache_changes_are_journalled_and_return_exactly() {
    use std::os::unix::fs::PermissionsExt;
    let h = Home::new();
    h.write(
        ".cache/bat/themes.bin",
        "original-cache-only-theme\0\u{00ff}",
    );
    h.write(
        ".config/kolira/themes/next/.config/bat/themes/kolira.tmTheme",
        "fixture",
    );
    h.write("bin/bat", "#!/bin/sh\n/bin/mkdir -p \"$XDG_CACHE_HOME/bat\"\nprintf 'new-cache' > \"$XDG_CACHE_HOME/bat/themes.bin\"\n");
    fs::set_permissions(h.path("bin/bat"), fs::Permissions::from_mode(0o755)).unwrap();
    let before = fs::read(h.path(".cache/bat/themes.bin")).unwrap();
    for name in ["next", "neutral"] {
        let out = h
            .command(&["apply", name, "--without-dependencies", "--json"])
            .env_remove("KTM_THEME_NO_APPLY")
            .env("PATH", h.path("bin"))
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        if name == "next" {
            assert_eq!(h.text(".cache/bat/themes.bin"), "new-cache");
        }
    }
    assert_eq!(fs::read(h.path(".cache/bat/themes.bin")).unwrap(), before);
}

#[test]
fn wofi_late_geometry_keeps_original_colours_without_a_dangling_import() {
    let h = Home::new();
    h.write(
        ".config/wofi/style.css",
        "window { color: #123456; margin: 4px; }\n",
    );
    h.palette(json!(["wofi"]));
    h.apply("next");
    let text = h.text(".config/wofi/style.css").replace("4px", "8px");
    h.write(".config/wofi/style.css", &text);
    h.apply("neutral");
    assert_eq!(
        h.text(".config/wofi/style.css"),
        "window { color: #123456; margin: 8px; }\n"
    );
    assert!(!h.path(".config/wofi/kolira.css").exists());
}
