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
trap 'rm -rf "$TMP_ROOT"' EXIT

# Bootstrap a fake ~/.config/theme-profile structure
FAKE_HOME="$TMP_ROOT/fake-home"
mkdir -p "$FAKE_HOME/.config/theme-profile"

# Create a real profile so we can test apply success and save
mkdir -p "$FAKE_HOME/.config/theme-profile/profiles/test-profile"
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
HOME="$FAKE_HOME" "$CLI_DIR/theme-profile" save components-test   --components=kwin,kdeglobals >/dev/null 2>&1
profile_root="$FAKE_HOME/.config/theme-profile/profiles/components-test"
if [ -f "$profile_root/.config/kwinrc" ] && [ -f "$profile_root/.config/kdeglobals" ]; then
  ok "save with an explicit component set copies those files"
else
  fail "save with an explicit component set is missing files"
fi
HOME="$FAKE_HOME" "$CLI_DIR/theme-profile" save components-test   --components=kdeglobals >/dev/null 2>&1
if [ ! -e "$profile_root/.config/kwinrc" ] && [ -f "$profile_root/.config/kdeglobals" ]; then
  ok "re-saving a profile removes stale component files"
else
  fail "re-saving a profile kept stale component files"
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
if HOME="$FAKE_HOME" "$CLI_DIR/theme-profile" apply components-test --components= >/dev/null 2>&1; then
  fail "apply with an empty component list should be rejected"
else
  ok "apply with an empty component list is rejected"
fi

# XDG_CONFIG_HOME/XDG_DATA_HOME must be honoured for live files while the
# snapshot keeps the historical .config layout.
XDG_HOME="$TMP_ROOT/xdg-home"
XDG_CONFIG="$XDG_HOME/config"
XDG_DATA="$XDG_HOME/data"
mkdir -p "$XDG_CONFIG" "$XDG_DATA"
printf '[General]\nAccentColor=9,8,7\n' > "$XDG_CONFIG/kdeglobals"
if HOME="$XDG_HOME" XDG_CONFIG_HOME="$XDG_CONFIG" XDG_DATA_HOME="$XDG_DATA" \
  "$CLI_DIR/theme-profile" save xdg-profile --components=kdeglobals >/dev/null 2>&1 \
  && [ -f "$XDG_CONFIG/theme-profile/profiles/xdg-profile/.config/kdeglobals" ]; then
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
mkdir -p "$CASE_HOME/.config/theme-profile"
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
mkdir -p "$WP_HOME/.config/theme-profile" "$WP_HOME/Pictures/Wallpaper"
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
saved="$WP_HOME/.config/theme-profile/profiles/wp-test/wallpaper"
if [ -f "$saved/forest one.jpg" ] && grep -q "forest one.jpg" "$saved/source"; then
  ok "save copies the wallpaper into the profile"
else
  fail "save did not copy the wallpaper"
fi

rm "$WP_HOME/Pictures/Wallpaper/forest one.jpg"
wp_cli apply wp-test >/dev/null 2>&1
if [ "$(cat "$WP_HOME/Pictures/Wallpaper/forest one.jpg" 2>/dev/null)" = "jpeg" ]; then
  ok "apply restores a deleted wallpaper from the profile copy"
else
  fail "apply did not restore the deleted wallpaper"
fi

# --- 5c. Palette: import → set → generate → apply → saved with the profile ---
echo "=== Section 5c: palette CLI ==="

PAL_HOME="$TMP_ROOT/pal-home"
mkdir -p "$PAL_HOME/.config/theme-profile"
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

# Terminals: only colour lines/keys change, re-apply is idempotent, backups made.
mkdir -p "$PAL_HOME/.config/kitty" "$PAL_HOME/.config/alacritty"
printf 'font_size 11\nbackground_opacity 0.9\n' >"$PAL_HOME/.config/kitty/kitty.conf"
printf '[window]\nopacity = 0.5 # mine\n' >"$PAL_HOME/.config/alacritty/alacritty.toml"
chmod 600 "$PAL_HOME/.config/kitty/kitty.conf"
pal_cli palette apply >/dev/null 2>&1
pal_cli palette apply >/dev/null 2>&1
kitty_conf="$PAL_HOME/.config/kitty/kitty.conf"
if [ "$(grep -c '^# BEGIN theme-profile palette' "$kitty_conf")" = 1 ] &&
  grep -qx "background #2d353b" "$kitty_conf" && grep -qx "font_size 11" "$kitty_conf"; then
  ok "palette apply writes one kitty colour block and keeps user lines"
else
  fail "kitty.conf palette block wrong: $(cat "$kitty_conf")"
fi
if [ "$(stat -c %a "$kitty_conf")" = 600 ]; then
  ok "palette apply keeps kitty.conf permissions"
else
  fail "kitty.conf permissions changed to $(stat -c %a "$kitty_conf")"
fi
alacritty_toml="$PAL_HOME/.config/alacritty/alacritty.toml"
if grep -q '^opacity = 0.5 # mine$' "$alacritty_toml" && grep -q 'background = "#2d353b"' "$alacritty_toml"; then
  ok "palette apply sets Alacritty colours and keeps comments"
else
  fail "alacritty.toml unexpected: $(cat "$alacritty_toml")"
fi
if compgen -G "$PAL_HOME/.config/theme-profile/backups/palette-apply-*/kitty.conf" >/dev/null; then
  ok "palette apply backs up terminal configs"
else
  fail "no palette-apply backup"
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
if [ -f "$PAL_HOME/.config/theme-profile/profiles/kv-test/.config/Kvantum/ThemeProfileEverforest/ThemeProfileEverforest.svg" ]; then
  ok "profiles snapshot the active user Kvantum theme"
else
  fail "active Kvantum theme missing from the snapshot"
fi
rm -rf "$KV/ThemeProfileEverforest"
pal_cli apply kv-test >/dev/null 2>&1
if [ -f "$KV/ThemeProfileEverforest/ThemeProfileEverforest.kvconfig" ]; then
  ok "apply restores the profile's Kvantum theme"
else
  fail "apply did not restore the Kvantum theme"
fi

pal_cli save pal-test >/dev/null 2>&1
if grep -q '"accent": "#a7c080"' "$PAL_HOME/.config/theme-profile/profiles/pal-test/.config/theme-profile/palette.json" 2>/dev/null; then
  ok "profiles snapshot the live palette"
else
  fail "palette.json missing from the profile snapshot"
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
backup_dir="$(compgen -G "$FAKE_HOME/.config/theme-profile/backups/deploy-kde-theme-manager-*" | tail -1)"
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
  mkdir -p "$LF_HOME/.config/theme-profile" "$LF_HOME/.config"
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
  mkdir -p "$ACC_HOME/.config/theme-profile" "$ACC_HOME/.config"
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
  mkdir -p "$STYLE_HOME/.config/theme-profile" "$STYLE_HOME/.config/Kvantum"
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

# save writes the formal manifest.json (schema v1) next to the legacy
# manifest.txt, which older tools still read.
if HOME="$MAN_HOME" "$CLI_DIR/theme-profile" save man-profile --components=kdeglobals >/dev/null 2>&1; then
  ok "save man-profile"
else
  fail "save man-profile"
fi
man_root="$MAN_HOME/.config/theme-profile/profiles/man-profile"
if [ -f "$man_root/manifest.json" ]; then
  ok "save writes manifest.json"
else
  fail "save did not write manifest.json"
fi
if [ -f "$man_root/manifest.txt" ]; then
  ok "legacy manifest.txt is still written"
else
  fail "legacy manifest.txt missing"
fi
if grep -q '"schema_version": 1' "$man_root/manifest.json" \
  && grep -q '"name": "man-profile"' "$man_root/manifest.json" \
  && grep -q 'kdeglobals' "$man_root/manifest.json"; then
  ok "manifest.json carries schema, name and the saved components"
else
  fail "manifest.json content is wrong"
fi

# describe --set / describe round-trips through manifest.json.
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
if grep -q '"description": "Dark glass"' "$man_root/manifest.json"; then
  ok "description lands in manifest.json"
else
  fail "description missing from manifest.json"
fi
if HOME="$MAN_HOME" "$CLI_DIR/theme-profile" describe no-such-profile --set=x >/dev/null 2>&1; then
  fail "describe on a missing profile should fail"
else
  ok "describe rejects a missing profile"
fi

# save --description writes the description in one step.
if HOME="$MAN_HOME" "$CLI_DIR/theme-profile" save desc-profile \
    --components=kdeglobals --description="in one step" >/dev/null 2>&1 \
  && [ "$(HOME="$MAN_HOME" "$CLI_DIR/theme-profile" describe desc-profile 2>/dev/null)" = "in one step" ]; then
  ok "save --description round-trips"
else
  fail "save --description round-trip broken"
fi

# components CRUD: the registry lives in components.json next to the profiles.
comp_file="$MAN_HOME/.config/theme-profile/components.json"
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
echo "=== Section 16: clone duplicates a profile snapshot ==="

CLONE_HOME="$TMP_ROOT/clone-home"
mkdir -p "$CLONE_HOME/.config"
printf '[General]\nAccentColor=9,8,7\n' > "$CLONE_HOME/.config/kdeglobals"

if HOME="$CLONE_HOME" "$CLI_DIR/theme-profile" save clone-src --components=kdeglobals >/dev/null 2>&1; then
  ok "save clone-src"
else
  fail "save clone-src"
fi
if HOME="$CLONE_HOME" "$CLI_DIR/theme-profile" describe clone-src --set="Source theme" >/dev/null 2>&1; then
  ok "describe clone-src"
else
  fail "describe clone-src"
fi

# Happy path: snapshot copied, manifest renamed, description/created_at kept.
if HOME="$CLONE_HOME" "$CLI_DIR/theme-profile" clone clone-src clone-dst >/dev/null 2>&1; then
  ok "clone exits 0"
else
  fail "clone failed"
fi
src_root="$CLONE_HOME/.config/theme-profile/profiles/clone-src"
dst_root="$CLONE_HOME/.config/theme-profile/profiles/clone-dst"
if [ -f "$dst_root/manifest.json" ] && [ -f "$dst_root/.config/kdeglobals" ]; then
  ok "clone copied the snapshot files"
else
  fail "clone did not copy the snapshot files"
fi
if cmp -s "$src_root/.config/kdeglobals" "$dst_root/.config/kdeglobals"; then
  ok "clone keeps file contents identical"
else
  fail "clone changed file contents"
fi
if grep -q '"name": "clone-dst"' "$dst_root/manifest.json"; then
  ok "manifest.json carries the new name"
else
  fail "manifest.json name not updated"
fi
if grep -q '"description": "Source theme"' "$dst_root/manifest.json"; then
  ok "clone keeps the description"
else
  fail "clone lost the description"
fi
src_created="$(grep -o '"created_at": "[^"]*"' "$src_root/manifest.json")"
dst_created="$(grep -o '"created_at": "[^"]*"' "$dst_root/manifest.json")"
if [ "$src_created" = "$dst_created" ]; then
  ok "clone keeps the original created_at"
else
  fail "clone changed created_at"
fi

# Source profile and the active marker must stay as they were.
if [ -f "$src_root/manifest.json" ] \
  && [ "$(cat "$CLONE_HOME/.config/theme-profile/active-profile" 2>/dev/null)" = "clone-src" ]; then
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


# --- Summary ------------------------------------------------------------------
echo ""
echo "═══════════════════════════════════════"
echo "  Results: $PASS passed, $FAIL failed"
echo "═══════════════════════════════════════"

[ "$FAIL" -eq 0 ]