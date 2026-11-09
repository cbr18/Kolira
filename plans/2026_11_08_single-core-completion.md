# Завершение переноса бизнес-логики в Rust

**Статус: DONE, 2026-11-08.** Полный перенос оставшейся логики выполнен
в исходниках. GUI остаётся Python/PySide6 + QML.
Установленная копия обновлена и Inspector проверен в Plasma/Wayland:
[отчёт](../docs/audits/2026_11_08_live-migration-check.md).

Связь: [MCP, агентский сценарий и полнота ядра](../tasks/2026_11_08_mcp-agent-workflow-core-audit.md),
A01 [аудита](../docs/audits/2026_11_08_codebase-audit.md).
Контракт и ограничения: [CORE_API.md](../docs/CORE_API.md).

## Реализованные этапы

| Этап | Исполнитель Rust | Сохранённый контракт |
|---|---|---|
| Inspector | `config_edit.rs`, `inspector_session.rs`, `inspector_view.rs`, общий `field.rs` | INI, JSONC, TOML, user.js; staging/Apply/Undo/Revert, выбор, валидация; read-only снимки |
| Темы приложений | `app_themes.rs`, `data/app_adapters.json` | 9 адаптеров; JSONC и mode-specific Zed, VS Code/Cursor, Obsidian, Kitty, btop, ограниченные Discord/Telegram; Flatpak/XDG |
| Ресурсы | `resource_clone.rs`, `bundle_view.rs` | Копия установленной основы с правками, проверка конфликта до записи, SVG/PNG, лицензия/provenance; метаданные и legacy bundles |
| Настройки/реестры | `gui_api.rs`, общие `tracked_apps.rs`/`custom_components.rs`/`paths.rs` | Defaults, типы, неизвестные ключи, automation, миграция, встроенные и свои компоненты |
| Обои | общий `wallpaper.rs`, Rust wallpaper binaries | Расширения, symlink, порядок, текущие обои Plasma, preference каталога, прямое применение и NO_APPLY |
| Удаление дублей | Python view adapters → один `cli_core.py` | Удалены Python writers, клоны, парсеры и локальная миграция, `fileutil.py`; скрытого fallback нет |

Добавлен локальный `theme-profile core METHOD` с JSON API v1. Старые команды
CLI сохраняются. Python выполняет Qt-сериализацию, перевод, представление и
GUI-навигацию. Экспорт отчёта Inspector тоже записывает Rust.

Состояние сессии хранит Rust в приватном XDG-кэше; файлы бэкапов сохраняют
исходные права. Общая блокировка preferences предотвращает потерю automation
при сохранении GUI-настроек; отдельная блокировка сессии и revision предотвращают
потерю staging и замену модели старым ответом. Это не глобальная транзакция Apply.

## Проверки и паритет

- Rust: **156 тестов** (125 unit + 31 integration), в том числе 16 новых
  межпроцессных сценариев `tests/gui_core.rs`: частичный Apply, ошибка после
  успешного выбора приложения, отказ сохранять профиль, retry, порядок Undo,
  параллельные staging/settings, типы, права, XDG, Kvantum, закрытие и NO_APPLY.
- Python/Qt: **312 тестов**, включая 6 новых контрактных сценариев: Qt JSON,
  версия API, устаревшие revision, согласованность компонентных id/метаданных,
  отсутствие Python-мутаций файлов. Существующие fixtures проверяют выходные
  конфиги/снимки, адаптеры, ресурсы, MCP и offscreen QML. Моки удалённых внутренних
  Python writers заменены реальными отказами файловой системы или транспортом.
- CLI/установщик: **256 assertions**; release build, Clippy `-D warnings`,
  shellcheck, qmllint и сборка переводов успешны. Каталог переводов:
  **731 законченный перевод**, незавершённых нет.
- Все шесть комплектных тем импортированы и применены через новый `core profile.apply`
  в изолированном HOME с нестандартными XDG-путями: active-profile совпадает,
  пользовательский fastfetch сохранён.
- Проверка не меняет пользовательские конфиги: временные HOME/XDG,
  KTM_OFFLINE=1 и KTM_THEME_NO_APPLY=1; тесты запуска используют безопасные заглушки.

Сохранена обычная семантика GUI/CLI. Дополнительно исправлены выявленные при
переносе потери комментариев/сложных значений TOML, неверный целевой Kvantum-файл
в батче, маскировка ошибки поля успешной темой приложения и Undo-токены с newline.

Первоначальный прогон был изолированным; затем установленная сборка проверена
в живой Plasma/Wayland: staging → Apply → Undo → Apply → Revert, исходный
`kdeglobals` восстановлен байт в байт. Полная смена темы/перезапуск plasmashell,
сетевые установки, AUR и удалённый CI в этом продолжении не проверялись. Доставка AGENTS.md в пользовательский каталог тем, полная
матрица возможностей и разделение prepare/apply в MCP остаются в исходной
задаче. Защита применения, журнал 10 смен тем и расписание имеют отдельные карточки.
