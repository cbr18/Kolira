# MCP для kolira

Репозиторий содержит локальный MCP-сервер для нейронных агентов. Он позволяет быстро собирать темы и профили через тот же `ThemeBundleCore`, который использует GUI, вместо ручного редактирования конфигов. Python — протокол/представление;
все операции выполняет единый Rust core через [API v1](CORE_API.md).

Сервер работает через stdio и не требует внешних Python-зависимостей сверх окружения приложения.

Некорректная форма JSON-запроса возвращает JSON-RPC `-32600`, неправильные
`tools/call` params/arguments — `-32602`. Ошибка одного запроса не завершает
сервер; следующий корректный запрос обслуживается. Имя `create_profile`
проверяется до подготовки и живых изменений. Допустимый сценарий создания
сохраняет прежнюю семантику (в том числе применение выбранных значений);
для подготовки без живых изменений используйте отдельный `prepare_theme` (старый
`create_profile` сохраняет совместимую семантику).
Ошибка загрузки списка тем возвращается как ошибка MCP, а не как пустой список.
`apply_profile` требует непустой `previewToken`; это проверяется и транспортом,
и Rust core для `source=mcp`. Отсутствующий или устаревший токен отклоняется
до изменения оформления.
Если Apply полностью или частично не удался, возвращается `ok: false`,
неудачные значения остаются подготовленными, последующее сохранение в
`create_profile` не выполняется. MCP помечает результат с `ok:false`
как `isError:true`. Штатный EOF закрывает кэш сессии Inspector, сохраняя бэкапы.

## Запуск

Из корня репозитория:

```bash
uv run --project gui/qt-app python mcp/kolira_profile_mcp.py
```

Для отключения внешних команд и D-Bus hooks (файловые записи сохраняются;
это не read-only режим):

```bash
KTM_THEME_NO_APPLY=1 uv run --project gui/qt-app python mcp/kolira_profile_mcp.py
```

Пример MCP-конфигурации:

```json
{
  "mcpServers": {
    "kolira": {
      "command": "uv",
      "args": [
        "run",
        "--project",
        "gui/qt-app",
        "python",
        "mcp/kolira_profile_mcp.py"
      ],
      "cwd": "/path/to/kolira"
    }
  }
}
```

## Установленный сервер и произвольный cwd

Полный установщик копирует сервер в
`$XDG_DATA_HOME/kolira/kolira_profile_mcp.py` (по умолчанию
`~/.local/share/kolira/kolira_profile_mcp.py`). Используйте абсолютные
пути к Python окружения GUI и серверу. Например, при установке с venv:

```json
{
  "mcpServers": {
    "kolira": {
      "command": "/home/USER/.local/share/kolira/.venv/bin/python",
      "args": ["/home/USER/.local/share/kolira/kolira_profile_mcp.py"]
    }
  }
}
```

Если установлено системное PySide6 без venv, `command` — абсолютный путь к
системному Python. Поле `cwd` не требуется. Подмена `KTM_THEME_PROFILE_BIN`
допустима для изолированных тестов; штатно сервер находит установленный CLI.

## Offline-сценарий и новые инструменты

1. `get_capabilities` → API/версия, доступные генераторы и ограничения.
2. `get_theme_schema` → JSON Schema пакета, поля с типами и поддержкой offline,
   соответствие компонентам. `list_installed_themes` возвращает реальные варианты.
3. Подготовьте палитру через `palette_import`/`palette_set` (изменяет сохранённое
   состояние палитры, не оформление). `prepare_theme` принимает `name`, `values`
   (ID **полей** → значения), `description?`, `use_palette?`.
4. Ответ: `name`, `package`, `path`, `missing`, `warnings` (`[{kind, path, message}]`, как `kolira lint --json`), `live_appearance_changed: false`.
   Проверяйте `validate_profile`, `list_dependencies` и `export_theme`.
5. Для явного применения: `preview_profile`, затем после просмотра
   `apply_profile` с выданным `previewToken`. Изменение состояния требует нового
   просмотра. Отсутствующие компоненты не означают успешную установку.
6. `list_operations` возвращает `busy`, `operations` с ID, источником, отчётом,
   исходной темой и здоровьем бэкапа. `restore_operation` (`id`, `confirmed:true`)
   создаёт страховочную транзакцию; `recover_operation` (`id`, `force?`)
   восстанавливает прерванную операцию с защитой последующих личных изменений.

Пример подготовки:

```json
{"name":"my-agent-theme","values":{"widget_style":"Breeze"},"use_palette":false}
```

Генераторы offline-подготовки: KDE, Kitty и Alacritty. Внутренние динамические
поля Kvantum пока отклоняются; выбор установленной семьи через `kvantum_theme`
доступен. Старые инструменты `create_profile`, `create_custom_theme`,
`apply_changes`, `palette_apply` могут менять живое оформление. Все live-мутации
нуждаются в соответствующем намерении пользователя.

Управление расписанием: `list_schedule`, `save_schedule_rule` (`rule`),
`delete_schedule_rule` (`id`), `enable_schedule` (`enabled`). Включение разрешает
будущие смены оформления; [контракт времени](SCHEDULING.md).

Для долгих зависимостей: `list_dependency_registry` → стабильные resource IDs,
`start_dependency_operation` (`action`, `ids`, `confirmed?`) → durable `id`,
`dependency_status` (`id`) → `status`, `results`, последний `progress`;
`cancel_dependency_operation` (`id`) запрашивает безопасную отмену. Старый
`install_dependencies` остаётся синхронным. Система не убивает привилегированную
транзакцию посередине записи пакетов.

Схемы аргументов проверяются до преобразований типов; например строка `"false"`
не принимается вместо boolean. Ошибки tools/call имеют `isError:true` и JSON
`{ok:false,error:{code,message,next_action}}`. Ошибки формы JSON-RPC остаются
протокольными. Успешный payload остаётся JSON в текстовом content; ошибки ядра
не попадают в протокольный stdout вне JSON-RPC ответа.

## Инструкции каталога тем

`core agent.install` вызывается установщиком CLI и доставляет канонические
`README.md` и `AGENTS.md` версии 2 и совместимый `CLAUDE.md` с `@AGENTS.md` в XDG-каталог `themes/`.
Источники — `rust_core/data/theme-agents.md` и
`rust_core/data/theme-workspace-readme.md`, встроенные в Rust CLI. README содержит
рабочие CLI/MCP-рецепты offline-подготовки, клонирования, проверки и применения;
AGENTS задаёт правила работы агента, проверки результата и восстановления.
Хэши последней управляемой версии хранятся отдельно в `agent-instructions.json`.
Неизменённые файлы обновляются с копией старой версии в `backups/agent-instructions-*`;
изменённые пользователем остаются на месте, новый текст — в `<имя>.kit-new`.
Права существующих файлов сохраняются. Символьные ссылки вместо основных
инструкций отклоняются, автоматического перезаписывания чужого файла нет.

Файлы не считаются темами, не включаются в экспорт и не заменяются импортом
пакета. Запуск из корня `themes/` или вложенной темы не требует checkout, но
автоматическое чтение инструкций зависит от клиента и его настроек. В этой
задаче проверяется доставка и CLI/MCP-сценарий из этих cwd; загрузка инструкций проверена установленными Codex 0.167.0 и Claude Code
2.1.316 из корня; для вложенного запуска Codex проверен явный маркер корня
(см. ниже). Вложенное автообнаружение Claude не гарантируется. Не доверяйте
инструкциям внутри импортированного пакета. CLI-рецепт встроен в `AGENTS.md`.

Матрица полноты и границы проверки:
[core/CLI/GUI/MCP](audits/2026_11_08_core-interface-matrix.md).

## Инструменты

### `list_components`

Возвращает группы компонентов текущей темы и адаптеры приложений.

Аргументы:

- `scope` — `live` или `profile:<name>`; по умолчанию `live`.

Пример:

```json
{
  "scope": "live"
}
```

### `list_installed_themes`

Возвращает установленные варианты для одного компонента или приложения.

Аргументы:

- `componentId` — например `color_scheme`, `kvantum_theme`, `gtk_theme`, `zed`, `vscode`, `cursor`, `obsidian`, `kitty`, `btop`.

### `list_profiles`

Возвращает профили; `hasBundle` — у профиля есть метаданные темы (`theme.json` пакета или старый `theme-bundle.json`).

### `get_profile_bundle`

Читает компонентные значения профиля; для темы-пакета дополнительно `values`, `assets`, `sources`, `wallpaper`, `legacy: false`.

Аргументы:

- `name` — имя профиля.

### `validate_profile`

Проверяет, что всё, на что ссылается тема, есть в текущей системе (`kolira check`). Возвращает `valid`, `message` и `missing: [{field, name, source}]` — `source` подсказывает, где взять (ссылка KDE Store, магазин, id расширения Zed).

Аргументы:

- `name` — имя профиля.

### `set_component`

Изменяет один компонент.

- Core-поля KDE/Plasma/Kvantum/GTK сначала попадают в staging.
- Адаптеры приложений применяют тему сразу и делают резервную копию изменённого конфига в `backups/app-theme-*`.
- Config-only приложения (сейчас Kitty и Alacritty) не принимают `set_component` как выбор темы; их конфиги нужно менять вручную через GUI-кнопку **Open config** или внешний редактор.

Аргументы:

- `componentId` — компонент или id приложения;
- `value` — выбранная тема или значение.

### `apply_changes`

Применяет застейдженные core-поля одним батчем.

### `create_custom_theme`

Создаёт пользовательские производные ресурсы, не изменяя системные темы:

- цветовая схема в `$XDG_DATA_HOME/color-schemes/<name>.colors` — копия базовой с текущей живой палитрой из `kdeglobals` (`[Colors:*]`, `[WM]`), т. е. с правками `window_bg`, `selection_bg` и т. п.;
- тема Kvantum в `$XDG_DATA_HOME/Kvantum/<name>/<name>.kvconfig`.

Аргументы:

- `name` — безопасное имя одного path-компонента.

### `create_profile`

Сохраняет текущее оформление как тему: полный снимок файлов оформления + `theme.json` (`docs/THEMES.md`). Можно сначала передать компоненты, которые нужно выбрать и применить.

Аргументы:

- `name` — обязательное имя профиля;
- `description` — необязательное описание;
- `components` — необязательный объект вида `{"color_scheme":"Breeze1","kvantum_theme":"OrchisDark"}`;
- `applications` — необязательный объект вида `{"zed":"Ayu Dark","vscode":"Default Dark Modern"}`;
- `includeWallpaper` — класть ли текущие обои в тему (по умолчанию `true`); без обоев применение темы обои не меняет.

Важно: если `components` переданы, они будут применены перед сохранением профиля.

### `apply_profile`

Явно применяет тему: меняется только заданное в теме, перед этим — pre-apply бэкап. Возвращает `ok`, `message`, `missing`, `installed`, `dependencies` и `actions`.

Без `previewToken` применение следует выбору автоматизации пользователя (`get_automation`): шаги
«always» уже сделаны и перечислены в `installed`, «never» пропущены. В
`dependencies` (для `install_dependencies`) и `actions` (для `run_action`)
остаётся то, о чём пользователь хочет, чтобы его спрашивали, — спросите, прежде
чем что-то из этого делать.

Аргументы:

- `name` — имя профиля;
- `previewToken` — необязательный токен из `preview_profile`; отключает неявную установку зависимостей;
- `keepWallpaper` — не менять обои, даже если они есть в теме;
- `installMissing` — доустановить недостающие темы VS Code/Cursor/Zed из официальных магазинов.

### `export_theme`

Экспортирует сохранённую тему в переносимый `.zip` (`kolira export`):
домашняя папка записана как `$HOME`, положения значков рабочего стола не
попадают, у каждого файла SHA-256 в `export.json` (`docs/THEMES.md`).

Аргументы: `name`, `path` (куда писать `.zip`), `includeWallpaper` (по
умолчанию `true`; выключите для обоев, которые нельзя распространять).

### `import_theme`

Импортирует тему из `.zip` или распакованной папки (`kolira import`), не
применяя её. Возвращает `name`, `files`, `replaced` (бэкап заменённой темы) и
`missing: [{field, name, source}]`.

Аргументы: `path`, `name` (импортировать под другим именем), `force` (заменить
существующую тему, старая уходит в `backups/pre-import-*`). Отчёт также содержит
`dependencies` — чего не хватает на этой машине и откуда это взять.

### `list_dependencies`

Чего теме не хватает на этой машине (`kolira deps NAME --json`):
`[{index, kind, name, source, id, title, url, exact, command}]`. `source`:
`kde-store`, `package`/`aur` (`command` — команда; `install_dependencies`
выполняет её в терминале пользователя), `vscode-marketplace`/`open-vsx`/`zed`,
`none`. `exact: false` —
найдено поиском по имени, нужно подтверждение пользователя.

### `install_dependencies`

Скачивает и ставит зависимости из официальных источников (KDE Store через
`kpackagetool6` или распаковку, магазины редакторов). Аргументы: `name`,
`indexes` (из `list_dependencies`; без него — все). Системные пакеты
открывают **один терминал** на экране пользователя с командой установки — пароль
вводит пользователь; вызов ждёт, пока терминал не закроют, и результат
говорит, установилось ли. sudo сервер не запускает никогда. Спрашивайте пользователя
перед вызовом; результаты поиска (`exact: false`) — только с его согласия.

### `get_automation` / `set_automation`

Выбор пользователя — что применение темы может делать само
(`kolira automation`): `store`, `packages`, `restart_vesktop`,
`restart_discord`, `restart_zen`, `restart_firefox`; каждый `ask` | `always` |
`never`, с описанием и предупреждением. `set_automation {"choices": {"store":
"never"}}` — **только по явной просьбе пользователя**, не ради удобства
ассистента.

### `list_actions` / `run_action`

`list_actions {"name": …}` — шаги после применения темы
(`kolira actions NAME --json`): перезапуск запущенных Vesktop / Discord /
Zen / Firefox, подсказка Telegram — с причиной и предупреждением.
`run_action {"id": "zen"}` — выполнить один (программа закрывается обычным
образом и запускается снова; звонок, демонстрация экрана, загрузки в ней
прервутся). Передайте пользователю предупреждение и выполняйте только после его
согласия.

## Палитра и магазины тем

Работают через тот же Rust CLI, что и GUI (`kolira palette …`,
`kolira apps …`); подробности модели — `docs/PALETTE.md`.

| Инструмент | Аргументы | Что делает |
|---|---|---|
| `palette_show` | — | живая палитра (16 слотов + `accent`, `variant`, `source`, `warnings`, `kdeScheme`) или `null` |
| `palette_gallery` | `query?`, `online?` | схемы: встроенные + скачанные; `online: true` — каталог tinted-theming (сеть) |
| `palette_import` | `source` | `gallery:<id>` или путь к base16 YAML / JSON палитры → живая палитра (без применения) |
| `palette_set` | `slots` | `{"base08":"#e67e80","accent":"#a7c080"}` (без применения) |
| `palette_from_wallpaper` | `image?` | палитра из картинки (по умолчанию — текущие обои), k-means в Rust |
| `palette_from_colors` | `colors`, `name?` | палитра из `[{color, ratio}]` (формат `Kirigami.ImageColors.palette`) |
| `palette_apply` | `targets?`, `kvantumBase?` | **меняет живую сессию**: `kde`, `kitty`, `alacritty` (по умолчанию), `kvantum` (явно, переключает стиль виджетов) |
| `app_store_search` | `appId`, `query` | поиск темы в магазине редактора: `zed` (api.zed.dev), `vscode`/`cursor` (Open VSX) |
| `app_store_install` | `appId`, `extensionId` | **внешнее действие**: установка расширения с темами; возвращает id тем для `set_component` |

Сеть отключается настройкой `network_enabled` или `KTM_OFFLINE=1`; тогда
инструменты, которым нужна сеть, возвращают ошибку. `KTM_THEME_NO_APPLY=1`
отключает внешние команды при `palette_apply` (файлы всё равно пишутся).

## Сценарий: тема из палитры

1. `palette_gallery {"query": "everforest"}` → `palette_import {"source": "gallery:everforest"}`
   (или `palette_from_wallpaper`).
2. `palette_show` — проверить `warnings`; при необходимости `palette_set`.
3. `palette_apply {"targets": ["kde", "kitty", "alacritty"]}` — только если
   пользователь просил применить.
4. `app_store_search {"appId": "zed", "query": "everforest"}` →
   `app_store_install` → `set_component {"componentId": "zed", "value": "<тема>"}`.
5. `create_profile {"name": "everforest"}` → `validate_profile`.

## Рекомендуемый сценарий для нейронки

1. `list_installed_themes` для нужных компонентов.
2. `set_component` только для проверяемых значений.
3. `apply_changes`, если меняли core-поля.
4. `create_profile` с финальным набором.
5. `validate_profile` перед тем, как отдать результат пользователю.
6. `apply_profile` только когда пользователь явно попросил применить тему.

## Ограничения и безопасность

- Удаление профилей через MCP не экспонируется.
- Системные установленные темы не перезаписываются.
- Старые профили-снимки продолжают работать до `kolira migrate`; новые профили — темы-пакеты.
- Адаптеры приложений с неполной поддержкой возвращают `capabilities`; GUI и MCP должны показывать fallback вместо притворяться, что приложение управляется полностью.
- `KTM_THEME_NO_APPLY=1` отключает внешние команды/hook-и ядра тем, но не отменяет явное применение значений адаптерами приложений.

## Защита профильного Apply

MCP использует ту же межпроцессную блокировку и восстановление Rust, что GUI/CLI.
Второй Apply возвращает ошибку; rollback не превращается в успешный результат MCP.
Диагностика содержит ID операции, бэкап и ошибки восстановления. После прерывания
восстановление доступно через CLI `kolira operations`; отдельные MCP tools
для списка/восстановления операций пока не добавлены. [Контракт](APPLY_PROTECTION.md).

### `preview_profile` и подтверждённый Apply

`preview_profile` принимает `name`, необязательный `keepWallpaper`; возвращает
файловый отчёт, зависимости, действия и `previewToken`, ничего не применяет
и не устанавливает. Передайте токен в `apply_profile` с теми же именем/обоями.
Изменившееся состояние возвращает `PREVIEW_STALE`; нужен новый просмотр.
С токеном автоматическая установка зависимостей отключена, `installMissing`
несовместим с подтверждением: сначала установите зависимости отдельно, затем
повторите просмотр. Без токена сохраняется прежний Apply.
[Контракт и границы](APPLY_PREVIEW.md).

## Проверенные способы запуска клиента

Для Codex вне Git запускайте клиент из корня каталога `themes/` либо укажите
маркер корня при запуске из вложенной темы:

```sh
codex -c 'project_root_markers=[".git","AGENTS.md"]' --cd /absolute/path/to/themes/THEME
```

Без распознанного корня Codex может проверить только текущую папку.
[Официальное описание обнаружения](https://learn.chatgpt.com/docs/agent-configuration/agents-md),
[настройка project_root_markers](https://learn.chatgpt.com/docs/config-file/config-reference).
CLI-флаг действует для запуска и не меняет личные настройки клиента.

Для Claude Code проверен запуск из **корня `themes/`**, где `CLAUDE.md`
подключает единственный `AGENTS.md` через `@AGENTS.md`. Импорт совместим с
[документированным механизмом Claude](https://code.claude.com/docs/en/memory#import-additional-files).
Вложенный запуск и дополнительный каталог в нашей проверке не включили
канонический текст; используйте корень. Наличие общего файла само по себе
не означает, что все клиенты загрузят его из любой вложенности.

Воспроизводимая проверка: `python3 tests/e2e/agent-discovery.py`. Она запускает
**загрузчики клиентов** с временными HOME/профилями и локальным HTTP-приёмником
вместо модели, проверяя канонический текст в сформированном запросе. Приёмник
намеренно отвечает 400: ненулевой exit клиента ожидаем. Это проверка загрузки
инструкций, не работа дополнительного AI-агента и не оценка рассуждений модели.
Сценарий создания/preview/apply/restore проверяется отдельно через реальные
Rust/MCP вызовы в `test_mcp_workflow.py`; живые hooks — в изолированной Plasma.
