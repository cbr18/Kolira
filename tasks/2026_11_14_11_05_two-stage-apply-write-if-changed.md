# Задача: двухэтапный apply тем (этап 1 + фоновый докатчик)

Ветка: `explore/two-stage-apply`
Источник гипотез: `tasks/2026_11_14_explore_two-stage-apply-hypotheses.md`

## Контекст

Проверено экспериментом: при повторном `apply` той же темы **ни один существующий
конфиг-файл не перезаписывается** — `write_if_changed` уже работает. Добавлять
явные `cmp -s` проверки не нужно.

`apply_profile_keeping()` (`src/profile.rs`) делает всё под `MutationGuard`:
останавливает plasmashell → восстанавливает файлы → перезапускает plasmashell.
Это медленная и нарушающая работу часть.

Установка тем редакторов (моя реализация в задаче editor-theme-downloads)
пока находится внутри `apply_inner()` (этап 1). Её нужно вынести на этап 2.

## Решение

- **Этап 1** (блокирующий, `MutationGuard`): KDE/Qt/GTK/обои — как сейчас, без изменений.
- **Этап 2** (фоновый, без блокировки): новая команда `kolira sync <тема>` —
  устанавливает темы редакторов, мессенджеров, терминалов, оболочки.
- `write_if_changed` — не добавлять (уже работает).

## Этапы реализации

### 1. Вынести установку тем редакторов из `apply_inner` в отдельную функцию

Файл: `gui/qt-app/src/core/rust_core/src/theme.rs`

В `apply_inner()` удалить блок установки editor_themes (который я добавил
ранее). Создать отдельную функцию `apply_editor_themes(name, package)`,
которую вызовет команда `sync`.

```rust
pub fn apply_editor_themes(name: &str, package: &Package) -> Result<Vec<String>> {
    let dir = profile_dir(name)?;
    let mut installed = Vec::new();
    for ed in &package.editor_themes {
        let cache_dir = dir.join(&ed.local_path);
        // install from cache or download
        ...
    }
    Ok(installed)
}
```

### 2. Создать CLI-команду `kolira sync <тема>`

Файл: `gui/qt-app/src/core/rust_core/src/bin/kolira.rs`

Добавить ветку `sync` в USAGE и в обработку команд:

```rust
kolira sync NAME [--json]  (фоновый докатчик: редакторы, мессенджеры, терминалы)
```

Реализация: загрузить `theme.json`, вызвать `theme::apply_editor_themes()`,
затем адаптеры мессенджеров (Discord, Vesktop), терминалов (Konsole OSC 50),
Obsidian, fish/starship/btop.

### 3. Добавить `sync` в ApplyReport

Файл: `gui/qt-app/src/core/rust_core/src/theme.rs`

В `ApplyReport` добавить поле `synced: Vec<String>` для отчёта этапа 2.

### 4. Python GUI: вызов `sync` после `apply`

Файлы: `gui/qt-app/src/controllers/theme_bundle.py`, `theme_inspector.py`

После успешного `apply` предложить запустить `sync` (галочка "также применить
темы приложений" или автоматический вызов в фоне).

### 5. Документация

- `docs/THEMES.md` — добавить описание команды `kolira sync`
- `docs/APPLY_PROTECTION.md` — уточнить, что этап 2 не блокирует MutationGuard
- `CHANGELOG.md` — запись под `Unreleased`

## Критерий готовности

1. `kolira apply glass` — только этап 1 (KDE/Qt/GTK/обои), без установки editor themes.
2. `kolira sync glass` — этап 2: устанавливает темы редакторов из кэша темы.
3. `kolira apply glass --with-apps` — оба этапа подряд (опционально, если добавим флаг).
4. Все 134 Rust unit-теста проходят, `cargo clippy` без предупреждений.

## Статус

Открыто. Не блокирует текущий релиз.
