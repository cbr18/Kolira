# Тема = только внешний вид + автоустановка зависимостей

Status: DONE (2026-11-04) — часть 1 (только оформление) и часть 2 (зависимости + окно установки)
Блокирует: [publish-repo-with-themes](2026_11_04_10_10_publish-repo-with-themes.md)

## Проблема

Тема хранит и восстанавливает файлы **целиком**, а почти каждый файл смешанный:
оформление лежит рядом с личными настройками. Чужая тема (и даже переключение
своих) перезаписывает личное.

## Аудит (2026-11-04, живые конфиги)

| Файл | Оформление | Личное (сейчас перезаписывается) |
|---|---|---|
| `kdeglobals` | `[Colors:*]`, `[ColorEffects:*]`, `[WM]`, `[Icons]`, шрифты, `General` ColorScheme/Accent*, `KDE` LookAndFeelPackage/widgetStyle/contrast/AnimationDurationFactor | **`[KScreen]` масштаб экрана**, `TerminalApplication`, `[KFileDialog Settings]`, `[PreviewSettings]`, поведение полос прокрутки |
| `kwinrc` | `[Effect-*]`, `[org.kde.kdecoration2]`, эффекты в `[Plugins]` | `[Desktops]` (рабочие столы), `[Tiling]` (раскладки), `[Windows]` (прилипание), `[Wayland]`, **скрипты** в `[Plugins]` (krohnkite, sticky-window-snapping) |
| `kwinrulesrc` | правила прозрачности/рамок/схемы | правила положения/размера/рабочего стола |
| `plasmarc` | `[Theme]` | `[Wallpapers] usersWallpapers` |
| `plasmashellrc` | `[PlasmaViews]` (панели: прозрачность, плавающая, толщина) | `[Updates]` |
| `appletsrc` | панели и виджеты | значки рабочего стола (уже сохраняются живые) |
| `gtk-3/4 settings.ini` | тема, иконки, курсор, шрифт, dark, кнопки заголовка | мигание курсора, звуки, `gtk-modules`, `primary-button-warps-slider` |
| `kitty.conf` | `font_*`, цвета (`color0-255`, fg/bg, cursor, selection, url, tab/border), `background_opacity/blur`, отступы | **`map` (клавиши)**, `scrollback_lines`, `listen_on`, `allow_remote_control`, `term`, `confirm_os_window_close`, размеры окна |
| `alacritty.toml` | `[colors.*]`, `[window]` opacity/blur/padding/decorations, `[font]` | `[env]`, **`[[keyboard.bindings]]`**, `[terminal]` |
| `btop.conf` | `color_theme`, `theme_background`, `truecolor`, `rounded_corners`, `graph_symbol*` | ~70 ключей поведения (сортировка, датчики, диски, сеть, GPU) |
| VS Code / Cursor `settings.json` | `workbench.colorTheme`, `workbench.colorCustomizations`, `editor.tokenColorCustomizations`, `workbench.iconTheme`, шрифты | **SSH-хост**, AI-автоподтверждение, переносы, inlay hints, рекомендации расширений |
| Zed `settings.json` | `theme`, `icon_theme`, `ui_font_*`, `buffer_font_*`, `experimental.theme_overrides` | **agent / модели**, панели, keymap, сессии, доверие |
| Zen `user.js` | весь файл (пишет только наше приложение) | — |
| целиком-оформление | `breezerc`, `klassy/*`, GTK `gtk.css`/`colors.css`, `wofi/*.css`, Zen `chrome/*.css`, Obsidian `appearance.json`, `Trolltech.conf`, Kvantum, цветовые схемы, L&F, fastfetch (вывод = внешний вид) | — |

## Решение (предложение)

1. **Фильтры оформления на файл** (декларативная таблица в Rust, общая для
   save/apply/export): режим `whole` | `ini` (разрешённые группы/ключи) |
   `kv` (kitty: префиксы директив) | `toml` (таблицы) | `jsonc` (ключи,
   позиционный патч — контракт JSONC) | `btop` (ключи).
   - **save**: в тему попадает только разрешённое;
   - **apply**: разрешённое **вливается** в живой файл, остальное не трогается
     (масштаб, клавиши, SSH, рабочие столы, раскладки остаются свои).
   - Пример райсеров: цвета выносят в отдельный файл и подключают `include`
     (kitty `include`, alacritty `general.import`, waybar/rofi `@import`) —
     можно использовать там, где merge неудобен.
2. **Зависимости с источниками** (`theme.json` → `dependencies[]`): тип, имя,
   где взять, как поставить. Источники определяются при сохранении
   автоматически:
   - KDE Store — из реестров KNewStuff `~/.local/share/knewstuff3/*.knsregistry`
     (id в api.kde-look.org уже записан KDE при установке): иконки, курсоры,
     GTK, цветовые схемы, стили Plasma, L&F, Aurorae, **виджеты панели**;
   - VS Code Marketplace / Open VSX / Zed — из установленных расширений (есть);
   - пакеты — `pacman -Qo` для системных путей (`/usr/share/icons/…`) → имя пакета;
   - иначе — ссылка поиска KDE Store.
3. **Окно при импорте/применении**: «Для темы нужно: … Установить?» —
   отмеченное ставится автоматически:
   - KDE Store → штатно через `kns://<knsrc>/api.kde-look.org/<id>`
     (`knewstuff-dialog6`: скачивание, распаковка, регистрация, удаление в
     «Параметрах системы») или напрямую через OCS API — выбрать;
   - VS Code/Cursor/Zed → из магазинов (есть);
   - пакеты → показать команду (`pacman`/`yay`), без sudo из GUI.
4. Проверка: применить чужую тему на временном HOME с «личными» значениями
   (масштаб, клавиши kitty, SSH-хост, рабочие столы) — они должны остаться.
   Регрессия: применение своей темы по-прежнему совпадает со снимком по всему
   оформлению.

## Итог части 2 (2026-11-04)

- `rust_core/src/deps.rs`: источники — реестры KNewStuff (точный id), pacman
  (репозиторий/AUR, с учётом пользовательской копии поверх пакетной), магазины
  редакторов, поиск OCS с ранжированием (аббревиатура, KDE/Plasma > форки) как
  кандидат на подтверждение. Установка: OCS → `kpackagetool6` (стили, L&F,
  виджеты) или распаковка (иконки, курсоры, GTK, Kvantum, Aurorae, схемы);
  пакеты — только команда. Проверено в чистом HOME: YAMIS и Simple Kickoff
  скачались и встали.
- `kcminputrc` отслеживается (только курсор).
- CLI `deps`, отчёты apply/import, окно «Теме нужно» с галочками, MCP
  `list_dependencies` / `install_dependencies`, тесты офлайн.
