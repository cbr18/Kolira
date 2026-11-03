# Проверить интеграцию с обоями: от чего она зависит

Status: DONE (ветка feat/wallpaper-integration)

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

## Решения (согласованы 2026-11-04)

- Каталог: `$WALLPAPER_DIR` → настройка GUI `wallpaper_directory` → `$(xdg-user-dir PICTURES)/Wallpaper` → `~/Pictures/Wallpaper`.
- Обои в профиле — **копия файла** (`<профиль>/wallpaper/<имя>` + `source`).
- Несколько мониторов — одни обои на все; «текущие» = основной экран.
- Акцент из обоев — **отдаём Plasma**, без ImageMagick.

## Что сделано

- [x] Одна реализация каталога в Rust (`wallpaper::wallpaper_dir`, `theme-profile wallpaper dir`);
      бинарники `wallpaper-*` и GUI используют её; дефолт настройки GUI — `""` (авто).
- [x] Копия обоев в профиле при `save` (если сохраняется `plasma_shell`);
      `apply` берёт оригинал, иначе возвращает копию (старый путь → каталог обоев,
      существующие файлы не перезаписываются) и применяет с ретраями после
      старта plasmashell. Старые профили — fallback на `wallpaper` из bundle.
- [x] `theme-profile apply` применяет обои сам; шаг `set_wallpaper` в Python убран.
- [x] Убраны `first_wallpaper_image*` (баг `~/.config/.config`), `wallpaper_accent_hex`,
      `parse_imagemagick_pixel`; поиск текущих обоев — одна функция
      `plasma_wallpaper_from` (основной экран, при активностях — последний `Image=`);
      GUI спрашивает `theme-profile wallpaper current` вместо своего парсера.
- [x] Мультимонитор: одни обои на все, текущие — `lastScreen=0`; тест.
- [x] magick больше не нужен для акцента; без magick/wofi picker сообщает
      «No displayable wallpapers» / «could not start wofi».
- [x] `validate_profile` сообщает об обоях, которых нет ни на диске, ни в копии.
- [x] Тесты: Rust (каталог en/ru/настройка/env, основной экран, активности,
      не-image плагин, восстановление копии, fallback на bundle), bash (CLI
      `wallpaper dir|current`, save копирует, apply восстанавливает, акцент
      отдаётся Plasma), Python (валидация обоев, apply без второго вызова).
- [x] Документация: `README.md`, `docs/CURRENT_APP.md`, `docs/ROLLBACK.md`,
      `gui/qt-app/README.md`, `AGENTS.md`.

## Заметки

- Проверено вживую: Plasma читает `accentColorFromWallpaper` только при старте
  plasmashell; после рестарта акцент пересчитывается и следует за сменой обоев
  (Everforest-обои → `77,154,84`, другие → `90,152,180`). `apply` перезапускает
  plasmashell, поэтому этого достаточно.
- Замечено: `plasma-apply-colorscheme --accent-color X <та же схема>` печатает
  «успешно назначен», но `AccentColor` в kdeglobals не меняется, если до этого
  акцент шёл из обоев; помогла явная запись `kwriteconfig6 --notify`. Явный
  акцент профиля в `apply` идёт через `--accent-color` — при регрессии
  переключения акцента смотреть сюда.
- `save` копирует обои при каждом сохранении (несколько МБ на профиль, плюс
  pre-save бэкап).

## Не входит в задачу

- Новые форматы обоев (видео, анимированные).
- Генерация палитры темы из обоев (matugen-подобное) — отдельная фича.
