# Задача: скачиваемые темы редакторов (VS Code, VSCodium, Zed и др.) как часть темы kolira

## Контекст

Kolira уже умеет хранить id темы редактора в `workbench.colorTheme` через адаптеры приложений (`tracked_apps.rs`, `stores.rs`), но не умеет:
- скачивать `.vsix` / тему Zed из интернета и класть её в папку темы;
- устанавливать расширение редактора при `kolira apply <тема>`;
- хранить собственные файлы тем редакторов прямо в репозитории/архиве темы.

Для обновления цветов подходит нативное расширение VS Code с `_watch: true`.
Kolira использует снимок и явный apply без фонового демона.

## Вопросы, которые нужно закрыть в задаче

1. **Где лежит тема редактора после первой установки?**
   - Внутри папки темы: `~/.config/kolira/themes/<имя>/assets/editor-themes/<источник>.<издатель>.<имя>/` (или `.vsix` рядом).
   - При повторном `apply` — берётся из кэша темы, не качается повторно.
   - При `kolira export <имя> --out=file.zip` — пакуется вместе с темой в ZIP.

2. **Можно ли класть свои файлы тем прямо в тему в архиве/репо?**
   - Да: пользователь может положить `.vsix` / тему Zed в `assets/editor-themes/` своей темы, а в `theme.json` указать локальный путь вместо URL.
   - Экспорт/импорт ZIP (`kolira export` / `kolira import`) должен включать эту папку.
   - Публикация темы в репозитории (готовые темы kolira) — возможно, если автор темы включил файлы редакторов в комплект.

## Требуемые доработки

### Rust core (`gui/qt-app/src/core/rust_core/src/`)

| Компонент | Что сделать |
|---|---|
| `theme.rs` / `types.rs` | Добавить в `theme.json` опциональный блок `editor_themes` (или `app_themes`): источник (`open-vsx`, `vsix`, `zed`, `local`), издатель, имя, версия, URL, локальный путь в assets. |
| `net.rs` | Скачивание `.vsix` / манифеста темы Zed по URL с проверкой хеша и кэшированием внутри папки темы. |
| `stores.rs` | Расширение поиска тем: Open VSX (VS Code/Cursor/VSCodium), Marketplace (VS Code), галерея тем Zed (`api.zed.dev`). |
| Адаптер редактора (новый или в `tracked_apps.rs`) | При `apply`: установить `.vsix` через CLI редактора (`code --install-extension`, `codium`, `cursor`, `zed`) **или** развернуть нативное расширение темы с `_watch: true` в `~/.vscode/extensions/` (и аналогах для Codium/Cursor/Windsurf). Прописать `workbench.colorTheme` (или эквивалент Zed) через JSONC‑патч. При `recover`: удалить расширение и вернуть старый id. |
| `paths.rs` | Валидация путей внутри `assets/editor-themes/` — защита от traversal, только внутри папки темы. |
| Нативное расширение с `_watch: true` | В `assets/editor-themes/<источник>.<издатель>.<имя>/` разворачивается структура расширения: `package.json` с `"_watch": true` + `themes/PTMTheme-color-theme.json`. При `apply` копируется в `~/.vscode/extensions/` (или `~/.vscode-oss/`, `~/.cursor/`, `~/.windsurf/`). Редактор перезагружает тему «на лету» без перезапуска окна и без конфликтов с `workbench.colorCustomizations` в `settings.json`. |


### GUI (`gui/qt-app/src/`)

| Компонент | Что сделать |
|---|---|
| `controllers/theme_bundle.py` / `theme_inspector.py` | Добавить выбор темы редактора: «Встроенная», «Из магазина», «Локальный файл», «Нет». |
| QML (страница темы) | Поле ввода URL / выбор магазина, прогресс скачивания, статус установки, предупреждение о перезагрузке окна редактора. |
| `core/cli_core.py` | Проксировать новые CLI-флаги kolira для тем редакторов (если появятся). |

### CLI

- `kolira save <имя> --vscode-theme open-vsx:dracula-theme.theme-dracula@3.0.0` — сохранить ссылку на тему в `theme.json` и скачать .vsix в assets темы.
- `kolira apply <имя>` — установить расширение редактора и применить id темы.
- `kolira check <имя>` — сообщить, если тема ссылается на несуществующий .vsix или редактор не установлен.

### Документация (обязательно в той же задаче)

- `docs/THEMES.md` — формат `theme.json`: блок `editor_themes`, локальные assets, экспорт в ZIP.
- `CHANGELOG.md` — запись под `Unreleased`.
- `docs/DEPENDENCIES.md` — если тема редактора считается зависимостью, описать её в реестре.

## Критерий готовности

`kolira save glass --vscode-theme open-vsx:dracula-theme.theme-dracula@3.0.0` скачивает `.vsix` в `themes/glass/assets/editor-themes/`, `kolira apply glass` устанавливает расширение в `~/.vscode/extensions/` и прописывает `workbench.colorTheme`, `kolira export glass` включает assets в ZIP.

## Статус

DONE 2026-11-14. Реализовано и проверено (134 Rust-тестов, clippy чистый).

## Уточнение по ревью 2026-11-15

Исходный план выше сохраняется как история. Актуальный контракт и найденные
отличия от плана записаны в [задаче ревью](2026_11_15_two-stage-review-and-merge.md)
и `docs/THEMES.md`: одна транзакция двух этапов, явные app_targets[], офлайн-кэш
и явная подготовка загрузок, проверка живого оформления перед сохранением палитры.
Chromium и несуществующий флаг save --vscode-theme не объявляются реализованными.
