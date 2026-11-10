# Пользовательские темы Kolira

Здесь лежат пакеты тем: `<имя>/theme.json` и файлы оформления внутри каталога
темы. Корень по умолчанию — `~/.config/kolira/themes/`, при XDG_CONFIG_HOME —
`$XDG_CONFIG_HOME/kolira/themes/`. Внутри пакета всегда остаются `.config/...` и
`.local/share/...`, независимо от XDG-путей текущего компьютера.

Агенту: прочитай [AGENTS.md](AGENTS.md). Ниже — маршрут создания темы без
предварительного изменения рабочего стола. Все команды доступны из любого cwd.

## 1. Проверить возможности

```sh
kolira list
kolira status
printf '%s\n' '{}' | kolira core agent.capabilities
printf '%s\n' '{}' | kolira core agent.schema
```

Через MCP: `get_capabilities`, `get_theme_schema`, `list_profiles`,
`list_components`, `list_installed_themes`. Схема возвращает field IDs, типы и
признак offline. Например `plasma_theme` — поле; `plasma_style` — компонент GUI.
Сбой чтения — ошибка, а не доказательство отсутствия тем.

## 2. Подготовить отдельный пакет

Вариант A — минимальная новая тема из известных полей:

```sh
printf '%s\n' '{"name":"my-dark","description":"Тёмные панели и Breeze","values":{"widget_style":"Breeze","plasma_theme":"breeze-dark"},"use_palette":false}' |
  kolira core agent.prepare
```

MCP-эквивалент: `prepare_theme` с тем же JSON. Ответ содержит `path`, `package`,
`missing`, `warnings` и `live_appearance_changed:false`. Существующее имя отклоняется.

Для палитры сначала выбери её: `kolira palette gallery`, затем, например,
`kolira palette import gallery:gruvbox-dark-medium`. Это сохраняет состояние
палитры без изменения оформления. Затем `agent.prepare` с `use_palette:true`
добавит KDE/Kitty/Alacritty. `kolira palette apply` уже меняет live-оформление.

Вариант B — взять за основу установленную тему, найденную через `kolira list`:

```sh
kolira clone glass my-glass
```

Клонирование не применяет тему. Правь файлы в каталоге `my-glass`, сохраняй
согласованность snapshot и `theme.json` (values, assets, dependencies, wallpaper).
Не заменяй стиль Kvantum или GTK одним названием: нужный ресурс должен быть
установлен или содержаться в пакете. Встроенные ресурсы объявляются в assets с
пакетным относительным path, происхождением, лицензией и redistributable.

Минимальная структура:

```text
my-dark/
  theme.json                  # schema: kolira/theme@2, name, values, assets…
  .config/kdeglobals          # только оформление
  .config/plasmarc            # [Theme] name=...
  wallpaper/                  # если тема задаёт обои
```

Все названия/пути — из возвращённой схемы и реальных ресурсов. Для своей картинки
скопируй файл в `wallpaper/`, укажи относительный путь в `theme.json.wallpaper`
и соответствующий asset. Оставь wallpaper пустым, если тема не должна менять фон.
`kolira save` сохраняет текущий рабочий стол, а не перечитывает правки отдельного
пакета. `kolira new --from=...` меняет live-вид; для подготовки используй способы выше.

## 3. Проверить и упаковать

```sh
kolira check my-dark --json
kolira lint my-dark --json
kolira deps my-dark --json
kolira preview my-dark --json
kolira export my-dark --out=/tmp/my-dark.zip --json
```

Проверь missing, состав diff, сбросы, зависимости, обои, шрифты и рестарты.
Успешный `check` проверяет доступность ресурсов; он не заменяет проверку CSS,
валидности форматов приложений и визуального результата. Перекрытие значков
Breeze и нечитаемые цвета схемы/терминала показывает `lint` (каждое предупреждение
либо исправь, либо явно назови пользователю); см. «Контракт оформления» в AGENTS.md. Export создаёт manifest
с хешами; его можно импортировать под новым именем для проверки переносимости:

```sh
kolira import /tmp/my-dark.zip --name=my-dark-copy --json
```

Это подготовка файлов. Ничего из этих шагов не требует Apply.

## 4. Применить по запросу пользователя

MCP: вызови `preview_profile` (`name`, `keepWallpaper`), просмотри результат и
передай его непустой `previewToken` в `apply_profile` с теми же параметрами.
Без токена MCP Apply отклоняется. После правок или установки зависимостей нужен
новый preview. Поле `installMissing` нельзя совмещать с подтверждённым токеном:
сначала установка, затем новый просмотр.

CLI: `kolira preview my-dark --json` возвращает `token`. Используй его буквально
вместо HASH в команде:

```sh
kolira apply my-dark --preview-token=HASH --without-dependencies --json
```

Чтобы сохранить текущую картинку, добавляй `--keep-wallpaper` и в preview, и в
apply. Прямой CLI Apply без токена остаётся доступным для ручного сценария.

## 5. Проверить результат и восстановиться

```sh
kolira status
kolira wallpaper current
kolira operations list
kolira operations show OPERATION_ID
```

Замени OPERATION_ID реальным ID из отчёта. Проверь errors/skipped/missing,
сохранённые файлы и заявленные эффекты. Файл обоев должен существовать и читаться;
наличие пути в конфиге не равно видимому фону. Уже открытое приложение может
прочитать стиль только после перезапуска — укажи это в результате.

Для прерванной операции: `kolira operations recover OPERATION_ID` сохраняет
последующие личные изменения. Возврат к состоянию перед завершённой операцией:
`kolira operations restore OPERATION_ID --confirm` создаёт новый backup.
Делай восстановление, когда оно входит в намерение пользователя.

## Обновление файлов этого каталога

`printf '%s\n' '{}' | kolira core agent.install` доставляет README.md, AGENTS.md
и совместимый CLAUDE.md. Неизменённые управляемые файлы обновляются с backup;
пользовательские дополнения сохраняются, новый текст помещается в `.kit-new`.
Эти имена зарезервированы для документов, не используй их как имена тем.

`kolira bundled sync /path/to/kit/themes` сохраняет изменённые локальные темы.
Для явного обновления используй `kolira import /path/to/package --force`:
старая копия перемещается в pre-import backup. Импорт сам по себе не применяет тему.
