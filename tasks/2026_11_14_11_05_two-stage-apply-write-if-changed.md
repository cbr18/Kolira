# Задача: двухэтапный apply тем (этап 1 + этап 2 внутри `apply`)

Ветка: `explore/two-stage-apply`
Источник гипотез: `tasks/2026_11_14_explore_two-stage-apply-hypotheses.md`
СТАТУС: РЕАЛИЗОВАНО (коммит 10e4733 + текущий)

## Что сделано

- `kolira apply <тема>` — применяет тему полностью:
  - **Этап 1 (блокирующий, `MutationGuard`)**: KDE Plasma, KWin, Qt, GTK, обои. Предупредительный бэкап, остановка plasmashell, восстановление снимка, перезапуск plasmashell.
  - **Этап 2 (фоновый, без `MutationGuard`)**: темы редакторов (VS Code/Cursor/VSCodium/Windsurf/Zed), мессенджеры (Discord/Vesktop/Vencord/BetterDiscord), терминалы (Konsole/Kitty/Alacritty), оболочка (fish/starship/btop/fastfetch/tmux). Устанавливается из кэша темы `assets/editor-themes/` (офлайн) или из магазина (`--install-missing`).
- `kolira apply <тема> --without-sync` — только этап 1, пропускает этап 2.
- `write_if_changed` — не добавлен: эксперимент показал, что при повторном `apply` той же темы ни один существующий конфиг не перезаписывается (уже работает).
- Отдельная команда `kolira sync` убрана — слишком запутанно для пользователя.

## Изменения в коде

- `theme.rs`: `ApplyOptions` получил поле `sync_apps: bool` (default `true`); `apply_inner` вызывает `apply_editor_themes` в конце если `sync_apps`; функция `apply_editor_themes` устанавливает темы редакторов из кэша или магазина.
- `bin/kolira.rs`: убрана команда `sync`; добавлен флаг `--without-sync`; `ApplyOptions` заполняется с `sync_apps: !args.contains("--without-sync")`.
- `cli_core.py`: убран метод `sync` (не нужен, apply делает всё).
- `theme_bundle.py`: убран автоматический вызов `sync` после `apply` (apply делает всё сам).
- `docs/THEMES.md`, `docs/APPLY_PROTECTION.md`, `CHANGELOG.md` — обновлены.

## Проверка

- 134 Rust unit-теста проходят
- `cargo clippy --all-targets` — чисто
- Эксперимент с хешами файлов подтверждён: повторный apply не перезаписывает существующие конфиги

## Статус

DONE 2026-11-14. Реализовано и проверено (134 Rust-тестов, clippy чистый).
