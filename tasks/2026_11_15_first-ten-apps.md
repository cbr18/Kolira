# Первые десять приложений по P0

Статус: DONE — 2026-11-15. Реализовано и проверено в пределах границ ниже.
Родительская [задача](2026_11_15_app-coverage-expansion.md),
[очередь](../docs/APP_COVERAGE_PRIORITIES.md),
[контракт реализации](../docs/APPLICATIONS.md).

## Реализация

- Cursor/Zed: установка и явный выбор кэшированной темы; отдельный owned ID и
  native registry Cursor, стабильный повторный capture/apply. Реестр сохраняет
  поздние установки при возврате, исходные расширения остаются.
- Neovim: только палитра-данные + фиксированные colorscheme и after/plugin loader;
  без редактирования init/specs и исполнения Lua из темы.
- Obsidian: index discovery, CLI/API выбора vault, portable snapshot slots,
  CSS snippet и разрешённые ключи appearance без размеров шрифта/заметок.
- Zen/Firefox: default profile native/XDG/Flatpak, только цвета/day-night;
  контраст foreground/icon, без переноса UI CSS, opacity/density/addon/privacy.
  Не включается отключённый личный CSS. prefs.js не редактируется.
- Pywalfox: явная цель, существующий baseline colors.json и штатный update.
- Discord/Vesktop: Flatpak, личные themeLinks/settings, CSS и возврат.
- Kitty: explicit palette target, общий Plan/include/reload и возврат inline цветов.
- Konsole/Yakuake: общий appearance профиль/схема; ручное обновление открытых
  сессий вместо D-Bus подмены целого личного профиля.
- Схема theme.json, field registry, docs/README/CHANGELOG актуализированы.

## Проверки

- Изначально `tests/first_ten.rs`: 14 интеграционных регрессий; 3 цикла возврата,
  поздние личные правки, cache recapture, native→Flatpak browser profile,
  Obsidian portable slots и ZIP в другом HOME. PASS.
- Настоящий Neovim headless: colorscheme активна, личный init/number сохраняются;
  переданный темой Lua не выполняется. PASS.
- Настоящие Firefox и Zen через Marionette: baseline → light → dark → neutral
  без browser configs; computed foreground/icons контрастны, нейтральная тема
  возвращает исходные computed цвета. PASS. Только временные HOME/профили.
- Native Cursor CLI распознаёт отдельное managed extension. PASS.
- Полный Rust: 243 passed; release build и clippy -D warnings PASS.
- Qt/Python: 345 passed. Shell: 258 passed. shellcheck и qmllint PASS.
- Переводы: 903 finished, 0 unfinished; используется текущий Python runner
  tools/update_translations.py (старый bash runner удалён ранее).
- Docker: installer, шесть тем (frutiger-aero-light, frutiger-aero, Paper,
  dracula, everforest, glass), возврат файлов/GSettings и GUI-контроллер —
  ALL PASSED, реальные KWin/plasmashell. Minecraft не входил в этот прогон.
  Смонтирован отдельный git snapshot текущего рабочего дерева, а не старый HEAD.
  Артефакты: /tmp/kolira-first-ten-e2e-out и /tmp/kolira-first-ten-docker.log.

## Границы

В Docker проверены файлы/процессы/GSettings, не screenshots.
Реальный GUI reload Zed/Obsidian/Discord и Pywalfox addon не проверялся.

Нет GUI picker vault: выбор через core CLI/API. Автоматический reload каждой
открытой Konsole/Yakuake сессии отложен; ручное действие показано явно.
Никаких применений к рабочей теме/браузерным профилям пользователя.
Установленная копия не обновлялась в рамках этой задачи; последующие установка,
обновление пакетов и расширение тестов до 15 — в
[отдельной карточке](2026_11_15_bundled-apps-and-material-you.md). Push здесь не выполнялся.


Последующее исправление 2026-11-16: [согласованность палитры и native browser accent](2026_11_16_palette-consistency.md).
Исторические проверки выше не гарантировали совпадение GTK/browser accent с
палитрой. Текущий контракт — native accent/day-night без chrome CSS.
