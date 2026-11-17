# Финальная проверка существующих адаптеров

Дата: 2026-11-17. Область — **56 существующих registry entries**, без новых
приложений и без увеличения 38 app_targets комплектных тем. Это системные
компоненты, приложения и группы клиентов, а не 56 независимо проверенных GUI.
По запросу пользователя дальнейшее расширение исключено из текущей задачи.

Нельзя обоснованно обещать «идеально во всех версиях». Проверка разделяет
файловый контракт, реальный parser/runtime и неподтверждённый полный UI.
P4 и оговорки ниже остаются ограничениями текущего продукта; они не объявляются
доказанной автоматизацией и не превращаются в продолжение закрываемой задачи.

| Группа | Registry IDs | Проверка и границы |
|---|---|---|
| KDE/GTK/Qt | `kwin`, `plasma_shell`, `kdeglobals`, `gtk3`, `gtk4`, `qt5ct`, `breeze`, `klassy`, `kvantum`, `color_schemes`, `look_and_feel`, `palette` | Общий Plan/undo; clean-machine KWin/Plasma 6 тем + GUI возврат, реальные preview 8 тем. Сторонние effects/assets зависят от установленных версий. |
| Редакторы/заметки | `zed`, `vscode`, `cursor`, `vscodium`, `neovim`, `vim`, `micro`, `obsidian`, `gimp` | Кэш/JSONC/vault/appearance-only, rollback и late edits. Native Cursor cache, Vim/Micro RGB и GIMP GTK на 8 темах; Neovim headless. Zed/Obsidian: полный native GUI остаётся непроверенным. |
| Браузеры | `zen`, `firefox` | Оба настоящих клиента: 8 тем + baseline/neutral, accent/day-night, контраст навигационных кнопок, личный CSS не включается. Pywalfox — опциональный отдельный baseline/addon. |
| Discord-клиенты | `discord` | Discord/Vesktop/Vencord: CSS/JSONC, Flatpak, сохранение личных settings и возврат. Авторизованная native сессия не проверялась; может требоваться перезапуск. |
| Терминалы | `kitty`, `alacritty`, `ghostty`, `konsole`, `foot`, `wezterm` | Kitty/include, Alacritty/Ghostty XDG/return, Konsole/Yakuake scheme/profile, Foot parser и WezTerm loader 8 тем. Для уже открытых Konsole-сессий сохраняется прежнее ограничение выбора схемы; пользовательские вкладки не подменяются. |
| Shell/session | `fish`, `pure`, `oh-my-posh`, `starship`, `tmux` | Native fish/fzf, OMP сохраняет существующий prompt, Starship RGB/Foot/WezTerm 8 тем, tmux runtime/identity/rollback. Не включаем prompt и не переписываем личные команды. |
| TUI/инструменты | `btop`, `cava`, `yazi`, `bat`, `lsd`, `glow`, `fzf`, `lazygit`, `zellij` | Native RGB btop/Yazi/lazygit и возврат с перезапуском; Micro/lsd/Glow/Vim/mpv/Cliamp 8 тем. Bat cache/format, fish/fzf actual invocation, Zellij native parser. Native открытая Zellij-сессия и fzf вне fish не подтверждены. |
| AI-клиенты | `opencode`, `claude-code`, `codex` | Только ANSI day/night/theme selector/tmTheme; models/auth/permissions/hooks не попадают в снимки. Авторизованные native сессии не запускались; полный UI обозначен как неподтверждённый. |
| Медиа | `spicetify`, `mpv`, `cliamp` | mpv/Cliamp native 8 тем. Spicetify: 6 новых регрессий safety/failure/return; нативный Spotify не установлен, его пропатченный runtime не сертифицирован (P4). |
| Игровые клиенты | `heroic`, `prismlauncher` | 8 тем: настоящий Heroic customCSS через CDP и настоящий Prism Qt load. Личные аккаунты, библиотеки и игры не использовались. |
| Лаунчеры | `wofi`, `rofi`, `fuzzel`, `walker` | Native Rofi/Fuzzel/GTK 8 тем и return; Wofi CSS/import/late geometry. Walker 2.x существующий CSS: GTK4/файловая проверка; встроенные/SCSS/external темы пропускаются (P4). |

## Найдено и исправлено при закрытии

1. **btop**: native клиент переписывает пробелы/порядок конфигурации. Возврат
   по совпадению строк иногда оставлял `kolira`. Для `Policy::Kv` используется
   восстановление значений ключей с сохранением late settings/comments.
   Есть отдельная регрессия и повторный настоящий RGB Apply/return.
2. **Spicetify**: внешний failed/timeout apply больше не скрывается за успехом;
   профиль откатывает файлы, palette target откатывает bytes/modes/absence.
   Refresh вызывается и при возврате к снимку без Spicetify config.
3. **Spicetify**: snapshot не меняет `inject_theme_js` и не поставляет исполняемые
   scripts; исключение действует и при прямом копировании legacy `Themes/`.
   Нативное применение отказывается создавать отсутствующую конфигурационную
   baseline, в том числе без stage 2. Snapshot selector ограничен одним именем
   каталога, чтобы не обращаться вне Themes/. Palette записи атомарные, с сохранением прав и проверкой путей.

Spicetify остаётся P4: rollback локальных файлов проверен fake native runner,
но частичные изменения установленного Spotify, которые делает внешняя утилита,
не являются доказанно восстанавливаемым runtime. Это ограничение нельзя скрывать
за успешным файловым тестом. Изоляция/native Spotify требует отдельного решения.

## Воспроизводимость

- `cargo test`: весь текущий набор, включая 7 closeout regressions.
- `tests/e2e/existing-tui-native.py`: отдельные HOME/Git/PTY по каждой из 8 тем,
  настоящий RGB до/после возврата btop/Yazi/lazygit, Zellij parser.
- `third-apps-native.py`, `third-apps-clients.py`, `third-apps-gtk.py`,
  `third-apps-heroic.py`, `editor-cache-native.py`: настоящие доступные клиенты.
- `browser-native-accent.py`: Firefox и Zen, отдельные headless profiles.
- `launchers-native.py`, `four-apps-native.py`: изолированные Docker-клиенты.
- Clean-machine Plasma: повторный clean-machine прогон исправленного среза —
  6 theme cycles и GUI return ALL PASSED. Дополнительно native guard Spicetify
  без stage 2 проверен отдельной регрессией.

Нативные проверки не используют личные browser/vault/game/account profiles.
Данные активного рабочего стола не применяются ради QA. Новые приложения,
зависимости AUR и дополнительные клиенты не добавлялись на пользовательскую систему.
Временные журналы: ignored `tmp/validation/adapter-closeout/`.

## Финальный результат

306 Rust (включая 7 closeout tests) + 3 generator tests, 345 Qt, 260 shell,
release build, clippy, fmt, shellcheck: PASS. Восемь нативных TUI Apply →
перезапуск → RGB return: PASS. Firefox/Zen: по 8 палитр + baseline/neutral;
Prism/Heroic/GTK и CLI matrix: 8 палитр PASS. Launcher/terminal Docker: PASS.
Повторная clean-machine Plasma: 6 CLI циклов и GUI controller туда/обратно,
файлы/GSettings/живые KWin/Plasma/GUI: ALL PASSED.

Исправления поставлены штатным installer `--only-cli --no-themes`; бинарник
совпадает с release. GUI использует этот единственный CLI core. Восемь локальных
пакетов не переимпортировались: цвета ресурсов не менялись. Активный профиль —
Everforest; `kdeglobals`, `kwinrc` и `active-profile` совпадают с сохранёнными
хешами. Живые `plasmashellrc`/`appletsrc` менялись в сессии раньше этой поставки
(в частности, геометрия панели); они не откатывались по устаревшему хешу.
Фактический Apply на рабочем столе ради QA не запускался.

Задача CLOSED без расширения покрытия. Ограничения P4 описывают пределы
существующего продукта, а не новую очередь этой задачи. Все новые кандидаты
и дополнительная автоматизация потребуют отдельного запроса.

Дополнительно повторён `tmux_native_smoke`: настоящие серверы, восемь тем,
restore/rollback, сохранение late colours/attributes и защита сменившегося
server identity — PASS в изолированном контейнере.
