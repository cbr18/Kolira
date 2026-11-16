# Карта файлов приложений для расширения покрытия

Дата: 2026-11-15. Это внутренняя карта путей Kolira и кандидатов на
расширение. Первая партия обновлена по реализации: [контракт](../APPLICATIONS.md). [Очередь](../APP_COVERAGE_PRIORITIES.md),
[план](../../plans/2026_11_15_app-coverage-expansion.md).

Пути кандидатов ниже — **ориентиры для исследования**, не уже реализованный
контракт Kolira и не обещание записи всех перечисленных файлов. `<theme-id>`
обозначает имя ресурса; конкретный namespace выбирается при реализации адаптера.
Нужны discovery версии/профиля/native/Flatpak, effective config и проверка
подключения. Наличие ресурса не доказывает автоматическое включение темы.

`C` = `$XDG_CONFIG_HOME`, `D` = `$XDG_DATA_HOME`, `K` = `$XDG_CACHE_HOME`.
Снимок хранит `.config/...`, live resolver учитывает XDG. В записях кандидатов
с `~/.config` отдельно проверить поддержку XDG. Никакие команды из этой карты
не запускаются автоматически.

## Kolira: текущие файлы приложений


Пути ниже соответствуют реестру/адаптерам. `.config`/`.local/share` в снимке
разрешаются через live path resolver, но native/Flatpak discovery отдельных
адаптеров имеет ограничения: наличие flatpak_id в реестре само по себе не
доказывает перенаправление всех путей в sandbox приложения.

| Запись | Файлы и границы |
|---|---|
| Kitty | `C/kitty/{kitty.conf,kolira.conf}` — host/include + оформление |
| Alacritty | `C/alacritty/{alacritty.toml,kolira.toml}` — host/import + цвета |
| Konsole | `C/konsolerc`, выбранный `D/konsole/<name>.profile` и его colorscheme |
| Ghostty | `C/ghostty/{config,kolira}` — host/config-file + оформление |
| Wofi | `C/wofi/{style.css,wallpaper.css}` |
| btop | `C/btop/btop.conf` и активный `themes/<name>.theme` |
| Zed | `C/zed/settings.json` и активные dark/light JSON в themes; кэш metadata отдельно |
| VS Code | `C/Code/User/settings.json`; extension tree из stores/cache. Детект codium/code-oss не превращает этот snapshot path в VSCodium path |
| Cursor | `C/Cursor/User/settings.json`, extension tree из cache |
| Zen | default profile native/XDG/Flatpak: user.js (day/night), chrome/kolira-colors.css; userChrome.css — только import host, не ресурс снимка |
| Firefox | default profile native/XDG/Flatpak: user.js (day/night), chrome/kolira-colors.css + import host; Pywalfox отдельно K/wal/colors.json с обязательным baseline |
| Discord | `C/{Vencord,vesktop}/{themes/,settings/settings.json}`; только тематические ключи settings |
| Spicetify | `C/spicetify/{config-xpui.ini,Themes/}`; runtime spicetify apply |
| Oh My Posh | `C/oh-my-posh/kolira.omp.json`, обнаруженные hosts `C/fish/config.fish`, `~/.zshrc`, `~/.bashrc` — путь --config |
| fish | `C/fish/fish_variables` — только цветовые переменные |
| Cava | `C/cava/config` — секция color |
| Yazi | `C/yazi/theme.toml`, `C/yazi/flavors/` |
| bat | `C/bat/config`, `C/bat/themes/`; runtime cache build |
| Obsidian | index discovery + выбранные vault, appearance.json (theme/cssTheme/enabledCssSnippets), свой snippet, активная custom theme; переносимые слоты снимка вместо пути автора |
| Neovim | C/nvim/kolira-palette.json, фиксированные colors/kolira.lua + after/plugin/kolira.lua; init/specs не изменяются |

VSCodium/Windsurf отсутствуют как отдельные записи snapshot-реестра; cache
installer знает `~/.vscode-oss/extensions` и `~/.windsurf/extensions`. Поэтому
полное обнаружение, выбор темы и capture этих редакторов — первая очередь плана.

## Личные конфиги и runtime первой очереди

| Приложение | Что требуется учитывать помимо файла темы |
|---|---|
| Редакторы | User/settings.json, package.json, registry extensions.json; выбор темы и ресурсы должны входить в один план |
| Neovim | `C/nvim/colors/kolira.lua`, отдельный управляемый loader; init.lua/init.vim не изменяются. Не выполнять личный Lua при discovery/preview, не устанавливать plugin manager |
| Obsidian | Index vault, `<vault>/.obsidian/snippets/kolira.css`, appearance.json; выбор vault явный, пути автора не переносить |
| tmux / Zellij | Effective tmux.conf/config.kdl и ресурс темы; отдельно проверить возврат live options и ограничения reload |
| Starship | `$STARSHIP_CONFIG` либо C/starship.toml; palette и её цвета, без замены модулей prompt |
| lazygit | Только gui.theme в config.yml; остальные YAML-значения, комментарии и формат сохранить |
| Foot / WezTerm / Rio | Выбор схемы/include в foot.ini, wezterm.lua, config.toml; не затенять system config и не переписывать произвольный Lua regex-ом |
| fzf | Файл цветовых options + управляемое подключение с сохранением личных FZF options |
| Firefox / Zen | Обнаруженные profile roots, chrome CSS и user.js; prefs.js не редактировать; при необходимости отдельно подготовить Pywalfox |
| Chromium family | Пользовательское расширение темы; не менять Preferences/Secure Preferences или /etc policies |
| Blender | Пользовательский binary userpref.blend: нужен способ менять/возвращать только оформление |
| LibreOffice | UI theme/extension registry и caches; отдельное исследование и проверка при закрытом приложении |
| Spicetify | Файл темы, ключи выбора и результат patch/apply клиента; внутренние записи требуют проверки в изоляции |

Все записи должны попасть в preview, before/after-журнал и обработку сбоев.
Применение темы без конфига приложения возвращает исходное состояние, а поздние
личные правки сохраняются. Это критерии готовности, не результат живой проверки.

## Кандидаты: ориентиры расположения ресурсов

Сгруппировано по приложению. Динамические пути профилей и workspace нужно
разрешать без побочных записей. Для Emacs проверить C/doom, C/emacs и ~/.emacs.d;
для Obsidian — выбранные vault. Не предполагается копирование произвольных hooks.

| Приложение | Примеры расположения ресурса |
|---|---|
| Alacritty | `$XDG_CONFIG_HOME/alacritty/themes/<theme-id>.toml` |
| Antigravity | `~/.gemini/config/theme.json` |
| Archetype (PokeMMO) | `$XDG_CACHE_HOME/<theme-id>/archetype-pokemmo/colors.json` |
| bat | `$XDG_CONFIG_HOME/bat/themes/<theme-id>.tmTheme` |
| Blender | `~/.cache/<theme-id>/blender_theme_rendered.py` |
| Brave | `$XDG_CACHE_HOME/<theme-id>/brave-theme/manifest.json` |
| Brave Origin | `$XDG_CACHE_HOME/<theme-id>/brave-origin-theme/manifest.json` |
| Btop | `$XDG_CONFIG_HOME/btop/themes/<theme-id>.theme` |
| Cava | `$XDG_CONFIG_HOME/cava/themes/<theme-id>` |
| Claude Code | `~/.claude/themes/<theme-id>.json` |
| Codex | `~/.codex/themes/<theme-id>.tmTheme` |
| darktable | `~/.config/darktable/themes/<theme-id>.css`<br>`~/.var/app/org.darktable.Darktable/config/darktable/themes/<theme-id>.css` |
| Discord | `$XDG_CONFIG_HOME/vesktop/themes/<theme-id>.theme.css`<br>`$XDG_CONFIG_HOME/vesktop/themes/<theme-id>-material.theme.css`<br>`$XDG_CONFIG_HOME/vesktop/themes/discord-system24.css`<br>`~/.var/app/dev.vencord.Vesktop/config/vesktop/themes/<theme-id>.theme.css`<br>`~/.var/app/dev.vencord.Vesktop/config/vesktop/themes/<theme-id>-material.theme.css`<br>`~/.var/app/dev.vencord.Vesktop/config/vesktop/themes/discord-system24.css`<br>`$XDG_CONFIG_HOME/webcord/themes/<theme-id>.theme.css`<br>`$XDG_CONFIG_HOME/webcord/themes/<theme-id>-material.theme.css`<br>`$XDG_CONFIG_HOME/webcord/themes/discord-system24.css`<br>`$XDG_CONFIG_HOME/legcord/themes/<theme-id>.theme.css`<br>`$XDG_CONFIG_HOME/legcord/themes/<theme-id>-material.theme.css`<br>`$XDG_CONFIG_HOME/legcord/themes/discord-system24.css`<br>`$XDG_CONFIG_HOME/equibop/themes/<theme-id>.theme.css`<br>`$XDG_CONFIG_HOME/equibop/themes/<theme-id>-material.theme.css`<br>`$XDG_CONFIG_HOME/equibop/themes/discord-system24.css`<br>`$XDG_CONFIG_HOME/Equicord/themes/<theme-id>.theme.css`<br>`$XDG_CONFIG_HOME/Equicord/themes/<theme-id>-material.theme.css`<br>`$XDG_CONFIG_HOME/Equicord/themes/discord-system24.css`<br>`$XDG_CONFIG_HOME/lightcord/themes/<theme-id>.theme.css`<br>`$XDG_CONFIG_HOME/lightcord/themes/<theme-id>-material.theme.css`<br>`$XDG_CONFIG_HOME/lightcord/themes/discord-system24.css`<br>`$XDG_CONFIG_HOME/dorion/themes/<theme-id>.theme.css`<br>`$XDG_CONFIG_HOME/dorion/themes/<theme-id>-material.theme.css`<br>`$XDG_CONFIG_HOME/dorion/themes/discord-system24.css`<br>`$XDG_CONFIG_HOME/Vencord/themes/<theme-id>.theme.css`<br>`$XDG_CONFIG_HOME/Vencord/themes/<theme-id>-material.theme.css`<br>`$XDG_CONFIG_HOME/Vencord/themes/discord-system24.css`<br>`~/.var/app/com.discordapp.Discord/config/Vencord/themes/<theme-id>.theme.css`<br>`~/.var/app/com.discordapp.Discord/config/Vencord/themes/<theme-id>-material.theme.css`<br>`~/.var/app/com.discordapp.Discord/config/Vencord/themes/discord-system24.css`<br>`$XDG_CONFIG_HOME/BetterDiscord/themes/<theme-id>.theme.css`<br>`$XDG_CONFIG_HOME/BetterDiscord/themes/<theme-id>-material.theme.css`<br>`$XDG_CONFIG_HOME/BetterDiscord/themes/discord-system24.css` |
| Emacs | `<config-root>/themes/<theme-id>-theme.el` |
| Fastfetch | `$XDG_CONFIG_HOME/fastfetch/themes/<theme-id>.jsonc` |
| Fcitx5 | `$XDG_DATA_HOME/fcitx5/themes/<theme-id>/theme.conf` |
| Feishin | `$XDG_CONFIG_HOME/feishin/custom.css` |
| Firefox | `$XDG_CACHE_HOME/<theme-id>/firefox/plasma-colors.css` |
| Foot | `$XDG_CONFIG_HOME/foot/themes/<theme-id>` |
| Fuzzel | `$XDG_CONFIG_HOME/fuzzel/themes/<theme-id>` |
| fzf | `$XDG_CONFIG_HOME/fzf/themes/<theme-id>.sh`<br>`$XDG_CONFIG_HOME/fzf/themes/<theme-id>.fish` |
| Ghostty | `$XDG_CONFIG_HOME/ghostty/themes/<theme-id>` |
| GIMP | `~/.config/GIMP/3.2/gimp.css`<br>`~/.var/app/org.gimp.GIMP/config/GIMP/3.2/gimp.css` |
| Glow | `$XDG_CONFIG_HOME/glow/<theme-id>.json` |
| Gruvbox Plus Icon Theme | `colors-final` |
| GTK 3 | `$XDG_CONFIG_HOME/gtk-3.0/<theme-id>.css` |
| GTK 4 | `$XDG_CONFIG_HOME/gtk-4.0/<theme-id>.css` |
| halloy | `$XDG_CONFIG_HOME/halloy/themes/<theme-id>.toml` |
| Helix | `$XDG_CONFIG_HOME/helix/themes/<theme-id>.toml` |
| herdr | `$XDG_CONFIG_HOME/herdr/<theme-id>-colors.toml` |
| Heroic Launcher | `$XDG_CONFIG_HOME/heroic/themes/matugen.css`<br>`~/.var/app/com.heroicgameslauncher.hgl/config/heroic/themes/matugen.css` |
| Inkscape | `~/.config/inkscape/ui/user.css`<br>`~/.var/app/org.inkscape.Inkscape/config/inkscape/ui/user.css` |
| Karere Custom CSS | `$XDG_CONFIG_HOME/karere/custom.css` |
| Kitty | `$XDG_CONFIG_HOME/kitty/themes/<theme-id>.conf` |
| Konsole | `$XDG_DATA_HOME/konsole/<theme-id>.colorscheme` |
| lazygit | `$XDG_CONFIG_HOME/lazygit/themes/<theme-id>.yml` |
| LazyVim | `$XDG_CONFIG_HOME/nvim/colors/<theme-id>.lua` |
| LibreOffice | `$XDG_STATE_HOME/<theme-id>/libreoffice-theme-staging/Theme_Colors.xcu` |
| lsd | `$XDG_CONFIG_HOME/lsd/colors.yaml` |
| Mailspring | `$XDG_CONFIG_HOME/Mailspring/packages/<theme-id>/package.json`<br>`$XDG_CONFIG_HOME/Mailspring/packages/<theme-id>/styles/ui-variables.less`<br>`~/.var/app/com.getmailspring.Mailspring/config/Mailspring/packages/<theme-id>/package.json`<br>`~/.var/app/com.getmailspring.Mailspring/config/Mailspring/packages/<theme-id>/styles/ui-variables.less` |
| micro | `$XDG_CONFIG_HOME/micro/colorschemes/<theme-id>.micro` |
| MuseScore4 | `$XDG_CONFIG_HOME/MuseScore/<theme-id>.ini` |
| nchat | `$XDG_CONFIG_HOME/nchat/color.conf` |
| Neovim | `$XDG_CONFIG_HOME/nvim/lua/matugen.lua` |
| OBS | `$XDG_CONFIG_HOME/obs-studio/themes/matugen.obt` |
| Obsidian | `<vault>/.obsidian/snippets/<theme-id>.css` |
| Oh My Posh | `$XDG_CONFIG_HOME/oh-my-posh/<theme-id>-palette.json` |
| Opencode | `$XDG_CONFIG_HOME/opencode/themes/matugen.json` |
| Pandora Launcher | `$XDG_DATA_HOME/PandoraLauncher/themes/<theme-id>.json` |
| Papirus Icon Theme | `~/.cache/<theme-id>/papirus-icons/colors-final` |
| Pear Desktop | `$XDG_CONFIG_HOME/YouTube Music/<theme-id>.css` |
| Pi Agent | `~/.pi/agent/themes/<theme-id>.json` |
| Prism Launcher | `$XDG_DATA_HOME/PrismLauncher/themes/Matugen/theme.json`<br>`~/.var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher/themes/Matugen/theme.json` |
| Pywalfox | `$XDG_CACHE_HOME/wal/colors.json` |
| qutebrowser | `$XDG_CONFIG_HOME/qutebrowser/<theme-id>/colors.py` |
| Ratune | `$XDG_CONFIG_HOME/ratune/theme.generated.toml` |
| rio | `$XDG_CONFIG_HOME/rio/themes/<theme-id>.toml` |
| Rofi | `$XDG_CONFIG_HOME/rofi/<theme-id>.rasi` |
| senpai | `$XDG_CONFIG_HOME/senpai/themes/<theme-id>.scfg` |
| Sidra | `$XDG_CONFIG_HOME/Sidra/custom-theme.json` |
| sioyek | `$XDG_CONFIG_HOME/sioyek/themes/<theme-id>.config` |
| Siyuan | `~/Documents/SiYuan/conf/appearance/themes/<theme-id>-siyuan/theme.css`<br>`~/SiYuan/conf/appearance/themes/<theme-id>-siyuan/theme.css`<br>`~/.siyuan/conf/appearance/themes/<theme-id>-siyuan/theme.css` |
| snappy-switcher | `$XDG_CONFIG_HOME/snappy-switcher/themes/<theme-id>.ini` |
| Sone | `$XDG_CONFIG_HOME/sone/theme.json`<br>`~/.var/app/io.github.lullabyX.sone/config/sone/theme.json`<br>`~/snap/sone/current/.config/sone/theme.json` |
| Sonora | `$XDG_CONFIG_HOME/sonora/themes/<theme-id>.json`<br>`~/.var/app/io.github.nolight132.sonora/config/sonora/themes/<theme-id>.json` |
| SpeedCrunch | `~/.local/share/SpeedCrunch/color-schemes/<theme-id>.json` |
| Spicetify | `$XDG_CONFIG_HOME/spicetify/Themes/Comfy/color.ini`<br>`$XDG_CONFIG_HOME/spicetify/Themes/Colorful/color.ini` |
| Spotifast | `$XDG_CONFIG_HOME/spotifast/themes/<theme-id>.json`<br>`~/.var/app/rocks.spotifast.Spotifast/config/spotifast/themes/<theme-id>.json` |
| Starship | `$XDG_CONFIG_HOME/starship/<theme-id>-palette.toml` |
| Steam | `~/.steam/steam/millennium/themes/Material-Theme/css/main/colors/matugen.css`<br>`~/.local/share/Steam/millennium/themes/Material-Theme/css/main/colors/matugen.css`<br>`~/.steam/steam/steamui/skins/Material-Theme/css/main/colors/matugen.css`<br>`~/.local/share/Steam/steamui/skins/Material-Theme/css/main/colors/matugen.css` |
| Strata | `$XDG_CONFIG_HOME/strata/themes/<theme-id>.toml` |
| superfile | `$XDG_CONFIG_HOME/superfile/theme/<theme-id>.toml` |
| Supersonic | `$XDG_CONFIG_HOME/supersonic/themes/<theme-id>.toml`<br>`~/.var/app/io.github.dweymouth.supersonic/config/supersonic/themes/<theme-id>.toml` |
| surgedm | `$XDG_CONFIG_HOME/surge/themes/<theme-id>.toml` |
| Tauon | `$XDG_DATA_HOME/TauonMusicBox/theme/<theme-id>.ttheme` |
| Thunderbird | `$XDG_CACHE_HOME/<theme-id>/thunderbird/<theme-id>.css` |
| tmux | `$XDG_CONFIG_HOME/tmux/themes/<theme-id>.conf` |
| tuios | `$XDG_CONFIG_HOME/tuios/themes/<theme-id>.json` |
| Ungoogled Chromium | `$XDG_CACHE_HOME/<theme-id>/ungoogled-chromium/theme/manifest.json` |
| velo | `$XDG_CONFIG_HOME/velo/palettes/current-<theme-id>-override.json` |
| Vicinae | `$XDG_DATA_HOME/vicinae/themes/<theme-id>.toml` |
| VSCode | `~/.vscode/extensions/<theme-id>.<theme-id>theme-0.0.5/themes/<theme-id>Theme-color-theme.json`<br>`~/.vscode/extensions/<theme-id>.<theme-id>-1.0.0/themes/<theme-id>Theme-color-theme.json`<br>`~/.vscode-oss/extensions/<theme-id>.<theme-id>theme-0.0.5-universal/themes/<theme-id>Theme-color-theme.json`<br>`~/.vscode-oss/extensions/<theme-id>.<theme-id>theme-0.0.5/themes/<theme-id>Theme-color-theme.json`<br>`~/.vscode-oss/extensions/<theme-id>.<theme-id>-1.0.0-universal/themes/<theme-id>Theme-color-theme.json`<br>`~/.vscode-oss/extensions/<theme-id>.<theme-id>-1.0.0/themes/<theme-id>Theme-color-theme.json`<br>`~/.antigravity-ide/extensions/<theme-id>.<theme-id>theme-0.0.5-universal/themes/<theme-id>Theme-color-theme.json`<br>`~/.antigravity-ide/extensions/<theme-id>.<theme-id>theme-0.0.5/themes/<theme-id>Theme-color-theme.json`<br>`~/.antigravity-ide/extensions/<theme-id>.<theme-id>-1.0.0-universal/themes/<theme-id>Theme-color-theme.json`<br>`~/.antigravity-ide/extensions/<theme-id>.<theme-id>-1.0.0/themes/<theme-id>Theme-color-theme.json`<br>`~/.cursor/extensions/<theme-id>.<theme-id>theme-0.0.5/themes/<theme-id>Theme-color-theme.json`<br>`~/.cursor/extensions/<theme-id>.<theme-id>-1.0.0/themes/<theme-id>Theme-color-theme.json`<br>`~/.windsurf/extensions/<theme-id>.<theme-id>theme-0.0.5/themes/<theme-id>Theme-color-theme.json`<br>`~/.windsurf/extensions/<theme-id>.<theme-id>-1.0.0/themes/<theme-id>Theme-color-theme.json` |
| Walker | `$XDG_CONFIG_HOME/walker/themes/<theme-id>/style.css` |
| Wezterm | `$XDG_CONFIG_HOME/wezterm/colors/<theme-id>.toml` |
| WhiteSur Icon Theme | `~/.cache/<theme-id>/whitesur-icons/colors-final` |
| yapfast | `$XDG_CACHE_HOME/<theme-id>/yapfast/theme.json` |
| Yazi | `$XDG_CONFIG_HOME/yazi/flavors/<theme-id>.yazi/flavor.toml`<br>`$XDG_CONFIG_HOME/yazi/flavors/<theme-id>.yazi/tmtheme.xml` |
| Yeet | `$XDG_CONFIG_HOME/yeet/style.css` |
| YTM Player | `$XDG_CONFIG_HOME/ytm-player/theme.toml` |
| Zapfast | `$XDG_CONFIG_HOME/zapfast/themes/<theme-id>.json` |
| Zathura | `$XDG_CONFIG_HOME/zathura/<theme-id>rc`<br>`~/.var/app/org.pwmt.zathura/config/zathura/<theme-id>rc` |
| Zed | `$XDG_CONFIG_HOME/zed/themes/<theme-id>.json`<br>`$XDG_CONFIG_HOME/gram/themes/<theme-id>.json` |
| Zellij | `$XDG_CONFIG_HOME/zellij/themes/<theme-id>.kdl` |
| Zen Browser | `$XDG_CACHE_HOME/<theme-id>/zen-browser/zen-userChrome.css`<br>`$XDG_CACHE_HOME/<theme-id>/zen-browser/zen-userContent.css` |

Для Vim: `~/.vim/colors/<theme-id>.vim` + управляемое подключение в vimrc.
Для fish/Pure: только цветовые universal variables; пользовательские функции и
shell config целиком не переносятся. Для приложений, наследующих KDE/GTK,
сначала проверить системное оформление, отдельный адаптер не добавлять автоматически.

## Границы проверки

Карта основана на статическом исследовании форматов и путей. Внутренние записи
самих приложений, binary storage, signal reload и native/Flatpak discovery должны
проверяться отдельно в изоляции при реализации. Новых адаптеров в этой задаче
не добавлено, темы на хосте не применялись.
