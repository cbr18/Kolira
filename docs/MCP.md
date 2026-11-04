# MCP для theme-profile-kit

Репозиторий содержит локальный MCP-сервер для нейронных агентов. Он позволяет быстро собирать темы и профили через тот же `ThemeBundleCore`, который использует GUI, вместо ручного редактирования конфигов.

Сервер работает через stdio и не требует внешних Python-зависимостей сверх окружения приложения.

## Запуск

Из корня репозитория:

```bash
uv run --project gui/qt-app python mcp/theme_profile_mcp.py
```

Для полностью неприменяющего режима внешних команд:

```bash
KTM_THEME_NO_APPLY=1 uv run --project gui/qt-app python mcp/theme_profile_mcp.py
```

Пример MCP-конфигурации:

```json
{
  "mcpServers": {
    "theme-profile": {
      "command": "uv",
      "args": [
        "run",
        "--project",
        "gui/qt-app",
        "python",
        "mcp/theme_profile_mcp.py"
      ],
      "cwd": "/home/cbr/projects/kde_mods/theme-profile-kit"
    }
  }
}
```

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

Проверяет, что всё, на что ссылается тема, есть в текущей системе (`theme-profile check`). Возвращает `valid`, `message` и `missing: [{field, name, source}]` — `source` подсказывает, где взять (ссылка KDE Store, магазин, id расширения Zed).

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

Явно применяет тему: меняется только заданное в теме, перед этим — pre-apply бэкап. Возвращает `ok`, `message`, `missing` и `installed`.

Аргументы:

- `name` — имя профиля;
- `keepWallpaper` — не менять обои, даже если они есть в теме;
- `installMissing` — доустановить недостающие темы VS Code/Cursor/Zed из официальных магазинов.

### `export_theme`

Экспортирует сохранённую тему в переносимый `.zip` (`theme-profile export`):
домашняя папка записана как `$HOME`, положения значков рабочего стола не
попадают, у каждого файла SHA-256 в `export.json` (`docs/THEMES.md`).

Аргументы: `name`, `path` (куда писать `.zip`), `includeWallpaper` (по
умолчанию `true`; выключите для обоев, которые нельзя распространять).

### `import_theme`

Импортирует тему из `.zip` или распакованной папки (`theme-profile import`), не
применяя её. Возвращает `name`, `files`, `replaced` (бэкап заменённой темы) и
`missing: [{field, name, source}]`.

Аргументы: `path`, `name` (импортировать под другим именем), `force` (заменить
существующую тему, старая уходит в `backups/pre-import-*`). Отчёт также содержит
`dependencies` — чего не хватает на этой машине и откуда это взять.

### `list_dependencies`

Чего теме не хватает на этой машине (`theme-profile deps NAME --json`):
`[{index, kind, name, source, id, title, url, exact, command}]`. `source`:
`kde-store`, `package`/`aur` (`command` — что выполнить, сами пакеты не
ставятся), `vscode-marketplace`/`open-vsx`/`zed`, `none`. `exact: false` —
найдено поиском по имени, нужно подтверждение пользователя.

### `install_dependencies`

Скачивает и ставит зависимости из официальных источников (KDE Store через
`kpackagetool6` или распаковку, магазины редакторов). Аргументы: `name`,
`indexes` (из `list_dependencies`; без него — все). Спрашивайте пользователя
перед вызовом; результаты поиска (`exact: false`) — только с его согласия.

## Палитра и магазины тем

Работают через тот же Rust CLI, что и GUI (`theme-profile palette …`,
`theme-profile apps …`); подробности модели — `docs/PALETTE.md`.

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
- Старые профили-снимки продолжают работать до `theme-profile migrate`; новые профили — темы-пакеты.
- Адаптеры приложений с неполной поддержкой возвращают `capabilities`; GUI и MCP должны показывать fallback вместо притворяться, что приложение управляется полностью.
- `KTM_THEME_NO_APPLY=1` отключает внешние команды/hook-и ядра тем, но не отменяет явное применение значений адаптерами приложений.
