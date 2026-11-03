# Палитра темы (base16 + акцент)

Палитра — единый источник цветов сгенерированной темы. Логика живёт в Rust
(`gui/qt-app/src/core/rust_core/src/palette.rs`), доступ — через
`theme-profile palette …`. План работ: `plans/2026_11_04_09_00_theme-constructor-palette.md`.

## Модель

- **base16** (`base00`…`base0F`, формат [tinted-theming](https://github.com/tinted-theming/schemes)):
  любую из сотен опубликованных схем можно импортировать как есть.
- **accent** — отдельный цвет акцента KDE (в base16 такого слота нет);
  по умолчанию `base0D`.
- **variant** — `dark`/`light` (из файла; у старого формата — по яркости `base00`).
- **source** — откуда палитра (`base16:<файл>`, `manual`, позже `gallery:`, `wallpaper:`).

Живая палитра: `$XDG_CONFIG_HOME/theme-profile/palette.json`. Профили сохраняют
её как компонент `palette` (`.config/theme-profile/palette.json` в снапшоте).

## CLI

```bash
theme-profile palette import FILE          # base16 YAML (новый/старый формат) или JSON палитры
theme-profile palette show [--json]        # слоты, акцент, предупреждения о контрасте
theme-profile palette set accent=#a7c080 base0F=#9da9a0
theme-profile palette generate kde         # напечатать сгенерированный .colors
theme-profile palette apply [--targets=kde]
```

`apply` пишет `$XDG_DATA_HOME/color-schemes/ThemeProfile<Имя>.colors` (имя с
префиксом — пользовательские схемы вроде `Everforest.colors` не
перезаписываются), выключает `accentColorFromWallpaper` и применяет схему и
акцент. `KTM_THEME_NO_APPLY=1` отключает внешние команды.

> `plasma-apply-colorscheme --accent-color X SCHEME` меняет **только** акцент и
> молча игнорирует схему, поэтому схема и акцент применяются двумя вызовами
> (так же сделано в `theme-profile apply`).

## Соответствие base16 → KDE

| Роль KDE | Тёмная палитра | Светлая палитра |
|---|---|---|
| Window фон / альт. | `base00` / `base01` | `base00` / `base01` |
| View (списки, редакторы) фон | `base00` + 22 % к чёрному | `base00` + 55 % к белому |
| View альт. | `base00` | `base00` |
| Button фон / альт. | `base01` / `base02` | то же |
| Header, Complementary, `[WM] activeBackground` | как View | как View |
| Header][Inactive, `[WM] inactiveBackground` | `base00` | `base00` |
| Tooltip | `base01` / `base00` | то же |
| Текст (`ForegroundNormal`) | `base05` | `base05` |
| Неактивный текст | `base03` | `base03` |
| Ссылка / посещённая | `base0D` / `base0E` | то же |
| Negative / Neutral / Positive | `base08` / `base0A` / `base0B` | то же |
| Selection фон, DecorationFocus, ForegroundActive | `accent` | `accent` |
| DecorationHover | `accent` + 25 % к `base05` | то же |
| Текст на выделении | лучший по контрасту из `base00`/`base07`/`base05` | то же |

`ColorEffects` берутся из Breeze. Plasma при применении акцента дополнительно
подкрашивает `[Colors:Selection]` своим алгоритмом — это ожидаемо.

## Контраст

`palette show` и генераторы проверяют WCAG-контраст (порог 4.5:1) для текста на
окне, текста на списках и текста на выделении; предупреждения выводятся строками
`warning=…`.

## Встроенные схемы

`rust_core/data/base16/*.yaml` — копии из tinted-theming (MIT, см. README в
каталоге); используются в тестах и как офлайн-набор будущей галереи.

## Дальше (P1)

kitty/Alacritty, шаблон Kvantum, галерея, палитра из обоев, GUI-страница,
инструменты MCP — см. план.
