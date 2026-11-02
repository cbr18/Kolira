# План: модель тем — формализация, кастомные приложения, редактор

Три этапа. Файлы-снимки остаются источником правды (CLI, бэкапы, rsync);
никакой SQLite — метаданные и реестр живут в JSON рядом с данными.

## Этап A — формализация темы (`manifest.json`) [сделано]

Профиль получает формальный манифест `manifest.json`:

```json
{
  "schema_version": 1,
  "created_at": "2026-10-28T12:00:00+10:00",
  "tool_version": "0.5.0",
  "name": "glass",
  "description": "Тёмное стекло",
  "wallpaper": "",
  "accent_from_wallpaper": false,
  "components": [
    { "id": "kwin", "paths": [".config/kwinrc", ".config/kwinrulesrc"] }
  ]
}
```

- Rust `profile.rs`: пишет json при save (и legacy `manifest.txt` для старых
  тулов); читает `created_at` и `description` из json (fallback на txt).
- CLI: `save NAME [--description=…]`, новый `describe NAME --set "…"`.
- Python `cli_core.py`: читает description из json; `set_description` —
  через `describe --set` (новый метод ProfileCore).
- GUI: ✅ поле «Описание» при save, показ в карточках, диалог правки описания
  (карандаш на карточке; `ProfileController.setProfileDescription`).
- Тесты: round-trip манифеста (bash 14), паритет created_at/description между
  Rust CLI и GUI-чтением manifest.json (pytest 6c/6e).

## Этап B — страница «Приложения» (`components.json`)

### Поведение страницы (зафиксировано)

- Список **всех** компонентов — 20 встроенных + пользовательские, сгруппированный
  по категориям. Строка: иконка, имя, статус (installed / not installed),
  галочка «отслеживать».
- **Галочка** = компонент участвует в save/apply. Снятая галочка заносит id в
  `disabled_components` (`app-preferences.json`); файлы и снимки профилей
  не трогаются, снять галочку и вернуть — можно в любой момент.
- **Встроенные**: галочку можно снять (отключить), удалить или отредактировать
  нельзя.
- **Пользовательские**: галочка + «Изменить» + «Удалить» (с подтверждением).
  Удаление вычищает id из `disabled_components` и `custom_components`.
- Кнопка «Добавить приложение…» открывает диалог создания.

### Поля пользовательского компонента

| Поле | Обяз. | Правило |
|---|---|---|
| `id` | авто | слаг из `name` (`[A-Za-z0-9][A-Za-z0-9._-]{0,63}`), read-only; сменить = удалить и создать заново |
| `name` | да | отображаемое имя |
| `config_paths` | да, ≥1 | относительные пути файлов/папок от `$HOME`; без `..` и абсолютных; папки копируются рекурсивно |
| `category` | нет | группировка на странице; по умолчанию «Custom», можно ввести свою |
| `description` | нет | однострочное примечание |
| `icon` | нет | имя иконки темы KDE; по умолчанию `application-x-executable` |
| `reload_command` | нет | одна команда после apply: бинарник из PATH, **без shell** (запускается напрямую, операторы `\| & ; < > $ \` " '` запрещены на этапе валидации) |
| `supports_live_reload` | авто | `true`, когда задана `reload_command` (приложение перечитывает конфиг без рестарта) |

Отказ при: дубликате id, id встроенного компонента, traversal в id/путях,
пустом списке путей (валидация уже есть в Rust; зеркало — в Python и в GUI).

### Выполнение `reload_command` при apply

После копирования файлов и live-хуков (kitty) выполняются команды тех
применённых пользовательских компонентов, у которых задана `reload_command`.
Best-effort: ошибка команды — предупреждение, apply не прерывается.
`KTM_THEME_NO_APPLY=1` — пропуск (как у остальных внешних команд).

### Слияние реестров

Python-детект (`tracked_apps.py`) читает `components.json` и мерджит его с
встроенным реестром; при совпадении id встроенный выигрывает (зеркало Rust
`merged_component_defs`). Данные для GUI (в т.ч. `reload_command`, флаг
«пользовательский») идут через расширенный `protocol.Component`.

### Критерии «готово» (этап B)

- [x] Rust: CRUD + валидация + слияние (`custom_components.rs`)
- [x] CLI: `theme-profile components list|add|edit|remove`
- [x] Python: CRUD-адаптер в `CliTrackedAppsCore` (json.dumps, экранирование)
- [x] Python: слияние реестров в детекте (встроенные выигрывают)
- [x] `disabled_components` в настройках + фильтр в `_settings_components()`
- [x] `reload_command` выполняется при apply (валидация, no-shell, `KTM_THEME_NO_APPLY`)
- [x] Страница «Приложения»: список с галочками, добавить/изменить/удалить
- [x] Тесты: Rust (валидация команды), bash (end-to-end с мок-приложением),
      pytest (слияние, фильтр, CRUD-контроллер)
- [x] Пример с реальным приложением задокументирован (см. `docs/CURRENT_APP.md`)

## Этап C — редактор параметров

- [x] C1: read-only дерево «приложение → файл → параметр» в ThemePage
      (`read_config_params` в theme_core: INI/JSON/JSONC/TOML/user.js/
      line-based; ≤300 строк на файл, файлы >512 KB — с пометкой; scope
      `live` и `profile:<name>`; модель `themeInspector.appConfigs`)
- [x] C2: правка INI/JSON по ключу с превью diff и undo по сессии
      (`ThemeCore.set_config_value`: INI — `_set_ini_text` с сохранением
      комментариев; JSON/JSONC — `_json_patch` по позиции с сохранением типа
      значения; generic-записи в общем стеке undo/revert; UI — диалог
      «было → станет» на ThemePage, только live-scope)
- [x] C3: TOML/JSONC/user.js по ключу + текстовый fallback для экзотики
      (`_toml_set`: `[section]` и dotted-заголовки, тип значения сохраняется,
      массивы — JSON-литералом; inline-таблицы без заголовка секции отклоняются;
      user.js — `_patch_userjs`; line-based «key value» (kitty.conf) пишется
      в своём формате; экзотика (CSS и пр.) — кнопка «Show in folder»)

## Критерии «готово»
- A: манифест v1, описание в GUI/CLI/карточках, 121 bash + 209 python + 10 rust тестов зелёные.
- B: кастомный компонент проходит save/apply с reload-командой, CRUD в GUI.
- C: инспектор редактирует параметры любого из 20 встроенных + кастомных приложений.