# Матрица Rust core / CLI / GUI / MCP

Актуальная архитектура: Rust исполняет операции, Python/PySide6 и stdio MCP —
адаптеры единого API v1. Это матрица текущих сценариев с границами доказательств,
а не утверждение о проверке всех приложений на настоящем desktop.

| Сценарий | Rust / CLI | GUI / MCP | Проверка и ограничение |
|---|---|---|---|
| Чтение тем, компонентов, схемы | `bundle_view`, `inspector_view`, `field`; `list`, `core agent.schema` | Inspector; `list_profiles`, `list_components`, `get_theme_schema` | `gui_core.rs`, `test_theme_bundle.py`, `test_mcp_workflow.py`; чтение реальных тестовых ресурсов |
| Создание без apply | `agent.prepare`; снимок + `theme::build`, атомарная публикация | MCP `prepare_theme`; отдельного GUI-мастера пока нет | Новый тест создаёт пакет, проверяет поля, отсутствие live-записей и экспорт |
| Создание с живыми изменениями | `inspector_session::create_profile`, `constructor::new_theme`; `new`, `save` | Конструктор; старый `create_profile` | `test_theme_bundle.py`; эта семантика сохранена и обозначена явно |
| Копирование, переименование, сохранение | `profile`, `theme`; `clone`, `rename`, `save` | GUI; MCP `create_profile`/импорт | Rust/Python/CLI suite; MCP не имеет отдельных clone/rename tools, доступен CLI |
| Редактирование полей и приложений | `field`, `app_themes`, `inspector_session`; `field`, `apps`, JSON API | Live Inspector, staging/apply; `set_component`, `apply_changes` | `gui_core.rs`, `audit.rs`, `test_theme_inspector.py`; снимки read-only |
| Палитра, генерация | `palette`, `gallery`, `extract`, `oklab`; `palette ...` | GUI палитры; MCP `palette_*` | Rust unit + Python; offline authoring генерирует KDE/Kitty/Alacritty; полный связный GUI-сценарий — отдельная задача |
| Установленные компоненты | `inspector_view::choices`, registry | Inspector; `list_installed_themes` | Изолированные реальные файлы; внешние онлайн-каталоги не подменяют список установленных |
| Ресурсы/лицензии | `theme`, `resource_clone`, `share`; `license`, `export --public` | Конструктор; `create_custom_theme`, экспорт | `audit.rs`, shell suite; клонирование текущего ресурса может менять live-селектор и не является offline API |
| Зависимости | `deps`, `dependency_registry`, `dependency_jobs`; `deps`, `dependencies` | Страница зависимостей; registry/start/status/cancel tools | Существующие циклы KDE Store/Zed/pacman подтверждены прежней задачей; AUR открыт отдельно. Здесь проверяется lifecycle transport и реестр |
| Validate/check/diff | `bundle_view`, `theme`, `preview`, `change_plan`; `check`, `preview`, `diff` | Inspector/предварительный diff; `validate_profile`, `preview_profile` | `preview.rs`, `test_apply_preview.py`, MCP stale-token regression; preview не пишет live-файлы |
| Применение | `operation::run`, `theme::apply`, `profile`; `apply` | Подтверждение GUI; `apply_profile` | Rust recovery + MCP round trip; ошибки/частичный отчёт сохраняются по ID, тесты NO_APPLY не доказывают внешние hooks |
| История/бэкапы/восстановление | `operation`; `operations list/show/recover/restore` | Страница истории; list/restore/recover tools | `apply_protection.rs`, `history_schedule_agent.rs`, MCP round trip: целостность, safety copy, лимит без удаления бэкапов |
| Импорт/экспорт | `share`; `import`, `export` | GUI; `import_theme`, `export_theme` | ZIP round trip, обход путей/симлинки/корневые инструкции; права третьих лиц не устанавливаются автоматически |
| Настройки/автоматизация/реестры | `gui_api`, `automation`, `tracked_apps`, custom components | Settings; `get/set_automation`, часть остальных настроек только JSON CLI | `gui_core.rs`, shell suite; MCP не предоставляет произвольную запись настроек |
| Обои | `wallpaper`; `wallpaper-*`/JSON API | GUI и Inspector/MCP staging | Rust/Python/shell suite; одни обои на все мониторы, расширенная библиотека отложена |
| Расписание | `schedule`; `schedule ...` | Страница расписания; CRUD/enable tools | Дни/время, persistent claims, конфликты и повторные события в изоляции; живой desktop фиксируется отдельно |
| Workspace-инструкции | `agent.install` | Установщик CLI; AGENTS.md + CLAUDE.md import | Доставка/обновление/пользовательские правки/cwd проверяются. Загрузка проверена Codex 0.167.0/Claude 2.1.316 из корня и Codex с явным root marker из вложенной темы; модель не использовалась |

## Выявленные пробелы и решения

- Прежде единственный MCP `create_profile` применял live-изменения. Добавлен
  отдельный offline `prepare_theme`; старое поведение оставлено совместимым.
- Установленный сервер раньше предполагал repo layout. Теперь установщик
  доставляет его рядом с `src/`, абсолютный запуск проверяется из вложенного cwd.
- Строковые/булевы входы могли молча преобразовываться. Входные схемы MCP
  проверяются до вызова адаптеров, ошибки содержат код и следующий шаг.
- Долгая установка блокировала MCP request loop. Добавлен запуск существующего
  Rust executor с durable ID, progress/status/cancel; синхронный API сохранён.
- Вторая бизнес-реализация в Python не вводится. Offline authoring, время,
  история, бэкапы, сравнение хэшей и обновление инструкций выполняются Rust.
- Динамические поля внутри Kvantum не поддерживаются offline authoring;
  семейство выбирается из установленного. Полный GUI workflow палитры остаётся
  [отдельной задачей](../../tasks/2026_11_08_palette-workflow.md).
- Реальные AUR cycles — [отдельная задача](../../tasks/2026_11_08_aur-dependency-cycle.md).
  Полная проверка UI после объединения параллельной ветки остаётся отдельной
  задачей. Для Claude проверен только корень каталога; для вложенного Codex
  документирован явный root marker. Проверяется контекст загрузчиков, не
  качество работы произвольной модели.
