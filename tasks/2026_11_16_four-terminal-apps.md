# Starship, tmux, Foot, WezTerm

Статус: DONE — 2026-11-17.

Запрос: добавить четыре интеграции и обновить комплектные темы.
Обычный перезапуск допустим, ручной выбор/импорт темы после Apply — нет.
Основной контракт: цвета из package palette, preview и общая транзакция, возврат
исходного состояния при отсутствии настроек приложения в следующей теме,
сохранение личных команд, модулей, Lua, шрифтов и поздних изменений.

## Реализовано

- `four_apps.rs` (новый модуль): native palette adapters для Starship, Foot,
  WezTerm, tmux — цвета из палитры темы, безопасное подключение (host wrapper),
  без импорта чужого Lua или команд.
- `tmux_runtime.rs` (новый модуль): capture/global appearance через `set-option`,
  before/after-журнал, безопасный возврат live options без остановки серверов.
- Регистрация Starship, tmux, Foot, WezTerm в `tracked_apps.rs` (get_tracked_apps).
- Все восемь комплектных тем содержат ресурсы four_apps (starship.toml, foot.ini,
  tmux/kolira.json, wezterm/kolira.toml) и добавлены в app_targets.
- `app_sync.rs`, `palette.rs`, `theme.rs` (TARGETS) обновлены под новые цели.
- `app_sync::plan` вызывает four_apps::plan, third_apps::plan, terminal_apps::plan.

## Проверка

- `tests/four_apps.rs`: 10 интеграционных тестов — возврат к теме без конфига,
  сохранение личных правок (3 цикла), отсутствие ресурсов при возврате,
  неверные данные и перенаправленные конфиги прерываются до записей,
  сырой tmux код в теме игнорируется, порядок поиска WezTerm, переезд XDG,
  личный tmux скрипт не заменяется, минимальные версии парсеров, контраст
  выбранного текста во всех 8 палитрах.
- `tests/audit.rs`: all_bundled_themes_still_validate_and_import — все 8 тем
  импортируются и применяются, включая four_apps ресурсы.
- `tests/preview.rs`: preview совпадает с применёнными файлами.
- `cargo test --all-targets`: все проходят; `cargo clippy --all-targets -- -D
  warnings` чисто; release build чист.

## Границы

- tmux: открытые серверы обновляются без остановки сессий; новые сессии читают
  подключение. Мышь, подчёркивание и локальные overrides окон сохраняются.
- WezTerm: Lua из снимка не импортируется и не исполняется; чужой Lua сохраняется.
- Native минимумы: Starship 1.27, tmux 3.2, Foot 1.28, WezTerm 20220807.
- Flatpak/portable и произвольные CLI config overrides этой партии не заявляются
  автоматически поддерживаемыми.

## Связанные изменения

- docs/APPLICATIONS.md — раздел "Starship, tmux, Foot, WezTerm".
- docs/APP_COVERAGE_PRIORITIES.md — Goals обновлены.
- tasks/2026_11_16_four-terminal-apps.md — эта карточка.
