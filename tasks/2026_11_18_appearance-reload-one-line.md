# Вместо восьми предупреждений — одна короткая строка

Статус: OPEN — 2026-11-18.

## Что было

После apply `kolira actions` возвращал по одному `Action` на каждое из восьми приложений, чьё оформление читается только при запуске:

Micro, Glow, GIMP, Heroic, Prism Launcher, Vim, mpv, Cliamp.

Каждое renderилось в диалоге "Завершение применения" как отдельная строка:
заголовок (`{App}: reopen or reload the native appearance`), причина
(`New sessions read the updated palette; existing sessions keep their current
settings`) и предупреждение (`Applications and documents are kept running`).
В summe — целый экран предупреждений на oito приложения.

Где формируется: `gui/qt-app/src/core/rust_core/src/actions.rs`, функция
`for_changes`, цикл над восьмью маркерами (строки ≈157–175).
Как показывается: `gui/qt-app/qml/Main.qml`, `AppDialog actionsDialog`,
Repeater над `actionsDialog.actions` (строки ≈1025–1066).

## Что сделать

Заменить восемь строк на одну короткую, без восклицательного знака:

```
Рекомендуется перезапустить такие-то программы.
```

Требования:
- одна строка, не весь экран;
- без `!`;
- показывается только если хотя бы одно из восьми приложений действительно
  изменило конфиг (как сейчас);
- существующие сессии не трогаются (приложения и документы не закрываются);
- перезапуск не выполняется автоматически — действие остаётся информационным
  (kind "manual", не входит в `selection()` и не уходит в `run_action`).

## План изменений

1. `actions.rs`: в `for_changes` убрать цикл, который пушит по одному
   `Action{id: "{id}_appearance" ...}` на приложение. Добавить одно
   `Action{id: "appearance_reload", kind: "manual", title:
   "Рекомендуется перезапустить такие-то программы.", reason: "", warning:
   ""}` когда хотя бы один маркер из восьми совпадает с `changes`.
2. `Main.qml`: у `Label` с `modelData.reason` добавить
   `visible: modelData.reason !== ""`, чтобы пустая причина не оставляла лишнюю
   пустую строку — получается ровно одна жирная строка сообщения.
3. Проверить, что нет ссылок на id `{micro,glow,gimp,heroic,prismlauncher,vim,mpv,cliamp}_appearance`
   вне `actions.rs` (Python/QML/тесты обращаются к полям обобщённо).
4. Добавить запись в `CHANGELOG.md` под `Unreleased` (изменение визуального
   поведения диалога "Завершение применения").
5. Прогнать проверки: `cargo test`, `cargo clippy`, `cargo fmt`, Python
   тест переводов/ядра (`test_gui.py::test_core_strings_are_in_catalogue` и
   смежные), `shellcheck` не требуется (bash не трогается).

## Проверка

- `cargo test --package rust_core` (или корень рабочей области Rust) — PASS.
- `cargo clippy --all-targets -- -D warnings` — PASS.
- `cargo fmt --check` — PASS.
- `QT_QPA_PLATFORM=offscreen uv run --project . --extra dev pytest gui/qt-app/tests -q` — PASS.
- Перевод: английский источник `"It is recommended to restart these programs."` +
  русский `"Рекомендуется перезапустить такие-то программы."` в
  `tools/i18n_ru.py` (CORE), собран через `gen_i18n_strings.py` + `pylupdate6` +
  `lrelease` в `i18n/kolira_ru.qm` (924 перевода, 0 незавершённых).
- В диалоге "Завершение применения" после apply темы, меняющей конфиг любого из
  восьми приложений, отображается одна строка на текущем языке без `!` и без
  восьми отдельных карточек.

## Документация

Формат предупреждений в диалоге не описан в docs отдельно — отдельной правки
docs не требуется. Изменение отразить в CHANGELOG.
