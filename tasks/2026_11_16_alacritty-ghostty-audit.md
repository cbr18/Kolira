# Аудит адаптеров терминалов Alacritty и Ghostty

Статус: OPEN — 2026-11-16.

## Запрос

Проверить существующие файловые адаптеры Alacritty и Ghostty на пробелы в покрытии,
возврат исходного состояния при отсутствии конфига в следующей теме, сохранение
поздних личных правок и надёжность повторного Apply. Адаптеры уже реализованы
(файловые пути известны), но полный цикл preview → apply → return без конфига
не проверялся для этих двух терминалов вместе.

## Что уже есть

| Приложение | Файлы | Механизм |
|---|---|---|
| Alacritty | `.config/alacritty/alacritty.toml` + `.config/alacritty/kolira.toml` | `alacritty_split`/`alacritty_host` в `appearance.rs`, генерация `palette::alacritty_with_palette` |
| Ghostty | `.config/ghostty/config` + `.config/ghostty/kolira` | `ghostty_split`/`ghostty_host` в `extras.rs`, генерация `ghostty_colors` |

Оба через общий `Plan`/`MutationGuard` с before/after-журналом. Alacritty имеет
`supports_live_reload: false`, Ghostty — тоже `false`.

## Что нужно проверить

- [ ] Preview корректно перечисляет все файлы, создания, удаления, ключи для обоих терминалов
- [ ] Apply меняет цвета терминала, шрифт/размер и поведение окна остаются личными
- [ ] Возврат к теме без конфига Alacritty/Ghostty восстанавливает исходный `alacritty.toml`/`config`
- [ ] Поздние личные правки (размер шрифта, padding, opacity) сохраняются при возврате
- [ ] Повторный Apply стабилен (нет дублирования includes/блоков)
- [ ] Перенос в другой HOME/XDG не ломает пути
- [ ] Live reload: Alacritty перечитывает конфиг сам, Ghostty нужен перезапуск
- [ ] Документация в `docs/APPLICATIONS.md` и `docs/APP_COVERAGE_PRIORITIES.md` актуальна

## Связанные задачи

- [Расширение покрытия приложений](../tasks/2026_11_15_app-coverage-expansion.md) — родительская задача
- [Приоритеты покрытия](../docs/APP_COVERAGE_PRIORITIES.md) — Alacritty № 2, Ghostty № 3 в P1
- [Карта файлов приложений](../docs/audits/2026_11_15_app-coverage-files.md) — текущие пути
