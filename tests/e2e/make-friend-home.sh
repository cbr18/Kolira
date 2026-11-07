#!/bin/bash
# A friend's plain Plasma home (stock Breeze, some apps configured).
H=$1; mkdir -p "$H"/.config/{gtk-3.0,gtk-4.0,kitty,alacritty,fish,btop,Code/User,zed,wofi,klassy} "$H"/.config/nvim "$H/.mozilla/firefox/abc.default-release/chrome" "$H/.zen/xyz.Default (release)" "$H/Pictures/Wallpapers" "$H/.local/share/konsole"
printf 'jpeg-of-friend' >"$H/Pictures/Wallpapers/my wall.jpg"
cat >"$H/.config/kdeglobals" <<E
[General]
ColorScheme=BreezeLight
TerminalApplication=konsole
font=Noto Sans,11,-1,5,400,0,0,0,0,0,0,0,0,0,0,1

[KDE]
LookAndFeelPackage=org.kde.breeze.desktop
SingleClick=false
widgetStyle=Breeze

[Icons]
Theme=breeze

[Colors:Window]
BackgroundNormal=239,240,241
ForegroundNormal=35,38,41

[Colors:View]
BackgroundNormal=252,252,252
BackgroundAlternate=245,245,245
ForegroundNormal=35,38,41

[Colors:Button]
BackgroundNormal=252,252,252

[Colors:Selection]
BackgroundNormal=61,174,233
ForegroundNormal=255,255,255

[KScreen]
ScreenScaleFactors=eDP-1=1.25;
E
cat >"$H/.config/kwinrc" <<E
[Desktops]
Number=4

[Plugins]
blurEnabled=true
wobblywindowsEnabled=false

[Effect-blur]
BlurStrength=7

[Windows]
FocusPolicy=FocusFollowsMouse

[org.kde.kdecoration2]
library=org.kde.breeze
theme=Breeze
E
printf '[General]\ncount=1\nrules=fr1\n\n[fr1]\nDescription=my firefox rule\nwmclass=firefox\nwmclassmatch=1\nminimize=true\nminimizerule=2\n' >"$H/.config/kwinrulesrc"
printf '[Theme]\nname=default\n' >"$H/.config/plasmarc"
printf '[Mouse]\ncursorTheme=breeze_cursors\ncursorSize=32\nPointerAcceleration=0.5\n' >"$H/.config/kcminputrc"
printf '[Style]\nMenuOpacity=100\n' >"$H/.config/breezerc"
cat >"$H/.config/plasmashellrc" <<E
[PlasmaViews][Panel 2]
floating=1
panelOpacity=0
panelVisibility=0

[PlasmaViews][Panel 2][Defaults]
thickness=44
E
cat >"$H/.config/plasma-org.kde.plasma.desktop-appletsrc" <<E
[Containments][1]
activityId=friend-activity
formfactor=0
lastScreen=0
location=0
plugin=org.kde.plasma.folder
wallpaperplugin=org.kde.image

[Containments][1][General]
positions={"1920x1080":["1","2","desktop:/friend.txt","0","0"]}

[Containments][1][Wallpaper][org.kde.image][General]
Image=file://$H/Pictures/Wallpapers/my%20wall.jpg

[Containments][2]
formfactor=2
lastScreen=0
location=4
plugin=org.kde.panel

[Containments][2][Applets][3]
plugin=org.kde.plasma.kickoff

[Containments][2][Applets][3][Configuration][General]
favorites=firefox.desktop

[Containments][2][Applets][4]
plugin=org.kde.plasma.icontasks

[Containments][2][General]
AppletOrder=3;4
E
printf '[Settings]\ngtk-application-prefer-dark-theme=false\ngtk-cursor-theme-name=breeze_cursors\ngtk-cursor-theme-size=32\ngtk-font-name=Noto Sans,  10\ngtk-icon-theme-name=breeze\ngtk-theme-name=Breeze\ngtk-modules=colorreload-gtk-module\n' | tee "$H/.config/gtk-3.0/settings.ini" >"$H/.config/gtk-4.0/settings.ini"
printf '@import "colors.css";\n' | tee "$H/.config/gtk-3.0/gtk.css" >"$H/.config/gtk-4.0/gtk.css"
printf '@define-color theme_bg_color #eff0f1;\n' | tee "$H/.config/gtk-3.0/colors.css" >"$H/.config/gtk-4.0/colors.css"
printf 'font_size 14\nbackground #101010\nforeground #eeeeee\nmap ctrl+a new_tab\nscrollback_lines 5000\n' >"$H/.config/kitty/kitty.conf"
printf '[font]\nsize = 13.0\n\n[colors.primary]\nbackground = "#101010"\n\n[keyboard]\nbindings = []\n' >"$H/.config/alacritty/alacritty.toml"
printf 'set number\n' >"$H/.config/nvim/init.vim"
printf 'SETUVAR fish_color_command:blue\nSETUVAR fish_user_paths:/opt/friend\n' >"$H/.config/fish/fish_variables"
printf 'color_theme = "Default"\nupdate_ms = 1000\n' >"$H/.config/btop/btop.conf"
printf '{\n  // my settings\n  "workbench.colorTheme": "Default Light Modern",\n  "editor.fontSize": 15,\n  "remote.SSH.remotePlatform": {"srv": "linux"}\n}\n' >"$H/.config/Code/User/settings.json"
printf '{\n  "theme": "One Light",\n  "ui_font_size": 15\n}\n' >"$H/.config/zed/settings.json"
printf 'window { background-color: white; }\n' >"$H/.config/wofi/style.css"
printf '[General]\nStartWithLastProfile=1\n\n[Profile0]\nName=default\nIsRelative=1\nPath=abc.default-release\nDefault=1\n' >"$H/.mozilla/firefox/profiles.ini"
printf '[AAA]\nDefault=abc.default-release\nLocked=1\n' >"$H/.mozilla/firefox/installs.ini"
printf 'user_pref("privacy.resistFingerprinting", true);\n' >"$H/.mozilla/firefox/abc.default-release/user.js"
printf 'user_pref("browser.theme.toolbar-theme", 0);\n' >"$H/.mozilla/firefox/abc.default-release/prefs.js"
printf '[General]\nStartWithLastProfile=1\n\n[Profile0]\nName=Default (release)\nIsRelative=1\nPath=xyz.Default (release)\nDefault=1\n' >"$H/.zen/profiles.ini"
printf '[AAA]\nDefault=xyz.Default (release)\nLocked=1\n' >"$H/.zen/installs.ini"
printf 'user_pref("zen.view.window.scheme", 2);\nuser_pref("browser.startup.page", 3);\n' >"$H/.zen/xyz.Default (release)/prefs.js"
printf '[Appearance]\nColorScheme=Breeze\nFont=Hack,12,-1,5,400,0,0,0,0,0\n\n[General]\nCommand=/bin/fish\nName=Mine\n' >"$H/.local/share/konsole/Mine.profile"
printf '[Desktop Entry]\nDefaultProfile=Mine.profile\n' >"$H/.config/konsolerc"
