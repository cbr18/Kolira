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
printf '%s' '{}' | kolira core quick_switch.get
printf '%s' '{}' | kolira core quick_switch.cards
printf '%s' '{"name":"glass"}' | kolira core quick_switch.prepare
printf '%s' '{"hotkey":"Meta+K"}' | kolira core quick_switch.set_hotkey
printf '%s' '{"id":"window_bg"}' | kolira core field.read
```

## Семейства методов

| Семейство | Назначение |
|---|---|
| `operation.list/recover` | Результаты и восстановление прерванных профильных Apply |
| `restore_point.*` | Каталог именованных ручных и автоматических точек оформления |
| `profile.list/info/active/is_package/selection/apply/clear_stale` | Профили, метаданные, применение и безопасная очистка устаревшего маркера |
| `profile.migrate/prepare_apply` | Конвертация старого снимка с копией оригинала и подготовка GUI-сводки |
| `profile.live_matches` | `names`: список тем. Возвращает текущий вид KDE, `profiles` со статусами `match`, `different`, `unknown` и массив `matches`. Сравнивает заданные в профиле параметры с читаемым Live. Маркер последнего применения учитывается отдельно. |
| `settings.defaults/get/set` | Предпочтения, проверка типов, сохранение неизвестных ключей и политик automation |
| `quick_switch.get/set/cards/prepare/thumbnail/set_hotkey` | Настройки и быстрый путь для полноэкранного оверлея: `quick_switch` живёт отдельным объектом в `app-preferences.json` (как `automation`), `cards` читает только `theme.json` (палитра, `previewColors`, `previewStyle`, обои), `thumbnail` кэширует уменьшенную копию обоев в `$XDG_CACHE_HOME/kolira/quick-switch/thumbs`, `prepare` возвращает план и `preview_token` без построчного diff (в 5–6 раз быстрее `profile.preview`), `set_hotkey` пишет сочетание в `kglobalshortcutsrc` через `kwriteconfig6` с бэкапом и уводит `voxtype-toggle` с `Meta+K` на `Meta+Shift+K` |
| `registry.*`, `paths.migrate` | Встроенные/пользовательские компоненты, обнаружение приложений, XDG-миграция |
| `field.read/reads/file/kvantum_file/choices/fonts/validate` | Значения, расположения, установленные варианты и диагностика |
| `config.*` | Парсинг, список параметров и подготовка точечного изменения |
| `inspector.state/stage/config/component/apply/undo/revert/reset/discard/create_profile/close` | Сессия редактирования и составные операции |
| `inspector.refresh/resolve` | Сверка черновика с Live и разрешение отдельных конфликтов |
| `app.*` | Метаданные, доступность, текущая/установленные темы, применение и запуск приложений |
| `resource.clone`, `bundle.components/metadata/validate` | Производные KDE/Kvantum-ресурсы и метаданные темы |
| `bundle.resource_previews` | `id`, `options`, `profile?`: локальные изображения доступных ресурсов по идентификаторам, с ревизиями файлов для кэша. Без записи и установки. |
| `palette.save_theme` | `name`, `description?`, `palette`: сохранить новый снимок после сверки палитры и живых KDE-цветов под общей блокировкой; ответ `{name}`, отказ без записи при несовпадении/существующем имени |
| `wallpaper.directory/list/current/set` | Каталог, общий порядок и прямое применение обоев |
| `desktop.load_blur/recover`, `report.export` | Ограниченные внешние вызовы и атомарная запись отчёта Inspector |

Точные аргументы находятся в `gui_api.rs` и вызываемых модулях; это локальный
интерфейс приложения, не сетевой сервис и не отдельная публичная версия MCP.
Пользовательский MCP-контракт описан в [MCP.md](MCP.md).

## Сессия Inspector

Ответ сессии содержит `draft` со списком подготовленных изменений.
`inspector.discard` очищает черновик, сохраняя уже применённые изменения и
историю Undo. С параметром `section` сбрасывает только черновые поля этой
категории из реестра Inspector. Возврат типизированного поля к исходному значению снимает
его изменение.

`inspector.app` с `stage: true` принимает `id` адаптера и `value` темы. В
черновике появляются целевой файл и исходный выбор. Ответы содержат выбранную
тему в `apps`, число черновых тем в `appDraftCount` и строку в `draft`. Сброс категории `apps` включает
черновые темы приложений. Старые вызовы `inspector.app` без флага и `app.apply`
сохраняют прямое применение.

`inspector.wallpaper` с `stage: true` принимает в `value` абсолютный путь к
локальному изображению. В ответе `wallpaperDraft` содержит путь или `null`.
Черновая правка также появляется в `draft` и входит в категорию `wallpaper` для
`inspector.discard`. Конфликт обоев содержит снимок `wallpapers` по идентификаторам
рабочих столов Plasma и краткое представление `display` в `before` и `current`.
Вызов без `stage` сохраняет прежнее прямое применение.

`inspector.refresh` сравнивает исходное значение, актуальное Live и
черновик. Совпавшая с Live правка снимается. `data.conflicts` содержит
`kind`, `id`, `label`, `source`, `before`, `current`, `after`,
`can_keep` и `token`. `inspector.resolve` принимает `token` и
`choice` со значением `live` или `draft`. Если значение изменилось
после просмотра, ядро возвращает обновлённый список. Снимок Live при
этих вызовах читается без записи в конфиги.

GUI передаёт `check_live: true` в `inspector.apply`. Конфликт
останавливает Apply до записи и возвращает `ok: false` внутри ответа
сессии вместе с `data.conflicts`. Ошибка чтения сохраняет черновик.
При смене типа в конфиге приложения `can_keep: false` позволяет сначала
принять Live, затем заново ввести значение соответствующего типа.

После проверки `inspector.apply` с `check_live: true` сохраняет страховочную копию
затронутых файлов и применяет черновик одной транзакцией. При ошибке ядро
восстанавливает файлы, права и состояние черновика. В `operation.list`
транзакция помечена `report.kind: "live"`. Она не меняет `last_profile_apply`.
`source: "gui"` помечает вызов из приложения, по умолчанию источник `cli`.
Вызов без проверки сохраняет прежнее поведение с повтором неудачных записей,
пока в сессии отсутствуют черновые темы приложений и обои. Черновик обоев
включает в страховочную копию конфигурацию Plasma и `kdeglobals`, где может
обновиться акцент. Применение ждёт сохранения выбранного изображения Plasma.

Первый `inspector.state` без `session` создаёт идентификатор. Последующие
запросы передают его. Rust хранит staging, порядок, Undo, оригиналы файлов
и монотонную `revision` в
`$XDG_CACHE_HOME/kolira/inspector-sessions/inspector-<random>/`.
Каталог имеет права 0700, состояние — 0600; запросы одной сессии сериализуются
файловой блокировкой. Python хранит лишь копии для отображения и игнорирует
ответы с меньшей revision.

Curated-поля и generic-конфиги накапливаются до Apply. Составное создание темы
проверяет все входы до записи и публикует профиль после успешного Apply.
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

## Черновики сохранённых тем

`bundle.editor` принимает `name` и возвращает `revision`, `values` по полям
редактора, карту `bundled` с названиями ресурсов из пакета и `choices` для
параметров с составным значением и выбора тем приложений.
Значение `null` означает отсутствие поля. `bundle.save_edits` принимает `name`,
`revision` и карту `changes`. Доступны `icon_theme`, `cursor_theme`, `ui_font`,
`plasma_style`, `widget_style`, `gtk_theme`, `gtk_dark`, `animations`,
`corner_radius`, `blur_strength`, `translucency`, `decoration_theme`,
`color_scheme`, `accent_color`, `accent_from_wallpaper`, `kitty_opacity`,
`kitty_blur`, `alacritty_opacity`, `alacritty_blur`, `vscode_theme`,
`cursor_ide_theme`, `btop_theme`, `btop_rounded`, `zed_mode`, `zed_light` и
`zed_dark`, `zen_accent_color`, `zen_mode`, `obsidian_mode` и `obsidian_theme`.
Для `ui_font` значение — семейство шрифта. Для `decoration_theme` передайте
объект выбранного варианта из `choices.decoration_theme`, например
`{"library":"org.kde.breeze","theme":"Breeze"}` или `{"disabled":true}`.
`null` освобождает выбор декорации, включая плагин, тему и признак отключения.
Размер рамки, расположение кнопок и другие поля `kwinrc` независимы.
`color_scheme` принимает идентификатор установленной или встроенной схемы KDE.
При выборе Rust сохраняет цветовые группы в `kdeglobals` и обновляет
значения ролей, которые использует превью. `null` исключает идентификатор,
цветовые группы и контраст схемы. `accent_color` принимает `#rrggbb`,
а `accent_from_wallpaper` — логическое значение. У каждого поля свой `null`.
Палитра Base16 и цели генерации цветов приложений хранятся отдельно.
`values.base16_palette` содержит объект `Palette` или `null`, а `palettePresets`
содержит встроенные шаблоны в том же формате. `changes.base16_palette`
принимает полную палитру с 16 цветами, вариантом и акцентом. Rust обновляет
ресурсы, которые распознаёт как результат генерации из прежней палитры.
При `null` профиль исключает палитру и отключает `app_targets`. Его сохранённые
снимки компонентов остаются доступны для применения через профиль.
`appTargets` содержит поддерживаемые приложения с полями `id` и `name`.
`values.app_targets` содержит выбранные идентификаторы. Передайте полный
список в `changes.app_targets` или `[]`, чтобы отключить генерацию. Для
непустого списка требуется Base16 в итоговом состоянии пакета. Идентификаторы
проверяются по реестру Rust. Палитру и цели можно сохранить одним вызовом.
Четыре поля терминалов обновляют соответствующие параметры снимка. При `null`
ключ исключается и Apply сохраняет исходное значение этого параметра.
Для VS Code, Cursor и btop передайте строковый идентификатор темы из `choices`.
`btop_rounded` принимает логическое значение. Текущий недоступный вариант может
присутствовать в `choices` с `available: false`. Сохранение правит отдельный
ключ JSONC или btop.conf. Apply накладывает заданные параметры на исходную
конфигурацию. Если явно задана тема btop, при включённой цели Base16 она имеет
приоритет над автоматически сгенерированной.
`zed_mode` принимает `system`, `light` или `dark`. Для `zed_light` и `zed_dark`
передайте имя темы. Частичный объект `theme` накладывает только включённые
ключи. Строковое значение `theme` в старом снимке отображается в обоих полях
выбора и остаётся строкой до первой правки параметров Zed. При такой правке
имя старой темы сохраняется для каждой стороны, которую пользователь оставил.
`zen_accent_color` принимает `#rrggbb`, а `zen_mode` — `light` или `dark`.
Значения берутся из снимка браузера либо из Base16, если пакет генерирует
соответствующее поле. Явный выбор или `null` через редактор сохраняется
в `zen_overrides` как указание пропустить генерацию этого поля. Прежние пакеты
без этой отметки используют исходные правила генерации. Настройки пишутся в
снимок `user.js` и при применении переносятся в профиль Zen получателя.
`obsidian_mode` принимает `light` или `dark`, а `obsidian_theme` — идентификатор
темы или пустую строку для встроенной темы. `null` исключает соответствующее
поле из применения. Когда значения в сохранённых слотах различаются,
`bundle.editor` возвращает `{"mixed":true}`. `bundle.save_edits` принимает
выбор или исключение. Явные правки записываются в `obsidian_overrides` пакета.
При применении они действуют на локально выбранные хранилища. Названия и
пути этих хранилищ в пакет не записываются.
Для остальных полей `null` снимает управление соответствующим параметром.
Отсутствие ключа в `changes` сохраняет его.

Сохранение проверяет ревизию и атомарно заменяет пакет. Применение, запись в
Live и изменение маркера активной темы остаются отдельными операциями.
Вызов возвращает актуальное состояние редактора в формате `bundle.editor`. При ошибке GUI хранит
черновик до следующей попытки или явного сброса. Если смена ресурса убирает
встроенные значки, стиль Plasma или тему Aurorae из активного состава, их описание и файлы
остаются в локальном пакете и доступны для повторного выбора. Неактивные
ресурсы пропускаются при применении и экспорте. Новый выбор сверяется с
локальными сведениями о зависимостях.

## Profile preview

`profile.preview` returns a read-only file plan and token.
`profile.prepare_apply` accepts `name`, `keep_wallpaper` and `without_sync`.
It converts a legacy snapshot before making the same report and rejects a
component selection. The report includes `migration_backup` only when that
request published a package. `profile.migrate` accepts `name` and returns
`{name, migrated, backup}`, with a null backup when the package was already
current. Conversion leaves Live, the active marker and apply history intact.
The GUI startup Apply also prepares an old snapshot when applying the whole
theme without a preview token. Direct CLI and MCP preview contracts stay read-only.

`profile.save_before_apply` saves current appearance under a new, unused name.
`profile.capture_live` creates a new package under an unused `name`.
It accepts `description`, `include_wallpaper` (default true) and
`make_look_and_feel` (default false). An optional `draft_session` overlays
the Inspector session's staged fields, application themes and supported config
edits on the private capture. With `include_wallpaper: true`, the staged image
is packaged when present. Omitting `draft_session` captures the current image.
Edits outside the theme's appearance policy fail before
publication. Capture leaves the Inspector draft in place.
The method publishes the package after capture
and leaves Live, the active marker and the apply journal untouched. The GUI
uses this method for both gallery capture and the save action in Apply review.
`profile.preview.without_sync` (default false) excludes generated app follow-ups
and cached editor installation, but keeps saved app settings in the snapshot.
It is returned in the report and participates in the token; pass the same value
to `profile.apply`. Both stages share the recovery transaction.
`profile.apply.preview_token` binds confirmation to the reviewed state and
forbids implicit dependency installation. Stale state returns `PREVIEW_STALE`.
[Parameters, response fields and limits](APPLY_PREVIEW.md).

## История, расписание и агентский сценарий

Все методы используют прежний envelope `{"api":1,"ok":true,"data":...}`;
ошибка: `{"api":1,"ok":false,"error":"..."}` и ненулевой exit code.

| Метод | Вход | Результат / эффект |
|---|---|---|
| `operation.list` | `{}` или `limit: 1..100`, `offset?` | `busy`, `operations`, `last_profile_apply`, `next_offset`. Старый вызов возвращает 10 завершённых и все незавершённые записи с проверкой копий. При пагинации незавершённые входят в первую страницу, а копия проверяется полностью перед восстановлением. |
| `operation.restore` | `id`, `confirmed: true`, `source?` | новая страховочная транзакция; `operation_id`, `restored_operation`, `backup` |
| `operation.recover` | `id`, `force?` | восстановление прерванной операции; проверять `recovery_errors` |
| `restore_point.list` | `{}` | `points`, `max_count`, `max_bytes`. Метаданные точек содержат `id`, `name`, `kind`, `created`, `pinned`, `bytes`. |
| `restore_point.create` | `name` | Снять текущее поддерживаемое оформление в ручную точку, вернуть `id`. |
| `restore_point.rename` | `id`, `name` | Сохранить имя точки, вернуть `id`. |
| `restore_point.pin` | `id`, `pinned` | Изменить закрепление и применить лимиты каталога, вернуть `id`. |
| `restore_point.delete` | `id`, `confirmed: true` | Удалить только указанную точку каталога. |
| `restore_point.restore` | `id`, `confirmed: true`, `source?` | Проверить данные и восстановить содержимое точки в страховочной транзакции. |
| `schedule.list` | `{}` | `enabled`, `running`, `rules` с `next` и `last` |
| `schedule.put` | `rule: {id,theme,time,days,enabled,keep_wallpaper?}` | проверка/атомарная запись правила, возвращает актуальное расписание |
| `schedule.delete` | `id` | удаление правила; отметка прошлого события остаётся против повторов |
| `schedule.enable` | `enabled` | управление исполнителем и XDG autostart |
| `agent.capabilities` | `{}` | версия, поддерживаемые методы, генераторы и ограничения |
| `agent.schema` | `{}` | схема темы, поля и их типы, соответствие компонентам |
| `agent.prepare` | `name`, `values`, `description?`, `use_palette?` | новый пакет без живого применения; `package`, `path`, `missing`, `warnings` (как `kolira lint --json`), `live_appearance_changed: false` |
| `agent.install` | `{}` | управляемые `README.md`/`AGENTS.md`/`CLAUDE.md` (версия 2); версия и результат по файлам |
| `dependency.list` | `{}` | обновлённый реестр со стабильными ID; не устанавливает ресурсы |
| `dependency.start` | `action`, `ids`, `confirmed?` | асинхронный существующий Rust job; возвращает его ID и состояние |

`profile.apply` принимает `source` из `cli/gui/mcp/schedule` (по умолчанию `cli`).
При `source=mcp` непустой `preview_token` обязателен: отсутствие отклоняется
до начала транзакции; устаревший токен требует нового `profile.preview`.
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

Additional gallery fields include named `previewColors`, `cursorTheme`,
`widgetStyle`, `unavailableResources` for packages, `wallpaperName` and
`base16Name` when the package has a palette.
`wallpaperPreview` is `null` or the declared package image's canonical local
path and revision. `profile.live_summary` returns the desktop identifiers
Kolira can read from Live: `color_scheme`, `icon_theme`, `plasma_theme`,
and `cursor_theme`. Neither call applies a theme or copies its wallpaper out
of the package.

Application errors add `message_template` and `message_args` while retaining
English `message` and machine `status`. `app.all` exposes `statusTemplate` and
`statusArgs`. API version 1 stays compatible. GUI translation precedes one-pass
substitution: paths and user values are not interpreted as further placeholders.
Unknown diagnostics remain available through their original message.

## Выбор Obsidian vault

`app.vault_selection` принимает `{}`; `app.select_vaults` принимает
`{"vaults":["Documents/MyVault"]}` — пути относительно HOME в порядке
переносимых слотов. Оба возвращают `{vaults: string[], selected: string[]}`.
Выбор проверяется по локальному index и записывается атомарно под MutationGuard;
абсолютные пути, traversal и симлинки не допускаются. При одном доступном vault
отсутствующий выбор означает его автоматический выбор, при нескольких — ни одного.
Явный пустой список отключает выбор. Подробности: [APPLICATIONS.md](APPLICATIONS.md).

### Application palette follow-ups (third batch)

`app_targets` also accepts micro, lsd, glow, gimp, heroic,
prismlauncher, vim, mpv, cliamp and vscodium. Registry count: 51.
These use the existing profile preview/apply operation and before/after recovery;
`palette apply` retains its own target list. LibreOffice is excluded, not a target.
[Native paths and limits](APPLICATIONS.md).
