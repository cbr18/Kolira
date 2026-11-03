# Проверить интеграцию с обоями: от чего она зависит

Status: TODO

## Проблема

Обои участвуют в трёх местах: Rust CLI (`wallpaper-picker/next/prev`), GUI
(`CliWallpaperCore` в `gui/qt-app/src/core/cli_core.py`) и профили
(`theme-bundle.json` → `wallpaper`, плюс акцент «из обоев» в
`profile.rs::reapply_color_scheme`). Нужно проверить, что интеграция не
зависит от неявных условий конкретной машины/локали/сессии, и устранить
найденные зависимости.

## Что уже замечено (2026-11-04, при создании профиля everforest)

1. **Захардкоженный каталог с русской локалью.** Rust-бинарники берут
   `~/Изображения/Wallpaper` (`bin/wallpaper-{next,prev,picker}.rs`), GUI — тот
   же путь как fallback (`cli_core.py::_resolve_wallpaper_dir`). На
   англоязычной системе (`~/Pictures`) обои «пропадут». `XDG_PICTURES_DIR`
   (`xdg-user-dir PICTURES`) не учитывается; GUI-настройка
   `wallpaper_directory` передаётся CLI только через `WALLPAPER_DIR`, а
   `theme-menu`/хоткеи запускают бинарники без неё.
2. **Профиль хранит абсолютный путь к обоям, а не файл.** `theme-bundle.json`
   содержит `/home/cbr/Изображения/Wallpaper/<file>`; снапшот профиля файл не
   копирует. Переименование/удаление картинки или перенос профиля ломает
   применение молча; `validate_profile_bundle` существование обоев не
   проверяет.
3. **Обои применяются только через GUI-путь.** `ThemeBundleCore.apply_profile`
   вызывает `set_wallpaper`, а `theme-profile apply NAME` (CLI, `theme-menu`,
   трей?) поле `wallpaper` из bundle игнорирует — восстанавливается только
   то, что лежит в снапшоте `plasma-org.kde.plasma.desktop-appletsrc`.
   У bundle профиля glass поля `wallpaper` нет вовсе (сообщение `apply glass`
   не содержит шага обоев) — проверить, что восстановление appletsrc
   действительно возвращает обои без перезапуска plasmashell.
4. **Баг пути в акценте «из обоев».** `profile.rs::first_wallpaper_image`:
   `config_home().join(".config/plasma-…-appletsrc")` → `~/.config/.config/…`;
   fallback на живой конфиг никогда не срабатывает.
5. **Разные правила «какие обои текущие».** `wallpaper.rs::plasma_wallpaper_from`
   и `cli_core.py::_plasma_wallpaper` берут *последний* `Image=` в группе
   `[Containments][N][Wallpaper][org.kde.image][General]`, а
   `profile.rs::first_wallpaper_image_in` — *первый* `Image=file://` в файле
   (любая группа). После применения everforest в appletsrc два разных
   `Image=` (несколько экранов/активностей) — ответы расходятся.
6. **Внешние зависимости.** `plasma-apply-wallpaperimage` (применение, «все
   рабочие столы» — разные обои на мониторах не поддержаны), `magick`
   (миниатюры и акцент из обоев; без него акцент молча не считается),
   `wofi` (picker). Акцент «из обоев» = `magick -colors 1` (средний цвет), а
   Plasma считает акцент своим алгоритмом — цвета могут отличаться.
7. **Обои без `org.kde.image`** (слайдшоу, `org.kde.potd`, видео-плагины)
   никак не распознаются — текущие обои «пустые», next/prev стартуют с 0.

## Что сделать

- [ ] Каталог обоев: `WALLPAPER_DIR` → настройка GUI → `xdg-user-dir PICTURES`/Wallpaper
      → `~/Pictures/Wallpaper`; одна реализация в Rust, GUI читает её же.
- [ ] Решить: копировать файл обоев в профиль (`profile/wallpaper.<ext>`) или
      хранить путь + проверять в `validate_profile_bundle` с понятной ошибкой.
- [ ] `theme-profile apply` должен применять `wallpaper` из bundle (единый путь
      для CLI/GUI/MCP, логика — в Rust).
- [ ] Исправить `first_wallpaper_image` (`config_home().join("plasma-…")`) и
      свести поиск текущих обоев к одной функции `plasma_wallpaper_from`.
- [ ] Мультимонитор: определить поведение (все экраны / по экрану) и покрыть тестом.
- [ ] Деградация без `magick`/`wofi`: явное сообщение вместо тихого пропуска.
- [ ] Тесты: локаль en/ru, отсутствующий файл обоев, два экрана, не-image плагин.
- [ ] Обновить `README.md`, `docs/CURRENT_APP.md`, `gui/qt-app/README.md`.

## Не входит в задачу

- Новые форматы обоев (видео, анимированные).
- Генерация палитры темы из обоев (matugen-подобное) — отдельная фича.
