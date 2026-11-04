# breezerc в профилях и единая политика размытия

Status: DONE

## Проблема (2026-11-04)

- Меню были прозрачными, но не размытыми: стиль Breeze + `~/.config/breezerc`
  `[Style] MenuOpacity=50`, а Better Blur DX стоял с `BlurMenus=false`
  (стандартный blur выключен). `breezerc` не входит ни в один компонент
  профиля — прозрачность меню профилями не управляется.
- Пользователь: политики «blur только для окон» нет — размытие для всего.
  Вживую и в снимках glass/everforest уже выставлено
  `[Effect-better-blur-dx] BlurMenus=true, BlurDocks=true`
  (бэкап: `backups/manual-blur-everything-20261104-103253`).

## Что сделать

- [x] Компонент `breeze` (`.config/breezerc`) в Rust `tracked_apps.rs` и Python-зеркале;
      поле `breeze_menu_opacity` в Theme Inspector.
- [x] Убрать из документации (repo docs, `~/AGENTS.md`, `~/docs/theme-system-analysis.md`)
      упоминания «window-only blur policy»; зафиксировать «размытие для всего».
- [x] Поля Theme Inspector для `BlurMenus`/`BlurDocks` Better Blur DX — уже были (`blur_menus`, `blur_docks`).
- [x] Тесты и документация.
