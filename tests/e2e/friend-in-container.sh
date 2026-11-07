#!/usr/bin/env bash
# The friend's whole story on a clean machine, against a real KWin and
# plasmashell. Run as a normal user inside the image from tests/e2e/Dockerfile:
#   friend-in-container.sh SOURCE OUT
# SOURCE: anything `git clone` takes. OUT: folder for screenshots and logs.
# Set E2E_POPULATED=1 to start from a configured home (make-friend-home.sh)
# instead of Plasma's first run. Prints PASS/FAIL lines and exits non-zero if
# anything failed. Screenshots are not possible here: KWin in a container crashes
# on any capture, so the look itself is not checked, only files and processes.
set -u

SOURCE="$1"
OUT="$2"
mkdir -p "$OUT"
FAILED=0
pass() { echo "PASS  $1"; }
fail() { echo "FAIL  $1"; FAILED=1; }

export XDG_RUNTIME_DIR="/tmp/xdg-$(id -u)"
mkdir -p -m 700 "$XDG_RUNTIME_DIR"
export LANG=C.UTF-8 PATH="$HOME/.local/bin:$PATH"
export LIBGL_ALWAYS_SOFTWARE=1 QT_QUICK_BACKEND=software KWIN_COMPOSE=Q
export WAYLAND_DISPLAY=wayland-e2e QT_QPA_PLATFORM=wayland XDG_SESSION_TYPE=wayland
export XDG_CURRENT_DESKTOP=KDE KDE_SESSION_VERSION=6 KDE_FULL_SESSION=true

# --- download and install, as the README says --------------------------------
git clone -q "$SOURCE" "$HOME/kde-theme-manager" || { fail "git clone"; exit 1; }
if (cd "$HOME/kde-theme-manager" && bash scripts/install-theme-profile-kit.sh) >"$OUT/install.log" 2>&1; then
  pass "installer finished"
else
  fail "installer failed: $(tail -3 "$OUT/install.log")"
  exit 1
fi
KIT="$HOME/kde-theme-manager"
if [ "${E2E_POPULATED:-0}" = 1 ]; then
  bash "$KIT/tests/e2e/make-friend-home.sh" "$HOME"
  pass "started from a configured home"
fi
for tool in theme-profile theme-menu kde-theme-manager; do
  command -v "$tool" >/dev/null && pass "$tool is installed" || fail "$tool is missing"
done

theme-profile list 2>/dev/null | grep -qx my-original-look && pass "the installer saved the original look" || fail "no my-original-look theme"

# systemd --user is not running in a container: only this service is needed.
SHIM="$HOME/.shim"
mkdir -p "$SHIM"
cat >"$SHIM/systemctl" <<'EOF'
#!/usr/bin/env bash
case "$*" in
  *"stop plasma-plasmashell.service"*) pkill -TERM -x plasmashell; for _ in $(seq 50); do pgrep -x plasmashell >/dev/null || exit 0; sleep 0.1; done; pkill -KILL -x plasmashell ;;
  *"start plasma-plasmashell.service"*) pgrep -x plasmashell >/dev/null || (nohup plasmashell >>"$E2E_OUT/plasmashell.log" 2>&1 &) ;;
  *"is-active"*) pgrep -x plasmashell >/dev/null || exit 3 ;;
esac
exit 0
EOF
chmod +x "$SHIM/systemctl"
export PATH="$SHIM:$PATH" E2E_OUT="$OUT"

# --- a real session ------------------------------------------------------------
kwin_wayland --virtual --no-lockscreen --width 1600 --height 900 --socket wayland-e2e >"$OUT/kwin.log" 2>&1 &
for _ in $(seq 60); do [ -S "$XDG_RUNTIME_DIR/wayland-e2e" ] && break; sleep 0.5; done
[ -S "$XDG_RUNTIME_DIR/wayland-e2e" ] && pass "kwin_wayland started" || { fail "kwin_wayland did not start"; exit 1; }
nohup plasmashell >>"$OUT/plasmashell.log" 2>&1 &
sleep 12
pgrep -x plasmashell >/dev/null && pass "plasmashell started" || fail "plasmashell did not start"

alive() {
  pgrep -x kwin_wayland >/dev/null && pgrep -x plasmashell >/dev/null
}
crashes() { grep -lE 'Traceback|Segmentation|core dumped|Fatal|panicked' "$OUT"/app.log "$OUT"/plasmashell.log 2>/dev/null; }

# Plasma's first start writes its layout; this is the friend's real starting point.
python3 "$KIT/tests/e2e/snapshot.py" "$HOME" "$OUT/first.json"
QT_QUICK_CONTROLS_STYLE=org.kde.desktop nohup kde-theme-manager >"$OUT/app.log" 2>&1 &
sleep 8
pgrep -f 'python.* -m src' >/dev/null && pass "KDE Theme Manager started" || fail "KDE Theme Manager did not start: $(tail -3 "$OUT/app.log")"

# --- the backup -----------------------------------------------------------------
if theme-profile save my-look --description='before anything' >"$OUT/save.log" 2>&1; then
  pass "own theme saved"
else
  fail "save failed: $(tail -2 "$OUT/save.log")"
fi
sleep 3
python3 "$KIT/tests/e2e/snapshot.py" "$HOME" "$OUT/before.json"
gtk_before="$(gsettings get org.gnome.desktop.interface gtk-theme) $(gsettings get org.gnome.desktop.interface color-scheme)"

themes=${E2E_THEMES:-"frutiger-aero-light frutiger-aero Paper dracula everforest glass"}
for theme in $themes; do
  report="$OUT/apply-$theme.json"
  if theme-profile apply "$theme" --json >"$report" 2>"$OUT/apply-$theme.err"; then
    pass "apply $theme"
  else
    fail "apply $theme: $(tail -2 "$OUT/apply-$theme.err")"
  fi
  python3 - "$report" "$theme" <<'PY'
import json, sys
try:
    d = json.load(open(sys.argv[1]))
except Exception as e:
    print(f"FAIL  {sys.argv[2]}: no JSON report ({e})"); sys.exit()
print(f"info  {sys.argv[2]}: errors={d['errors']} missing={[m.get('name') for m in d['missing']]}")
if d["errors"]:
    print(f"FAIL  {sys.argv[2]}: errors in the report")
PY
  sleep 6
  alive && pass "$theme: kwin and plasmashell alive" || fail "$theme: the session died"
  pgrep -f 'python.* -m src' >/dev/null && pass "$theme: app alive" || fail "$theme: app died: $(tail -3 "$OUT/app.log")"

  if theme-profile apply my-look --json >"$OUT/back-$theme.json" 2>"$OUT/back-$theme.err"; then
    pass "back from $theme"
  else
    fail "back from $theme: $(tail -2 "$OUT/back-$theme.err")"
  fi
  sleep 6
  alive && pass "back from $theme: session alive" || fail "back from $theme: the session died"
  pgrep -f 'python.* -m src' >/dev/null && pass "back from $theme: app alive" || fail "back from $theme: app died"
  python3 "$KIT/tests/e2e/snapshot.py" "$HOME" "$OUT/after-$theme.json"
  gtk_after="$(gsettings get org.gnome.desktop.interface gtk-theme) $(gsettings get org.gnome.desktop.interface color-scheme)"
  [ "$gtk_after" = "$gtk_before" ] && pass "back from $theme: GSettings as they were" || fail "back from $theme: GSettings $gtk_before -> $gtk_after"
  if diff_text="$(python3 "$KIT/tests/e2e/compare.py" "$OUT/before.json" "$OUT/after-$theme.json")"; then
    pass "back from $theme: files as they were"
  else
    fail "back from $theme: files differ:
$diff_text"
  fi
done

# The same, through the app's own controller (what its Apply button calls).
if python3 "$KIT/tests/e2e/gui-apply.py" frutiger-aero-light >"$OUT/gui1.log" 2>&1 && sleep 6 && python3 "$KIT/tests/e2e/gui-apply.py" my-look >"$OUT/gui2.log" 2>&1; then
  pass "there and back through the app's controller"
else
  fail "app controller: $(tail -3 "$OUT/gui1.log" "$OUT/gui2.log")"
fi
sleep 6
alive && pass "after the app's controller: session alive" || fail "after the app's controller: the session died"
python3 "$KIT/tests/e2e/snapshot.py" "$HOME" "$OUT/after-gui.json"
if diff_text="$(python3 "$KIT/tests/e2e/compare.py" "$OUT/before.json" "$OUT/after-gui.json")"; then
  pass "after the app's controller: files as they were"
else
  fail "after the app's controller: files differ:
$diff_text"
fi
if KTM_THEME_NO_APPLY=1 theme-profile status >/dev/null 2>"$OUT/status.err"; then pass "theme-profile status"; else fail "status: $(cat "$OUT/status.err")"; fi

bad="$(crashes)"
[ -z "$bad" ] && pass "no crash in the logs" || fail "crash messages in: $bad"
pkill -x plasmashell; pkill -x kwin_wayland; pkill -f 'python.* -m src'
echo "=== RESULT: $([ "$FAILED" = 0 ] && echo ALL PASSED || echo FAILURES)"
exit "$FAILED"
