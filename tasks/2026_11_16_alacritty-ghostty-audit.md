# Аудит адаптеров терминалов Alacritty и Ghostty

Статус: DONE — 2026-11-16 (статический аудит кода, живая проверка не проводилась).

## Что есть

| Приложение | Файлы | Механизм |
|---|---|---|
| Alacritty | `.config/alacritty/alacritty.toml` + `.config/alacritty/kolira.toml` | `alacritty_split`/`alacritty_host` в `appearance.rs`, генерация `palette::alacritty_with_palette` |
| Ghostty | `.config/ghostty/config` + `.config/ghostty/kolira` | `ghostty_split`/`ghostty_host` в `extras.rs`, генерация `ghostty_colors` |

Оба через общий `Plan`/`MutationGuard` с before/after-журналом. `supports_live_reload: false` для обоих.

## Результаты статической проверки кода

### Alacritty

**Сильные стороны:**
- `alacritty_split` использует `toml_edit` — не	reserialisiert весь файл, сохраняет комментарии, порядок ключей и личные настройки вне блока внешнего вида.
- Разделение по категориям: `colors` (все уходит в тему), `font` (только семейства шрифтов, размер/смещения остаются личными), `window`/`cursor` (только ключи из `ALACRITTY_WINDOW`/`ALACRITTY_CURSOR` — opacity, blur, padding, decorations и т.п.).
- `alacritty_host` поддерживает оба формата импорта: старый `import` и новый `general.import` (Alacritty ≥ 0.14). Импорт не дублируется при повторном Apply.
- `palette::alacritty_with_palette` генерирует `[colors.*]` из палитры (primary, cursor, selection, normal/bright ANSI), сохраняя все остальные ключи в файле.

**Пробелы и риски:**
1. **Жёсткий путь `~/.config/alacritty/kolira.toml`** (`ALACRITTY_IMPORT`) — не использует `XDG_CONFIG_HOME`. При нестандартном `XDG_CONFIG_HOME` импортируемый файл не будет найден.
2. **Нет обработки отсутствующего `alacritty.toml`** — если хост-конфига нет, `alacritty_split` вернёт ошибку парсинга пустой строки. Нужно ли создавать новый конфиг с нуля?
3. **Нет проверки циклических импортов** — если пользователь вручную добавил импорт `kolira.toml` в другое место, может быть дублирование.
4. **`font.size` остаётся личным** — только семейства шрифтов уходят в тему. Это корректно (размер шрифта — личное), но должно быть задокументировано.

### Ghostty

**Сильные стороны:**
- `ghostty_split` разделяет по явному списку `GHOSTTY_LOOK` (цвета, шрифт, opacity, blur, padding, window-декорации и т.п.). Остальное остаётся личным.
- `ghostty_host` добавляет `config-file = ?kolira` в конец хост-конфига, не дублируя при повторном Apply.
- `ghostty_colors` генерирует ANSI палитру (16 цветов) + background/foreground/cursor/selection.
- `replace_block` заменяет блок `# BEGIN kolira palette` / `# END kolira palette` на месте, дубли не создаются при повторном Apply.
- `ghostty_apply_palette` делает одноразовое перемещение внешнего вида из хоста в тему-файл.

**Пробелы и риски:**
1. **`home_dir()` вместо XDG resolver** — `ghostty_apply_palette` и пути в `tracked_apps.rs` используют домашний каталог напрямую, не черезライブ XDG resolver. При нестандартном `XDG_CONFIG_HOME` пути могут не совпадать.
2. **Возврат хост-конфига** — неясно, как именно восстанавливается `.config/ghostty/config` при возврате к теме без ghostty конфига. Нужно проверить, что `plan` содержит правильный before/after для хоста.
3. **Одна тема-файл на всех** — `.config/ghostty/kolira` — один файл для всех тем. При смене темы блок заменяется, что корректно, но нет версионирования по имени темы.

## Общие проблемы для обоих

| Проблема | Статус |
|---|---|
| Live reload | Alacritty перечитывает конфиг сам; Ghostty требует перезапуска. Оба помечены `supports_live_reload: false` — корректно. |
| Повторный Apply | Alacritty: импорт не дублируется. Ghostty: блок заменяется на месте. Оба стабильны. |
| Возврат без конфига | Обрабатывается через общий before/after-журнал `operation::run`. Конкретное поведение для этих двух терминалов не проверялось в тестах. |
| Перенос в другой HOME | Пути в теме хранятся относительно `$XDG_CONFIG_HOME`, но `ALACRITTY_IMPORT` использует `~`. Потенциальная несоответственность. |

## Рекомендации

1. Исправить `ALACRITTY_IMPORT` на использование `XDG_CONFIG_HOME` вместо `~/.config`.
2. Добавить тесты для `alacritty_split`/`alacritty_host` с нестандартным содержимым `alacritty.toml` (без `[colors]`, с вложенными таблицами, с несколькими импортами).
3. Добавить тест для `ghostty_apply_palette` с проверкой возврата хост-конфига.
4. Документировать в `docs/APPLICATIONS.md`, что `font.size` в Alacritty остаётся личным.
5. Живая проверка: запустить Alacritty и Ghostty с тестовой палитрой, проверить изменение цветов и возврат.

## Связанные задачи

- [Расширение покрытия приложений](../tasks/2026_11_15_app-coverage-expansion.md) — родительская задача
- [Приоритеты покрытия](../docs/APP_COVERAGE_PRIORITIES.md) — Alacritty № 2, Ghostty № 3 в P1
- [Карта файлов приложений](../docs/audits/2026_11_15_app-coverage-files.md) — текущие пути

## Живая проверка

Не проводилась. Требуется запуск Alacritty/Ghostty на хосте с тестовой палитрой и проверкой Apply → возврат без конфига.
