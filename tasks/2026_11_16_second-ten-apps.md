# Вторая партия приложений: P0 № 11–20

Статус: DONE — 2026-11-16.

## Объём

fish / Oh My Posh / Pure; btop / Cava; Yazi / bat; Wofi; fzf; lazygit;
Zellij; OpenCode; Claude Code; Codex. Это десять строк очереди, не десять бинарников.
Сохраняются единый Rust Plan/preview/MutationGuard/journal и возврат к baseline,
в том числе когда следующая тема не содержит конфига приложения. Late edits и
посторонние настройки не заменяются целиком.

## Контракты

- fish/Pure: только universal colour variables, без включения prompt/functions.
  OMP: только уже существующее подключение, структура prompt сохраняется.
- btop/Cava: palette resource/gradient, без изменения polling/audio/process settings.
- Yazi: native theme styles, не yazi.toml/keymap/plugins; bat — tmTheme + выбор.
- Wofi: отдельный colour CSS/import, без запуска/перезапуска приложения.
- fzf: только --color data + фиксированная fish glue, без execute/bind hooks.
- lazygit: только gui.theme в YAML; остальной документ сохраняется.
- Zellij: отдельная KDL theme и корневые theme selectors; не keybinds/plugins.
- OpenCode V2 2.2.7: cli.json theme.name=system + mode. opencode.json не меняется.
- Claude Code 2.1.322: native ANSI day/night, с сохранением daltonized выбора;
  только settings.json theme. Никаких credentials/permissions/hooks.
- Codex CLI 0.167.0: CODEX_HOME/themes/kolira.tmTheme + [tui].theme;
  только syntax highlighting, не обещание палитры всего TUI. Не auth/history/models.

## Источники контрактов

- Установленные man/docs: fish/Pure, fzf 0.74.5, lazygit 0.66.3, bat 0.26.1.
- https://yazi-rs.github.io/docs/configuration/theme/ (26.9.1)
- https://zellij.dev/documentation/themes.html (0.46.0)
- https://opencode.ai/docs/themes/ (V2 cli.json, не устаревший opencode.json)
- https://code.claude.com/docs/en/settings-reference#theme
- https://github.com/openai/codex/blob/rust-v0.167.0/codex-rs/tui/src/render/highlight.rs

## Проверка

- `cargo test --all-targets`: 262 PASS, включая 13 интеграционных второй партии;
  `cargo clippy --all-targets -- -D warnings`: PASS; release build PASS.
- Qt/Python: 345 PASS (offscreen; локальный сокет проверен вне sandbox).
- Shell/installer: 260 PASS. Fixture fish исправлен на настоящий формат universal
  variables v3: без заголовка fish игнорирует значения. Ожидание OMP проверяет
  локальный разрешённый XDG-путь, а не устаревший literal `$HOME/.config`.
- shellcheck, qmllint, diff whitespace: PASS; 910 переводов, все завершены.
- Настоящие fish/fzf, bat cache/list-themes и `zellij setup --check` PASS в
  изолированных HOME. Восемь Wofi CSS проходят настоящий GTK3 CssProvider без ошибок.
- Все восемь repo/staging пакетов обновлены. Повторное обновление всех repo
  пакетов побайтно идентично: манифесты стабилизированы, OMP повторно не перекрашивается.
- Тест Wofi late geometry сохраняет личную правку и убирает управляемый import;
  ресурс сохраняется при остающейся личной ссылке. Бинарный bat cache возвращается
  побайтно; timeout rebuild откатывает операцию.
- Импорт допускает лишь appearance-only Claude/Codex hosts; другие поля,
  auth/instructions/hooks отклоняются, сохраняется прежняя защита agent metadata.
- Docker: восемь тем, возврат файлов/GSettings, живые KWin/Plasma/GUI и возврат
  через app controller — ALL PASSED. Лог `/tmp/kolira-second-ten-docker.log`;
  сохранённые промежуточные артефакты `/tmp/kolira-second-ten-e2e-out`.
  Snapshot контейнера предшествовал последней правке Wofi late-CSS inverse и
  стабилизации maintenance-примера; финальные правки покрыты полными Rust/shell
  и отдельной регрессией Wofi, а не объявляются отдельным новым Docker-прогоном.
- CLI/GUI установлены через штатный installer, GUI перезапущен. Восемь локальных
  тем обновлены через force import с pre-import backup; KDE/GTK/обои сохранены.
  Deploy backup: `~/.config/kolira/backups/deploy-kolira-20261116-071821`.
  Результаты/пути backup: `/tmp/kolira-second-ten-import-results.json`.
- Preview всех восьми установленным CLI на реальных конфигах PASS:
  `/tmp/kolira-second-ten-live-preview.json`. Active profile не менялся.


## Границы

Контракты и native-path ограничения перечислены в
[APPLICATIONS.md](../docs/APPLICATIONS.md#вторая-партия-p0--1120).
fzf автоматически подключается только в fish; OMP/Pure не включаются сами.
OpenCode проверен для V2 cli.json, Claude — native ANSI/accessibility day/night,
Codex — только syntax highlighting. Авторизованные AI-сеансы и визуальный reload
не запускались. Настоящие btop/lazygit/Yazi/Wofi окна не считаются проверенными.
Рабочий desktop Apply не запускался; обновление пакетов не меняет active profile.



## Установка и живое применение по запросу пользователя, 2026-11-16

После отдельного запроса «установи мне все новое» штатный installer повторно
обновил CLI/GUI и перезапустил GUI. Восемь локальных пакетов уже актуальны;
повторный force import не выполнялся. Обновлённая активная `material-you`
применена с подтверждённым preview token и `keep_wallpaper=true`.
Операция `apply-KnvVpV`: `missing`, `errors`, `skipped`, `dependencies` пусты.
Код/ресурсы и конфиги приложений обновлены; это не установка новых пакетов программ.

- Deploy backup: `~/.config/kolira/backups/deploy-kolira-20261116-073854`.
- Pre-apply backup:
  `~/.config/kolira/backups/pre-apply-material-you-1256502-1794778742238683199-0`.
- Report: `/tmp/kolira-activate-profile-apply.json`.
- Active profile остался `material-you`, текущие обои сохранены.
- Zen не завершался автоматически: отчёт рекомендует штатный перезапуск.
  
  Успех файлового Apply не объявляется визуальной проверкой каждого приложения.


Последующее исправление 2026-11-16: [согласованность палитры и native browser accent](2026_11_16_palette-consistency.md).
Исторические проверки выше не гарантировали совпадение GTK/browser accent с
палитрой. Текущий контракт — native accent/day-night без chrome CSS.
