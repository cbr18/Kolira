# Политика хранения бэкапов и журнала операций

**Статус:** DONE (2026-11-24). Реализовано в Rust core, покрыто тестами и
документацией. [Контракт](../docs/APPLY_PROTECTION.md#политика-хранения).
[Задача защиты применения](2026_11_08_apply-protection.md).

## Проблема

`restore_points.rs` уже ограничен (`COUNT=20`, `BYTES=2 GiB`), но остальные
артефакты применения росли без потолка:

- `backups/pre-apply-*` — по 63–98 МБ каждый, накопление ~1.4 ГБ;
- `backups/deploy-kolira*` — копии установщика, без ограничения;
- `backups/pre-import-* / pre-prune-* / pre-split-* / pre-save-* /
  pre-normalize-* / palette-apply-*` — одиночные страховочные копии без cap;
- `operations/<id>.json` — записи смен тем (GUI показывал 10, на диске все).

Это было помечено в `APPLY_PROTECTION.md` как отдельная незакрытая задача.

## Требования

1. Ограничить рост `backups/` и `operations/` по классам, не ломая
   `restore_completed` / `recover` / `list_page` (они читают `record.backup`).
2. Эвиктить запись операции и её `pre-apply-*` бэкап атомарно, как пару
   (`tempfile` + `rename`), без «висячих» записей с удалённым бэкапом.
3. Защищать: `pending`/interrupted, `restored_operation`-цели, бэкап активной
   операции, `restore-points` (своя retention).
4. Эвиктнутая запись в `restore_completed`/`recover` — понятная ошибка, не
   «corrupt».
5. Лимиты (согласовано): `pre-apply` — 12 записей / 1 GiB; `deploy-kolira` —
   5 последних; одиночные `pre-import/prune/split/save/normalize` и
   `palette-apply` — 10 последних / 14 дней.
6. Лёгкая очистка в конце успешного apply под `operation-history.lock`.
7. Тесты (Rust) и обновление документации в той же задаче.

## Приёмка

- [x] Новый модуль `retention.rs` с лимитами по классам и атомарной эвикцией
      пары запись↔бэкап.
- [x] `pre-apply-*` эвиктятся вместе с `operations/<id>.json`; pending/current/
      restored и занятые живой операцией каталоги защищены.
- [x] `deploy-kolira*` (≤5) и одиночные страховочные копии (≤10 / 14 дней) под
      отдельными cap.
- [x] Вызов после успешного apply под `operation-history.lock`; best-effort.
- [x] `restore_completed`/`recover` на эвиктнутой записи сообщают об отсутствии
      бэкапа, а не о повреждении.
- [x] Rust-тесты: count-cap, byte-cap, защита pending/current/restored,
      атомарность пары, классы бэкапов, deploy-cap (9 тестов).
- [x] Обновлены `APPLY_PROTECTION.md`, `THEMES.md`, `CHANGELOG.md`,
      `CURRENT_APP.md`.

## Результат

- Rust core: новый `retention.rs`, 9 юнит-тестов; весь набор — **359 PASS**,
  `cargo clippy --all-targets -D warnings` и `cargo fmt --check` чисты, release
  собирается.
- Лимиты константами в `retention.rs` (`APPLY_COUNT/BYTES`, `DEPLOY_COUNT`,
  `SAFETY_COUNT`, `SAFETY_MAX_AGE_DAYS`).

## Границы

- Локальная гигиена `~/.config/kolira/`; не трогает темы, `apply-journal`
  (один, перезаписывается) и `restore-points` (своя retention).
- Эвикция необратима: старую смену после удаления не восстановить.
- Проверено в изоляции; живой Apply на Plasma в этой задаче не запускался.
- Отдельной команды `kolira cleanup` и кнопки GUI нет (по согласованию):
  очистка автоматическая после успешного применения.
- UI со списком последних 10 смен тем остаётся отдельной задачей; хранение теперь
  ограничено и от него не зависит.