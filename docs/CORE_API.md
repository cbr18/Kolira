# Единое ядро GUI и MCP

Core/CLI — Rust; GUI остаётся Python (PySide6 + QML). Python отвечает за Qt,
перевод, представление моделей, диалоги и открытие страниц System Settings.
Бизнес-операции выполняет Rust через единственный транспорт
`gui/qt-app/src/core/cli_core.py`. Старые текстовые команды CLI поддерживаются.
Второго backend, FFI и скрытого Python-fallback нет. CLI и GUI обновляются вместе.

## Протокол API v1

`kolira core METHOD` читает один JSON-объект из stdin (до 32 MiB),
выполняет запрос и возвращает ровно один JSON-объект в stdout:

```json
{"api":1,"ok":true,"data":{}}
```

Ошибка аргумента/исполнения: `{"api":1,"ok":false,"error":"описание"}`,
код выхода 1. Успешная обработка — код 0. Ответы `inspector.*` дополнительно
содержат `data.ok` и `data.message`: ошибка применения возвращает актуальное
состояние сессии с `data.ok:false`, сохраняя staging для повторной попытки.
Это корректный протокольный ответ, его внешний `ok` остаётся `true`.
Адаптер проверяет версию API и сообщает о необходимости совместного обновления.
QUrl/QColor/QJSValue преобразуются в обычные JSON-значения на границе Qt.

```bash
printf '%s' '{}' | kolira core settings.get
printf '%s' '{"settings":{"theme":"paper"}}' | kolira core settings.set
printf '%s' '{"id":"window_bg"}' | kolira core field.read
```

## Семейства методов

| Семейство | Назначение |
|---|---|
| `operation.list/recover` | Результаты и восстановление прерванных профильных Apply |
| `profile.list/info/active/is_package/selection/apply/clear_stale` | Профили, метаданные, применение и безопасная очистка устаревшего маркера |
| `settings.defaults/get/set` | Предпочтения, проверка типов, сохранение неизвестных ключей и политик automation |
| `registry.*`, `paths.migrate` | Встроенные/пользовательские компоненты, обнаружение приложений, XDG-миграция |
| `field.read/reads/file/kvantum_file/choices/fonts/validate` | Значения, расположения, установленные варианты и диагностика |
| `config.*` | Парсинг, список параметров и подготовка точечного изменения |
| `inspector.state/stage/config/component/apply/undo/revert/reset/create_profile/close` | Сессия редактирования и составные операции |
| `app.*` | Метаданные, доступность, текущая/установленные темы, применение и запуск приложений |
| `resource.clone`, `bundle.components/metadata/validate` | Производные KDE/Kvantum-ресурсы и метаданные темы |
| `wallpaper.directory/list/current/set` | Каталог, общий порядок и прямое применение обоев |
| `desktop.load_blur/recover`, `report.export` | Ограниченные внешние вызовы и атомарная запись отчёта Inspector |

Точные аргументы находятся в `gui_api.rs` и вызываемых модулях; это локальный
интерфейс приложения, не сетевой сервис и не отдельная публичная версия MCP.
Пользовательский MCP-контракт описан в [MCP.md](MCP.md).

## Сессия Inspector

Первый `inspector.state` без `session` создаёт идентификатор. Последующие
запросы передают его. Rust хранит staging, порядок, Undo, оригиналы файлов
и монотонную `revision` в
`$XDG_CACHE_HOME/kolira/inspector-sessions/inspector-<random>/`.
Каталог имеет права 0700, состояние — 0600; запросы одной сессии сериализуются
файловой блокировкой. Python хранит лишь копии для отображения и игнорирует
ответы с меньшей revision.

Curated-поля и generic-конфиги накапливаются до Apply; темы приложений и обои
сохраняют прежнюю семантику немедленного выбора. Составное создание темы
предварительно проверяет все входы и не сохраняет профиль после неудачного Apply.
Undo снимает последнее staged-изменение, затем отменяет применённые изменения;
ошибка восстановления сохраняет запись для повтора. Revert восстанавливает
оригинальные файлы сессии. Перед первой записью Rust сохраняет постоянные копии
в `$XDG_CONFIG_HOME/kolira/backups/inspector-<random>/` (0700).
Копии и заменяемые файлы сохраняют исходные права; новые конфиги создаются 0600.
Закрытие сессии удаляет её кэш, сохраняя постоянные бэкапы. GUI при штатном выходе
дожидается фоновых задач до 5 секунд и закрывает сессию, если они завершились;
MCP закрывает её при EOF. После аварии кэш можно удалить при закрытом приложении.

INI, JSONC, TOML, key/value и `user.js` обрабатываются в Rust. JSONC патчится
по позиции значения; комментарии не теряются. TOML сохраняет комментарии,
массивы и вложенные таблицы. Generic-запись ограничена зарегистрированными
конфигами; снимки профилей read-only, `prefs.js` не редактируется.
Атомарная запись выполняется через соседний временный файл и rename.
Настройки GUI и automation используют одну блокировку preferences.

`KTM_THEME_NO_APPLY=1` отключает внешние команды и D-Bus, включая установку
обоев и восстановление plasmashell; конфиги во временном HOME по-прежнему
записываются. Это режим тестирования, а не запрет файловых изменений.

Профильный Apply сериализуется между GUI/CLI и будущим расписанием, сохраняет
ID операции и восстанавливает файлы при ошибке; [контракт и границы](APPLY_PROTECTION.md).
Inspector использует ту же блокировку, сохраняя свою семантику частичного Apply.
История последних 10 смен тем остаётся отдельной задачей. Проверки на
синтетических данных не подтверждают реальные Plasma-хуки и сетевые установки.

## Profile preview

`profile.preview` returns a read-only file plan and token;
`profile.save_before_apply` saves current appearance under a new, unused name.
`profile.apply.preview_token` binds confirmation to the reviewed state and
forbids implicit dependency installation. Stale state returns `PREVIEW_STALE`.
[Parameters, response fields and limits](APPLY_PREVIEW.md).

## История, расписание и агентский сценарий

Все методы используют прежний envelope `{"api":1,"ok":true,"data":...}`;
ошибка: `{"api":1,"ok":false,"error":"..."}` и ненулевой exit code.

| Метод | Вход | Результат / эффект |
|---|---|---|
| `operation.list` | `{}` | `busy`, `operations`: последние 10 завершённых + все pending, отчёты и здоровье копий |
| `operation.restore` | `id`, `confirmed: true`, `source?` | новая страховочная транзакция; `operation_id`, `restored_operation`, `backup` |
| `operation.recover` | `id`, `force?` | восстановление прерванной операции; проверять `recovery_errors` |
| `schedule.list` | `{}` | `enabled`, `running`, `rules` с `next` и `last` |
| `schedule.put` | `rule: {id,theme,time,days,enabled,keep_wallpaper?}` | проверка/атомарная запись правила, возвращает актуальное расписание |
| `schedule.delete` | `id` | удаление правила; отметка прошлого события остаётся против повторов |
| `schedule.enable` | `enabled` | управление исполнителем и XDG autostart |
| `agent.capabilities` | `{}` | версия, поддерживаемые методы, генераторы и ограничения |
| `agent.schema` | `{}` | схема темы, поля и их типы, соответствие компонентам |
| `agent.prepare` | `name`, `values`, `description?`, `use_palette?` | новый пакет без живого применения; `package`, `path`, `missing`, `live_appearance_changed: false` |
| `agent.install` | `{}` | управляемые `AGENTS.md`/`CLAUDE.md`; версия и результат по файлам |
| `dependency.list` | `{}` | обновлённый реестр со стабильными ID; не устанавливает ресурсы |
| `dependency.start` | `action`, `ids`, `confirmed?` | асинхронный существующий Rust job; возвращает его ID и состояние |

`profile.apply` принимает `source` из `cli/gui/mcp/schedule` (по умолчанию `cli`).
`agent.prepare` использует ID полей из `agent.schema`, не GUI-названия приложений.
Поддерживаются поля снимка; динамические поля внутренностей Kvantum отклоняются.
`use_palette: true` использует сохранённую палитру и генераторы KDE/Kitty/Alacritty.
Неизвестные/неподдерживаемые поля и пустой результат дают ошибку до публикации
пакета. Создание не перезаписывает существующее имя, включая вариант регистра.
Ресурсы могут копироваться из установленного оформления только внутрь пакета.

Политики времени и фоновых зависимостей: [расписание](SCHEDULING.md).
Копии/история: [защита применения](APPLY_PROTECTION.md).

Корневые пути инструкций/override/local-вариантов зарезервированы отдельно от
синтаксического regex имён. Импорт отвергает агентские управляющие файлы и
каталоги на любой глубине, экспорт исключает их из ресурсов; это дополнение
к проверке безопасного относительного пути и контрольных сумм.

## Profile fingerprints and display messages

`profile.summaries` accepts `{"names":["Paper","dracula"]}` and returns a map
indexed by profile name. Healthy entries contain `accent`, `swatches`, `scheme`,
`lookAndFeel`, `plasmaTheme`, `kvantum`, `radius`, `blur`, `blurEnabled`, `kitty`,
`iconTheme`, `gtkTheme`, `decoration`, `sizeBytes`, `fileCount`. Missing values are
empty. Snapshot/package values never fall back to live settings. Statistics
count regular files recursively without following symlinks. An invalid name
rejects the request; a missing/damaged profile or symlink root produces a local
`error` entry without losing other profiles. Use an asynchronous client cache.

Application errors add `message_template` and `message_args` while retaining
English `message` and machine `status`. `app.all` exposes `statusTemplate` and
`statusArgs`. API version 1 stays compatible. GUI translation precedes one-pass
substitution: paths and user values are not interpreted as further placeholders.
Unknown diagnostics remain available through their original message.
