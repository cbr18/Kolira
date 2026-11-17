# Приложения: контракт оформления

Telegram/Kotatogram исключены из поддержки: старые targets и пути в темах
игнорируются, личные конфиги и данные клиентов сохраняются.
[Изменение и проверка](../tasks/2026_11_16_remove-telegram.md).

Перезапуск приложения после Apply допустим (уточнение 2026-11-16).
Ручной выбор/импорт/сброс темы и дополнительная настройка перенесены в P4 в конце
[очереди](APP_COVERAGE_PRIORITIES.md). Неподтверждённое применение отмечается
отдельно; запись ресурса сама по себе не доказывает безопасный Apply/возврат.

Реализованные партии P0 № 1–7, 9–24, 26–29 и расширение VS Code/VSCodium из [приоритетов](APP_COVERAGE_PRIORITIES.md).
Все записи проходят общий preview, before/after-журнал, backup и rollback Rust.
Если следующая тема не содержит конфига приложения, предыдущие изменения
обращаются к исходному состоянию. Поздние личные правки сохраняются при
совпадении записанного after; это не принудительная замена всего конфига.
Новые приложения не устанавливаются автоматически от наличия палитры.

## Выбор целей

В `theme.json` генерация требует `palette` и явного `app_targets`:

```json
"app_targets": ["neovim", "obsidian", "zen", "firefox", "pywalfox",
                "discord", "kitty", "konsole"]
```

Также допустимы прежние `ghostty` и `cava`. Cursor/Zed используют снимок
настроек и `editor_themes[]`, а не отдельный palette target. Сохранённый ресурс
имеет приоритет над генерацией. `--without-sync` отключает генерацию и
установку кэша редакторов, но сохраняет восстановление снимка.
Этот список относится к Apply темы; отдельная команда `palette apply`
сохраняет собственный список целей из [PALETTE.md](PALETTE.md).

## Редакторы

**Cursor.** JSONC выбора темы патчится по позиции; шрифт/размеры, команды,
привязки и комментарии сохраняются согласно политике снимка. Кэш расширения
устанавливается отдельно как `kolira-cache.<publisher>-<name>` в каталоге
`<managed-id>-kolira`, регистрируется в `extensions.json`. Исходное расширение
остаётся. Из манифеста включены только theme contributions, без исполняемых
точек входа. Явное значение `cursor_ide_theme` выбирает тему только при
совпадении id/label с кэшем. Повторный save/apply управляемой копии сохраняет
её ID и локальный источник: фиктивная marketplace-зависимость не создаётся.
Обратный переход удаляет добавленные записи реестра и сохраняет расширения,
которые пользователь установил позже. Реальный reload зависит от редактора.

**Zed.** Кэш JSON-семейств помещается в `zed/themes/`. Сохранённые dark/light/mode
выбираются в JSONC, личные настройки сохраняются. Автоматическая перезагрузка
зависит от версии Zed; наличие файла не объявляется проверкой работающего GUI.

**Neovim.** Управляемые файлы `$XDG_CONFIG_HOME/nvim/kolira-palette.json`,
`colors/kolira.lua` и `after/plugin/kolira.lua`. Палитра — данные base16 + ANSI;
Lua — фиксированный код Kolira. Тема не может передать исполняемый Lua.
`init.lua`, `init.vim`, plugin specs и список плагинов не изменяются и не
выполняются для discovery. При обычном запуске loader включает colorscheme;
в уже открытой сессии нужна `:colorscheme kolira` или перезапуск. После возврата
удаляются только созданные файлы / восстанавливаются ранее существовавшие;
работающему Neovim нужен перезапуск для возвращения личной схемы.

## Obsidian

Discovery читает native/Flatpak `obsidian.json`, а также старые
`~/obsidian/*` / `~/Obsidian/*`. Учитываются только существующие хранилища
внутри HOME, без симлинков в пути. Один vault выбирается автоматически;
при нескольких нужен явный локальный выбор:

```bash
printf '%s' '{}' | kolira core app.vault_selection
printf '%s' '{"vaults":["Documents/MyVault"]}' | kolira core app.select_vaults
```

Порядок выбранных vault задаёт переносимые слоты снимка:
`.config/kolira/app-snapshots/obsidian/<номер>/`. В другом HOME они отображаются
на локально выбранные хранилища по порядку, а не на путь автора. Локальный
выбор хранится в `$XDG_CONFIG_HOME/kolira/app-selection.json`; он не переносится
в тему. Пока отдельного GUI выбора vault нет; app theme chooser работает с
первым выбранным vault, профильный Apply — со всеми выбранными.

Снимок включает `theme`, `cssTheme`, `enabledCssSnippets`, активную custom theme
и собственный `snippets/kolira.css`. Генерация добавляет собственный snippet и
меняет day/night, сохраняя выбранную custom theme, личные snippets и размеры
шрифта. Заметки, vault index, Preferences и плагины не изменяются. Обратный
переход восстанавливает исходное оформление выбранных vault.

## Zen и Firefox

Только **цвета и day/night**. Обнаруживается существующий default profile из
`installs.ini` / `profiles.ini` (порядок ключей не важен), без абсолютных путей,
traversal и симлинков. Поддержаны `.zen`, `$XDG_CONFIG_HOME/zen`,
Flatpak `app.zen_browser.zen/.zen`, `.mozilla/firefox`,
`$XDG_CONFIG_HOME/mozilla/firefox`, Flatpak `org.mozilla.firefox/.mozilla/firefox`
(Flatpak-пути находятся под `~/.var/app/`). Снимок отображается на default
профиль получателя в пределах того же семейства браузера.

- Браузерам задаются только **native accent и day/night**: `ui.accentcolor`,
  `ui.accentcolortext` (нативная контрастная пара для акцента),
  `ui.systemUsesDarkTheme`, `browser.theme.toolbar-theme`,
  `browser.theme.content-theme`, `layout.css.prefers-color-scheme.content-override`;
  Zen дополнительно получает `zen.theme.accent-color` и `zen.view.window.scheme`.
  При явной цели `firefox`/`zen` и палитре её акцент/режим имеют приоритет над
  устаревшими browser values в snapshot. `prefs.js` не редактируется.
- `userChrome.css` не меняется, stylesheet support не включается. Генерация
  `kolira-colors.css` прекращена: нет переопределений toolbar/background,
  кнопок, подписей, icon fill/stroke, размеров, прозрачности или раскладки.
  Старые управляемые CSS-файлы обезвреживаются до комментария при чтении старой
  темы; прежний journal по-прежнему может обратить старые CSS-изменения.
- Privacy, addons, выбранная theme extension, workspace gradients Zen и личный
  CSS остаются личными. Поддержка акцента зависит от native роли браузера;
  Kolira не пытается перекрасить пользовательское theme extension поверх него.
- Для GTK/Breeze также обновляются named colours из той же палитры: Firefox
  в system theme может наследовать их. Исправлены stale красные GTK overrides
  Everforest и остальных комплектных тем. Прозрачность ресурсов сохраняется.

Смена темы и обратный переход могут требовать перезапуска браузера: `user.js`
читается при старте, работающий процесс кэширует prefs. Если исходного user.js
не было, журнал создаёт reset только управляемых pref, не меняя privacy.
Автоматический перезапуск регулируется обычной политикой automation.

**Pywalfox** — отдельный opt-in target. Требуется уже настроенный addon/native
host и существующий валидный `$XDG_CACHE_HOME/wal/colors.json`: это необходимый
baseline. Kolira меняет цветовые данные, затем вызывает `pywalfox update`.
Отсутствие baseline вызывает ошибку preview до изменений. Наличие бинарника
не означает установленный addon; установка/настройка addon не выполняется.

## Мессенджеры и терминалы

**Discord / Vesktop / Vencord.** Цветовой CSS и точечное включение в JSONC
сохраняют личные themeLinks и другие настройки. Учтён Flatpak Vesktop.
Stock Discord без theme framework не объявляется умеющим custom CSS.

**Kitty.** Палитра записывается в `kitty/kolira.conf`, подключается из host,
сохраняя размер шрифта и привязки. Возврат восстанавливает прежние inline цвета
и include; reload выполняется штатным механизмом Kitty.

**Konsole / Yakuake.** Общая схема Konsole + `[Appearance]` default profile;
команда, прокрутка, keybindings и размер шрифта сохраняются. Уже открытые сессии
не переключаются на целый чужой profile по D-Bus: это могло бы изменить их
поведение. Для работающего терминала выдаётся ручное действие обновить выбор
цветовой схемы. Автоматический выбор/reload каждой открытой сессии остаётся
ограничением, а не скрытым побочным эффектом Apply.

## Проверка

`rust_core/tests/first_ten.rs` проверяет несколько циклов возврата к теме без
конфига, поздние личные изменения, перенос vault в другой HOME и ZIP,
перенос native Firefox → Flatpak, browser path safety и кэш редакторов.
Neovim проверяется настоящим `nvim --headless` при его наличии.

Дополнительные изолированные проверки (после `cargo build`):

```bash
python3 tests/e2e/browser-native-accent.py --kolira gui/qt-app/src/core/rust_core/target/debug/kolira --browser firefox
python3 tests/e2e/browser-native-accent.py --kolira gui/qt-app/src/core/rust_core/target/debug/kolira --browser zen-browser --zen
python3 tests/e2e/editor-cache-native.py
```

Browser-тест через Marionette запускает временный профиль, проверяет computed
цвет текста/иконок в light/dark и baseline после возврата к нейтральной теме.
Editor-тест проверяет регистрацию кэша настоящим Cursor. Рабочие конфиги и
браузерные профили пользователя эти проверки не меняют. Полный отчёт и границы
живой проверки — в [задаче](../tasks/2026_11_15_first-ten-apps.md).

## Комплектные темы

Восемь комплектных пакетов, включая material-you, обновлены для этой партии.
Отсутствующие base16-палитры получены из собственных KDE-схем, существующие
сохранены. Cursor/VS Code используют встроенные темы с цветовыми overrides,
Zed — собственные MIT-ресурсы; при Apply кэш расширений и сеть не нужны.
В комплектных темах Firefox меняет штатный day/night без подключения CSS:
отключённые личные UI skins не включаются. Zen получает цвета/day-night.
Pywalfox остаётся отдельным opt-in. Конкретные ресурсы и команда обслуживания:
[themes/README.md](../themes/README.md). Браузерные цветовые файлы экспортируются
из профиля снимка, даже если у экспортирующей машины другое имя default profile.

Обновление app-ресурсов идемпотентно: повторный запуск не меняет байты пакета.
JSONC-объекты/массивы заменяются по span, без добавления повторяющихся ключей;
Inspector сохраняет контракт списка только scalar leaves.

Обслуживание старых пакетов переносит импортируемый GTK theme-profile.css
в kolira.css вместе со ссылкой в gtk.css, сохраняя содержимое; конфликт с другим
kolira.css останавливает подготовку. Live-конфиги этим примером не изменяются.


## Вторая партия P0 № 11–20

Это десять строк приоритетов, включая сгруппированные приложения. Генерация
дополнительно принимает `fish`, `pure`, `oh-my-posh`, `btop`, `yazi`, `bat`,
`wofi`, `fzf`, `lazygit`, `zellij`, `opencode`, `claude-code`, `codex`.
Cava сохраняет прежний target `cava`. Эти цели не расширяют отдельный CLI
`palette apply`. После второй партии в реестре было 41 приложение; текущий реестр содержит 56.

| Приложение | Управляемая часть; личные настройки сохраняются |
|---|---|
| fish / Pure | `fish/fish_variables`: только `fish_color_*`, `fish_pager_color_*`, `pure_color_*`; заголовок universal variables v3. Pure не включается. |
| Oh My Posh | `oh-my-posh/kolira.omp.json`: при генерации — перекрашенная копия уже подключённого prompt со своей структурой, шаблонами и сегментами; явный снимок prompt сохраняет прежний контракт ресурса. Меняется только распознанный существующий init в shell host. |
| btop | `btop/themes/kolira.theme`, только выбор `color_theme`; polling/process-настройки не меняются генерацией. |
| Cava | Только `[color]` в `cava/config`; audio/input/general сохраняются. Штатный reload по сигналу, включая возврат. |
| Yazi | Разрешённые style-ключи в `yazi/theme.toml`; icons/filetype rules/keymap/plugins не заменяются. |
| bat | `bat/themes/kolira.tmTheme` и `--theme=kolira` в `bat/config`; paging/options сохраняются. |
| Wofi | `wofi/kolira.css` с фиксированными цветовыми правилами и import в `style.css`; генерация не меняет геометрию. |
| fzf | `fzf/kolira.opts`: только проверенный `--color`; фиксированный `fish/conf.d/kolira-fzf.fish` подключает данные в новой interactive fish, сохраняя `FZF_DEFAULT_OPTS`. |
| lazygit | Только известные цветовые ключи `gui.theme` в `lazygit/config.yml`. |
| Zellij | `zellij/themes/kolira.kdl` и root `theme`, `theme_dark`, `theme_light` в `config.kdl`. |
| OpenCode V2 | `opencode/cli.json`: только `theme={name:system,mode:dark/light}`; использует ANSI палитру терминала. `opencode.json` не меняется. |
| Claude Code | `.claude/settings.json`: только `theme`, ANSI dark/light; существующий `*-daltonized` сохраняется при смене day/night. |
| Codex | `.codex/themes/kolira.tmTheme`, только `[tui].theme` в `config.toml`: подсветка синтаксиса, не цвета всего TUI. |

## Лаунчеры

| Приложение | Управляемая часть; личные настройки сохраняются |
|---|---|
| Rofi ≥ 1.7 | Цветовые Rasi overrides в ограниченном блоке в `config.rasi`. Личный `@theme`, layout, размеры, font, режимы и привязки остаются. Перезапуск окна лаунчера. |
| Fuzzel ≥ 1.15 | Только RGB нативных ключей `[colors]` в `fuzzel.ini`; RGBA записывается как `RRGGBBAA`. Сохраняются личные alpha, font, launch-prefix и остальные секции. Следующий запуск. |
| Walker 2.x — ограниченная поддержка, P4 | Цветовой блок в **существующем** `themes/<выбранная theme>/style.css`; выбранная тема и XML layout сохраняются. Нужен перезапуск клиента/сервиса. Полный native Walker ещё не проверен. |

Каждый пакет содержит данные `rofi/themes/kolira.rasi`, `fuzzel/fuzzel.ini`,
`walker/kolira.css`, полученные из общей base16/accent-палитры. Применение идёт
через `theme.json.app_targets[]` и общий Plan/backup/journal. Эти приложения не
добавлены в отдельную команду `palette apply`. Ресурс снимка имеет приоритет;
работает и снимок без палитры. Maintenance refresh обновляет все три ресурса.

Rasi/CSS ресурсы принимают только известные селекторы/свойства и буквальные
RGB; импортируемые файлы, команды, размеры и шрифты в них запрещены.
Личные Rofi/Walker hosts не сохраняются в тему и игнорируются в чужом снимке;
из Fuzzel сохраняется только whitelist `[colors]`. Позиционное слияние INI
сохраняет личные настройки; Rofi дополняется inline-цветами без `@theme`/include.
Совпадающие с after значения возвращаются к before, поздние личные цвета,
комментарии, форматирование и CSS-свойства остаются. Поздняя alpha Fuzzel
сохраняется и при возврате старого RGB. Битые/повторные маркеры, связанные hosts
и конфликт с личным ресурсом останавливают preflight до записи.

**Walker остаётся в P4.** При встроенном CSS, SCSS, внешнем каталоге тем или
неподдержанной версии клиент пропускается автоматически, без ручных действий
после Apply. Путь старого поколения `walker/style.css` сохраняется личным.
Произвольные CLI overrides (`rofi -config/-theme`, `walker --theme`) не
обнаруживаются автоматически. Для полноценного Walker-адаптера остаётся native
Apply/перезапуск/возврат, включая встроенный default CSS; замена его одним
цветовым файлом потеряла бы штатное оформление.

Проверки форматов, файлового возврата и границы native-проверки:
[задача ревью и исправлений](../tasks/2026_11_17_launcher-review.md).

Пути под `.config` разрешаются через XDG; `.codex` и `.claude` учитывают
`CODEX_HOME` и `CLAUDE_CONFIG_DIR`. Переносимый снимок хранит канонические пути,
а не каталоги автора. Авторизация, API keys, модели, permissions, hooks, история
и project instructions не входят в снимок. Исключение из запрета agent metadata
сделано только для двух appearance hosts и собственного tmTheme Codex.

YAML patcher поддерживает block mappings с обычными отступами; flow `gui: {...}`,
неоднозначные/повторные ключи и tabs в отступах отклоняются на preflight.
`LG_CONFIG_FILE` и нестандартный `theme_dir` Zellij требуют отдельной настройки:
операция отказывается писать в неиспользуемый default. Другие нестандартные пути
Yazi/bat через CLI flags не обнаруживаются. Не следует применять эти targets,
если приложение запускается с такими overrides.

fzf автоматически подключается только в fish; другим оболочкам нужна нативная
настройка чтения файла цветов. Личные options, bind и команды в него не попадают.
Совпадающий по имени, но изменённый личный loader не перезаписывается: preflight
сообщает конфликт. Тема не может поставить исполняемый shell loader.
Существующие shell/AI/TUI-сессии могут требовать повторного открытия или выбора
темы штатной командой; процессы не завершаются. Нативные настройки AI проверены
по контрактам текущих версий; визуальный reload авторизованных сессий не проверялся.

bat rebuild выполняется внутри операции до фиксации журнала. `$XDG_CACHE_HOME/bat`
включён в safety backup и before/after-журнал: возвращается даже исходная тема,
сохранившаяся только в бинарном кэше. Ошибка/таймаут rebuild откатывает операцию.
`KTM_THEME_NO_APPLY=1` не запускает rebuild и reload.

Проверки второй партии: `rust_core/tests/second_ten.rs` — повторные циклы возврата,
late edits, секреты при save/export/import, path safety, shell injection/conflicts,
перенесённые AI roots и кэш bat. При наличии бинарников запускаются настоящие
fish/fzf, bat cache и `zellij setup --check` в изолированном HOME. Нативные GUI
Wofi/btop/lazygit и работающие AI-сеансы не объявляются визуально проверенными.


## Проверка согласованности палитры

[Аудит восьми тем](audits/2026_11_16_palette-consistency.md) отделяет активные
palette resources, нативные ANSI/system режимы и отдельные style/library assets.
`tests/first_ten.rs` для каждого пакета проверяет фактические значения браузеров,
GTK/KDE, редакторов, терминалов, shell/TUI и AI selectors, затем возврат.
`tests/e2e/browser-native-accent.py` запускает каждый браузер в disposable HOME
для baseline → восемь пакетов → neutral, читает computed AccentColor/day-night
и контраст штатных иконок с учётом прозрачных родительских поверхностей.


## Третья партия: нативные цвета без личных данных

Все восемь комплектных тем содержат сгенерированные из `theme.json.palette`
ресурсы и нативные ключи выбора. Новые `app_targets`: `micro`, `lsd`, `glow`, `gimp`, `heroic`,
`prismlauncher`, `vim`, `mpv`, `cliamp`, `vscodium`. LibreOffice исключён пользователем: стандартное оформление,
`.config/libreoffice/` не отслеживается и защищён от применения старых снимков.

| Приложение | Файлы снимка и границы |
|---|---|
| Micro | `.config/micro/settings.json`: только `colorscheme`; `colorschemes/kolira.micro`: только `color-link`. Плагины и bindings не устанавливаются. |
| lsd | `.config/lsd/config.yaml`: только `color.theme`; `themes/kolira.yaml`: нативная таблица цветов. `LS_COLORS`, classic, icons и sorting остаются личными. |
| Glow | `.config/glow/glow.yml`: только `style`; `kolira.json`: Glamour. Снимок хранит переносимое `kolira`, Apply разрешает его в XDG-путь получателя. Внешние личные style paths не переносятся. |
| GIMP 3 | `.config/GIMP/3.0/gimprc`: только `theme` и `theme-color-scheme`; `themes/Kolira/gimp.css`: импорт установленного Default dark/light и цветовые определения. Layout, инструменты и плагины не переносятся. Требуется штатный `/usr/share/gimp/3.0/themes/Default`. |
| Heroic | `.config/heroic/config.json`: управляемый блок цветовых переменных внутри `defaultSettings.customCSS`. Личный CSS, игры, аккаунты, `store/config.json` и другие настройки не захватываются; поздние правки вне блока сохраняются. |
| Prism | `.local/share/PrismLauncher/prismlauncher.cfg`: только `[General] ApplicationTheme`; `themes/kolira/theme.json`: Qt palette на штатном Fusion, без QSS. Instances, аккаунты, Java и настройки Minecraft не меняются. |
| Vim | `.vim/kolira-palette.json`: данные. Core создаёт фиксированные `.vim/colors/kolira.vim` и `.vim/after/plugin/kolira.vim`; тема не может принести Vimscript. `vimrc`, plugins и termguicolors не меняются; конфликт с личным loader останавливает preview. |
| mpv | `.config/mpv/mpv.conf`: только root `osd-color`, `osd-border-color`, `osd-shadow-color`. Цвета `#rrggbb` записываются в кавычках: иначе mpv воспринимает их как комментарий. Секции `[profile]`, video/audio, scripts и fonts остаются личными. |
| Cliamp | `.config/cliamp/config.toml`: только `theme`; `themes/kolira.toml`: нативные RGB-цвета. Playlist/provider/player параметры сохраняются. |
| VSCodium | `.config/VSCodium/User/settings.json`: позиционная JSONC-политика только цветов, builtin Modern + palette overrides; кэш использует общий адаптер редакторов. |

Открытые GIMP/Heroic/Prism, редакторы, проигрыватели и TUI не завершаются:
перечитать тему или открыть новую сессию нужно штатным способом. Для Vim требуется
обычный runtimepath с включёнными startup plugins. Нестандартные CLI flags,
Flatpak-пути этой партии и portable-установки не обнаруживаются автоматически.
`MICRO_CONFIG_HOME`, `LSD_CONFIG_DIR`, `GLOW_CONFIG_HOME`, `MPV_HOME` и
`GIMP3_DIRECTORY` отклоняются при генерации вместо записи в неиспользуемый default.
JSONC сохраняет позиции значений, включая вставку отсутствующего ключа в
существующий вложенный объект; окружающие комментарии не пересериализуются.

Проверки: `rust_core/tests/third_ten.rs` (циклы, отсутствие файлов, поздние правки,
конфликт loaders, небезопасные ресурсы, исключение LibreOffice),
`tests/e2e/third-apps-native.py` (настоящие Micro/lsd/Glow/Vim/mpv в disposable HOME),
`tests/e2e/third-apps-gtk.py` (GTK3 parser всех GIMP CSS, нужен display/Xvfb).
Результаты и границы визуальной проверки — в
[карточке партии](../tasks/2026_11_16_third-ten-apps.md).

Для Heroic отдельно проверяются нормальные и hover-пары текста кнопок,
для Prism — основные QPalette foreground/background роли во всех восьми темах.
Hover primary усиливает контраст с selection text; tertiary не получает
акцентный фон под обычным светлым текстом. Проверка native load Prism/Chromium:
`python3 tests/e2e/third-apps-clients.py` (одноразовые профили, Qt offscreen/headless).
Heroic дополнительно проверяется настоящим Electron-клиентом через локальный CDP:
`xvfb-run -a python3 tests/e2e/third-apps-heroic.py` (Python Playwright, без скачивания
браузера, одноразовый HOME без аккаунтов). Проверяются computed CSS variables и
color-scheme. Игры/аккаунты и Google Chrome не проверялись. Cliamp дополнительно рендерится
в PTY с проверкой фактического ANSI accent; без воспроизведения, ALSA null и
одноразовые IPC roots исключают подключение к личному проигрывателю.

## Starship, tmux, Foot, WezTerm

Цели `starship`, `tmux`, `foot`, `wezterm` входят в `theme.json.app_targets[]`.
Все восемь комплектных тем содержат ресурсы из своей base16/accent-палитры.
В реестре 56 приложений, в комплектных пакетах по 38 targets. Наличие файла
в снимке имеет приоритет над генерацией из палитры; приложения не устанавливаются
и prompt не включается при Apply.

| Приложение | Что меняется | Когда видно |
|---|---|---|
| Starship | `$XDG_CONFIG_HOME/starship.toml`: root `palette`, только `[palettes.kolira]`; модули, commands, format, symbols и shell init сохраняются | Следующий prompt |
| tmux | `.tmux.conf` либо `$XDG_CONFIG_HOME/tmux/tmux.conf`: только фиксированное подключение; `tmux/kolira.json` — RGB-данные, `kolira.conf` генерируется core | Открытые серверы обновляются без остановки сессий; новые читают подключение |
| Foot | `$XDG_CONFIG_HOME/foot/foot.ini`: foreground/background/selection/cursor и ANSI в `colors-dark`/`colors-light`; оба варианта соответствуют выбранной теме | Новое окно/перезапуск Foot; в server mode нужно перечитать конфиг сервера перезапуском в удобный момент |
| WezTerm | `$XDG_CONFIG_HOME/wezterm/kolira.toml`: только native RGB/ANSI. Core оборачивает существующий Lua в фиксированное подключение, сохраняя его тело | Native config reload или перезапуск; palette-only изменения не принуждают завершать окна |

Starship сохраняет личные style-строки. Стандартные именованные ANSI-цвета
переопределяются палитрой; пользовательские RGB aliases выбранной палитры
переносятся в собственную таблицу с пересчётом через существующий OKLab mapper.
Явные RGB-литералы внутри личных `format`/`style` не переписываются. Ничего из
`[custom]` не импортируется из чужой темы и не исполняется для discovery.

WezTerm: сначала явный стандартный `WEZTERM_CONFIG_FILE`, затем XDG-конфиг, затем
`~/.wezterm.lua`. Произвольный `--config-file`, нестандартный env path и legacy
`~/.config/wezterm` за пределами перенесённого XDG требуют отдельной интеграции:
операция отказывается писать в неиспользуемый host. Lua из снимка игнорируется;
экспорт содержит только TOML палитры. Внутри core wrapper выполняется только
существующий личный Lua, уже предназначенный для WezTerm, при штатной загрузке
приложением. Сам Kolira его не запускает. Shebang/BOM, ссылки на host и конфликт
зарезервированного namespace отклоняются до записи. При снятии wrapper поздние
правки тела Lua сохраняются; при личном изменении wrapper и исчезновении палитры
он безопасно возвращает исходный config без попытки открыть отсутствующую схему.
Применяются только управляемые RGB/ANSI-поля; личные indexed colours и настройки
вкладок, включая их атрибуты, остаются в исходном config.colors.

tmux: управляются только foreground/background восьми global style options:
`status-style`, `message-style`, `message-command-style`, `pane-border-style`,
`pane-active-border-style`, `window-status-style`, `window-status-current-style`,
`mode-style`. Атрибуты bold/underline и локальные overrides отдельных окон/сессий
сохраняются. Runtime-записи выполняются аргументами `set-option`, а не через
`source-file` личного конфига. Серверы не создаются и не завершаются. Обнаружение
охватывает собственные sockets в `${TMUX_TMPDIR:-/tmp}/tmux-UID` и socket из
`TMUX`; дополнительные нестандартные socket paths вне этой области автоматически
не ищутся. Явный `tmux -f`/плагины с динамическим повторным переопределением цветов
не гарантируют такое же поведение при следующем старте сервера.

Личные tmux commands не сохраняются в тему. Файл `kolira.conf` содержит только
фиксированный whitelist команд из RGB-JSON; принесённый темой script игнорируется.
Конфликт с существующим личным файлом этого имени останавливает Apply до записи.
Runtime before/after учитываются в общем apply-journal и safety snapshot вместе
с identity socket/server. Возврат меняет только цвета, всё ещё совпадающие с
записанным after; поздние личные цвета и атрибуты сохраняются. Старый журнал
не меняет новый сервер на переиспользованном socket path.

Native минимумы: Starship 1.27, tmux 3.2, Foot 1.28, WezTerm build 20220807.
Старые/неопределяемые версии отклоняются на preflight до правок, `NO_APPLY` не
запускает version probes или runtime commands. Flatpak/portable и произвольные
CLI config overrides этой партии не заявляются автоматически поддерживаемыми.
Проверки и их границы: [задача](../tasks/2026_11_16_four-terminal-apps.md).

## Финальный срез существующих адаптеров

По решению пользователя задача покрытия закрыта без добавления следующих
приложений. [Матрица 56 registry entries](audits/2026_11_17_existing-adapters-closeout.md)
показывает, что проверено настоящим клиентом, а что — только файловым контрактом.
Повторная проверка btop выявила native переформатирование конфига: возврат теперь
восстанавливает selector по значению, сохраняя late settings/comments.
Spicetify получил checked refresh, обновление при возврате, атомарные palette
записи с rollback и исключение исполняемых theme scripts. Его настоящий Spotify
runtime остаётся неподтверждённым; авторизация клиентов для QA не используется.
