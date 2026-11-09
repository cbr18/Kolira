#!/usr/bin/env bash
# test-scripts.sh — test suite for theme-profile-kit (Rust CLI + installers)
# Run: bash tests/test-scripts.sh
# Requires: the Rust CLI built (cargo build --release in gui/qt-app/src/core/rust_core)
#           bash, shellcheck (optional but recommended)

set -euo pipefail

# KConfig tools honour XDG_CONFIG_HOME even when HOME is overridden, so the
# fake-home tests below would otherwise write into the real user config.
unset XDG_CONFIG_HOME XDG_DATA_HOME XDG_CACHE_HOME XDG_STATE_HOME

PASS=0
FAIL=0

# --- test helpers -----------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CLI_DIR="${KTM_CLI_DIR:-$SCRIPT_DIR/gui/qt-app/src/core/rust_core/target/release}"
# The kit installer builds the CLI with cargo; a fake HOME must not reset the
# crate cache, so the real cargo home is passed through by the tests below.
REAL_CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
REAL_RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
# env -i below strips the rustup bin dir from PATH; the kit installer
# needs cargo to build the CLI, so carry the real toolchain through
# (cargo on hosted runners is a rustup proxy and resolves its toolchain
# via RUSTUP_HOME, which defaults to $HOME/.rustup).
REAL_CARGO_BIN="$(dirname "$(command -v cargo 2>/dev/null || echo /usr/bin/cargo)")"

ok()   { PASS=$((PASS + 1)); echo "  ✅ $1"; }
fail() { FAIL=$((FAIL + 1)); echo "  ❌ $1"; }

# Silence all output from a command
run_silent() { "$@" >/dev/null 2>&1; }

# Safety net: the CLI talks to the live desktop session (plasma-apply-*,
# kvantummanager, KWin/plasmashell over D-Bus, systemctl --user). A fake HOME
# does not isolate those, so every such tool is shadowed by a safe stub for
# the whole run. Mandatory apply tools/systemctl acknowledge success without
# touching the desktop; failing-command rollback is covered by Rust integration
# tests. Sections that observe calls put recording mocks in front of this dir.
SAFE_BIN="$(mktemp -d)"
for tool in plasma-apply-lookandfeel plasma-apply-colorscheme plasma-apply-desktoptheme \
  plasma-apply-cursortheme plasma-apply-wallpaperimage kvantummanager qdbus6 dbus-send \
  systemctl kbuildsycoca6 kitty gsettings; do
  case "$tool" in
    plasma-apply-*|kvantummanager|systemctl) printf '#!/usr/bin/env bash\nexit 0\n' >"$SAFE_BIN/$tool" ;;
    *) printf '#!/usr/bin/env bash\nexit 1\n' >"$SAFE_BIN/$tool" ;;
  esac
  chmod +x "$SAFE_BIN/$tool"
done
export PATH="$SAFE_BIN:$PATH"
# …and never the network (saving a theme searches the KDE Store for sources).
export KTM_OFFLINE=1

# --- 0. Precondition: the Rust CLI is built ----------------------------------
echo "=== Section 0: Rust CLI binaries are built ==="

for cli_bin in theme-profile theme-menu wallpaper-picker wallpaper-next wallpaper-prev; do
  if [ -x "$CLI_DIR/$cli_bin" ]; then
    ok "built: $cli_bin"
  else
    fail "missing: $CLI_DIR/$cli_bin — run 'cargo build --release' in gui/qt-app/src/core/rust_core first"
  fi
done

# --- 1. Syntax check (bash -n) on installer scripts ---------------------------
echo "=== Section 1: Syntax check (bash -n) ==="

for script in \
    "$SCRIPT_DIR/scripts/install-theme-profile-kit.sh" \
    "$SCRIPT_DIR/scripts/uninstall-theme-profile-kit.sh" \
    "$SCRIPT_DIR/scripts/update-theme-profile-kit.sh"; do

  if run_silent bash -n "$script"; then
    ok "bash -n: $script"
  else
    fail "bash -n: $script"
  fi
done

# --- 2. shellcheck (if available) --------------------------------------------
echo "=== Section 2: shellcheck (if installed) ==="

if command -v shellcheck >/dev/null 2>&1; then
  for script in \
      "$SCRIPT_DIR/scripts/install-theme-profile-kit.sh" \
      "$SCRIPT_DIR/scripts/uninstall-theme-profile-kit.sh" \
      "$SCRIPT_DIR/scripts/update-theme-profile-kit.sh"; do

    if shellcheck "$script" 2>/dev/null; then
      ok "shellcheck: $script"
    else
      fail "shellcheck: $script"
    fi
  done
else
  echo "  ⏭️  shellcheck not installed — skipping"
fi

# --- 3. theme-profile: --help exits 0 -----------------------------------------
echo "=== Section 3: --help flags ==="

if "$CLI_DIR/theme-profile" --help >/dev/null 2>&1; then
  ok "theme-profile --help exits 0"
else
  fail "theme-profile --help should exit 0"
fi

if "$CLI_DIR/theme-profile" -h >/dev/null 2>&1; then
  ok "theme-profile -h exits 0"
else
  fail "theme-profile -h should exit 0"
fi

for installer in install-theme-profile-kit.sh uninstall-theme-profile-kit.sh; do
  if "$SCRIPT_DIR/scripts/$installer" --help >/dev/null 2>&1; then
    ok "$installer --help exits 0"
  else
    fail "$installer --help should exit 0"
  fi
done

if "$SCRIPT_DIR/scripts/install-theme-profile-kit.sh" --bogus >/dev/null 2>&1; then
  fail "install-theme-profile-kit.sh should reject unknown options"
else
  ok "install-theme-profile-kit.sh rejects unknown options"
fi

# --- 4. theme-profile: invalid profile names rejected -------------------------
echo "=== Section 4: Profile name validation ==="

# Use a temporary HOME so save/apply don't touch the real profile dir
TMP_ROOT="$(mktemp -d)"
export TMP_ROOT
trap 'rm -rf "$TMP_ROOT" "$SAFE_BIN"' EXIT

# Bootstrap a fake ~/.config/kde-theme-profile structure
FAKE_HOME="$TMP_ROOT/fake-home"
mkdir -p "$FAKE_HOME/.config/kde-theme-profile"

# Create a real profile so we can test apply success and save
mkdir -p "$FAKE_HOME/.config/kde-theme-profile/themes/test-profile"
if HOME="$FAKE_HOME" "$CLI_DIR/theme-profile" save test-profile >/dev/null 2>&1; then
  ok "save test-profile (valid name)"
else
  fail "save test-profile (valid name) — should succeed"
fi

# Test invalid names → should exit non-zero
for bad_name in "../x" "a b" "a/b" "a;b" "a|b" "a\$b" ""; do
  if HOME="$FAKE_HOME" "$CLI_DIR/theme-profile" save "$bad_name" >/dev/null 2>&1; then
    fail "save '$bad_name' should have been rejected"
  else
    ok "save '$bad_name' correctly rejected"
  fi
done

# Re-saving with a smaller component set must not keep files from the old
# snapshot (the old implementation copied into the existing directory).
mkdir -p "$FAKE_HOME/.config"
printf '[General]\nAccentColor=1,2,3\n' > "$FAKE_HOME/.config/kdeglobals"
printf '[General]\nBorderSize=1\n' > "$FAKE_HOME/.config/kwinrc"
HOME="$FAKE_HOME" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" save components-test >/dev/null 2>&1
profile_root="$FAKE_HOME/.config/kde-theme-profile/themes/components-test"
if grep -q '"accent_color": "1,2,3"' "$profile_root/theme.json" 2>/dev/null &&
  [ -f "$profile_root/.config/kdeglobals" ] && [ -f "$profile_root/.config/kwinrc" ]; then
  ok "save writes a full theme (whole config files + theme.json with the values)"
else
  fail "save did not write a theme package: $(ls -A "$profile_root" 2>&1)"
fi
printf '[General]\nAccentColor=4,5,6\n' > "$FAKE_HOME/.config/kdeglobals"
HOME="$FAKE_HOME" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" save components-test >/dev/null 2>&1
if grep -q '"accent_color": "4,5,6"' "$profile_root/theme.json" &&
  [ "$(find "$profile_root" -name '.save-*' | wc -l)" = 0 ]; then
  ok "re-saving replaces the package atomically"
else
  fail "re-saving did not replace the package"
fi

# Regression (2026-11-04): a values-only format lost terminal colours, wofi,
# GTK4 and Zen settings; then whole-file restore overwrote personal settings.
# A theme must bring back the whole look and leave everything personal alone.
RT_HOME="$TMP_ROOT/roundtrip-home"
rt_put() { mkdir -p "$(dirname "$RT_HOME/$1")"; printf '%b' "$2" >"$RT_HOME/$1"; }
rt_put ".config/kitty/kitty.conf" 'background #fffbf4\nfont_size 11\nmap ctrl+a new_tab\n'
rt_put ".config/wofi/style.css" 'window { background: #fffbf4; }\n'
rt_put ".config/gtk-4.0/settings.ini" '[Settings]\ngtk-theme-name=Light\ngtk-cursor-blink=true\n'
rt_put ".config/kdeglobals" '[General]\nColorScheme=Paper\nTerminalApplication=kitty\n\n[KScreen]\nScreenScaleFactors=eDP=1;\n'
rt_put ".config/kwinrulesrc" '[General]\ncount=1\nrules=r1\n\n[r1]\nDescription=theme rule\nopacityactive=90\n'
rt_put ".zen/abc.Default (release)/user.js" 'user_pref("zen.view.window.scheme", 1);\n'
rt_cli() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$RT_HOME" KTM_THEME_NO_APPLY=1 PATH="/usr/bin:/bin" "$CLI_DIR/theme-profile" "$@"; }
rt_cli save rt >/dev/null 2>&1
# Someone else's look + their own personal settings.
rt_put ".config/kitty/kitty.conf" 'background #282a36\nmap ctrl+b close_tab\n'
rt_put ".config/wofi/style.css" 'window { background: #282a36; }\n'
rt_put ".config/gtk-4.0/settings.ini" '[Settings]\ngtk-theme-name=Dark\ngtk-cursor-blink=false\n'
rt_put ".config/kdeglobals" '[General]\nColorScheme=Dracula\nTerminalApplication=konsole\n\n[KScreen]\nScreenScaleFactors=eDP=2;\n'
rt_put ".config/kwinrulesrc" '[General]\ncount=1\nrules=mine\n\n[mine]\nDescription=my rule\n'
rt_put ".zen/abc.Default (release)/user.js" 'user_pref("zen.view.window.scheme", 0);\n'
rt_cli apply rt >/dev/null 2>&1
rt_has() { grep -qF -- "$2" "$RT_HOME/$1" || rt_bad="$rt_bad [$1: no '$2']"; }
rt_not() { ! grep -qF -- "$2" "$RT_HOME/$1" || rt_bad="$rt_bad [$1: still '$2']"; }
rt_bad=""
rt_has .config/kitty/theme-profile.conf "background #fffbf4"
rt_not .config/kitty/theme-profile.conf "font_size"
rt_has .config/kitty/kitty.conf "include theme-profile.conf"
rt_not .config/kitty/kitty.conf "background #282a36"
rt_has .config/wofi/style.css "#fffbf4"
rt_has .config/gtk-4.0/settings.ini "gtk-theme-name=Light"
rt_has .config/kdeglobals "ColorScheme=Paper"
rt_has .config/kwinrulesrc "opacityactive=90"
rt_has ".zen/abc.Default (release)/user.js" '"zen.view.window.scheme", 1'
if [ -z "$rt_bad" ]; then
  ok "apply brings back the whole look (terminal colours, wofi, GTK4, KDE, window rules, Zen)"
else
  fail "apply lost appearance:$rt_bad"
fi
rt_bad=""
rt_has .config/kitty/kitty.conf "map ctrl+b close_tab"
rt_not .config/kitty/kitty.conf "font_size 11"   # the theme's size never arrives
rt_not .config/kitty/kitty.conf "map ctrl+a new_tab"
rt_has .config/gtk-4.0/settings.ini "gtk-cursor-blink=false"
rt_has .config/kdeglobals "TerminalApplication=konsole"
rt_has .config/kdeglobals "ScreenScaleFactors=eDP=2;"
rt_has .config/kwinrulesrc "Description=my rule"
if [ -z "$rt_bad" ]; then
  ok "apply keeps personal settings (key bindings, screen scale, terminal, own window rules)"
else
  fail "apply overwrote personal settings:$rt_bad"
fi
if ! grep -q "TerminalApplication\|KScreen" "$RT_HOME/.config/kde-theme-profile/themes/rt/.config/kdeglobals" &&
  [ ! -e "$RT_HOME/.config/kde-theme-profile/themes/rt/.config/kitty/kitty.conf" ] &&
  ! grep -q "map ctrl" "$RT_HOME/.config/kde-theme-profile/themes/rt/.config/kitty/theme-profile.conf"; then
  ok "save stores only the look (no terminal choice, screen scale or key bindings)"
else
  fail "save stored personal settings in the theme"
fi

if HOME="$FAKE_HOME" "$CLI_DIR/theme-profile" apply ../x >/dev/null 2>&1; then
  fail "apply '../x' should have been rejected"
else
  ok "apply '../x' correctly rejected"
fi
if HOME="$FAKE_HOME" "$CLI_DIR/theme-profile" diff test-profile ../x >/dev/null 2>&1; then
  fail "diff '../x' should have been rejected"
else
  ok "diff '../x' correctly rejected"
fi


# XDG_CONFIG_HOME/XDG_DATA_HOME must be honoured for live files while the
# snapshot keeps the historical .config layout.
XDG_HOME="$TMP_ROOT/xdg-home"
XDG_CONFIG="$XDG_HOME/config"
XDG_DATA="$XDG_HOME/data"
mkdir -p "$XDG_CONFIG" "$XDG_DATA"
printf '[General]\nAccentColor=9,8,7\n' > "$XDG_CONFIG/kdeglobals"
if HOME="$XDG_HOME" XDG_CONFIG_HOME="$XDG_CONFIG" XDG_DATA_HOME="$XDG_DATA" KTM_THEME_NO_APPLY=1 \
  "$CLI_DIR/theme-profile" save xdg-profile >/dev/null 2>&1 \
  && grep -q '"accent_color": "9,8,7"' "$XDG_CONFIG/kde-theme-profile/themes/xdg-profile/theme.json"; then
  ok "XDG_CONFIG_HOME is used for profiles and live config"
else
  fail "XDG_CONFIG_HOME was ignored"
fi

# --- 5. theme-profile: apply nonexistent → fails fast (no side effects) ------
echo "=== Section 5: apply nonexistent doesn't break Plasma ==="

# We can't easily test the trap, but we can verify it exits 1 before
# any real systemctl call by using a mock PATH
MOCK_BIN="$TMP_ROOT/mock-bin"
mkdir -p "$MOCK_BIN"

# Create mock systemctl that tracks calls
cat > "$MOCK_BIN/systemctl" <<'MOCK'
#!/usr/bin/env bash
echo "MOCK systemctl called: $*" >> "$MOCK_CALLS"
exit 0
MOCK
  chmod +x "$MOCK_BIN/systemctl"

MOCK_CALLS="$TMP_ROOT/mock-calls.log"
echo "" > "$MOCK_CALLS"

# Pre-apply validation should reject before stop_plasma_shell_for_restore
if HOME="$FAKE_HOME" PATH="$MOCK_BIN:$PATH" "$CLI_DIR/theme-profile" apply nonexistent-profile >/dev/null 2>&1; then
  fail "apply nonexistent-profile should exit non-zero"
else
  ok "apply nonexistent-profile correctly exits non-zero"
fi

# The mock systemctl should NOT have been called with "stop" because
# profile validation happens first now
if ! grep -q "stop plasma-plasmashell" "$MOCK_CALLS" 2>/dev/null; then
  ok "apply nonexistent-profile does NOT stop plasma (validation runs first)"
else
  fail "apply nonexistent-profile stopped plasma before validation failed!"
fi

# --- 5a. Names differing only by case are rejected ---------------------------
CASE_HOME="$TMP_ROOT/case-home"
mkdir -p "$CASE_HOME/.config/kde-theme-profile"
case_cli() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$CASE_HOME" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
case_cli save Paper >/dev/null 2>&1
if case_cli save paper >/dev/null 2>&1 || case_cli clone Paper PAPER >/dev/null 2>&1; then
  fail "save/clone accepted a name differing only by case"
else
  ok "save/clone reject names differing only by case"
fi
if case_cli save Paper >/dev/null 2>&1; then
  ok "re-saving the same profile name is still allowed"
else
  fail "re-saving Paper was rejected"
fi

# --- 5b. Wallpaper travels with the profile ----------------------------------
echo "=== Section 5b: profile wallpaper copy ==="

WP_HOME="$TMP_ROOT/wp-home"
mkdir -p "$WP_HOME/.config/kde-theme-profile" "$WP_HOME/Pictures/Wallpaper"
printf 'jpeg' >"$WP_HOME/Pictures/Wallpaper/forest one.jpg"
cat >"$WP_HOME/.config/plasma-org.kde.plasma.desktop-appletsrc" <<APPLETS
[Containments][1]
lastScreen=0

[Containments][1][Wallpaper][org.kde.image][General]
Image=file://$WP_HOME/Pictures/Wallpaper/forest%20one.jpg
APPLETS

wp_cli() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u WALLPAPER_DIR HOME="$WP_HOME" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }

if [ "$(wp_cli wallpaper dir)" = "$WP_HOME/Pictures/Wallpaper" ]; then
  ok "wallpaper dir: no localised hard-coded folder (~/Pictures/Wallpaper)"
else
  fail "wallpaper dir unexpected: $(wp_cli wallpaper dir)"
fi
if [ "$(wp_cli wallpaper current)" = "$WP_HOME/Pictures/Wallpaper/forest one.jpg" ]; then
  ok "wallpaper current decodes the primary screen's image"
else
  fail "wallpaper current unexpected: $(wp_cli wallpaper current)"
fi

wp_cli save wp-test >/dev/null 2>&1
saved="$WP_HOME/.config/kde-theme-profile/themes/wp-test"
if [ -f "$saved/wallpaper/forest one.jpg" ] && grep -q '"wallpaper": "wallpaper/forest one.jpg"' "$saved/theme.json"; then
  ok "save copies the wallpaper into the theme package"
else
  fail "save did not copy the wallpaper"
fi
wp_cli save wp-nowall --no-wallpaper >/dev/null 2>&1
if grep -q '"wallpaper": null' "$WP_HOME/.config/kde-theme-profile/themes/wp-nowall/theme.json" &&
  [ ! -e "$WP_HOME/.config/kde-theme-profile/themes/wp-nowall/wallpaper" ]; then
  ok "save --no-wallpaper keeps the wallpaper out (applying leaves it untouched)"
else
  fail "save --no-wallpaper still stored a wallpaper"
fi

report=$(wp_cli apply wp-nowall --json 2>/dev/null || true)
if printf '%s' "$report" | python3 -c 'import json,sys; d=json.load(sys.stdin); assert {"applied","skipped","installed","missing","errors","backup"} <= set(d)' 2>/dev/null; then
  ok "apply --json prints the apply report (GUI/MCP read missing parts from it)"
else
  fail "apply --json did not print a JSON report: $report"
fi

rm "$WP_HOME/Pictures/Wallpaper/forest one.jpg"
wp_cli apply wp-test >/dev/null 2>&1
if [ "$(cat "$WP_HOME/Pictures/Wallpaper/forest one.jpg" 2>/dev/null)" = "jpeg" ]; then
  ok "apply restores a deleted wallpaper from the profile copy"
else
  fail "apply did not restore the deleted wallpaper"
fi

# Wallpaper only changes when the theme carries one and the user did not opt out.
WP_MOCK="$TMP_ROOT/wp-mock"
mkdir -p "$WP_MOCK"
printf '#!/usr/bin/env bash\necho "$*" >>"%s/calls"\nexit 0\n' "$WP_MOCK" >"$WP_MOCK/plasma-apply-wallpaperimage"
chmod +x "$WP_MOCK/plasma-apply-wallpaperimage"
wp_live() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u WALLPAPER_DIR HOME="$WP_HOME" PATH="$WP_MOCK:$PATH" "$CLI_DIR/theme-profile" "$@"; }
# The theme's appletsrc names "forest one.jpg"; the desktop now shows another.
printf 'jpeg2' >"$WP_HOME/Pictures/Wallpaper/other.jpg"
set_live_wallpaper() { sed -i "s|^Image=.*|Image=file://$WP_HOME/Pictures/Wallpaper/$1|" "$WP_HOME/.config/plasma-org.kde.plasma.desktop-appletsrc"; }
set_live_wallpaper other.jpg
: >"$WP_MOCK/calls"
wp_live apply wp-nowall >/dev/null 2>&1
set_live_wallpaper other.jpg
wp_live apply wp-test --keep-wallpaper >/dev/null 2>&1
if grep -q "other.jpg" "$WP_HOME/.config/plasma-org.kde.plasma.desktop-appletsrc" && ! grep -q "forest" "$WP_HOME/.config/plasma-org.kde.plasma.desktop-appletsrc" && ! grep -q "forest" "$WP_MOCK/calls"; then
  ok "no wallpaper in the theme, or --keep-wallpaper → the current wallpaper stays"
else
  fail "wallpaper rule broken: $(cat "$WP_MOCK/calls")"
fi
set_live_wallpaper other.jpg
wp_live apply wp-test >/dev/null 2>&1
if grep -q "forest%20one.jpg" "$WP_HOME/.config/plasma-org.kde.plasma.desktop-appletsrc"; then
  ok "a theme with a wallpaper applies it (restored desktop layout)"
else
  fail "the theme wallpaper was not applied"
fi

# Legacy snapshot profiles convert to packages; check reports what is missing.
LEG="$WP_HOME/.config/kde-theme-profile/themes/legacy"
mkdir -p "$LEG/.config"
printf '[General]\nColorScheme=NotInstalledScheme\nAccentColor=1,2,3\n' >"$LEG/.config/kdeglobals"
printf '{"schema_version":1,"name":"legacy","description":"old one"}\n' >"$LEG/manifest.json"
if wp_cli migrate legacy | grep -q "^Migrated legacy" && grep -q '"description": "old one"' "$LEG/theme.json" &&
  [ "$(cat "$LEG/.config/kdeglobals")" = "$(printf '[General]\nColorScheme=NotInstalledScheme\nAccentColor=1,2,3')" ]; then
  ok "migrate adds theme.json to a snapshot profile without changing its files"
else
  fail "migrate did not convert the legacy profile"
fi
if wp_cli check legacy | grep -q "^missing=color_scheme=NotInstalledScheme"; then
  ok "check lists assets the theme needs but this system lacks"
else
  fail "check did not report the missing colour scheme"
fi

# --- 5c. Palette: import → set → generate → apply → saved with the profile ---
echo "=== Section 5c: palette CLI ==="

PAL_HOME="$TMP_ROOT/pal-home"
mkdir -p "$PAL_HOME/.config/kde-theme-profile"
pal_cli() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$PAL_HOME" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
BASE16="$SCRIPT_DIR/gui/qt-app/src/core/rust_core/data/base16/everforest.yaml"

if pal_cli palette import "$BASE16" | grep -qx "base0B=#a7c080"; then
  ok "palette import reads a tinted-theming base16 file"
else
  fail "palette import failed"
fi
if pal_cli palette set accent=#A7C080 | grep -qx "accent=#a7c080"; then
  ok "palette set updates the accent"
else
  fail "palette set did not update the accent"
fi
if pal_cli palette set base99=#000000 >/dev/null 2>&1; then
  fail "palette set accepted an unknown slot"
else
  ok "palette set rejects unknown slots"
fi
if pal_cli palette generate kde | grep -q "^ColorScheme=ThemeProfileEverforest$"; then
  ok "palette generate kde prints a colour scheme"
else
  fail "palette generate kde output unexpected"
fi
pal_cli palette apply --targets=kde >/dev/null 2>&1
if grep -q "^Name=Everforest$" "$PAL_HOME/.local/share/color-schemes/ThemeProfileEverforest.colors" 2>/dev/null; then
  ok "palette apply writes ThemeProfile*.colors (never a user scheme name)"
else
  fail "palette apply did not write the colour scheme"
fi
if pal_cli palette apply --targets=gtk4 >/dev/null 2>&1; then
  fail "palette apply accepted an unknown target"
else
  ok "palette apply rejects unknown targets"
fi
# Gallery (offline: never touches the network in tests).
gal_cli() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u XDG_CACHE_HOME HOME="$PAL_HOME" KTM_OFFLINE=1 KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
if [ "$(gal_cli palette gallery | grep -c "	bundled	")" -ge 15 ] &&
  gal_cli palette gallery rose | grep -q "^rose-pine-dawn	bundled	light	"; then
  ok "palette gallery lists the bundled schemes offline (with search)"
else
  fail "palette gallery offline listing unexpected"
fi
if gal_cli palette import gallery:nord | grep -qx "name=Nord" &&
  gal_cli palette show | grep -qx "source=gallery:nord"; then
  ok "palette import gallery:ID uses the bundled scheme"
else
  fail "palette import gallery:nord failed"
fi
if gal_cli palette import gallery:not-bundled-x >/dev/null 2>&1 ||
  gal_cli palette import gallery:../etc >/dev/null 2>&1; then
  fail "gallery import accepted an unavailable/invalid id offline"
else
  ok "gallery import rejects unknown ids offline and invalid ids"
fi
# Palette from colours (Kirigami.ImageColors shape) and from an image.
if echo '{"colors":[{"color":"#1f3326","ratio":0.6},{"color":"#8fb573","ratio":0.3}],"name":"Fx"}' |
  pal_cli palette from-colors - | grep -qx "name=Fx" && pal_cli palette show | grep -qx "variant=dark"; then
  ok "palette from-colors builds a dark palette from ImageColors-style JSON"
else
  fail "palette from-colors failed"
fi
if pal_cli palette from-colors '{"colors":[]}' >/dev/null 2>&1; then
  fail "palette from-colors accepted an empty colour list"
else
  ok "palette from-colors rejects an empty colour list"
fi
if command -v magick >/dev/null 2>&1; then
  magick -size 64x64 xc:'#f2efe6' -fill '#8a8a45' -draw 'rectangle 40,0 63,63' "$PAL_HOME/light.png"
  if pal_cli palette from-wallpaper "$PAL_HOME/light.png" | grep -qx "variant=light" &&
    ! pal_cli palette show | grep -q "^warning="; then
    ok "palette from-wallpaper reads an image (light, readable)"
  else
    fail "palette from-wallpaper result unexpected"
  fi
else
  ok "palette from-wallpaper image check skipped (no magick to make a test image)"
fi

pal_cli palette import "$BASE16" >/dev/null
pal_cli palette set accent=#a7c080 >/dev/null

# Editor theme stores: offline/invalid input never reaches the network.
out="$(gal_cli apps store-search zed everforest 2>&1 || true)" # exits 1 by design
case "$out" in
  *"network is disabled"*) ok "apps store-search respects the offline switch" ;;
  *) fail "apps store-search did not report the disabled network: $out" ;;
esac
if gal_cli apps store-search btop x >/dev/null 2>&1 || gal_cli apps store-install zed ../x >/dev/null 2>&1 ||
  gal_cli apps store-install cursor not-an-id >/dev/null 2>&1; then
  fail "apps store accepted an unknown app or an invalid id"
else
  ok "apps store rejects unknown apps and invalid ids"
fi

# Terminals: the palette goes into the include files; the user's kitty.conf /
# alacritty.toml only lose their look (moved, backed up) and gain the include.
mkdir -p "$PAL_HOME/.config/kitty" "$PAL_HOME/.config/alacritty"
printf 'font_size 11\nbackground_opacity 0.9\nmap ctrl+a new_tab\n' >"$PAL_HOME/.config/kitty/kitty.conf"
printf '[window]\nopacity = 0.5 # mine\nstartup_mode = "Maximized"\n' >"$PAL_HOME/.config/alacritty/alacritty.toml"
chmod 600 "$PAL_HOME/.config/kitty/kitty.conf"
pal_cli palette apply >/dev/null 2>&1
pal_cli palette apply >/dev/null 2>&1
kitty_conf="$PAL_HOME/.config/kitty/kitty.conf"
kitty_theme="$PAL_HOME/.config/kitty/theme-profile.conf"
if [ "$(grep -c '^# BEGIN theme-profile palette' "$kitty_theme")" = 1 ] &&
  grep -qx "background #2d353b" "$kitty_theme" && ! grep -q "font_size" "$kitty_theme" &&
  [ "$(grep -c '^include theme-profile.conf' "$kitty_conf")" = 1 ] && grep -qx "map ctrl+a new_tab" "$kitty_conf" &&
  grep -qx "font_size 11" "$kitty_conf" && ! grep -q "background" "$kitty_conf"; then
  ok "palette apply writes kitty colours to theme-profile.conf; kitty.conf keeps keys, font size + one include"
else
  fail "kitty theme/include wrong: $(cat "$kitty_conf") // $(cat "$kitty_theme" 2>&1)"
fi
if [ "$(stat -c %a "$kitty_conf")" = 600 ]; then
  ok "palette apply keeps kitty.conf permissions"
else
  fail "kitty.conf permissions changed to $(stat -c %a "$kitty_conf")"
fi
alacritty_toml="$PAL_HOME/.config/alacritty/alacritty.toml"
alacritty_theme="$PAL_HOME/.config/alacritty/theme-profile.toml"
if grep -q 'opacity = 0.5 # mine' "$alacritty_theme" && grep -q 'background = "#2d353b"' "$alacritty_theme" &&
  grep -q 'startup_mode' "$alacritty_toml" && grep -qF 'theme-profile.toml"' "$alacritty_toml" &&
  ! grep -q 'opacity' "$alacritty_toml"; then
  ok "palette apply sets Alacritty colours in theme-profile.toml (imported), keeps comments"
else
  fail "alacritty unexpected: $(cat "$alacritty_toml") // $(cat "$alacritty_theme" 2>&1)"
fi
if compgen -G "$PAL_HOME/.config/kde-theme-profile/backups/pre-split-kitty-*/.config/kitty/kitty.conf" >/dev/null &&
  compgen -G "$PAL_HOME/.config/kde-theme-profile/backups/palette-apply-*/theme-profile.conf" >/dev/null; then
  ok "the original kitty.conf and the previous theme file are backed up"
else
  fail "no pre-split / palette-apply backup"
fi
rm "$alacritty_toml"
if pal_cli palette apply --targets=alacritty | grep -q "skipped"; then
  ok "palette apply skips a missing Alacritty config instead of creating it"
else
  fail "palette apply did not skip the missing config"
fi
[ ! -e "$alacritty_toml" ] || fail "palette apply created alacritty.toml"

# Kvantum: recolour an installed base theme; profiles carry the active theme.
KV="$PAL_HOME/.config/Kvantum"
mkdir -p "$KV/FakeBase"
printf '[GeneralColors]\nwindow.color=#383c4a\nwindow.text.color=#ffffff\nhighlight.color=#5294e2\n' >"$KV/FakeBase/FakeBase.kvconfig"
printf '<rect style="fill:#5294e2"/>' >"$KV/FakeBase/FakeBase.svg"
if pal_cli palette apply --targets=kvantum --kvantum-base=FakeBase | grep -q "recoloured from FakeBase" &&
  grep -q "#a7c080" "$KV/ThemeProfileEverforest/ThemeProfileEverforest.svg"; then
  ok "palette apply --targets=kvantum recolours an installed base theme"
else
  fail "Kvantum recolour failed"
fi
if pal_cli palette apply --targets=kvantum --kvantum-base=Missing >/dev/null 2>&1; then
  fail "palette apply accepted a missing Kvantum base"
else
  ok "palette apply reports a missing Kvantum base"
fi
printf '[General]\ntheme=ThemeProfileEverforest\n' >"$KV/kvantum.kvconfig"
pal_cli save kv-test >/dev/null 2>&1
if [ -f "$PAL_HOME/.config/kde-theme-profile/themes/kv-test/.config/Kvantum/ThemeProfileEverforest/ThemeProfileEverforest.svg" ]; then
  ok "theme packages bundle the active user Kvantum theme"
else
  fail "active Kvantum theme missing from the package"
fi
rm -rf "$KV/ThemeProfileEverforest"
pal_cli apply kv-test >/dev/null 2>&1
if [ -f "$KV/ThemeProfileEverforest/ThemeProfileEverforest.kvconfig" ]; then
  ok "apply restores the profile's Kvantum theme"
else
  fail "apply did not restore the Kvantum theme"
fi

# (KTM_THEME_NO_APPLY skips plasma-apply-colorscheme: set the scheme as it would.)
printf '[General]\nColorScheme=ThemeProfileEverforest\n' >"$PAL_HOME/.config/kdeglobals"
pal_cli save pal-test >/dev/null 2>&1
pal_pkg="$PAL_HOME/.config/kde-theme-profile/themes/pal-test"
if grep -q '"accent": "#a7c080"' "$pal_pkg/theme.json" 2>/dev/null &&
  [ -f "$pal_pkg/.local/share/color-schemes/ThemeProfileEverforest.colors" ]; then
  ok "theme packages carry the palette and its generated colour scheme"
else
  fail "palette/scheme missing from the package"
fi

# --- 6. Wallpaper index wrapping ---------------------------------------------
echo "=== Section 6: Wallpaper index wrapping ==="

WALL_DIR="$TMP_ROOT/walls"
mkdir -p "$WALL_DIR"
# Create 3 fake wallpaper files
for i in 1 2 3; do
  touch "$WALL_DIR/wallpaper-$i.png"
done

# next: 0 → 1
COUNT=3
INDEX=$(( (0 + 1) % COUNT ))
if [ "$INDEX" -eq 1 ]; then ok "wallpaper-next: 0→1"; else fail "wallpaper-next: expected 1 got $INDEX"; fi

# next: 2 → 0 (wrap)
INDEX=$(( (2 + 1) % COUNT ))
if [ "$INDEX" -eq 0 ]; then ok "wallpaper-next: 2→0 (wrap)"; else fail "wallpaper-next: expected 0 got $INDEX"; fi

# prev: 0 → 2 (wrap)
INDEX=$(( (0 - 1 + COUNT) % COUNT ))
if [ "$INDEX" -eq 2 ]; then ok "wallpaper-prev: 0→2 (wrap)"; else fail "wallpaper-prev: expected 2 got $INDEX"; fi

# prev: 1 → 0
INDEX=$(( (1 - 1 + COUNT) % COUNT ))
if [ "$INDEX" -eq 0 ]; then ok "wallpaper-prev: 1→0"; else fail "wallpaper-prev: expected 0 got $INDEX"; fi

# --- 7. wallpaper-prev stderr leak test ---------------------------------------
echo "=== Section 7: wallpaper-prev handles missing state file ==="

STATE_FILE="$TMP_ROOT/missing-state"
rm -f "$STATE_FILE"
# A real run against a fake dir with no files must exit 1 quietly (no stderr
# leak when the state file is missing).
mkdir -p "$TMP_ROOT/empty-walls"
ERR_OUT="$(
  WALLPAPER_DIR="$TMP_ROOT/empty-walls" WALLPAPER_STATE="$STATE_FILE" \
    WALLPAPER_DRY_RUN=1 PLASMA_APPLETSRC="$TMP_ROOT/nope" \
    "$CLI_DIR/wallpaper-prev" 2>&1 1>/dev/null || true
)"
if [ -z "$ERR_OUT" ]; then
  ok "wallpaper-prev: no stderr when state file missing"
else
  fail "wallpaper-prev: stderr leaked: $ERR_OUT"
fi

# --- 8. Wallpaper list order, next/prev anchoring -----------------------------
echo "=== Section 8: Wallpaper list order and next/prev anchoring ==="

WALL_TMP="$TMP_ROOT/wall-order"
PLASMA_TMP="$TMP_ROOT/plasma-appletsrc"
rm -rf "$WALL_TMP"
mkdir -p "$WALL_TMP"

for name in apple.png Zebra.png -dash.jpg 1number.webp Beta.bmp UP.PNG Обои.jpg; do
  magick -size 40x30 xc:blue "png:$WALL_TMP/$name"
done
touch "$WALL_TMP/notes.txt"
ln -s apple.png "$WALL_TMP/linked.png"

# Byte order, symlinks followed, .bmp and uppercase extensions accepted.
EXPECTED=(
  "-dash.jpg" "1number.webp" "Beta.bmp" "UP.PNG"
  "Zebra.png" "apple.png" "linked.png" "Обои.jpg"
)

wall_run() { # script state
  WALLPAPER_DIR="$WALL_TMP" \
    WALLPAPER_STATE="$2" \
    WALLPAPER_DRY_RUN=1 \
    PLASMA_APPLETSRC="$TMP_ROOT/no-such-appletsrc" \
    "$CLI_DIR/$1"
}

STATE_O="$TMP_ROOT/wall-order-state"
printf '%s' "$((${#EXPECTED[@]} - 1))" > "$STATE_O"

WALKED=()
for _ in "${EXPECTED[@]}"; do
  WALKED+=("$(basename "$(wall_run wallpaper-next "$STATE_O")")")
done

if [ "${WALKED[*]}" = "${EXPECTED[*]}" ]; then
  ok "wallpaper-next walks the byte-ordered list incl. symlink/.bmp/UPPER.PNG"
else
  fail "wallpaper-next order: got [${WALKED[*]}] want [${EXPECTED[*]}]"
fi

printf '0' > "$STATE_O"
if [ "$(basename "$(wall_run wallpaper-prev "$STATE_O")")" = "Обои.jpg" ]; then
  ok "wallpaper-prev: 0 → last (wrap, Unicode-safe)"
else
  fail "wallpaper-prev: 0 → last (wrap, Unicode-safe)"
fi

# next/prev must step from the wallpaper Plasma actually shows, not from a
# stale cached index (a file added/removed shifts every index).
{
  printf '[Containments][281][Wallpaper][org.kde.image][General]\n'
  printf 'Image=file://%s/Zebra.png\n' "$WALL_TMP"
  printf '[Containments][281][Wallpaper][org.kde.slideshow][General]\n'
  printf 'Image=file://%s/apple.png\n' "$WALL_TMP"
} > "$PLASMA_TMP"

printf '0' > "$STATE_O"
got="$(basename "$(
  WALLPAPER_DIR="$WALL_TMP" WALLPAPER_STATE="$STATE_O" WALLPAPER_DRY_RUN=1 \
    PLASMA_APPLETSRC="$PLASMA_TMP" "$CLI_DIR/wallpaper-next"
)")"
if [ "$got" = "apple.png" ]; then
  ok "wallpaper-next anchors on the applied wallpaper (not the stale index)"
else
  fail "wallpaper-next anchoring: got $got want apple.png"
fi

got="$(basename "$(
  WALLPAPER_DIR="$WALL_TMP" WALLPAPER_STATE="$STATE_O" WALLPAPER_DRY_RUN=1 \
    PLASMA_APPLETSRC="$PLASMA_TMP" "$CLI_DIR/wallpaper-prev"
)")"
if [ "$got" = "UP.PNG" ]; then
  ok "wallpaper-prev anchors on the applied wallpaper"
else
  fail "wallpaper-prev anchoring: got $got want UP.PNG"
fi

# Garbage in the state file must not abort the CLI (equivalent of set -e).
printf 'oops' > "$STATE_O"
if WALLPAPER_DIR="$WALL_TMP" WALLPAPER_STATE="$STATE_O" WALLPAPER_DRY_RUN=1 \
    PLASMA_APPLETSRC="$TMP_ROOT/no-such-appletsrc" \
    run_silent "$CLI_DIR/wallpaper-next"; then
  ok "wallpaper-next survives a non-numeric state file"
else
  fail "wallpaper-next dies on a non-numeric state file"
fi

# --- 9. Wallpaper CLI binaries share one enumeration --------------------------
echo "=== Section 9: wallpaper-* binaries share the enumeration ==="

# The three binaries are built from the same Rust module (wallpaper.rs), so
# the enumeration cannot drift between them. The actual order is verified by
# walking `wallpaper-next` (section 8) and by TestWallpaperOrdering in pytest.
all_wp_bins=1
for wp_bin in wallpaper-picker wallpaper-next wallpaper-prev; do
  [ -x "$CLI_DIR/$wp_bin" ] || all_wp_bins=0
done
if [ "$all_wp_bins" -eq 1 ]; then
  ok "wallpaper-picker/next/prev are present and executable (shared enumeration)"
else
  fail "a wallpaper CLI binary is missing from $CLI_DIR"
fi

# --- 10. wallpaper-picker maps wofi's line number to the clicked image --------
echo "=== Section 10: wallpaper-picker index mapping ==="

STUB_DIR="$TMP_ROOT/stub-bin"
mkdir -p "$STUB_DIR"
# wofi with `--define dmenu-print_line_num=true` prints the *0-based* index of
# the selected line (modes/dmenu.c: line_num starts at 0). This stub answers
# with the line the test "clicks".
cat > "$STUB_DIR/wofi" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "${WOFI_CHOICE:-0}"
STUB
chmod +x "$STUB_DIR/wofi"

PICK_STATE="$TMP_ROOT/pick-state"

pick() { # line
  printf '99' > "$PICK_STATE"
  WOFI_CHOICE="$1" PATH="$STUB_DIR:$PATH" \
    WALLPAPER_DIR="$WALL_TMP" \
    WALLPAPER_STATE="$PICK_STATE" \
    WALLPAPER_THUMB_DIR="$TMP_ROOT/thumbs" \
    WALLPAPER_DRY_RUN=1 \
    "$CLI_DIR/wallpaper-picker"
}

got="$(basename "$(pick 0)")"
if [ "$got" = "${EXPECTED[0]}" ] && [ "$(cat "$PICK_STATE")" = "0" ]; then
  ok "wallpaper-picker: line 0 → 1st image (used to apply line 1 / nothing)"
else
  fail "wallpaper-picker: line 0 → ${EXPECTED[0]}, got '$got' state $(cat "$PICK_STATE")"
fi

got="$(basename "$(pick 3)")"
if [ "$got" = "${EXPECTED[3]}" ] && [ "$(cat "$PICK_STATE")" = "3" ]; then
  ok "wallpaper-picker: line 3 → 4th image, cached index 3"
else
  fail "wallpaper-picker: line 3 → ${EXPECTED[3]}, got '$got' state $(cat "$PICK_STATE")"
fi

last=$(( ${#EXPECTED[@]} - 1 ))
got="$(basename "$(pick "$last")")"
if [ "$got" = "${EXPECTED[$last]}" ]; then
  ok "wallpaper-picker: last line → last image"
else
  fail "wallpaper-picker: line $last → ${EXPECTED[$last]}, got '$got'"
fi

if [ -z "$(pick 'some/typed/path.png' 2>/dev/null)" ]; then
  ok "wallpaper-picker: a typed custom entry is not treated as a position"
else
  fail "wallpaper-picker: typed custom entry was applied"
fi

# --- 11. GUI install/uninstall round trip in a throwaway HOME -----------------
echo "=== Section 11: kde-theme-manager install / uninstall ==="

FAKE_HOME="$TMP_ROOT/fake-home"
mkdir -p "$FAKE_HOME"

gui_install() {
  env -i HOME="$FAKE_HOME" XDG_DATA_HOME="$FAKE_HOME/.local/share" \
    PATH="/usr/bin:/bin" \
    bash "$SCRIPT_DIR/scripts/install-theme-profile-kit.sh" --only-gui --no-venv
}

if gui_install >/dev/null 2>&1; then
  ok "install --only-gui runs"
else
  fail "install --only-gui failed"
fi

check_installed() { # path description
  if [ -e "$1" ]; then ok "$2"; else fail "$2 (missing: $1)"; fi
}

check_installed "$FAKE_HOME/.local/share/kde-theme-manager/src/__main__.py" \
  "app copy installed"
check_installed "$FAKE_HOME/.local/share/kde-theme-manager/qml/Main.qml" \
  "app QML shipped with the copy"
check_installed "$FAKE_HOME/.local/share/kde-theme-manager/.installed-by-theme-profile-kit" \
  "app copy carries the install marker"
check_installed "$FAKE_HOME/.local/bin/kde-theme-manager" "launcher installed"
check_installed "$FAKE_HOME/.local/share/applications/kde-theme-manager.desktop" \
  "desktop entry installed"
check_installed "$FAKE_HOME/.local/share/icons/hicolor/scalable/apps/ktm-theme-manager.svg" \
  "app icon installed"

if [ -x "$FAKE_HOME/.local/bin/kde-theme-manager" ]; then
  ok "launcher is executable"
else
  fail "launcher must be executable"
fi


# The dev venv, caches and the Rust build tree must not be copied.
if [ -e "$FAKE_HOME/.local/share/kde-theme-manager/.venv" ] ||
  [ -e "$FAKE_HOME/.local/share/kde-theme-manager/src/core/rust_core/target" ]; then
  fail "installer copied .venv or the Rust build tree"
else
  ok "installer skips .venv / rust_core target"
fi

# Killing the app directory must break the launcher with a clear message.
mv "$FAKE_HOME/.local/share/kde-theme-manager" "$FAKE_HOME/moved-app"
out="$(env -i HOME="$FAKE_HOME" XDG_DATA_HOME="$FAKE_HOME/.local/share" \
  PATH="/usr/bin:/bin" bash "$FAKE_HOME/.local/bin/kde-theme-manager" 2>&1 || true)"
mv "$FAKE_HOME/moved-app" "$FAKE_HOME/.local/share/kde-theme-manager"
case "$out" in
  *"not installed"*) ok "launcher reports a missing app instead of crashing" ;;
  *) fail "launcher message unexpected: $out" ;;
esac

# A re-install must keep working (and back up the previous tree) without
# dragging dev artifacts into the backup.
mkdir -p "$FAKE_HOME/.local/share/kde-theme-manager/.venv" \
  "$FAKE_HOME/.local/share/kde-theme-manager/src/core/rust_core/target"
if gui_install >/dev/null 2>&1; then
  ok "re-install succeeds"
else
  fail "re-install failed"
fi
backup_dir="$(compgen -G "$FAKE_HOME/.config/kde-theme-profile/backups/deploy-kde-theme-manager-*" | tail -1)"
if [ -n "$backup_dir" ] && [ -f "$backup_dir/app/src/__main__.py" ]; then
  ok "re-install backs up the previous app directory"
else
  fail "re-install produced no usable backup"
fi
if [ -e "$backup_dir/app/.venv" ] || [ -e "$backup_dir/app/src/core/rust_core/target" ]; then
  fail "backup kept .venv / rust_core target"
else
  ok "backup prunes .venv and rust_core target"
fi

# A copy running from the installed app dir keeps the old code: the installer
# must warn about it, and --restart must stop it. The fake process mimics the
# launcher (cwd = app dir, `-m src` in argv); `; :` stops bash from exec'ing.
fake_gui() {
  (cd "$FAKE_HOME/.local/share/kde-theme-manager" && exec bash -c 'sleep 30; :' fake -m src) &
  FAKE_GUI_PID=$!
  sleep 0.3
}
fake_gui
out="$(gui_install 2>&1 || true)"
case "$out" in
  *"$FAKE_GUI_PID"*"старой версии"*) ok "installer warns about a running old copy" ;;
  *) fail "installer did not warn about the running copy: $out" ;;
esac
if kill -0 "$FAKE_GUI_PID" 2>/dev/null; then
  ok "without --restart the running copy is left alone"
else
  fail "installer killed the running copy without --restart"
fi
env -i HOME="$FAKE_HOME" XDG_DATA_HOME="$FAKE_HOME/.local/share" \
  PATH="/usr/bin:/bin" KTM_INSTALL_NO_LAUNCH=1 \
  bash "$SCRIPT_DIR/scripts/install-theme-profile-kit.sh" --only-gui --no-venv --restart >/dev/null 2>&1 || true
sleep 0.3
if kill -0 "$FAKE_GUI_PID" 2>/dev/null; then
  fail "--restart did not stop the running copy"
  kill "$FAKE_GUI_PID" 2>/dev/null || true
else
  ok "--restart stops the running copy"
fi
wait "$FAKE_GUI_PID" 2>/dev/null || true

env -i HOME="$FAKE_HOME" XDG_DATA_HOME="$FAKE_HOME/.local/share" \
  PATH="/usr/bin:/bin" \
  bash "$SCRIPT_DIR/scripts/uninstall-theme-profile-kit.sh" --only-gui --force >/dev/null 2>&1

check_gone() { # path description
  if [ -e "$1" ]; then fail "still present: $1"; else ok "$2"; fi
}

check_gone "$FAKE_HOME/.local/share/kde-theme-manager" "app copy removed"
check_gone "$FAKE_HOME/.local/bin/kde-theme-manager" "launcher removed"
check_gone "$FAKE_HOME/.local/share/applications/kde-theme-manager.desktop" \
  "desktop entry removed"
check_gone "$FAKE_HOME/.local/share/icons/hicolor/scalable/apps/ktm-theme-manager.svg" \
  "app icon removed"

# An unrelated directory must survive removal.
mkdir -p "$FAKE_HOME/.local/share/kde-theme-manager-not-ours"
printf 'keep me\n' > "$FAKE_HOME/.local/share/kde-theme-manager-not-ours/data"
env -i HOME="$FAKE_HOME" XDG_DATA_HOME="$FAKE_HOME/.local/share" \
  PATH="/usr/bin:/bin" \
  bash "$SCRIPT_DIR/scripts/uninstall-theme-profile-kit.sh" --only-gui --force >/dev/null 2>&1
if [ -f "$FAKE_HOME/.local/share/kde-theme-manager-not-ours/data" ]; then
  ok "uninstall leaves unrelated data alone"
else
  fail "uninstall removed an unrelated directory"
fi

# The kit installer must bring up both the CLI and the GUI (and --only-cli must
# install the CLI only).
if env -i HOME="$FAKE_HOME" XDG_DATA_HOME="$FAKE_HOME/.local/share" \
  PATH="/usr/bin:/bin:$REAL_CARGO_BIN" CARGO_HOME="$REAL_CARGO_HOME" RUSTUP_HOME="$REAL_RUSTUP_HOME" \
  bash "$SCRIPT_DIR/scripts/install-theme-profile-kit.sh" --no-venv >/dev/null 2>&1; then
  ok "install-theme-profile-kit.sh runs"
else
  fail "install-theme-profile-kit.sh failed"
fi

for f in theme-profile theme-menu wallpaper-picker wallpaper-next wallpaper-prev; do
  if [ -x "$FAKE_HOME/.local/bin/$f" ]; then
    ok "kit installer deployed $f"
  else
    fail "kit installer did not deploy $f"
  fi
done

if [ -x "$FAKE_HOME/.local/bin/kde-theme-manager" ]; then
  ok "kit installer also installed the GUI launcher"
else
  fail "kit installer did not install the GUI launcher"
fi

if python3 - "$FAKE_HOME/.config/kde-theme-profile/installed-assets.json" <<'REGISTRY'
import json, sys
registry = json.load(open(sys.argv[1]))
assert registry["schema"] == 2
assert any(r["application"] and r["dependency"]["kind"] == "python"
           for r in registry["resources"].values())
REGISTRY
then
  ok "installer registers application dependencies in the Rust registry"
else
  fail "installer did not register application dependencies"
fi

# The bundled themes (themes/) come with the app, never overwriting a theme.
bundled_ok=1
for dir in "$SCRIPT_DIR/themes"/*/; do
  name="$(basename "$dir")"
  [ -f "$FAKE_HOME/.config/kde-theme-profile/themes/$name/theme.json" ] || bundled_ok=0
done
bundled_names="$(find "$SCRIPT_DIR/themes" -mindepth 1 -maxdepth 1 -type d -printf '%f ' | sort)"
if [ "$bundled_ok" = 1 ] && [ -n "$bundled_names" ]; then
  ok "kit installer added the bundled themes ($bundled_names)"
else
  fail "bundled themes missing from $FAKE_HOME/.config/kde-theme-profile/themes"
fi
first_theme="$(find "$SCRIPT_DIR/themes" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' | sort | head -1)"
printf 'mine\n' >"$FAKE_HOME/.config/kde-theme-profile/themes/$first_theme/marker"
env -i HOME="$FAKE_HOME" XDG_DATA_HOME="$FAKE_HOME/.local/share" \
  PATH="/usr/bin:/bin:$REAL_CARGO_BIN" CARGO_HOME="$REAL_CARGO_HOME" RUSTUP_HOME="$REAL_RUSTUP_HOME" \
  bash "$SCRIPT_DIR/scripts/install-theme-profile-kit.sh" --only-cli >/dev/null 2>&1
if [ -f "$FAKE_HOME/.config/kde-theme-profile/themes/$first_theme/marker" ]; then
  ok "re-installing never replaces an existing theme"
else
  fail "re-install replaced the user's theme $first_theme"
fi

rm -rf "$FAKE_HOME/.local/share/kde-theme-manager" "$FAKE_HOME/.local/bin/kde-theme-manager"
env -i HOME="$FAKE_HOME" XDG_DATA_HOME="$FAKE_HOME/.local/share" \
  PATH="/usr/bin:/bin:$REAL_CARGO_BIN" CARGO_HOME="$REAL_CARGO_HOME" RUSTUP_HOME="$REAL_RUSTUP_HOME" \
  bash "$SCRIPT_DIR/scripts/install-theme-profile-kit.sh" --only-cli >/dev/null 2>&1
if [ ! -e "$FAKE_HOME/.local/bin/kde-theme-manager" ]; then
  ok "--only-cli installs the CLI without the app"
else
  fail "--only-cli still installed the app"
fi

if env -i HOME="$FAKE_HOME" XDG_DATA_HOME="$FAKE_HOME/.local/share" \
  PATH="/usr/bin:/bin" \
  bash "$SCRIPT_DIR/scripts/uninstall-theme-profile-kit.sh" --force --only-cli >/dev/null 2>&1 &&
  [ ! -e "$FAKE_HOME/.local/bin/theme-profile" ]; then
  ok "uninstall-theme-profile-kit.sh --force --only-cli removes the CLI"
else
  fail "kit uninstaller (--only-cli) did not remove the CLI"
fi

# --- 12. The GUI install ships no Rust library --------------------------------
# The second core (libkde_theme_core.so) was removed; the installer must not
# reference rust flags or install any library.
echo "=== Section 12: no Rust library in the GUI install ==="

FAKE_HOME_RUST="$TMP_ROOT/fake-home-rust"
mkdir -p "$FAKE_HOME_RUST"

if bash "$SCRIPT_DIR/scripts/install-theme-profile-kit.sh" --help 2>&1 |
  grep -q -- "--with-rust"; then
  fail "installer --help still documents the removed --with-rust / --no-rust"
else
  ok "installer --help has no Rust library flags"
fi

if env -i HOME="$FAKE_HOME_RUST" XDG_DATA_HOME="$FAKE_HOME_RUST/.local/share" \
  PATH="/usr/bin:/bin" \
  bash "$SCRIPT_DIR/scripts/install-theme-profile-kit.sh" --only-gui --no-venv >/dev/null 2>&1; then
  if [ -e "$FAKE_HOME_RUST/.local/share/kde-theme-manager/lib" ]; then
    fail "installer still creates a Rust library directory"
  else
    ok "installer installs no Rust library"
  fi
else
  fail "GUI install (--no-venv) failed"
fi

# The installed app copy must not carry the ctypes binding either.
if [ -e "$FAKE_HOME_RUST/.local/share/kde-theme-manager/src/core/rust_core.py" ]; then
  fail "installed app still ships src/core/rust_core.py"
else
  ok "installed app ships no rust_core.py binding"
fi


# --- 13. apply restores LookAndFeelPackage from the profile (not --help) ------
echo "=== Section 13: apply_live_hooks reads LookAndFeelPackage from the profile ==="

# This section asserts through the real (sandboxed-by-HOME) KConfig CLI tools,
# which are KDE tooling absent from CI runners — skip there instead of failing.
if ! command -v kreadconfig6 >/dev/null 2>&1 || ! command -v kwriteconfig6 >/dev/null 2>&1; then
  echo "  \u23ed  kreadconfig6/kwriteconfig6 not installed (KDE tooling) — skipping"
else
  LF_HOME="$TMP_ROOT/lf-home"
  mkdir -p "$LF_HOME/.config/kde-theme-profile" "$LF_HOME/.config"
  # Themes only apply installed assets: provide the Look & Feel package.
  mkdir -p "$LF_HOME/.local/share/plasma/look-and-feel/paper"
  echo '{}' >"$LF_HOME/.local/share/plasma/look-and-feel/paper/metadata.json"
  cat > "$LF_HOME/.config/kdeglobals" <<'EOF'
[General]
ColorScheme=Breeze1

[KDE]
LookAndFeelPackage=paper
EOF

  MOCK_BIN_LF="$TMP_ROOT/mock-bin-lf"
  mkdir -p "$MOCK_BIN_LF"
  for cmd in systemctl qdbus6 kbuildsycoca6 plasma-apply-wallpaperimage dbus-send; do
    cat > "$MOCK_BIN_LF/$cmd" <<'MOCK'
#!/usr/bin/env bash
exit 0
MOCK
    chmod +x "$MOCK_BIN_LF/$cmd"
  done

  if HOME="$LF_HOME" PATH="$MOCK_BIN_LF:$PATH" \
    "$CLI_DIR/theme-profile" save lf-test >/dev/null 2>&1; then
    ok "save copies the live kdeglobals into the profile"
  else
    fail "save lf-test failed"
  fi

  # Change the live value: applying the profile must restore the profile's one.
  HOME="$LF_HOME" kwriteconfig6 --file kdeglobals --group KDE --key LookAndFeelPackage glass

  if HOME="$LF_HOME" PATH="$MOCK_BIN_LF:$PATH" \
    "$CLI_DIR/theme-profile" apply lf-test >/dev/null 2>&1; then
    ok "apply lf-test exits 0 with mocked session commands"
  else
    fail "apply lf-test failed"
  fi

  lf_value="$(HOME="$LF_HOME" kreadconfig6 --file kdeglobals --group KDE --key LookAndFeelPackage)"
  if [ "$lf_value" = "paper" ]; then
    ok "apply restores LookAndFeelPackage from the profile (paper)"
  else
    fail "LookAndFeelPackage after apply is '$lf_value' (expected paper)"
  fi

  if grep -q "Usage: kreadconfig6" "$LF_HOME/.config/kdeglobals"; then
    fail "apply wrote kreadconfig6 --help into kdeglobals"
  else
    ok "apply does not write command help into kdeglobals"
  fi
fi

# --- 13b. apply re-applies the color scheme and accent ------------------------
echo "=== Section 13b: apply re-applies color scheme and accent ==="
if ! command -v kreadconfig6 >/dev/null 2>&1 || ! command -v kwriteconfig6 >/dev/null 2>&1; then
  echo "  ⏭  kreadconfig6/kwriteconfig6 not installed (KDE tooling) — skipping"
else
  ACC_HOME="$TMP_ROOT/accent-home"
  ACC_CALLS="$TMP_ROOT/accent-calls.log"
  export ACC_CALLS
  mkdir -p "$ACC_HOME/.config/kde-theme-profile" "$ACC_HOME/.config" "$ACC_HOME/.local/share/color-schemes"
  printf '[General]\nName=Breeze1\n' >"$ACC_HOME/.local/share/color-schemes/Breeze1.colors"
  : > "$ACC_CALLS"

  cat > "$ACC_HOME/.config/kdeglobals" <<'EOF'
[General]
ColorScheme=Breeze1
AccentColor=154,151,77
accentColorFromWallpaper=false
EOF
  cat > "$ACC_HOME/.config/plasma-org.kde.plasma.desktop-appletsrc" <<EOF
[Containments][1][Wallpaper][org.kde.image][General]
Image=file://$ACC_HOME/wallpaper.jpg
EOF
  touch "$ACC_HOME/wallpaper.jpg"

  MOCK_BIN_ACC="$TMP_ROOT/mock-bin-accent"
  mkdir -p "$MOCK_BIN_ACC"
  for cmd in systemctl qdbus6 kbuildsycoca6 plasma-apply-wallpaperimage dbus-send; do
    cat > "$MOCK_BIN_ACC/$cmd" <<'MOCK'
#!/usr/bin/env bash
exit 0
MOCK
    chmod +x "$MOCK_BIN_ACC/$cmd"
  done
  cat > "$MOCK_BIN_ACC/plasma-apply-colorscheme" <<'MOCK'
#!/usr/bin/env bash
echo "plasma-apply-colorscheme $*" >> "$ACC_CALLS"
exit 0
MOCK
  for cmd in dbus-send kwriteconfig6; do
    cat > "$MOCK_BIN_ACC/$cmd" <<MOCK
#!/usr/bin/env bash
echo "$cmd \$*" >> "\$ACC_CALLS"
exit 0
MOCK
    chmod +x "$MOCK_BIN_ACC/$cmd"
  done
  cat > "$MOCK_BIN_ACC/magick" <<'MOCK'
#!/usr/bin/env bash
echo "magick $*" >> "$ACC_CALLS"
printf 'srgb(154,151,77)'
MOCK
  chmod +x "$MOCK_BIN_ACC/plasma-apply-colorscheme" "$MOCK_BIN_ACC/magick"

  HOME="$ACC_HOME" PATH="$MOCK_BIN_ACC:$PATH" \
    "$CLI_DIR/theme-profile" save accent-explicit >/dev/null 2>&1
  HOME="$ACC_HOME" kwriteconfig6 --file kdeglobals --group General --key AccentColor 200,200,200
  if HOME="$ACC_HOME" PATH="$MOCK_BIN_ACC:$PATH" \
    "$CLI_DIR/theme-profile" apply accent-explicit >/dev/null 2>&1 \
    && grep -qx -- "plasma-apply-colorscheme Breeze1" "$ACC_CALLS" \
    && grep -qx -- "plasma-apply-colorscheme --accent-color #9a974d" "$ACC_CALLS"; then
    # Separate calls: with --accent-color the scheme argument is ignored.
    ok "apply re-applies an explicit profile accent"
  else
    fail "apply did not call plasma-apply-colorscheme with the explicit accent"
  fi

  : > "$ACC_CALLS"
  cat > "$ACC_HOME/.config/kdeglobals" <<'EOF'
[General]
ColorScheme=Breeze1
accentColorFromWallpaper=true
EOF
  HOME="$ACC_HOME" PATH="$MOCK_BIN_ACC:$PATH" \
    "$CLI_DIR/theme-profile" save accent-wallpaper >/dev/null 2>&1
  if HOME="$ACC_HOME" PATH="$MOCK_BIN_ACC:$PATH" \
    "$CLI_DIR/theme-profile" apply accent-wallpaper >/dev/null 2>&1 \
    && grep -qx -- "plasma-apply-colorscheme Breeze1" "$ACC_CALLS" \
    && ! grep -q -- "--accent-color" "$ACC_CALLS" \
    && ! grep -q '^magick ' "$ACC_CALLS"; then
    # Plasma's accent service recomputes it from the wallpaper when
    # plasmashell restarts — no ImageMagick approximation of our own.
    ok "apply leaves a wallpaper accent to Plasma (no magick, no forced accent)"
    # The accent service learns the flag only from KConfig notifications, and the
    # copied kdeglobals makes plasma-apply-colorscheme a no-op → notify + broadcast.
    if grep -q -- "kwriteconfig6 --notify --file kdeglobals --group General --key accentColorFromWallpaper false" "$ACC_CALLS" &&
      tail -n +1 "$ACC_CALLS" | grep -- "accentColorFromWallpaper" | tail -1 | grep -q "true$" &&
      grep -q "dbus-send --session --type=signal /KGlobalSettings org.kde.KGlobalSettings.notifyChange int32:0 int32:0" "$ACC_CALLS"; then
      ok "apply notifies the accent flag and broadcasts the palette change"
    else
      fail "apply did not notify the flag / broadcast the palette: $(cat "$ACC_CALLS")"
    fi
  else
    fail "apply forced/derived the wallpaper accent itself: $(cat "$ACC_CALLS")"
  fi
fi

# --- 13c. apply re-applies the Qt widget style and Kvantum theme --------------
echo "=== Section 13c: apply re-applies Qt widget style and Kvantum theme ==="
if ! command -v kreadconfig6 >/dev/null 2>&1 || ! command -v kwriteconfig6 >/dev/null 2>&1; then
  echo "  ⏭  kreadconfig6/kwriteconfig6 not installed (KDE tooling) — skipping"
else
  STYLE_HOME="$TMP_ROOT/style-home"
  STYLE_CALLS="$TMP_ROOT/style-calls.log"
  export STYLE_CALLS
  mkdir -p "$STYLE_HOME/.config/kde-theme-profile" "$STYLE_HOME/.config/Kvantum/Sandstone"
  : >"$STYLE_HOME/.config/Kvantum/Sandstone/Sandstone.kvconfig"
  : > "$STYLE_CALLS"
  cat > "$STYLE_HOME/.config/kdeglobals" <<'EOF'
[General]
ColorScheme=Breeze1

[KDE]
LookAndFeelPackage=Paper
widgetStyle=kvantum
EOF
  cat > "$STYLE_HOME/.config/Kvantum/kvantum.kvconfig" <<'EOF'
[General]
theme=Sandstone
EOF

  MOCK_BIN_STYLE="$TMP_ROOT/mock-bin-style"
  mkdir -p "$MOCK_BIN_STYLE"
  for cmd in systemctl qdbus6 kbuildsycoca6 plasma-apply-wallpaperimage dbus-send; do
    cat > "$MOCK_BIN_STYLE/$cmd" <<'MOCK'
#!/usr/bin/env bash
exit 0
MOCK
    chmod +x "$MOCK_BIN_STYLE/$cmd"
  done
  cat > "$MOCK_BIN_STYLE/plasma-apply-colorscheme" <<'MOCK'
#!/usr/bin/env bash
echo "plasma-apply-colorscheme $*" >> "$STYLE_CALLS"
exit 0
MOCK
  cat > "$MOCK_BIN_STYLE/kvantummanager" <<'MOCK'
#!/usr/bin/env bash
echo "kvantummanager $*" >> "$STYLE_CALLS"
exit 0
MOCK
  chmod +x "$MOCK_BIN_STYLE/plasma-apply-colorscheme" "$MOCK_BIN_STYLE/kvantummanager"

  HOME="$STYLE_HOME" PATH="$MOCK_BIN_STYLE:$PATH" \
    "$CLI_DIR/theme-profile" save qt-style-test >/dev/null 2>&1
  if HOME="$STYLE_HOME" PATH="$MOCK_BIN_STYLE:$PATH" \
    "$CLI_DIR/theme-profile" apply qt-style-test >/dev/null 2>&1 \
    && grep -q '^kvantummanager --set Sandstone$' "$STYLE_CALLS"; then
    ok "apply re-applies the Kvantum Qt widget theme"
  else
    fail "apply did not re-apply the Kvantum Qt widget theme"
  fi
fi

# --- 14. manifest.json, describe and custom components -------------------------
echo "=== Section 14: manifest.json / describe / components ==="

MAN_HOME="$TMP_ROOT/man-home"
mkdir -p "$MAN_HOME/.config"
printf '[General]\nAccentColor=5,6,7\n' > "$MAN_HOME/.config/kdeglobals"

# save writes the theme package manifest theme.json (schema theme@2).
if HOME="$MAN_HOME" "$CLI_DIR/theme-profile" save man-profile >/dev/null 2>&1; then
  ok "save man-profile"
else
  fail "save man-profile"
fi
man_root="$MAN_HOME/.config/kde-theme-profile/themes/man-profile"
if grep -q '"schema": "theme-profile/theme@2"' "$man_root/theme.json" 2>/dev/null \
  && grep -q '"name": "man-profile"' "$man_root/theme.json" \
  && grep -q '"accent_color": "5,6,7"' "$man_root/theme.json"; then
  ok "theme.json carries schema, name and the theme values"
else
  fail "theme.json content is wrong"
fi

# describe --set / describe round-trips through theme.json.
if HOME="$MAN_HOME" "$CLI_DIR/theme-profile" describe man-profile --set="Dark glass" >/dev/null 2>&1; then
  ok "describe --set exits 0"
else
  fail "describe --set failed"
fi
desc="$(HOME="$MAN_HOME" "$CLI_DIR/theme-profile" describe man-profile 2>/dev/null)"
if [ "$desc" = "Dark glass" ]; then
  ok "describe reads the description back"
else
  fail "describe round-trip broken (got: $desc)"
fi
if grep -q '"description": "Dark glass"' "$man_root/theme.json"; then
  ok "description lands in theme.json"
else
  fail "description missing from theme.json"
fi
if HOME="$MAN_HOME" "$CLI_DIR/theme-profile" describe no-such-profile --set=x >/dev/null 2>&1; then
  fail "describe on a missing profile should fail"
else
  ok "describe rejects a missing profile"
fi

# save --description writes the description in one step.
if HOME="$MAN_HOME" "$CLI_DIR/theme-profile" save desc-profile \
    --description="in one step" >/dev/null 2>&1 \
  && [ "$(HOME="$MAN_HOME" "$CLI_DIR/theme-profile" describe desc-profile 2>/dev/null)" = "in one step" ]; then
  ok "save --description round-trips"
else
  fail "save --description round-trip broken"
fi

# components CRUD: the registry lives in components.json next to the profiles.
comp_file="$MAN_HOME/.config/kde-theme-profile/components.json"
comp_json='{"id":"myapp","name":"My App","category":"Custom","config_paths":[".config/myapp/settings.json"],"description":"test"}'
if HOME="$MAN_HOME" "$CLI_DIR/theme-profile" components add "$comp_json" >/dev/null 2>&1; then
  ok "components add"
else
  fail "components add failed"
fi
if [ -f "$comp_file" ] && grep -q '"schema_version": 1' "$comp_file" && grep -q '"myapp"' "$comp_file"; then
  ok "components.json is written with schema v1"
else
  fail "components.json missing or malformed"
fi
if [ "$(HOME="$MAN_HOME" "$CLI_DIR/theme-profile" components list 2>/dev/null)" = "myapp" ]; then
  ok "components list returns the new id"
else
  fail "components list did not return myapp"
fi
if HOME="$MAN_HOME" "$CLI_DIR/theme-profile" components add "$comp_json" >/dev/null 2>&1; then
  fail "components add with a duplicate id should fail"
else
  ok "components add rejects a duplicate id"
fi
if HOME="$MAN_HOME" "$CLI_DIR/theme-profile" components add \
    '{"id":"../escape","config_paths":[".config/x"]}' >/dev/null 2>&1; then
  fail "components add with a traversal id should fail"
else
  ok "components add rejects a traversal id"
fi
comp_json_v2='{"id":"myapp","name":"My App 2","category":"Custom","config_paths":[".config/myapp/settings.json"],"description":"v2"}'
if HOME="$MAN_HOME" "$CLI_DIR/theme-profile" components edit "$comp_json_v2" >/dev/null 2>&1 \
  && grep -q 'My App 2' "$comp_file"; then
  ok "components edit updates the entry"
else
  fail "components edit failed"
fi
if HOME="$MAN_HOME" "$CLI_DIR/theme-profile" components remove myapp >/dev/null 2>&1 \
  && [ -z "$(HOME="$MAN_HOME" "$CLI_DIR/theme-profile" components list 2>/dev/null)" ]; then
  ok "components remove deletes the entry"
else
  fail "components remove failed"
fi

# --- 15. Custom component end-to-end: file sync + reload command ---------------
echo "=== Section 15: custom component (mock app) ==="

APP_HOME="$TMP_ROOT/app-home"
mkdir -p "$APP_HOME/.config/mockapp"
printf 'theme=dark\n' > "$APP_HOME/.config/mockapp/settings.conf"

# Mock the Plasma session commands (apply stops/starts the shell) and provide
# the mock app's reload command, which records its invocations.
MOCK_BIN_APP="$TMP_ROOT/mock-bin-app"
mkdir -p "$MOCK_BIN_APP"
for cmd in systemctl qdbus6 kbuildsycoca6 plasma-apply-wallpaperimage dbus-send; do
  cat > "$MOCK_BIN_APP/$cmd" <<'MOCK'
#!/usr/bin/env bash
exit 0
MOCK
  chmod +x "$MOCK_BIN_APP/$cmd"
done
RELOAD_LOG="$TMP_ROOT/reload.log"
cat > "$MOCK_BIN_APP/mockapp-reload" <<MOCK
#!/usr/bin/env bash
echo "reloaded" >> "$RELOAD_LOG"
MOCK
chmod +x "$MOCK_BIN_APP/mockapp-reload"

# Register the custom component: one config file + a reload command.
# (Registration validates the command's binary via PATH, so the mock dir must
# be on PATH for every CLI call that touches the registry.)
APP_PATH="$MOCK_BIN_APP:$PATH"
app_json='{"id":"mockapp","name":"Mock App","category":"Custom","config_paths":[".config/mockapp/settings.conf"],"reload_command":"mockapp-reload","supports_live_reload":true}'
if HOME="$APP_HOME" PATH="$APP_PATH" "$CLI_DIR/theme-profile" components add "$app_json" >/dev/null 2>&1; then
  ok "custom component registered"
else
  fail "custom component registration"
fi

# Save it into a profile, change the live file, apply: the profile value must
# come back and the reload command must have fired.
if HOME="$APP_HOME" PATH="$APP_PATH" "$CLI_DIR/theme-profile" save app-profile --components=mockapp >/dev/null 2>&1; then
  ok "save with the custom component"
else
  fail "save with the custom component"
fi
printf 'theme=light\n' > "$APP_HOME/.config/mockapp/settings.conf"
if HOME="$APP_HOME" PATH="$APP_PATH" "$CLI_DIR/theme-profile" apply app-profile --components=mockapp >/dev/null 2>&1; then
  ok "apply with the custom component"
else
  fail "apply with the custom component"
fi

synced="$(cat "$APP_HOME/.config/mockapp/settings.conf" 2>/dev/null)"
if [ "$synced" = "theme=dark" ]; then
  ok "custom component file restored from the profile"
else
  fail "custom component file not restored (got: $synced)"
fi

if [ -f "$RELOAD_LOG" ] && grep -q reloaded "$RELOAD_LOG"; then
  ok "reload command fired after apply"
else
  fail "reload command did not fire"
fi

# A shell-injection reload command must be rejected at registration.
if HOME="$APP_HOME" "$CLI_DIR/theme-profile" components add \
    '{"id":"evil","config_paths":[".config/mockapp/settings.conf"],"reload_command":"sh -c echo hi; rm -rf x"}' \
    >/dev/null 2>&1; then
  fail "components add accepted a reload command with shell metacharacters"
else
  ok "components add rejects shell metacharacters in reload_command"
fi


# --- 16. clone: profile duplication -------------------------------------------
echo "=== Section 16: clone duplicates a theme package ==="

CLONE_HOME="$TMP_ROOT/clone-home"
mkdir -p "$CLONE_HOME/.config"
printf '[General]\nAccentColor=9,8,7\n' > "$CLONE_HOME/.config/kdeglobals"

if HOME="$CLONE_HOME" "$CLI_DIR/theme-profile" save clone-src >/dev/null 2>&1; then
  ok "save clone-src"
else
  fail "save clone-src"
fi
if HOME="$CLONE_HOME" "$CLI_DIR/theme-profile" describe clone-src --set="Source theme" >/dev/null 2>&1; then
  ok "describe clone-src"
else
  fail "describe clone-src"
fi

# Happy path: package copied, theme.json renamed, description/created kept.
if HOME="$CLONE_HOME" "$CLI_DIR/theme-profile" clone clone-src clone-dst >/dev/null 2>&1; then
  ok "clone exits 0"
else
  fail "clone failed"
fi
src_root="$CLONE_HOME/.config/kde-theme-profile/themes/clone-src"
dst_root="$CLONE_HOME/.config/kde-theme-profile/themes/clone-dst"
if [ -f "$dst_root/theme.json" ] && grep -q '"accent_color": "9,8,7"' "$dst_root/theme.json"; then
  ok "clone copied the theme package"
else
  fail "clone did not copy the theme package"
fi
if grep -q '"name": "clone-dst"' "$dst_root/theme.json"; then
  ok "theme.json carries the new name"
else
  fail "theme.json name not updated"
fi
if grep -q '"description": "Source theme"' "$dst_root/theme.json"; then
  ok "clone keeps the description"
else
  fail "clone lost the description"
fi
src_created="$(grep -o '"created": "[^"]*"' "$src_root/theme.json")"
dst_created="$(grep -o '"created": "[^"]*"' "$dst_root/theme.json")"
if [ "$src_created" = "$dst_created" ]; then
  ok "clone keeps the original created_at"
else
  fail "clone changed created_at"
fi

# Source profile and the active marker must stay as they were.
if [ -f "$src_root/theme.json" ] \
  && [ "$(cat "$CLONE_HOME/.config/kde-theme-profile/active-profile" 2>/dev/null)" = "clone-src" ]; then
  ok "clone leaves the source and the active marker untouched"
else
  fail "clone disturbed the source or the active marker"
fi

# Error paths.
if HOME="$CLONE_HOME" "$CLI_DIR/theme-profile" clone clone-src clone-dst >/dev/null 2>&1; then
  fail "clone onto an existing profile should fail"
else
  ok "clone rejects an existing target"
fi
if HOME="$CLONE_HOME" "$CLI_DIR/theme-profile" clone no-such-profile dst2 >/dev/null 2>&1; then
  fail "clone from a missing profile should fail"
else
  ok "clone rejects a missing source"
fi
if HOME="$CLONE_HOME" "$CLI_DIR/theme-profile" clone clone-src "../evil" >/dev/null 2>&1; then
  fail "clone with a traversal target should fail"
else
  ok "clone rejects a traversal target"
fi


# --- 17. export / import (.zip and unpacked directory) ------------------------
echo "=== Section 17: export / import ==="

EX_A="$TMP_ROOT/export-a"   # the author
EX_B="$TMP_ROOT/export-b"   # the receiver (different home path)
mkdir -p "$EX_A/.config/kitty" "$EX_A/.config/wofi" "$EX_A/Pictures/Wallpaper" "$EX_B"
printf 'jpeg' >"$EX_A/Pictures/Wallpaper/sea.jpg"
printf 'background #282a36\nmap ctrl+a new_tab\n' >"$EX_A/.config/kitty/kitty.conf"
printf '@import "%s/.config/wofi/colors.css";\n' "$EX_A" >"$EX_A/.config/wofi/style.css"
cat >"$EX_A/.config/plasma-org.kde.plasma.desktop-appletsrc" <<APPLETS
[Containments][1]
lastScreen=0

[Containments][1][General]
positions={"1x1":["1","1","desktop:/secret-file.txt","0","0"]}
sortMode=-1

[Containments][1][Wallpaper][org.kde.image][General]
Image=file://$EX_A/Pictures/Wallpaper/sea.jpg
APPLETS
ex_a() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u WALLPAPER_DIR HOME="$EX_A" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
ex_b() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u WALLPAPER_DIR HOME="$EX_B" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
ex_a save sea >/dev/null 2>&1
ZIP="$TMP_ROOT/sea.ktmtheme.zip"
if ex_a export sea --out="$ZIP" >/dev/null 2>&1 && [ -f "$ZIP" ]; then
  ok "export writes a .zip"
else
  fail "export did not write $ZIP"
fi
EX_X="$TMP_ROOT/export-x"
mkdir -p "$EX_X" && (cd "$EX_X" && python3 -c 'import zipfile,sys; zipfile.ZipFile(sys.argv[1]).extractall(".")' "$ZIP")
# shellcheck disable=SC2016  # the literal token $HOME is what the export must contain
if ! grep -rqF "$EX_A" "$EX_X" && grep -qF '$HOME/.config/wofi/colors.css' "$EX_X/.config/wofi/style.css" &&
  grep -qx "background #282a36" "$EX_X/.config/kitty/theme-profile.conf" && [ ! -e "$EX_X/.config/kitty/kitty.conf" ] &&
  ! grep -q "secret-file" "$EX_X/.config/plasma-org.kde.plasma.desktop-appletsrc" &&
  ! grep -q "sortMode" "$EX_X/.config/plasma-org.kde.plasma.desktop-appletsrc" &&
  grep -q "Wallpaper/sea.jpg" "$EX_X/.config/plasma-org.kde.plasma.desktop-appletsrc" && [ -f "$EX_X/export.json" ]; then
  ok "export is portable: \$HOME for the home path, only the look (no kitty.conf keys, no layout), no desktop icons"
else
  fail "export leaks the home path or desktop icons"
fi

if ex_b import "$ZIP" | grep -q "^Imported theme: sea" &&
  grep -qF "@import \"$EX_B/.config/wofi/colors.css\"" "$EX_B/.config/kde-theme-profile/themes/sea/.config/wofi/style.css" &&
  [ -f "$EX_B/.config/kde-theme-profile/themes/sea/wallpaper/sea.jpg" ]; then
  ok "import expands \$HOME to the receiver's home and brings the wallpaper"
else
  fail "import did not restore the theme for the receiver"
fi
mkdir -p "$EX_B/.config/kitty"
printf 'map ctrl+b close_tab\n' >"$EX_B/.config/kitty/kitty.conf"
ex_b apply sea >/dev/null 2>&1
if grep -qx "background #282a36" "$EX_B/.config/kitty/theme-profile.conf" &&
  grep -qx "map ctrl+b close_tab" "$EX_B/.config/kitty/kitty.conf" && grep -qx "include theme-profile.conf" "$EX_B/.config/kitty/kitty.conf" &&
  grep -qF "$EX_B/.config/wofi/colors.css" "$EX_B/.config/wofi/style.css"; then
  ok "an imported theme applies on the receiver's machine and keeps their kitty keys"
else
  fail "applying the imported theme failed"
fi
if ex_b import "$ZIP" >/dev/null 2>&1; then
  fail "import overwrote an existing theme without --force"
else
  ok "import refuses an existing name without --force"
fi
if ex_b import "$ZIP" --force >/dev/null 2>&1 &&
  compgen -G "$EX_B/.config/kde-theme-profile/backups/pre-import-sea-*" >/dev/null; then
  ok "import --force replaces the theme and backs up the old one"
else
  fail "import --force did not back up the replaced theme"
fi
if ex_b import "$EX_X" --name=sea-dir | grep -q "^Imported theme: sea-dir" &&
  grep -q '"name": "sea-dir"' "$EX_B/.config/kde-theme-profile/themes/sea-dir/theme.json"; then
  ok "import accepts an unpacked theme directory and --name"
else
  fail "directory import / --name failed"
fi

# Hostile or damaged archives.
EVIL="$TMP_ROOT/evil.zip"
python3 -c 'import zipfile,sys; z=zipfile.ZipFile(sys.argv[1],"w"); z.writestr("../../evil.txt","x"); z.close()' "$EVIL"
if ex_b import "$EVIL" >/dev/null 2>&1 || [ -e "$TMP_ROOT/evil.txt" ]; then
  fail "zip-slip archive was accepted"
else
  ok "import refuses paths that escape the theme (zip-slip)"
fi
printf 'tampered\n' >>"$EX_X/.config/kitty/theme-profile.conf"
if ex_b import "$EX_X" --name=tampered >/dev/null 2>&1; then
  fail "a modified file passed the checksum check"
else
  ok "import refuses files whose SHA-256 does not match export.json"
fi
if [ -e "$EX_B/.config/kde-theme-profile/themes/tampered" ] || compgen -G "$EX_B/.config/kde-theme-profile/themes/.import-*" >/dev/null; then
  fail "a refused import left files behind"
else
  ok "a refused import leaves nothing behind"
fi

# --- 18. dependencies: sources detected at save, listed where missing ---------
echo "=== Section 18: theme dependencies ==="
DEP_A="$TMP_ROOT/deps-a"; DEP_B="$TMP_ROOT/deps-b"
mkdir -p "$DEP_A/.local/share/icons/Candy" "$DEP_A/.local/share/knewstuff3" "$DEP_A/.config" "$DEP_B"
printf '[Icon Theme]\nName=Candy\n' >"$DEP_A/.local/share/icons/Candy/index.theme"
printf '[Icons]\nTheme=Candy\n' >"$DEP_A/.config/kdeglobals"
cat >"$DEP_A/.local/share/knewstuff3/icons.knsregistry" <<REG
<?xml version="1.0" encoding="UTF-8"?>
<hotnewstuffregistry>
 <stuff category="132">
  <name>Candy icons</name>
  <providerid>api.kde-look.org</providerid>
  <installedfile>$DEP_A/.local/share/icons/Candy/*</installedfile>
  <id>1305251</id>
 </stuff>
</hotnewstuffregistry>
REG
dep_a() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$DEP_A" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
dep_b() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$DEP_B" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
dep_a save candy >/dev/null 2>&1
if python3 -c 'import json,sys; d=json.load(open(sys.argv[1]))["dependencies"]; assert any(x["kind"]=="icons" and x["name"]=="Candy" and x["source"]=="kde-store" and x["id"]=="1305251" and x["exact"] for x in d), d' \
  "$DEP_A/.config/kde-theme-profile/themes/candy/theme.json" 2>/dev/null; then
  ok "save records the KDE Store id of icons installed via Get New… (KNewStuff registry)"
else
  fail "dependency source not detected: $(cat "$DEP_A/.config/kde-theme-profile/themes/candy/theme.json")"
fi
if [ "$(dep_a deps candy)" = "All dependencies are installed." ]; then
  ok "deps lists nothing on the machine that has everything"
else
  fail "deps on the author's machine: $(dep_a deps candy)"
fi
dep_a export candy --out="$TMP_ROOT/candy.zip" >/dev/null 2>&1
report=$(dep_b import "$TMP_ROOT/candy.zip" --json 2>/dev/null)
if printf '%s' "$report" | python3 -c 'import json,sys; r=json.load(sys.stdin); d=r["dependencies"]; assert d and d[0]["id"]=="1305251" and d[0]["url"].endswith("/p/1305251"), r' 2>/dev/null &&
  dep_b deps candy | grep -q "icons Candy — kde-store"; then
  ok "import reports the missing icons with their KDE Store source"
else
  fail "import/deps did not report the missing dependency: $report"
fi
if out=$(env -u KTM_THEME_NO_APPLY -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$DEP_B" KTM_OFFLINE=1 "$CLI_DIR/theme-profile" deps candy --install=0 2>&1); then
  fail "install worked offline?! $out"
else
  case "$out" in *"network is disabled"*) ok "installing respects the offline switch" ;; *) fail "unexpected install output: $out" ;; esac
fi
if out=$(dep_b deps candy --install=0 2>&1); then
  fail "install worked with KTM_THEME_NO_APPLY=1: $out"
elif [[ "$out" == *"installation disabled"* ]] && [ ! -e "$DEP_B/.local/share/icons/Candy" ]; then
  ok "KTM_THEME_NO_APPLY=1 blocks dependency installation without creating files"
else
  fail "unexpected no-apply install result: $out"
fi

# --- 19. Konsole: scheme + profile appearance, user's behaviour kept ----------
echo "=== Section 19: Konsole ==="
KO_A="$TMP_ROOT/konsole-a"; KO_B="$TMP_ROOT/konsole-b"
mkdir -p "$KO_A/.config" "$KO_A/.local/share/konsole" "$KO_B/.config" "$KO_B/.local/share/konsole"
printf '[Desktop Entry]\nDefaultProfile=Mine.profile\n' >"$KO_A/.config/konsolerc"
printf '[Appearance]\nColorScheme=Night\nFont=Inter,11,-1,5,50,0,0,0,0,0\n\n[General]\nCommand=/bin/zsh\nName=Mine\n\n[Scrolling]\nHistorySize=9999\n' >"$KO_A/.local/share/konsole/Mine.profile"
printf '[Background]\nColor=1,2,3\n\n[General]\nDescription=Night\nOpacity=0.8\n' >"$KO_A/.local/share/konsole/Night.colorscheme"
ko_a() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$KO_A" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
ko_b() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$KO_B" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
ko_a save night >/dev/null 2>&1
ko_store="$KO_A/.config/kde-theme-profile/themes/night/.local/share/konsole"
if grep -q "ColorScheme=Night" "$ko_store/Mine.profile" && ! grep -q "Command\|HistorySize" "$ko_store/Mine.profile" &&
  grep -q "Opacity=0.8" "$ko_store/Night.colorscheme"; then
  ok "save stores the Konsole scheme and only the profile's [Appearance]"
else
  fail "Konsole save wrong: $(cat "$ko_store/Mine.profile" 2>&1)"
fi
ko_a export night --out="$TMP_ROOT/night.zip" >/dev/null 2>&1
# Receiver: own profile with another name, own command and font size.
printf '[Desktop Entry]\nDefaultProfile=Work.profile\n' >"$KO_B/.config/konsolerc"
printf '[Appearance]\nColorScheme=Breeze\nFont=Hack,14,-1,5,50,0,0,0,0,0\n\n[General]\nCommand=/usr/bin/fish\nName=Work\n' >"$KO_B/.local/share/konsole/Work.profile"
ko_b import "$TMP_ROOT/night.zip" --apply >/dev/null 2>&1
ko_work="$KO_B/.local/share/konsole/Work.profile"
if grep -q "ColorScheme=Night" "$ko_work" && grep -q "Font=Inter,14," "$ko_work" && grep -q "Command=/usr/bin/fish" "$ko_work" &&
  [ -f "$KO_B/.local/share/konsole/Night.colorscheme" ]; then
  ok "applying puts the scheme into the receiver's own profile, keeping their command and font size"
else
  fail "Konsole apply wrong: $(cat "$ko_work") // $(ls "$KO_B/.local/share/konsole")"
fi
# A receiver without any profile gets a "Theme Profile" made default.
KO_C="$TMP_ROOT/konsole-c"; mkdir -p "$KO_C"
env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$KO_C" KTM_THEME_NO_APPLY=1 PATH="$SAFE_BIN:/usr/bin:/bin" "$CLI_DIR/theme-profile" import "$TMP_ROOT/night.zip" --apply >/dev/null 2>&1
if command -v konsole >/dev/null 2>&1; then
  if grep -q "ColorScheme=Night" "$KO_C/.local/share/konsole/Theme Profile.profile" 2>/dev/null &&
    grep -q "Name=Theme Profile" "$KO_C/.local/share/konsole/Theme Profile.profile" &&
    grep -q "DefaultProfile=Theme Profile.profile" "$KO_C/.config/konsolerc"; then
    ok "without a Konsole profile a \"Theme Profile\" is created and made default"
  else
    fail "Theme Profile not created: $(ls -R "$KO_C/.local/share/konsole" 2>&1) // $(cat "$KO_C/.config/konsolerc" 2>&1)"
  fi
else
  ok "Konsole not installed here: profile creation skipped"
fi

# --- 20. Firefox: userChrome + look prefs, profile folder differs ------------
echo "=== Section 20: Firefox ==="
FF_A="$TMP_ROOT/ff-a"; FF_B="$TMP_ROOT/ff-b"
mkdir -p "$FF_A/.mozilla/firefox/aaa.default-release/chrome" "$FF_B/.mozilla/firefox/zzz.default-release"
printf '[Install1]\nDefault=aaa.default-release\n' >"$FF_A/.mozilla/firefox/installs.ini"
printf '[Install9]\nDefault=zzz.default-release\n' >"$FF_B/.mozilla/firefox/installs.ini"
printf 'user_pref("toolkit.legacyUserProfileCustomizations.stylesheets", true);\nuser_pref("privacy.resistFingerprinting", true);\n' >"$FF_A/.mozilla/firefox/aaa.default-release/user.js"
printf '#nav-bar { background: #282a36; }\n' >"$FF_A/.mozilla/firefox/aaa.default-release/chrome/userChrome.css"
printf 'user_pref("privacy.resistFingerprinting", false);\nuser_pref("browser.startup.page", 3);\n' >"$FF_B/.mozilla/firefox/zzz.default-release/user.js"
ff() { local h="$1"; shift; env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$h" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
ff "$FF_A" save fox >/dev/null 2>&1
ff "$FF_A" export fox --out="$TMP_ROOT/fox.zip" >/dev/null 2>&1
ff "$FF_B" import "$TMP_ROOT/fox.zip" --apply >/dev/null 2>&1
ffb="$FF_B/.mozilla/firefox/zzz.default-release"
if grep -q "#282a36" "$ffb/chrome/userChrome.css" 2>/dev/null && grep -q "legacyUserProfileCustomizations.stylesheets\", true" "$ffb/user.js" &&
  grep -q "resistFingerprinting\", false" "$ffb/user.js" && grep -q "browser.startup.page" "$ffb/user.js" &&
  [ ! -e "$FF_B/.mozilla/firefox/aaa.default-release" ]; then
  ok "Firefox look lands in the receiver's own profile; their privacy/startup prefs stay"
else
  fail "Firefox apply wrong: $(find "$FF_B/.mozilla/firefox" | head -20) // $(cat "$ffb/user.js" 2>&1)"
fi

# --- 22. Discord (Vesktop/Vencord): generated CSS theme, only theme settings --
echo "=== Section 22: Discord ==="
DC_A="$TMP_ROOT/dc-a"; DC_B="$TMP_ROOT/dc-b"; DC_C="$TMP_ROOT/dc-c"
mkdir -p "$DC_A/.config/vesktop/settings" "$DC_B/.config/Vencord/settings" "$DC_C"
printf '{\n  "minimizeToTray": true,\n  "enabledThemes": []\n}\n' >"$DC_A/.config/vesktop/settings/settings.json"
printf '{\n  "autoUpdate": false,\n  "plugins": {"x": {"enabled": true}}\n}\n' >"$DC_B/.config/Vencord/settings/settings.json"
dc() { local h="$1"; shift; env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$h" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
dc "$DC_A" palette import "$BASE16" >/dev/null 2>&1
dc "$DC_A" palette apply --targets=discord >/dev/null 2>&1
if grep -q "midnight-discord" "$DC_A/.config/vesktop/themes/theme-profile.theme.css" 2>/dev/null &&
  grep -q '"theme-profile.theme.css"' "$DC_A/.config/vesktop/settings/settings.json" &&
  grep -q '"minimizeToTray": true' "$DC_A/.config/vesktop/settings/settings.json"; then
  ok "palette apply writes and enables a Discord theme in Vesktop, keeping other settings"
else
  fail "Discord palette apply wrong: $(cat "$DC_A/.config/vesktop/settings/settings.json")"
fi
dc "$DC_A" save disc >/dev/null 2>&1
dc_store="$DC_A/.config/kde-theme-profile/themes/disc/.config/vesktop"
if grep -q enabledThemes "$dc_store/settings/settings.json" && ! grep -q minimizeToTray "$dc_store/settings/settings.json" &&
  [ -f "$dc_store/themes/theme-profile.theme.css" ]; then
  ok "save stores the Discord themes and only enabledThemes/themeLinks"
else
  fail "Discord save wrong: $(cat "$dc_store/settings/settings.json" 2>&1)"
fi
dc "$DC_A" export disc --out="$TMP_ROOT/disc.zip" >/dev/null 2>&1
dc "$DC_B" import "$TMP_ROOT/disc.zip" --apply >/dev/null 2>&1
if [ -f "$DC_B/.config/Vencord/themes/theme-profile.theme.css" ] && grep -q '"autoUpdate": false' "$DC_B/.config/Vencord/settings/settings.json" &&
  grep -q '"theme-profile.theme.css"' "$DC_B/.config/Vencord/settings/settings.json" && [ ! -e "$DC_B/.config/vesktop" ]; then
  ok "a Vesktop theme lands in a Vencord user's client, keeping their settings"
else
  fail "Vencord settings changed: $(cat "$DC_B/.config/Vencord/settings/settings.json")"
fi
dc "$DC_C" import "$TMP_ROOT/disc.zip" >/dev/null 2>&1
if dc "$DC_C" deps disc | grep -q "discord-client Vesktop"; then
  ok "without Vencord/Vesktop the receiver is told to get a Discord theming client"
else
  fail "no discord-client dependency: $(dc "$DC_C" deps disc)"
fi

# --- 23. Spotify (Spicetify): generated colour scheme, only theme keys ------
echo "=== Section 23: Spicetify ==="
SP_A="$TMP_ROOT/sp-a"; SP_B="$TMP_ROOT/sp-b"
mkdir -p "$SP_A/.config/spicetify" "$SP_B/.config/spicetify"
printf '[Setting]\nspotify_path = /opt/spotify-a\ncurrent_theme = Old\ncolor_scheme = dark\n\n[AdditionalOptions]\nextensions = mine.js\n' >"$SP_A/.config/spicetify/config-xpui.ini"
printf '[Setting]\nspotify_path = /usr/share/spotify\ncurrent_theme = Other\n\n[AdditionalOptions]\nextensions = theirs.js\n' >"$SP_B/.config/spicetify/config-xpui.ini"
sp() { local h="$1"; shift; env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$h" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
sp "$SP_A" palette import "$BASE16" >/dev/null 2>&1
sp "$SP_A" palette apply --targets=spicetify >/dev/null 2>&1
spa="$SP_A/.config/spicetify"
if grep -q "^\[ThemeProfile\]" "$spa/Themes/ThemeProfile/color.ini" 2>/dev/null && grep -q "current_theme=ThemeProfile" "$spa/config-xpui.ini" &&
  grep -q "spotify_path = /opt/spotify-a" "$spa/config-xpui.ini"; then
  ok "palette apply writes a Spicetify colour scheme and selects it, keeping spotify_path"
else
  fail "Spicetify palette apply wrong: $(cat "$spa/config-xpui.ini")"
fi
sp "$SP_A" save spot >/dev/null 2>&1
sp_store="$SP_A/.config/kde-theme-profile/themes/spot/.config/spicetify"
if grep -q "current_theme" "$sp_store/config-xpui.ini" && ! grep -q "spotify_path\|extensions" "$sp_store/config-xpui.ini"; then
  ok "save stores only the Spicetify theme keys"
else
  fail "Spicetify save wrong: $(cat "$sp_store/config-xpui.ini" 2>&1)"
fi
sp "$SP_A" export spot --out="$TMP_ROOT/spot.zip" >/dev/null 2>&1
sp "$SP_B" import "$TMP_ROOT/spot.zip" --apply >/dev/null 2>&1
spb="$SP_B/.config/spicetify"
if grep -q "current_theme=ThemeProfile" "$spb/config-xpui.ini" && grep -q "spotify_path = /usr/share/spotify" "$spb/config-xpui.ini" &&
  grep -q "extensions = theirs.js" "$spb/config-xpui.ini" && [ -f "$spb/Themes/ThemeProfile/color.ini" ]; then
  ok "the receiver gets the Spicetify theme and keeps their Spotify path and extensions"
else
  fail "Spicetify apply wrong: $(cat "$spb/config-xpui.ini")"
fi

# --- 24. Shell & small apps: oh-my-posh, fish colours, cava, ghostty, bat ----
echo "=== Section 24: oh-my-posh, fish, cava, ghostty, bat ==="
SA="$TMP_ROOT/sh-a"; SB="$TMP_ROOT/sh-b"
mkdir -p "$SA/.config/fish" "$SA/themes" "$SA/.config/cava" "$SA/.config/ghostty" "$SA/.config/bat" \
  "$SB/.config/fish" "$SB/.config/cava" "$SB/.config/ghostty" "$SB/.config/bat"
printf '{"blocks":[],"version":3,"name":"aliens"}\n' >"$SA/themes/aliens.omp.json"
printf "oh-my-posh init fish --config '%s/themes/aliens.omp.json' | source  # [omp-manager]\nabbr -a g git\n" "$SA" >"$SA/.config/fish/config.fish"
printf 'SETUVAR __fish_initialized:4300\nSETUVAR fish_color_command:bd93f9\nSETUVAR fish_user_paths:/a\n' >"$SA/.config/fish/fish_variables"
printf '[general]\nframerate = 60\n\n[color]\ngradient = 1\ngradient_color_1 = %s\n' "'#ff0000'" >"$SA/.config/cava/config"
printf 'font-size = 12\nbackground = #282a36\nkeybind = ctrl+a=new_tab\n' >"$SA/.config/ghostty/config"
printf -- '--theme="Dracula"\n--style=numbers\n' >"$SA/.config/bat/config"
sh_cli() { local h="$1"; shift; env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$h" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
sh_cli "$SA" save shells >/dev/null 2>&1
st="$SA/.config/kde-theme-profile/themes/shells"
if grep -q '"name":"aliens"' "$st/.config/oh-my-posh/theme-profile.omp.json" 2>/dev/null && [ ! -e "$st/.config/fish/config.fish" ] &&
  grep -q "fish_color_command" "$st/.config/fish/fish_variables" && ! grep -q "fish_user_paths" "$st/.config/fish/fish_variables" &&
  grep -q "gradient_color_1" "$st/.config/cava/config" && ! grep -q "framerate" "$st/.config/cava/config" &&
  grep -q "background = #282a36" "$st/.config/ghostty/theme-profile" && [ ! -e "$st/.config/ghostty/config" ] &&
  grep -q -- '--theme="Dracula"' "$st/.config/bat/config" && ! grep -q -- "--style" "$st/.config/bat/config" &&
  grep -q "oh-my-posh init fish --config '$SA/themes/aliens.omp.json'" "$SA/.config/fish/config.fish"; then
  ok "save stores the prompt theme, fish colours, cava [color], ghostty look, bat theme — no personal lines, no live change"
else
  fail "small-apps save wrong: $(find "$st/.config" -maxdepth 2 | head -30)"
fi
sh_cli "$SA" export shells --out="$TMP_ROOT/shells.zip" >/dev/null 2>&1
printf "eval (oh-my-posh init fish --config ~/mine.omp.json)\nset -g fish_greeting\n" >"$SB/.config/fish/config.fish"
printf 'SETUVAR fish_color_command:blue\nSETUVAR fish_user_paths:/b\n' >"$SB/.config/fish/fish_variables"
printf '[general]\nframerate = 144\n\n[color]\nforeground = %s\n' "'#00ff00'" >"$SB/.config/cava/config"
printf 'font-size = 15\nkeybind = ctrl+b=close_surface\n' >"$SB/.config/ghostty/config"
printf -- '--theme="TwoDark"\n--paging=never\n' >"$SB/.config/bat/config"
sh_cli "$SB" import "$TMP_ROOT/shells.zip" --apply >/dev/null 2>&1
bad=""
# shellcheck disable=SC2016  # the literal $HOME is what the shell config must contain
grep -qF 'oh-my-posh init fish --config "$HOME/.config/oh-my-posh/theme-profile.omp.json")' "$SB/.config/fish/config.fish" || bad="$bad omp-line"
grep -q "set -g fish_greeting" "$SB/.config/fish/config.fish" || bad="$bad fish-config"
[ -f "$SB/.config/oh-my-posh/theme-profile.omp.json" ] || bad="$bad omp-file"
grep -q "fish_color_command:bd93f9" "$SB/.config/fish/fish_variables" && grep -q "fish_user_paths:/b" "$SB/.config/fish/fish_variables" || bad="$bad fish-vars"
grep -q "framerate = 144" "$SB/.config/cava/config" && grep -q "gradient_color_1" "$SB/.config/cava/config" && ! grep -q "#00ff00" "$SB/.config/cava/config" || bad="$bad cava"
grep -q "font-size = 15" "$SB/.config/ghostty/config" && [ "$(tail -1 "$SB/.config/ghostty/config")" = "config-file = ?theme-profile" ] && grep -q "background = #282a36" "$SB/.config/ghostty/theme-profile" || bad="$bad ghostty"
grep -q -- '--theme="Dracula"' "$SB/.config/bat/config" && grep -q -- "--paging=never" "$SB/.config/bat/config" || bad="$bad bat"
if [ -z "$bad" ]; then
  ok "applying brings prompt, fish colours, cava, ghostty, bat looks and keeps the receiver's own settings"
else
  fail "small-apps apply wrong:$bad"
fi

# --- 25. Only the theme's own assets; prune old themes safely ----------------
echo "=== Section 25: own assets only / prune ==="
PR="$TMP_ROOT/prune-home"
mkdir -p "$PR/.config/Kvantum/Mine" "$PR/.config/Kvantum/Other" "$PR/.local/share/color-schemes"
printf '[General]\ntheme=Mine\n' >"$PR/.config/Kvantum/kvantum.kvconfig"
printf '[%%General]\n' >"$PR/.config/Kvantum/Mine/Mine.kvconfig"
printf '[%%General]\n' >"$PR/.config/Kvantum/Other/Other.kvconfig"
printf '[General]\nColorScheme=MyScheme\n\n[KDE]\nwidgetStyle=kvantum\n' >"$PR/.config/kdeglobals"
printf '[General]\nName=MyScheme\n' >"$PR/.local/share/color-schemes/MyScheme.colors"
printf '[General]\nName=Unused\n' >"$PR/.local/share/color-schemes/Unused.colors"
pr() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$PR" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
pr save own >/dev/null 2>&1
own="$PR/.config/kde-theme-profile/themes/own"
if [ -f "$own/.config/Kvantum/Mine/Mine.kvconfig" ] && [ ! -e "$own/.config/Kvantum/Other" ] &&
  [ -f "$own/.local/share/color-schemes/MyScheme.colors" ] && [ ! -e "$own/.local/share/color-schemes/Unused.colors" ]; then
  ok "save carries only the theme's own Kvantum family and colour scheme"
else
  fail "save carried foreign assets: $(find "$own" -name '*.kvconfig' -o -name '*.colors' | sed "s|$own/||")"
fi
# An old-style theme with foreign assets and leftovers is pruned (backup first).
cp -a "$PR/.config/Kvantum/Other" "$own/.config/Kvantum/"
cp "$PR/.local/share/color-schemes/Unused.colors" "$own/.local/share/color-schemes/"
mkdir -p "$own/metadata/kde"; printf 'diag\n' >"$own/metadata/kde/summary.txt"; printf '[FileDialog]\n' >"$own/.config/QtProject.conf"
if pr prune own | grep -q "^Pruned own: 4 file" && [ ! -e "$own/.config/Kvantum/Other" ] && [ ! -e "$own/metadata" ] &&
  [ ! -e "$own/.config/QtProject.conf" ] && [ -f "$own/.config/Kvantum/Mine/Mine.kvconfig" ] &&
  compgen -G "$PR/.config/kde-theme-profile/backups/pre-prune-own-*/.config/Kvantum/Other/Other.kvconfig" >/dev/null; then
  ok "prune drops foreign assets, diagnostics and untracked leftovers (full backup first)"
else
  fail "prune wrong: $(find "$own" -type f | sed "s|$own/||" | sort | tr '\n' ' ')"
fi
# A receiver with another active scheme still gets the theme's own scheme restored.
PR2="$TMP_ROOT/prune-recv"; mkdir -p "$PR2/.config"
printf '[General]\nColorScheme=BreezeDark\n' >"$PR2/.config/kdeglobals"
pr export own --out="$TMP_ROOT/own.zip" >/dev/null 2>&1
env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$PR2" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" import "$TMP_ROOT/own.zip" --apply >/dev/null 2>&1
if [ -f "$PR2/.local/share/color-schemes/MyScheme.colors" ] && [ -f "$PR2/.config/Kvantum/Mine/Mine.kvconfig" ] &&
  grep -q "ColorScheme=MyScheme" "$PR2/.config/kdeglobals"; then
  ok "applying restores the theme's own assets even where another scheme was active"
else
  fail "theme assets not restored: $(find "$PR2" -type f | sed "s|$PR2/||" | grep -v theme-profile)"
fi

# --- 26. GTK follows icon/cursor/GTK fields; own store-install record -------
echo "=== Section 26: GTK mirror and own install record ==="
GM="$TMP_ROOT/gtk-mirror"; mkdir -p "$GM/.config/gtk-3.0" "$GM/.config/gtk-4.0"
printf '[Settings]\ngtk-icon-theme-name=breeze\ngtk-xft-dpi=98304\n' >"$GM/.config/gtk-3.0/settings.ini"
printf '[Settings]\ngtk-theme-name=Old\n' >"$GM/.config/gtk-4.0/settings.ini"
gm() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$GM" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
gm field set icon_theme=Seven cursor_theme=aero gtk_theme=Glass gtk_dark=true >/dev/null 2>&1
if grep -qx "gtk-icon-theme-name=Seven" "$GM/.config/gtk-4.0/settings.ini" && grep -qx "gtk-icon-theme-name=Seven" "$GM/.config/gtk-3.0/settings.ini" &&
  grep -qx "gtk-cursor-theme-name=aero" "$GM/.config/gtk-4.0/settings.ini" && grep -qx "gtk-theme-name=Glass" "$GM/.config/gtk-4.0/settings.ini" &&
  grep -qx "gtk-application-prefer-dark-theme=true" "$GM/.config/gtk-4.0/settings.ini" && grep -qx "gtk-xft-dpi=98304" "$GM/.config/gtk-3.0/settings.ini"; then
  ok "icon/cursor/GTK fields are mirrored into GTK 3 and GTK 4 settings (other keys kept)"
else
  fail "GTK settings not mirrored: $(cat "$GM/.config/gtk-3.0/settings.ini" "$GM/.config/gtk-4.0/settings.ini" | tr '\n' ' ')"
fi
# A KDE Store item installed by `deps --install` is remembered with its id, so
# a later save records the exact source (KNewStuff knows only its own installs).
mkdir -p "$GM/.local/share/icons/Seven" "$GM/.config/kde-theme-profile"
printf '[Icon Theme]\nName=Seven\n' >"$GM/.local/share/icons/Seven/index.theme"
printf '{"%s": {"store": "kde-store", "id": "1631627", "title": "Seven Icons"}}\n' "$GM/.local/share/icons/Seven" >"$GM/.config/kde-theme-profile/installed-assets.json"
gm save seven >/dev/null 2>&1
if python3 -c 'import json,sys; d=json.load(open(sys.argv[1]))["dependencies"]; assert any(x["name"]=="Seven" and x["id"]=="1631627" and x["exact"] for x in d), d' \
  "$GM/.config/kde-theme-profile/themes/seven/theme.json" 2>/dev/null; then
  ok "save records the KDE Store id of assets that deps --install fetched"
else
  fail "own install record ignored: $(cat "$GM/.config/kde-theme-profile/themes/seven/theme.json" 2>/dev/null | head -c 600)"
fi

# Editor themes: no editor here → nothing to fetch, nothing reported.
ED="$TMP_ROOT/editor-absent"; mkdir -p "$ED/.config/kde-theme-profile/themes/ed" "$ED/bin"
cat >"$ED/.config/kde-theme-profile/themes/ed/theme.json" <<'JSON'
{"schema": "theme-profile/theme@2", "name": "ed", "values": {"vscode_theme": "Night Owl"}, "assets": [], "appearance_only": true,
 "sources": {"vscode_theme": {"store": "vscode-marketplace", "id": "sdras.night-owl", "url": ""}},
 "dependencies": [{"kind": "vscode", "name": "Night Owl", "source": "vscode-marketplace", "id": "sdras.night-owl", "title": "", "url": "https://marketplace.visualstudio.com/items?itemName=sdras.night-owl", "exact": true}]}
JSON
ed_cli() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$ED" PATH="$ED/bin" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
if [ "$(ed_cli deps ed)" = "All dependencies are installed." ] && ed_cli check ed | grep -q "^Everything this theme needs"; then
  ok "a theme for an editor that is not installed is neither listed nor reported missing"
else
  fail "absent editor still reported: $(ed_cli deps ed; ed_cli check ed)"
fi
printf '#!/bin/sh\nexit 0\n' >"$ED/bin/code"; chmod +x "$ED/bin/code"
if ed_cli deps ed | grep -q "vscode Night Owl" && ed_cli check ed | grep -q "missing=vscode_theme=Night Owl"; then
  ok "with VS Code installed its missing theme is listed with the source"
else
  fail "installed editor's theme not listed: $(ed_cli deps ed; ed_cli check ed)"
fi

# Palette → libadwaita: translucent colours in gtk-4.0/theme-profile.css,
# imported last from gtk.css (KDE's colors.css import kept).
GP="$TMP_ROOT/gtk-palette"; mkdir -p "$GP/.config/gtk-4.0"; printf "@import 'colors.css';\n" >"$GP/.config/gtk-4.0/gtk.css"
gp() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$GP" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
gp palette import gallery:dracula >/dev/null 2>&1
gp palette apply --targets=gtk --gtk-opacity=0.6 >/dev/null 2>&1; gp palette apply --targets=gtk --gtk-opacity=0.6 >/dev/null 2>&1
if grep -q "@define-color window_bg_color alpha(#[0-9a-f]*, 0.60);" "$GP/.config/gtk-4.0/theme-profile.css" 2>/dev/null &&
  [ "$(cat "$GP/.config/gtk-4.0/gtk.css")" = "$(printf "@import 'colors.css';\n@import 'theme-profile.css';")" ]; then
  ok "palette gtk target writes translucent libadwaita colours and imports them once"
else
  fail "gtk target: $(cat "$GP/.config/gtk-4.0/gtk.css" "$GP/.config/gtk-4.0/theme-profile.css" 2>&1 | head -5)"
fi

# A user Aurorae decoration travels inside the theme (not a dependency).
AU="$TMP_ROOT/aurorae"; mkdir -p "$AU/.local/share/aurorae/themes/MineAero" "$AU/.config"
printf '[General]\nActiveTextColor=0,0,0,255\n' >"$AU/.local/share/aurorae/themes/MineAero/MineAerorc"
printf '[org.kde.kdecoration2]\nlibrary=org.kde.kwin.aurorae\ntheme=__aurorae__svg__MineAero\n' >"$AU/.config/kwinrc"
au() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$AU" KTM_THEME_NO_APPLY=1 KTM_OFFLINE=1 "$CLI_DIR/theme-profile" "$@"; }
au save deco >/dev/null 2>&1
if python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); assert any(a["kind"]=="aurorae" and a["name"]=="MineAero" for a in d["assets"]), d["assets"]; assert not any(x["kind"]=="aurorae" for x in d["dependencies"]), d["dependencies"]' \
  "$AU/.config/kde-theme-profile/themes/deco/theme.json" 2>/dev/null &&
  [ -f "$AU/.config/kde-theme-profile/themes/deco/.local/share/aurorae/themes/MineAero/MineAerorc" ]; then
  ok "a user Aurorae decoration is carried as a theme asset, not a dependency"
else
  fail "aurorae not carried: $(cat "$AU/.config/kde-theme-profile/themes/deco/theme.json" 2>/dev/null | head -c 700)"
fi

# Rounded Corners keeps its settings in [Round-Corners]: part of the theme.
RC="$TMP_ROOT/round-corners"; mkdir -p "$RC/.config"
printf '[Plugins]\nkwin4_effect_shapecornersEnabled=true\n\n[Round-Corners]\nSize=10\nShadowSize=45\n\n[Desktops]\nNumber=4\n' >"$RC/.config/kwinrc"
env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$RC" KTM_THEME_NO_APPLY=1 KTM_OFFLINE=1 "$CLI_DIR/theme-profile" save rc >/dev/null 2>&1
snap="$RC/.config/kde-theme-profile/themes/rc/.config/kwinrc"
if grep -q "^Size=10" "$snap" 2>/dev/null && grep -q "^kwin4_effect_shapecornersEnabled=true" "$snap" && ! grep -q "Desktops" "$snap"; then
  ok "rounded-corner settings and the effect switch travel with the theme (desktops do not)"
else
  fail "Round-Corners not captured: $(cat "$snap" 2>/dev/null | tr '\n' ' ')"
fi

# Follow-up actions: nothing for a theme without restart-worthy apps; an
# unknown action is refused.
AC="$TMP_ROOT/actions"; mkdir -p "$AC/.config/kde-theme-profile/themes/plain"
printf '{"schema": "theme-profile/theme@2", "name": "plain", "values": {}, "assets": [], "dependencies": []}\n' >"$AC/.config/kde-theme-profile/themes/plain/theme.json"
ac() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$AC" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
if [ "$(ac actions plain)" = "Nothing to do." ] && [ "$(ac actions plain --json)" = "[]" ] && ! ac action bogus >/dev/null 2>&1; then
  ok "actions lists nothing for a plain theme; unknown actions are refused"
else
  fail "actions: $(ac actions plain 2>&1) / $(ac action bogus 2>&1)"
fi

# Automation choices are the user's (app-preferences.json), kept beside the
# app's own settings; unknown keys and values are refused.
AU2="$TMP_ROOT/automation"; mkdir -p "$AU2/.config/kde-theme-profile"
printf '{\n  "theme": "glass",\n  "network_enabled": false\n}\n' >"$AU2/.config/kde-theme-profile/app-preferences.json"
au2() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$AU2" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
if au2 automation --json | grep '"configured":false' >/dev/null &&
  au2 automation set store=always restart_zen=never &&
  au2 automation --json | grep '"configured":true' >/dev/null &&
  au2 automation | grep -x "store=always  # .*" >/dev/null && au2 automation | grep "^restart_zen=never" >/dev/null &&
  grep -q '"theme": "glass"' "$AU2/.config/kde-theme-profile/app-preferences.json" &&
  ! au2 automation set store=maybe 2>/dev/null && ! au2 automation set bogus=ask 2>/dev/null &&
  printf 'never\n\n\n\n\n\n' | au2 automation ask >/dev/null && au2 automation | grep "^store=never" >/dev/null; then
  ok "automation choices are stored per user beside the app settings; bad keys/values refused; ask reads answers"
else
  fail "automation: $(au2 automation 2>&1 | tr '\n' ' ') // $(cat "$AU2/.config/kde-theme-profile/app-preferences.json")"
fi

# Apply follows the choices: "never" drops a missing store dependency from
# what is offered, "ask" keeps it.
mkdir -p "$AU2/.config/kde-theme-profile/themes/needs"
printf '{"schema": "theme-profile/theme@2", "name": "needs", "values": {}, "assets": [], "dependencies": [{"kind": "icons", "name": "NoSuchIcons", "source": "kde-store", "id": "999999999", "title": "No Such Icons", "url": "https://store.kde.org/p/999999999", "exact": true}]}\n' >"$AU2/.config/kde-theme-profile/themes/needs/theme.json"
deps_offered() { au2 apply needs --json 2>/dev/null | python3 -c 'import json,sys; print(len(json.load(sys.stdin)["dependencies"]))'; }
au2 automation set store=never >/dev/null; never_count="$(deps_offered)"
au2 automation set store=ask >/dev/null; ask_count="$(deps_offered)"
if [ "$never_count" = 0 ] && [ "$ask_count" = 1 ]; then
  ok "apply drops dependencies the user set to never, offers them on ask"
else
  fail "apply policies: never=$never_count ask=$ask_count"
fi

# Theme constructor: a new theme from the neutral template or from a theme;
# a Look & Feel package (defaults only, no layout) for the live look.
NT="$TMP_ROOT/new-theme"; mkdir -p "$NT/.config"
printf '[General]\nColorScheme=Mine\n' >"$NT/.config/kdeglobals"
nt() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$NT" KTM_THEME_NO_APPLY=1 KTM_OFFLINE=1 "$CLI_DIR/theme-profile" "$@"; }
nt new fresh --from=neutral-dark >/dev/null 2>&1
nt new copy --from=fresh >/dev/null 2>&1
if python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); v=d["values"]; assert v.get("color_scheme")=="BreezeDark" and v.get("blur_enabled") is False, v' \
     "$NT/.config/kde-theme-profile/themes/fresh/theme.json" 2>/dev/null &&
  [ -f "$NT/.config/kde-theme-profile/themes/copy/theme.json" ] &&
  [ "$(cat "$NT/.config/kde-theme-profile/active-profile")" = copy ] &&
  ! nt new fresh --from=neutral-light >/dev/null 2>&1 && ! nt new other --from=nosuch >/dev/null 2>&1; then
  ok "new: neutral template saved as a theme, a copy of a theme applied; names and bases checked"
else
  fail "new: $(cat "$NT/.config/kde-theme-profile/themes/fresh/theme.json" 2>/dev/null | head -c 400)"
fi
nt look-and-feel copy >/dev/null 2>&1
LAF="$NT/.local/share/plasma/look-and-feel/theme-profile-copy"
if grep -qx "ColorScheme=BreezeDark" "$LAF/contents/defaults" 2>/dev/null && [ -f "$LAF/metadata.json" ] &&
  [ ! -e "$LAF/contents/layouts" ] && grep -q "LookAndFeelPackage=theme-profile-copy" "$NT/.config/kdeglobals"; then
  ok "look-and-feel: defaults from the live look, no layout, selected"
else
  fail "look-and-feel: $(cat "$LAF/contents/defaults" 2>&1) // $(grep -i lookandfeel "$NT/.config/kdeglobals")"
fi

# --- Layouts stay the user's (layout.rs) ---------------------------------------
LA="$TMP_ROOT/layout-author"
LU="$TMP_ROOT/layout-user"
mkdir -p "$LA/.config" "$LU/.config" "$LA/Pictures/Wallpaper" "$LU/.local/share/icons/hicolor/64x64/apps"
: >"$LU/.local/share/icons/hicolor/64x64/apps/author-logo.png" # the receiver has this launcher icon
printf 'jpeg' >"$LA/Pictures/Wallpaper/dune.jpg"
cat >"$LA/.config/plasma-org.kde.plasma.desktop-appletsrc" <<APPLETS
[Containments][1]
activityId=author-activity
formfactor=0
lastScreen=0
location=0
plugin=org.kde.plasma.folder
wallpaperplugin=org.kde.image

[Containments][1][Wallpaper][org.kde.image][General]
Image=file://$LA/Pictures/Wallpaper/dune.jpg

[Containments][2]
formfactor=2
location=4
plugin=org.kde.panel

[Containments][2][Applets][3]
plugin=org.kde.plasma.kickoff

[Containments][2][Applets][3][Configuration][General]
favorites=author-app.desktop
icon=author-logo

[Containments][2][Applets][4]
plugin=org.kde.plasma.weather

[Containments][2][Applets][4][Configuration][General]
source=author home town

[Containments][2][General]
AppletOrder=3;4
APPLETS
printf '[PlasmaViews][Panel 2]\nfloating=0\npanelOpacity=1\npanelVisibility=1\n\n[PlasmaViews][Panel 2][Defaults]\nthickness=80\n' >"$LA/.config/plasmashellrc"
printf '[General]\nOrder=r1,r2\n\n[r1]\nDescription=see-through kitty\nwmclass=kitty\nwmclassmatch=1\nopacityactive=85\nopacityactiverule=2\nposition=0,0\npositionrule=3\n\n[r2]\nDescription=pinned size\nsize=900,700\nsizerule=3\n' >"$LA/.config/kwinrulesrc"
cat >"$LU/.config/plasma-org.kde.plasma.desktop-appletsrc" <<'APPLETS'
[Containments][10]
formfactor=0
lastScreen=0
location=0
plugin=org.kde.desktopcontainment
wallpaperplugin=org.kde.image

[Containments][10][Wallpaper][org.kde.image][General]
Image=file:///nowhere/mine.jpg

[Containments][20]
formfactor=2
location=4
plugin=org.kde.panel

[Containments][20][Applets][21]
plugin=org.kde.plasma.icontasks

[Containments][20][Applets][22]
plugin=org.kde.plasma.kickoff

[Containments][20][Applets][22][Configuration][General]
favorites=my-app.desktop
icon=start-here

[Containments][20][General]
AppletOrder=22;21
APPLETS
printf '[PlasmaViews][Panel 20]\nfloating=1\npanelOpacity=2\npanelVisibility=0\n\n[PlasmaViews][Panel 20][Defaults]\nthickness=44\n' >"$LU/.config/plasmashellrc"
printf '[General]\nOrder=mine\n\n[mine]\nDescription=my rule\nwmclass=firefox\nwmclassmatch=1\nminimize=true\nminimizerule=2\n' >"$LU/.config/kwinrulesrc"
la() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u WALLPAPER_DIR HOME="$LA" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
lu() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u WALLPAPER_DIR HOME="$LU" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
la save dune >/dev/null 2>&1
STORED="$LA/.config/kde-theme-profile/themes/dune/.config/plasma-org.kde.plasma.desktop-appletsrc"
if ! grep -q "icon=author-logo\|org.kde.plasma.kickoff\|weather\|author home town\|favorites\|AppletOrder\|author-activity" "$STORED" &&
  ! grep -q "thickness\|panelVisibility" "$LA/.config/kde-theme-profile/themes/dune/.config/plasmashellrc" &&
  ! grep -q "pinned size\|position" "$LA/.config/kde-theme-profile/themes/dune/.config/kwinrulesrc"; then
  ok "save keeps the shell's look only: no widgets, widget settings, panel sizes or behaviour rules"
else
  fail "save stored layout: $(cat "$STORED")"
fi
DZIP="$TMP_ROOT/dune.ktmtheme.zip"
la export dune --out="$DZIP" >/dev/null 2>&1
lu import "$DZIP" --apply >/dev/null 2>&1
LIVE_RC="$LU/.config/plasma-org.kde.plasma.desktop-appletsrc"
if grep -q "Wallpaper/dune.jpg" "$LIVE_RC" && grep -q "icon=start-here" "$LIVE_RC" && ! grep -q "author-logo\|mine.jpg" "$LIVE_RC" &&
  grep -q "favorites=my-app.desktop" "$LIVE_RC" && grep -q "AppletOrder=22;21" "$LIVE_RC" && grep -q "org.kde.plasma.icontasks" "$LIVE_RC" &&
  ! grep -q "weather\|author-app\|Containments\]\[2\]" "$LIVE_RC"; then
  ok "apply brings the wallpaper and preserves the launcher in the user's own layout (widgets, order, favourites kept)"
else
  fail "apply changed the layout: $(cat "$LIVE_RC")"
fi
SHELLRC="$LU/.config/plasmashellrc"
if grep -qx "floating=0" "$SHELLRC" && grep -qx "panelOpacity=1" "$SHELLRC" && grep -qx "panelVisibility=0" "$SHELLRC" &&
  grep -qx "thickness=44" "$SHELLRC" && ! grep -q "Panel 2\]" "$SHELLRC"; then
  ok "panels take the theme's floating/opacity (matched by screen edge), keep their size and auto-hide"
else
  fail "panel views: $(cat "$SHELLRC")"
fi
RULES="$LU/.config/kwinrulesrc"
if grep -qx "Order=mine,r1" "$RULES" && ! grep -q "count=\|^rules=" "$RULES" && grep -q "opacityactive=85" "$RULES" &&
  grep -q "minimize=true" "$RULES" && ! grep -q "pinned size\|position=" "$RULES"; then
  ok "window rules: the theme's look rule joins the user's (KWin 6 Order= kept, behaviour not carried)"
else
  fail "window rules: $(cat "$RULES")"
fi

# --- GTK theme reaches GSettings (Wayland GTK apps read it, not settings.ini) ---
GS="$TMP_ROOT/gsettings-home"
GS_MOCK="$TMP_ROOT/gsettings-mock"
mkdir -p "$GS/.config/gtk-3.0" "$GS/.config/gtk-4.0" "$GS_MOCK" "$GS/.local/share/themes/LightOne" "$GS/.local/share/themes/DarkOne" "$GS/.local/share/themes/Other"
# Remembers values like the real one (a file per key).
cat >"$GS_MOCK/gsettings" <<GSMOCK
#!/usr/bin/env bash
echo "\$*" >>"$GS_MOCK/calls"
case "\$1" in
  set) printf "%s" "\$4" >"$GS_MOCK/value-\$3" ;;
  get) [ -f "$GS_MOCK/value-\$3" ] && printf "'%s'\\n" "\$(cat "$GS_MOCK/value-\$3")" || exit 1 ;;
esac
exit 0
GSMOCK
chmod +x "$GS_MOCK/gsettings"
printf '[Settings]\ngtk-theme-name=LightOne\ngtk-application-prefer-dark-theme=false\n' >"$GS/.config/gtk-3.0/settings.ini"
gs() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u WALLPAPER_DIR HOME="$GS" PATH="$GS_MOCK:$PATH" "$CLI_DIR/theme-profile" "$@"; }
KTM_THEME_NO_APPLY=1 gs save gslight --no-wallpaper >/dev/null 2>&1
printf '[Settings]\ngtk-theme-name=DarkOne\ngtk-application-prefer-dark-theme=true\n' >"$GS/.config/gtk-3.0/settings.ini"
: >"$GS_MOCK/calls"
gs apply gslight >/dev/null 2>&1
if grep -qx "set org.gnome.desktop.interface gtk-theme LightOne" "$GS_MOCK/calls" &&
  grep -qx "set org.gnome.desktop.interface color-scheme prefer-light" "$GS_MOCK/calls"; then
  ok "apply writes the theme's GTK theme and colour scheme to GSettings"
else
  fail "apply did not update GSettings: $(cat "$GS_MOCK/calls")"
fi
: >"$GS_MOCK/calls"
gs field set gtk_theme=Other gtk_dark=true >/dev/null 2>&1
if grep -qx "set org.gnome.desktop.interface gtk-theme Other" "$GS_MOCK/calls" &&
  grep -qx "set org.gnome.desktop.interface color-scheme prefer-dark" "$GS_MOCK/calls"; then
  ok "field set mirrors the GTK theme and dark preference to GSettings"
else
  fail "field set did not update GSettings: $(cat "$GS_MOCK/calls")"
fi
: >"$GS_MOCK/calls"
KTM_THEME_NO_APPLY=1 gs apply gslight >/dev/null 2>&1
if [ ! -s "$GS_MOCK/calls" ]; then
  ok "KTM_THEME_NO_APPLY keeps GSettings untouched"
else
  fail "GSettings written under KTM_THEME_NO_APPLY: $(cat "$GS_MOCK/calls")"
fi

# GSettings go back: a theme's values leave with it, `default` stays `default`.
GR="$TMP_ROOT/gsettings-return"
mkdir -p "$GR/.config/gtk-3.0"
gr() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u WALLPAPER_DIR HOME="$GR" PATH="$GS_MOCK:$PATH" "$CLI_DIR/theme-profile" "$@"; }
printf 'Adwaita' >"$GS_MOCK/value-gtk-theme"
printf 'default' >"$GS_MOCK/value-color-scheme"
printf '[Settings]\ngtk-theme-name=Breeze\ngtk-application-prefer-dark-theme=false\n' >"$GR/.config/gtk-3.0/settings.ini"
gr save friend-look --no-wallpaper >/dev/null 2>&1
printf '[Settings]\ngtk-theme-name=AuthorDark\ngtk-application-prefer-dark-theme=true\n' >"$GR/.config/gtk-3.0/settings.ini"
gr save author-dark --no-wallpaper >/dev/null 2>&1
printf 'Adwaita' >"$GS_MOCK/value-gtk-theme"
printf 'default' >"$GS_MOCK/value-color-scheme"
for round in 1 2; do
  gr apply friend-look >/dev/null 2>&1
  gr apply author-dark >/dev/null 2>&1
  [ "$round" = 1 ] && dark_seen="$(cat "$GS_MOCK/value-color-scheme")"
  gr apply friend-look >/dev/null 2>&1
done
if [ "$dark_seen" = prefer-dark ] && [ "$(cat "$GS_MOCK/value-color-scheme")" = default ] && [ "$(cat "$GS_MOCK/value-gtk-theme")" = Adwaita ] &&
  grep -qx "gtk-theme-name=Breeze" "$GR/.config/gtk-3.0/settings.ini"; then
  ok "back to a saved theme: GSettings and settings.ini are what they were, every time"
else
  fail "GSettings after return: $(cat "$GS_MOCK/value-gtk-theme") $(cat "$GS_MOCK/value-color-scheme"); $(cat "$GR/.config/gtk-3.0/settings.ini")"
fi

# --- Settings the theme's look does not use are not carried -------------------
UA="$TMP_ROOT/unused-author"
UB="$TMP_ROOT/unused-user"
COLORIZER=luisbocanegra.panel.colorizer
mkdir -p "$UA/.config/klassy" "$UB/.config/klassy" "$UA/.local/share/plasma/plasmoids/$COLORIZER"
printf '[KDE]\nwidgetStyle=kvantum\n' >"$UA/.config/kdeglobals"
printf '[Effect-blur]\nBlurStrength=10\n\n[Effect-better-blur-dx]\nBlurStrength=5\n\n[Effect-translucency]\nMenus=96\n\n[Plugins]\nbetter_blur_dxEnabled=true\nblurEnabled=false\n\n[org.kde.kdecoration2]\nlibrary=org.kde.kwin.aurorae\ntheme=__aurorae__svg__Any\n' >"$UA/.config/kwinrc"
printf '[Style]\nMenuOpacity=50\n' >"$UA/.config/breezerc"
printf '[Windeco]\nWindowCornerRadius=4\n' >"$UA/.config/klassy/klassyrc"
printf '[Containments][2]\nformfactor=2\nlocation=4\nplugin=org.kde.panel\n\n[Containments][2][Applets][3]\nplugin=%s\n\n[Containments][2][Applets][3][Configuration][General]\nglobalSettings={author}\nhideWidget=true\nisEnabled=false\n' "$COLORIZER" >"$UA/.config/plasma-org.kde.plasma.desktop-appletsrc"
printf '[Effect-blur]\nBlurStrength=3\n\n[Effect-better-blur-dx]\nBlurStrength=9\n\n[Plugins]\nblurEnabled=true\n' >"$UB/.config/kwinrc"
printf '[Style]\nMenuOpacity=77\n' >"$UB/.config/breezerc"
printf '[Windeco]\nWindowCornerRadius=8\n' >"$UB/.config/klassy/klassyrc"
printf '[Containments][20]\nformfactor=2\nlocation=4\nplugin=org.kde.panel\n\n[Containments][20][Applets][21]\nplugin=%s\n\n[Containments][20][Applets][21][Configuration][General]\nglobalSettings={mine}\nisEnabled=true\n' "$COLORIZER" >"$UB/.config/plasma-org.kde.plasma.desktop-appletsrc"
ua() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u WALLPAPER_DIR HOME="$UA" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
ub() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u WALLPAPER_DIR HOME="$UB" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
ua save unused --no-wallpaper >/dev/null 2>&1
UP="$UA/.config/kde-theme-profile/themes/unused"
if [ ! -e "$UP/.config/breezerc" ] && [ ! -e "$UP/.config/klassy/klassyrc" ] &&
  ! grep -q "Effect-blur\]\|Effect-translucency" "$UP/.config/kwinrc" && grep -q "Effect-better-blur-dx" "$UP/.config/kwinrc" &&
  ! grep -q "globalSettings\|hideWidget" "$UP/.config/plasma-org.kde.plasma.desktop-appletsrc" &&
  grep -q "isEnabled=false" "$UP/.config/plasma-org.kde.plasma.desktop-appletsrc"; then
  ok "save leaves out Breeze/Klassy settings, off effects and a switched-off Panel Colorizer's styling"
else
  fail "save stored unused settings: $(ls "$UP/.config") // $(cat "$UP/.config/kwinrc")"
fi
if ! grep -q "$COLORIZER" "$UP/theme.json"; then
  ok "a switched-off Panel Colorizer is not a dependency"
else
  fail "switched-off Panel Colorizer listed: $(grep -n "$COLORIZER" "$UP/theme.json")"
fi
UZIP="$TMP_ROOT/unused.ktmtheme.zip"
ua export unused --out="$UZIP" >/dev/null 2>&1
ub import "$UZIP" --apply >/dev/null 2>&1
if grep -qx "MenuOpacity=77" "$UB/.config/breezerc" && grep -qx "WindowCornerRadius=8" "$UB/.config/klassy/klassyrc" &&
  grep -A1 "^\[Effect-blur\]" "$UB/.config/kwinrc" | grep -qx "BlurStrength=3" &&
  grep -qx "BlurStrength=5" "$UB/.config/kwinrc" && grep -qx "blurEnabled=false" "$UB/.config/kwinrc" &&
  grep -q "globalSettings={mine}" "$UB/.config/plasma-org.kde.plasma.desktop-appletsrc" &&
  grep -qx "isEnabled=false" "$UB/.config/plasma-org.kde.plasma.desktop-appletsrc"; then
  ok "apply keeps the user's settings the theme does not use, and still follows its switches"
else
  fail "apply touched unused settings: $(cat "$UB/.config/kwinrc" "$UB/.config/breezerc")"
fi

# --- A theme without a wallpaper: any kind of wallpaper stays, nothing of the author's --
NA="$TMP_ROOT/nowall-author"
NU="$TMP_ROOT/nowall-user"
mkdir -p "$NA/.config" "$NA/Pictures" "$NU/.config"
printf 'jpeg' >"$NA/Pictures/author.jpg"
printf '[Containments][1]\nformfactor=0\nlastScreen=0\nlocation=0\nplugin=org.kde.plasma.folder\nwallpaperplugin=org.kde.image\n\n[Containments][1][Wallpaper][org.kde.image][General]\nImage=file://%s/Pictures/author.jpg\n' "$NA" >"$NA/.config/plasma-org.kde.plasma.desktop-appletsrc"
printf '[Containments][7]\nformfactor=0\nlastScreen=0\nlocation=0\nplugin=org.kde.plasma.folder\nwallpaperplugin=org.kde.potd\n\n[Containments][7][Wallpaper][org.kde.potd][General]\nProvider=apod\n' >"$NU/.config/plasma-org.kde.plasma.desktop-appletsrc"
na() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u WALLPAPER_DIR HOME="$NA" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
nu() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u WALLPAPER_DIR HOME="$NU" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
na save nowall --no-wallpaper >/dev/null 2>&1
na save withwall >/dev/null 2>&1
if ! grep -q "author.jpg\|wallpaperplugin" "$NA/.config/kde-theme-profile/themes/nowall/.config/plasma-org.kde.plasma.desktop-appletsrc"; then
  ok "a theme saved without a wallpaper does not name one"
else
  fail "no-wallpaper theme names: $(cat "$NA/.config/kde-theme-profile/themes/nowall/.config/plasma-org.kde.plasma.desktop-appletsrc")"
fi
na export nowall --out="$TMP_ROOT/nowall.zip" >/dev/null 2>&1
na export withwall --no-wallpaper --out="$TMP_ROOT/withwall.zip" >/dev/null 2>&1
if ! python3 -c 'import sys,zipfile; z=zipfile.ZipFile(sys.argv[1]); sys.exit(any(b"author.jpg" in z.read(n) for n in z.namelist()))' "$TMP_ROOT/withwall.zip"; then
  fail "export --no-wallpaper still names the wallpaper"
else
  ok "export --no-wallpaper does not name it either"
fi
nu import "$TMP_ROOT/nowall.zip" --apply >/dev/null 2>&1
NRC="$NU/.config/plasma-org.kde.plasma.desktop-appletsrc"
if grep -qx "wallpaperplugin=org.kde.potd" "$NRC" && grep -qx "Provider=apod" "$NRC" && ! grep -q "author.jpg" "$NRC"; then
  ok "a theme without a wallpaper leaves picture-of-the-day and any other kind alone"
else
  fail "no-wallpaper theme changed the desktop: $(cat "$NRC")"
fi
cp "$NA/.config/kde-theme-profile/themes/withwall" -r "$NU/.config/kde-theme-profile/themes/withwall"
nu apply withwall --keep-wallpaper >/dev/null 2>&1
if grep -qx "wallpaperplugin=org.kde.potd" "$NRC" && ! grep -q "author.jpg" "$NRC"; then
  ok "--keep-wallpaper leaves any kind of wallpaper alone"
else
  fail "--keep-wallpaper changed the desktop: $(cat "$NRC")"
fi
nu apply withwall >/dev/null 2>&1
if grep -qx "wallpaperplugin=org.kde.image" "$NRC" && grep -q "Image=" "$NRC"; then
  ok "a theme with a wallpaper does apply it"
else
  fail "theme wallpaper not applied: $(cat "$NRC")"
fi

# --- Neovim is not the app's business --------------------------------------------
NV="$TMP_ROOT/nvim-untouched"
mkdir -p "$NV/.config/nvim"
printf 'vim.opt.number = true\n' >"$NV/.config/nvim/init.lua"
nv() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u WALLPAPER_DIR HOME="$NV" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
nv import "$SCRIPT_DIR/themes/frutiger-aero-light" --apply >/dev/null 2>&1
if [ "$(cat "$NV/.config/nvim/init.lua")" = 'vim.opt.number = true' ] && [ "$(find "$NV/.config/nvim" -type f | wc -l)" = 1 ] &&
  ! nv palette apply --targets=nvim >/dev/null 2>&1; then
  ok "themes and the palette leave Neovim alone"
else
  fail "Neovim touched: $(find "$NV/.config/nvim" -type f)"
fi

# --- What a theme does not specify goes back to the program's default ----------------
US="$TMP_ROOT/unspecified"
mkdir -p "$US/.config/vesktop/settings" "$US/.config/vesktop/themes" "$US/.config/ghostty" "$US/.config/gtk-3.0" "$US/.local/share/konsole"
printf '{\n  "minimizeToTray": true,\n  "enabledThemes": ["theme-profile.theme.css"],\n  "transparent": true\n}\n' >"$US/.config/vesktop/settings/settings.json"
printf 'light css\n' >"$US/.config/vesktop/themes/theme-profile.theme.css"
printf 'mine\n' >"$US/.config/vesktop/themes/my-own.theme.css"
printf 'background = #ffffff\n' >"$US/.config/ghostty/theme-profile"
printf 'window { }\n' >"$US/.config/gtk-3.0/theme-profile.css"
printf '[Appearance]\nColorScheme=Light\n\n[General]\nName=Theme Profile\nParent=FALLBACK/\n' >"$US/.local/share/konsole/Theme Profile.profile"
printf '[Desktop Entry]\nDefaultProfile=Theme Profile.profile\n\n[Other]\nkeep=1\n' >"$US/.config/konsolerc"
us() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u WALLPAPER_DIR HOME="$US" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
us import "$SCRIPT_DIR/themes/dracula" --apply >/dev/null 2>&1
VS="$US/.config/vesktop/settings/settings.json"
if ! grep -q "enabledThemes\|transparent" "$VS" && grep -q "minimizeToTray" "$VS" && python3 -c 'import json,sys; json.load(open(sys.argv[1]))' "$VS" &&
  [ ! -e "$US/.config/vesktop/themes/theme-profile.theme.css" ] && [ -f "$US/.config/vesktop/themes/my-own.theme.css" ]; then
  ok "a theme without Vesktop turns ours off and keeps the user's settings and files"
else
  fail "Vesktop after a theme without it: $(cat "$VS"); $(ls "$US/.config/vesktop/themes")"
fi
if [ ! -e "$US/.config/ghostty/theme-profile" ] && [ ! -e "$US/.config/gtk-3.0/theme-profile.css" ]; then
  ok "generated files a theme does not carry are removed"
else
  fail "generated files stayed: $(ls "$US/.config/ghostty" "$US/.config/gtk-3.0")"
fi
if [ ! -e "$US/.local/share/konsole/Theme Profile.profile" ] && ! grep -q DefaultProfile "$US/.config/konsolerc" && grep -q "keep=1" "$US/.config/konsolerc"; then
  ok "a theme without Konsole removes the profile we made; Konsole is back to its default"
else
  fail "Konsole after a theme without it: $(cat "$US/.config/konsolerc")"
fi
printf '[Desktop Entry]\nDefaultProfile=Mine.profile\n' >"$US/.config/konsolerc"
printf '[Appearance]\nColorScheme=Dark\n\n[General]\nCommand=/bin/fish\nName=Mine\n' >"$US/.local/share/konsole/Mine.profile"
us apply dracula >/dev/null 2>&1
if grep -q "DefaultProfile=Mine.profile" "$US/.config/konsolerc" && grep -q "Command=/bin/fish" "$US/.local/share/konsole/Mine.profile" && ! grep -q "ColorScheme" "$US/.local/share/konsole/Mine.profile"; then
  ok "the user's own Konsole profile stays, without the look the theme does not specify"
else
  fail "own Konsole profile: $(cat "$US/.local/share/konsole/Mine.profile")"
fi

# --- What this machine lacks stays as it is ------------------------------------------
MS="$TMP_ROOT/missing-assets"
mkdir -p "$MS/.config/gtk-3.0"
printf '[Icons]\nTheme=breeze\n\n[KDE]\nwidgetStyle=Breeze\n' >"$MS/.config/kdeglobals"
printf '[Mouse]\ncursorTheme=breeze_cursors\n' >"$MS/.config/kcminputrc"
printf '[Theme]\nname=default\n' >"$MS/.config/plasmarc"
printf '[org.kde.kdecoration2]\nlibrary=org.kde.breeze\ntheme=Breeze\n' >"$MS/.config/kwinrc"
printf '[Settings]\ngtk-theme-name=Breeze\ngtk-icon-theme-name=breeze\n' >"$MS/.config/gtk-3.0/settings.ini"
ms() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u WALLPAPER_DIR HOME="$MS" XDG_DATA_DIRS="$MS/none" PATH="$SAFE_BIN:/usr/bin:/bin" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
ms import "$SCRIPT_DIR/themes/frutiger-aero" >/dev/null 2>&1
report="$(ms apply frutiger-aero --json 2>/dev/null)"
if [ -d /usr/share/icons/Windows-7 ] || [ -d /usr/share/aurorae/themes/Se7enAero ]; then
  echo "  ⏭️  the theme's assets are installed system-wide — skipping"
elif grep -qx "Theme=breeze" "$MS/.config/kdeglobals" && grep -qx "library=org.kde.breeze" "$MS/.config/kwinrc" &&
  grep -qx "name=default" "$MS/.config/plasmarc" &&
  printf '%s' "$report" | grep -q "is not installed, kept"; then
  ok "icons, cursor, window decoration and Plasma style a machine lacks keep the current ones, and the report says so"
else
  fail "theme switched to what is missing: $(grep -h "Theme=\|library=\|^name=\|cursorTheme=" "$MS/.config/kdeglobals" "$MS/.config/kwinrc" "$MS/.config/plasmarc" "$MS/.config/kcminputrc" | tr '\n' ' ')"
fi
if [ -s "$MS/.config/kde-theme-profile/apply.log" ] && grep -q "apply frutiger-aero: done" "$MS/.config/kde-theme-profile/apply.log"; then
  ok "every apply leaves a step log to read after a freeze"
else
  fail "no apply log: $(ls "$MS/.config/kde-theme-profile")"
fi
if ! grep -rqi --exclude='*.md' "simplekickoff\|Simple Application Launcher" "$SCRIPT_DIR/themes" "$SCRIPT_DIR/gui/qt-app/qml"; then
  ok "the launcher widget is not named or required anywhere"
else
  fail "launcher widget still named: $(grep -rli --exclude='*.md' "simplekickoff\|Simple Application Launcher" "$SCRIPT_DIR/themes" | head -3)"
fi

# --- Data under the old folder names moves on the first run ---------------------------
OL="$TMP_ROOT/old-names"
mkdir -p "$OL/.config/theme-profile/profiles/mine/.config/theme-profile" "$OL/.config/theme-profile/backups/one"
printf '{"schema":"theme-profile/theme@2","name":"mine","description":"","created":"2026-01-01T00:00:00Z","generator":"t","values":{},"assets":[],"sources":{},"appearance_only":true}\n' >"$OL/.config/theme-profile/profiles/mine/theme.json"
printf 'palette' >"$OL/.config/theme-profile/profiles/mine/.config/theme-profile/palette.json"
printf 'mine\n' >"$OL/.config/theme-profile/active-profile"
ol() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u WALLPAPER_DIR HOME="$OL" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
if [ "$(ol list 2>/dev/null)" = "mine" ] && [ ! -e "$OL/.config/theme-profile" ] &&
  [ -f "$OL/.config/kde-theme-profile/themes/mine/.config/kde-theme-profile/palette.json" ] &&
  [ -d "$OL/.config/kde-theme-profile/backups/one" ] && [ "$(cat "$OL/.config/kde-theme-profile/active-profile")" = mine ]; then
  ok "themes, backups and settings move from theme-profile/profiles to kde-theme-profile/themes"
else
  fail "old folders not moved: $(find "$OL/.config" -maxdepth 3)"
fi

# --- Ready themes follow the kit unless the user changed them --------------------------
BS="$TMP_ROOT/bundled-home"
BK="$TMP_ROOT/bundled-kit"
mkdir -p "$BS/.config" "$BK"
cp -r "$SCRIPT_DIR/themes/glass" "$BK/glass"
cp -r "$SCRIPT_DIR/themes/dracula" "$BK/dracula"
bs() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u WALLPAPER_DIR HOME="$BS" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
kit_change() { python3 - "$BK/$1" <<'KITPY'
import hashlib, json, sys
root = sys.argv[1]
with open(root + "/.config/kwinrc", "a", encoding="utf-8") as f:
    f.write("\n# new kit version\n")
e = json.load(open(root + "/export.json"))
e["files"][".config/kwinrc"] = hashlib.sha256(open(root + "/.config/kwinrc", "rb").read()).hexdigest()
json.dump(e, open(root + "/export.json", "w"))
KITPY
}
if bs bundled sync "$BK" | grep -q "Added theme: glass" && [ -z "$(bs bundled sync "$BK")" ]; then
  ok "new ready themes are added, and an unchanged kit does nothing"
else
  fail "bundled sync add: $(bs bundled sync "$BK")"
fi
kit_change glass
if bs bundled sync "$BK" | grep -q "Updated theme: glass" && grep -q "new kit version" "$BS/.config/kde-theme-profile/themes/glass/.config/kwinrc" &&
  [ -n "$(find "$BS/.config/kde-theme-profile/backups" -mindepth 1 -print -quit)" ]; then
  ok "a ready theme the user did not change follows the kit, the old one is backed up"
else
  fail "bundled update: $(find "$BS/.config/kde-theme-profile/backups" -maxdepth 1)"
fi
printf 'my edit\n' >>"$BS/.config/kde-theme-profile/themes/dracula/.config/kwinrc"
kit_change dracula
if bs bundled sync "$BK" | grep -q "Kept theme dracula: it may hold your changes" && grep -q "my edit" "$BS/.config/kde-theme-profile/themes/dracula/.config/kwinrc" &&
  [ -z "$(bs bundled sync "$BK")" ]; then
  ok "a ready theme the user changed is kept, said once, with the command to replace it"
else
  fail "bundled kept: $(bs bundled sync "$BK")"
fi
rm "$BS/.config/kde-theme-profile/bundled-themes.json"
kit_change glass
if bs bundled sync "$BK" | grep -q "Kept theme glass: it may hold" && ! bs bundled sync "$BK" | grep -q glass && grep -q "new kit version" "$BS/.config/kde-theme-profile/themes/glass/.config/kwinrc"; then
  ok "a theme with no record of its install is left alone, said once"
else
  fail "bundled no record: $(bs bundled sync "$BK")"
fi

# --- The update script passes options to the installer ----------------------------------
UP="$TMP_ROOT/update-script"
mkdir -p "$UP/remote" "$UP/kit/scripts"
git -C "$UP/remote" init -q --bare
cp "$SCRIPT_DIR/scripts/update-theme-profile-kit.sh" "$UP/kit/scripts/"
printf '#!/usr/bin/env bash\necho "installer got: $*"\n' >"$UP/kit/scripts/install-theme-profile-kit.sh"
chmod +x "$UP/kit/scripts/"*.sh
git -C "$UP/kit" init -q -b main
git -C "$UP/kit" add -A
git -C "$UP/kit" -c user.name=t -c user.email=t@t commit -qm init
git -C "$UP/kit" remote add origin "$UP/remote"
git -C "$UP/kit" push -q -u origin main 2>/dev/null
if bash "$UP/kit/scripts/update-theme-profile-kit.sh" --restart 2>&1 | grep -q "installer got: --restart"; then
  ok "update-theme-profile-kit.sh --restart reaches the installer"
else
  fail "update script dropped its options"
fi

# --- Forced blur leaves panels and notifications their own area ---------------------
BLUR_OK=1
for theme in "$SCRIPT_DIR"/themes/*/; do
  grep -qxF 'WindowClasses=plasmashell\nswaync' "$theme/.config/kwinrc" || BLUR_OK=0
done
if [ "$BLUR_OK" = 1 ]; then
  ok "every bundled theme excludes plasmashell and swaync from forced blur"
else
  fail "a bundled theme lacks the blur exceptions"
fi

# --- The first install saves the current look -------------------------------------
OH="$TMP_ROOT/original-home"
mkdir -p "$OH/.config"
printf '[General]\nColorScheme=FriendScheme\n' >"$OH/.config/kdeglobals"
oi() {
  local home="$1"
  shift
  env -i HOME="$home" PATH="$SAFE_BIN:$REAL_CARGO_BIN:/usr/bin:/bin" CARGO_HOME="$REAL_CARGO_HOME" RUSTUP_HOME="$REAL_RUSTUP_HOME" \
    bash "$SCRIPT_DIR/scripts/install-theme-profile-kit.sh" --only-cli "$@"
}
OTHEME="$OH/.config/kde-theme-profile/themes/my-original-look"
if oi "$OH" >"$TMP_ROOT/original.log" 2>&1 && grep -q "ColorScheme=FriendScheme" "$OTHEME/.config/kdeglobals" && grep -q "no-original-theme" "$TMP_ROOT/original.log"; then
  ok "the first install saves the current look as my-original-look and says so"
else
  fail "no original theme: $(tail -3 "$TMP_ROOT/original.log")"
fi
if [ -d "$OH/.config/kde-theme-profile/themes/glass" ] && [ -f "$OTHEME/theme.json" ]; then
  ok "the bundled themes are added next to it"
else
  fail "after the first install: $(ls "$OH/.config/kde-theme-profile/themes")"
fi
printf '[General]\nColorScheme=SomeTheme\n' >"$OH/.config/kdeglobals"
if oi "$OH" >/dev/null 2>&1 && grep -q "ColorScheme=FriendScheme" "$OTHEME/.config/kdeglobals"; then
  ok "an update does not overwrite my-original-look"
else
  fail "my-original-look changed: $(grep ColorScheme= "$OTHEME/.config/kdeglobals")"
fi
ON="$TMP_ROOT/original-none"
mkdir -p "$ON/.config"
if oi "$ON" --no-themes --no-original-theme >/dev/null 2>&1 &&
  [ ! -e "$ON/.config/kde-theme-profile/themes/my-original-look" ]; then
  ok "--no-original-theme skips it"
else
  fail "--no-original-theme still saved a theme"
fi

# --- The login screen is not the app's business ------------------------------------
SD="$TMP_ROOT/sddm-home"
mkdir -p "$SD/.config"
sd() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u WALLPAPER_DIR HOME="$SD" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
sd save old-theme --no-wallpaper >/dev/null 2>&1
SDJ="$SD/.config/kde-theme-profile/themes/old-theme/theme.json"
python3 - "$SDJ" <<'SDPY'
import json, sys
d = json.load(open(sys.argv[1]))
d["dependencies"].append({"kind": "sddm", "name": "astronaut", "source": "aur", "id": "astronaut", "title": "", "url": "", "exact": True})
json.dump(d, open(sys.argv[1], "w"))
SDPY
if ! sd deps old-theme --json 2>/dev/null | grep -qi "astronaut\|sddm"; then
  ok "a login-screen dependency in an old theme is ignored"
else
  fail "SDDM still offered: $(sd deps old-theme --json 2>&1 | head -c 300)"
fi
if ! grep -rqi "sddm" "$SCRIPT_DIR/gui/qt-app/qml" "$SCRIPT_DIR/mcp" "$SCRIPT_DIR/themes"; then
  ok "nothing in the GUI, MCP or bundled themes mentions the login screen"
else
  fail "SDDM mentioned: $(grep -rli sddm "$SCRIPT_DIR/gui/qt-app/qml" "$SCRIPT_DIR/mcp" "$SCRIPT_DIR/themes")"
fi

# --- A tool that hangs must not leave the user without a desktop ------------------
HANG="$TMP_ROOT/hang-home"
HANG_MOCK="$TMP_ROOT/hang-mock"
mkdir -p "$HANG/.config/Kvantum/Present" "$HANG/.config" "$HANG_MOCK"
printf '#!/usr/bin/env bash\necho "$*" >>"%s/calls"\nsleep 300\n' "$HANG_MOCK" >"$HANG_MOCK/kvantummanager"
chmod +x "$HANG_MOCK/kvantummanager"
printf '#!/usr/bin/env bash\necho "$*" >>"%s/calls"\nexit 0\n' "$HANG_MOCK" >"$HANG_MOCK/systemctl"
chmod +x "$HANG_MOCK/systemctl"
printf '[KDE]\nwidgetStyle=kvantum\n' >"$HANG/.config/kdeglobals"
printf '[General]\ntheme=Absent\n' >"$HANG/.config/Kvantum/kvantum.kvconfig"
hang() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u WALLPAPER_DIR HOME="$HANG" PATH="$HANG_MOCK:$PATH" KTM_COMMAND_TIMEOUT=1 "$CLI_DIR/theme-profile" "$@"; }
KTM_THEME_NO_APPLY=1 hang save absent --no-wallpaper >/dev/null 2>&1
: >"$HANG_MOCK/calls"
hang apply absent >/dev/null 2>&1
if ! grep -q "^--set" "$HANG_MOCK/calls" && grep -q "start plasma-plasmashell" "$HANG_MOCK/calls"; then
  ok "a missing Kvantum theme is not handed to kvantummanager (it would wait on a dialog)"
else
  fail "kvantummanager called for a missing theme: $(cat "$HANG_MOCK/calls")"
fi
printf '[General]\ntheme=Present\n' >"$HANG/.config/Kvantum/kvantum.kvconfig"
KTM_THEME_NO_APPLY=1 hang save present --no-wallpaper >/dev/null 2>&1
: >"$HANG_MOCK/calls"
started=$SECONDS
hang_failed=no
if hang apply present >"$HANG_MOCK/result" 2>&1; then
  hang_failed=no
else
  hang_failed=yes
fi
if [ "$hang_failed" = yes ] && grep -q "operation apply-" "$HANG_MOCK/result" &&
  grep -q "recovery errors:" "$HANG_MOCK/result" &&
  grep -q "^--set Present" "$HANG_MOCK/calls" && grep -q "start plasma-plasmashell" "$HANG_MOCK/calls" && [ $((SECONDS - started)) -lt 60 ]; then
  ok "a hanging tool fails Apply with recovery diagnostics and plasmashell still comes back"
else
  fail "hanging tool: $((SECONDS - started))s, $(cat "$HANG_MOCK/calls")"
fi

# --- A friend's round trip: own theme → a bundled theme → back = as it was ------
RT="$TMP_ROOT/friend-home"
rt() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u XDG_CACHE_HOME -u WALLPAPER_DIR HOME="$RT" KTM_THEME_NO_APPLY=1 "$CLI_DIR/theme-profile" "$@"; }
bash "$SCRIPT_DIR/tests/e2e/make-friend-home.sh" "$RT"
rt save my-look >/dev/null 2>&1
python3 "$SCRIPT_DIR/tests/e2e/snapshot.py" "$RT" "$TMP_ROOT/rt-before.json"
for theme in "$SCRIPT_DIR"/themes/*/; do
  name="$(basename "$theme")"
  rt import "$theme" >/dev/null 2>&1
  rt apply "$name" >/dev/null 2>&1
  rt apply my-look >/dev/null 2>&1
  python3 "$SCRIPT_DIR/tests/e2e/snapshot.py" "$RT" "$TMP_ROOT/rt-after.json"
  if diff_out="$(python3 "$SCRIPT_DIR/tests/e2e/compare.py" "$TMP_ROOT/rt-before.json" "$TMP_ROOT/rt-after.json")"; then
    ok "round trip through $name leaves the friend's files as they were"
  else
    fail "round trip through $name: $diff_out"
  fi
done
for name in Paper frutiger-aero-light glass dracula; do rt apply "$name" >/dev/null 2>&1; done
rt apply my-look >/dev/null 2>&1
python3 "$SCRIPT_DIR/tests/e2e/snapshot.py" "$RT" "$TMP_ROOT/rt-after.json"
if diff_out="$(python3 "$SCRIPT_DIR/tests/e2e/compare.py" "$TMP_ROOT/rt-before.json" "$TMP_ROOT/rt-after.json")"; then
  ok "a chain of themes and back leaves the friend's files as they were"
else
  fail "chain of themes: $diff_out"
fi
# What the user changed after a theme stays theirs.
rt apply dracula >/dev/null 2>&1
printf 'my own edit\n' >"$RT/.config/cava/config"
rt apply my-look >/dev/null 2>&1
if [ "$(cat "$RT/.config/cava/config" 2>/dev/null)" = "my own edit" ]; then
  ok "a file the user edited after a theme is not undone"
else
  fail "the user's edit was lost"
fi
rm -f "$RT/.config/cava/config"
# A pref a theme set goes back through user.js (it stays in prefs.js otherwise).
PREFS="$RT/.zen/xyz.Default (release)"
rt apply frutiger-aero-light >/dev/null 2>&1
if grep -q 'zen.view.window.scheme", 1' "$PREFS/user.js" 2>/dev/null && [ ! -d "$RT/.zen/gp0y8wrv.Default (release)" ]; then
  ok "the theme's Zen prefs go to the receiver's own profile"
else
  fail "Zen profile: $(ls "$RT/.zen")"
fi
sed -i 's/"zen.view.window.scheme", 2/"zen.view.window.scheme", 1/' "$PREFS/prefs.js" # what Zen stores when it runs
rt apply my-look >/dev/null 2>&1
if grep -q 'zen.view.window.scheme", 2' "$PREFS/user.js" 2>/dev/null; then
  ok "a pref left in prefs.js is set back to its value before the theme"
else
  fail "Zen pref not restored: $(cat "$PREFS/user.js" 2>&1)"
fi

PF="$TMP_ROOT/preserved-fastfetch"
pf() { env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u XDG_CACHE_HOME HOME="$PF" KTM_THEME_NO_APPLY=1 KTM_OFFLINE=1 "$CLI_DIR/theme-profile" "$@"; }
mkdir -p "$PF/.config/fastfetch" "$PF/.config/kde-theme-profile/themes/legacy/.config/fastfetch"
printf 'my config\n' >"$PF/.config/fastfetch/config.jsonc"
printf 'my logo\n' >"$PF/.config/fastfetch/logo"
printf 'author config\n' >"$PF/.config/kde-theme-profile/themes/legacy/.config/fastfetch/config.jsonc"
cat >"$PF/.config/kde-theme-profile/themes/legacy/theme.json" <<'JSON'
{"schema":"theme-profile/theme@2","name":"legacy","assets":[{"kind":"fastfetch-logo","name":"config","path":".config/fastfetch/config.jsonc","origin":"user","redistributable":false}]}
JSON
pf save own >/dev/null 2>&1
if [ ! -e "$PF/.config/kde-theme-profile/themes/own/.config/fastfetch" ]; then
  ok "saving a theme excludes fastfetch"
else
  fail "fastfetch was saved"
fi
mkdir -p "$PF/.config/kde-theme-profile/apply-journal/originals"
PF_HASH="$(sha256sum "$PF/.config/fastfetch/config.jsonc" | cut -d' ' -f1)"
PF_LOGO_HASH="$(sha256sum "$PF/.config/fastfetch/logo" | cut -d' ' -f1)"
printf 'old config\n' >"$PF/.config/kde-theme-profile/apply-journal/originals/old"
printf '{"created":{".config/fastfetch/logo":"%s"},"replaced":{".config/fastfetch/config.jsonc":["%s","old"]}}\n' "$PF_LOGO_HASH" "$PF_HASH" >"$PF/.config/kde-theme-profile/apply-journal/journal.json"
if pf apply legacy >/dev/null 2>&1 && pf apply own >/dev/null 2>&1 &&
  [ "$(cat "$PF/.config/fastfetch/config.jsonc")" = 'my config' ] &&
  [ "$(cat "$PF/.config/fastfetch/logo")" = 'my logo' ]; then
  ok "legacy themes and old undo journals neither overwrite nor remove fastfetch"
else
  fail "legacy theme or journal changed fastfetch"
fi
if pf export legacy --out="$TMP_ROOT/preserved.zip" >/dev/null 2>&1 &&
  ! unzip -Z1 "$TMP_ROOT/preserved.zip" | grep -q '^.config/fastfetch/'; then
  ok "export excludes legacy fastfetch assets"
else
  fail "export still contains fastfetch"
fi

python3 - "$PF/.config/kde-theme-profile/themes/legacy" "$TMP_ROOT/legacy-fastfetch.zip" <<'PY'
import hashlib
import json
from pathlib import Path
import sys
import zipfile
source = Path(sys.argv[1])
files = {str(path.relative_to(source)): path.read_bytes() for path in source.rglob('*') if path.is_file()}
manifest = {"format": "theme-profile/export@1", "theme": "legacy", "exported": "", "generator": "test", "home_token": "$HOME", "files": {name: hashlib.sha256(data).hexdigest() for name, data in files.items()}}
with zipfile.ZipFile(sys.argv[2], 'w') as archive:
    for name, data in files.items():
        archive.writestr(name, data)
    archive.writestr('export.json', json.dumps(manifest))
PY
if pf import "$TMP_ROOT/legacy-fastfetch.zip" --name=imported-preserved >/dev/null 2>&1 &&
  [ ! -e "$PF/.config/kde-theme-profile/themes/imported-preserved/.config/fastfetch" ] &&
  pf apply imported-preserved >/dev/null 2>&1 &&
  [ "$(cat "$PF/.config/fastfetch/config.jsonc")" = 'my config' ] &&
  [ "$(cat "$PF/.config/fastfetch/logo")" = 'my logo' ]; then
  ok "import discards legacy fastfetch files and applying keeps the user's files"
else
  fail "legacy fastfetch import failed or changed the user's files"
fi

# --- Summary ------------------------------------------------------------------
echo ""
echo "═══════════════════════════════════════"
echo "  Results: $PASS passed, $FAIL failed"
echo "═══════════════════════════════════════"

[ "$FAIL" -eq 0 ]
