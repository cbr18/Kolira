# Minecraft: панели, локальная копия и Zen

Статус: DONE, 2026-11-10.

## Исправления

- Minecraft явно выбирает breeze-dark в .config/plasmarc, theme.json и
  defaults Look and Feel. Обновлены manifest и SHA-256 всех 54 файлов экспорта.
- Применение GTK больше не записывает независимый monospace-font-name.
  Undo игнорирует этот ключ в старых журналах: они могли восстановить Monocraft
  поверх позднее выбранного пользователем Hack. Добавлена интеграционная регрессия
  с настоящим CLI и подставным gsettings, проверяющая остальные операции Undo.
- Локальный пользовательский пакет обновлён штатным import --force; прежняя
  версия полностью сохранена в
  ~/.config/kolira/backups/pre-import-minecraft-overworld-dark-16728-1794263533070808777-0.
  Автоматическая синхронизация по-прежнему сохраняет пользовательские отличия;
  явный импорт и его backup описаны в docs/THEMES.md.
- Исправлен обнаруженный дефект живого ресурса: contents/defaults был пустым
  каталогом вместо файла. Он перемещён в указанный backup как
  malformed-live-defaults-directory. Первая попытка apply-q4kijN откатилась без
  ошибок восстановления. Этот дефект локальных данных не воспроизводился
  в корректном пакете репозитория.

## Проверка

- Полный Rust-прогон: 200 passed; новая отдельная интеграционная регрессия: 1 passed.
  Release build и rustfmt прошли, CLI установлен штатным установщиком.
- Финальный apply-fTdwSS успешно применил Minecraft с --keep-wallpaper;
  errors/missing/skipped пусты. Backup:
  ~/.config/kolira/backups/pre-apply-minecraft-overworld-dark-24385-1794264043761711261-0.
- Живой GSettings: monospace-font-name остался Hack 10 после повторного Apply;
  обычный font-name — Monocraft 10 согласно Minecraft-теме. Plasma Style breeze-dark.
  Обои сохранены: glass-saturn-mono.jpg.
- KWin подтвердил обычное несвёрнутое окно Zen 1934×1314, PID 2910, до и после
  применения. Наличие окна подтверждено; историческая причина жалобы на запуск
  не объявляется установленной. Browser не закрывали и не перезапускали.

Полный визуальный аудит всех приложений не является частью этой задачи.

## Значки «Параметров системы» (2026-11-10)

Жалоба: в System Settings у всех модулей один и тот же значок (верстак).
Причина: KIconLoader, не найдя `preferences-desktop-mouse` и т.п., укорачивает
имя по дефисам **внутри текущей темы** раньше, чем переходит к `Inherits=breeze`,
и находил `preferences-desktop.svg` / `preferences-system.svg` из иконпака.
Исправление: оба файла удалены из темы, `export.json`, установленной копии темы
и живого `~/.local/share/icons/minecraft-overworld-dark`; кэш значков сброшен.
Правило для иконпаков тем: не класть короткие общие имена-префиксы
(`preferences-*`, `folder`, `games` …), если не хотим, чтобы они перекрыли все
более специфичные значки Breeze. Визуально после перезапуска systemsettings —
не проверено.

## Аудит темы и пути создания (2026-11-10)

Найдено и исправлено:
- `folder.svg` перекрывал 192 `folder-*` Breeze, `games.svg` — 24 `games-*`.
  `games.svg` удалён (`applications-games` остался); для сундука сгенерированы
  варианты с пиксельным бейджем: `folder-open`, `folder-documents`,
  `folder-download(s)`, `folder-music`, `folder-pictures/images/image`,
  `folder-videos/video`, `folder-desktop`, `user-desktop`, `user-home`.
  Остальные `folder-*` по-прежнему показывают простой сундук;
  `utilities-system` перекрывает `utilities-system-monitor` (оставлено).
- `[Colors:Selection]`: `Foreground{Inactive,Active,Link,Visited,Negative,
  Neutral,Positive}` имели контраст 1.0–1.6 к зелёному фону; заменены тёмными
  (3.9–8.1). Поправка: у BreezeDark Selection.Inactive = 1.05, и Dolphin рисует
  выделение через ForegroundNormal — утверждение о пропадающих колонках не было
  проверено, видимой поломки, вероятно, не было. Live после Apply: KDE пересчитал фон выделения от акцента
  (89,135,49), Normal/Inactive = 0,0,0.
- `export.json` пересчитан; `import` во временный XDG и `check` прошли.
- Установленная копия: `import --force` (backup pre-import-…-117682-…),
  Apply apply-jAvSwA с `--keep-wallpaper`: errors/missing пусты.
  Лишний `games.svg` удалён из live-иконпака вручную (Apply не удаляет файлы).
- Путь создания: AGENTS.md (v2, `data/theme-agents.md`) и README рабочего
  каталога описывают перекрытие значков и контраст выделения; в `kolira --help`
  добавлены `apply --without-dependencies` и `bundled sync`, на которые
  ссылался README.

Не исправлено / на заметку:
- `.config/plasmarc` темы не добавлен в git (untracked) — нужен в коммите.
- Look and Feel без `contents/previews/` — в «Глобальной теме» нет миниатюры.
- В `~/.local/share/plasma/look-and-feel/minecraft` лежит старый пакет.
- Тесты: Rust зелёный; pytest 342/343 (`test_i18n_pipeline_wired`: удалён
  `scripts/update-translations.sh` в рабочем дереве до этой задачи); bash
  256/258 (`reset-failed plasma-plasmashell` в заглушках kvantummanager /
  hanging tool) — не связаны с изменениями.
- Установщик не запускался: AGENTS.md в каталоге тем обновится при следующем
  `install-kolira.sh`.

## Линтер и kitty (2026-11-10)

- Жалоба: в kitty автоподсказка fish цвета фона. color8 `#404040` на `#252525`
  (1.48:1). Исправлено на `#808080` (3.9:1) в kitty, Alacritty (`[colors.bright]
  black`) и Konsole (`Color0Intense`). Apply apply-UUWLgP: errors/missing пусты,
  live `~/.config/kitty/kolira.conf` color8 #808080. Открытые окна kitty без
  remote control могут потребовать новый экземпляр — визуально не проверено.
- `src/lint.rs`, `kolira lint NAME [--json]`; текстовый `check` дописывает
  warning=…; `agent.prepare` (и MCP `prepare_theme`) — поле `warnings`.
  Пороги выбраны прогоном по всем `/usr/share/color-schemes`: Selection не
  проверяется (стандартный Breeze в нём низкий), Normal ≥ 4.5, Inactive ≥ 2,
  terminal fg ≥ 4.5, bright black ≥ 2.
- Проверка проверок: unit-тесты (4); прогон по всем установленным темам —
  Paper/Glass/Dracula/Frutiger чистые; на старой копии Minecraft пойманы три
  терминала с #404040; Everforest — kitty bright black #475258 на #2d353b
  (1.55, не исправлялось); возврат `preferences-desktop.svg`/`preferences-system.svg`
  во временную копию даёт 51 и 89 перекрытых значков — исходный баг ловится.
- Остаточные предупреждения Minecraft (оставлены сознательно): `folder` → 84
  редких `folder-*`, `folder-image` → folder-image-people, `folder-open` →
  folder-open-recent, `application-x-executable` → …-script, `utilities-system`
  → utilities-system-monitor.

## Развёртывание и bundled sync (2026-11-10)

- `install-kolira.sh` выполнен: CLI с `lint`, GUI, AGENTS/README в каталоге тем.
- Установщик писал «Kept theme minecraft-overworld-dark» хотя копия была из
  комплекта: ручной `import --force` не обновлял запись sync, отпечаток не
  совпадал навсегда. Исправлено в `share::sync_bundled`
  (`installed_matches_kit`: те же файлы с раскрытым `$HOME`, без лишних →
  запись обновляется молча). Тест `bundled_sync_follows_a_theme_that_matches_the_kit`
  падает на старом коде с тем же сообщением и проходит на новом.
- Второй источник расхождения: в `theme.json` комплекта не было
  `"optional": false`, импорт пересохранял файл. Комплект приведён к каноничной форме.
- Повторный install/`bundled sync themes`: сообщений нет. Rust 209 passed, clippy чистый.
