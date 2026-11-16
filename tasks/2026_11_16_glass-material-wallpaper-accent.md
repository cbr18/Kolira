# Glass и Material You: акцент с обоев не применяется

Статус: DONE, 2026-11-16.

## Проверка

- Все 8 тем пройдены: в каждом `.config/kdeglobals` ровно один ключ `accentColorFromWallpaper`, дублей `AccentColorFromWallpaper` нет нигде.
- `glass` и `material-you`: `accentColorFromWallpaper=true` + `theme.json values.accent_from_wallpaper: true` + `manifest.json accent_from_wallpaper: true` — акцент с обоев включен. Цветовая схема `BreezeDark` (как было до uncommitted шума).
- `Paper`, `dracula`, `everforest`, `frutiger-aero-light`, `frutiger-aero`, `minecraft-overworld-dark`: кастомные `Kolira<Theme>.colors` возвращены (как было до uncommitted шума), `accentColorFromWallpaper=false` везде — акцент с обоев выключен.
- Все `theme.json` — валидный JSON.
- `CHANGELOG.md` обновлён (раздел Unreleased).

Применение через `kolira apply glass|material-you` теперь запишет `accentColorFromWallpaper=true`, Plasma возьмёт акцент из обоев.

## Причина

В темах `glass` и `material-you` заявлен акцент из обоев, но `theme.json` `values.accent_from_wallpaper` был `false`. При Apply Rust-ядро (`field::apply`, `APPLY_FIRST`) пишет `false` из `theme.json` вライブ `kdeglobals`, перебивая `true`. Plasma перестаёт вычислять акцент из обоев.

Кроме того, в `.config/kdeglobals` были дублирующие ключи `accentColorFromWallpaper` (lowercase, читается Rust) и `AccentColorFromWallpaper` (uppercase).

Кастомные цвета KDE (`Kolira<Theme>.colors`) генерируются автоматически из палитры темы (`palette.rs: kde_scheme_id`, `kde_color_scheme`). Для всех тем кроме `glass` и `material-you` они возвращены в исходное состояние.

## Исправление

1. `themes/glass/theme.json`: `values.accent_from_wallpaper: false` → `true`
2. `themes/material-you/theme.json`: `values.accent_from_wallpaper: false` → `true`
3. `themes/glass/.config/kdeglobals`: убрал дубль `AccentColorFromWallpaper=false`, оставил `accentColorFromWallpaper=true`
4. `themes/material-you/.config/kdeglobals`: то же
5. `themes/{Paper,dracula,everforest,frutiger-aero-light,frutiger-aero,minecraft-overworld-dark}/.config/kdeglobals`: удалены дубли `AccentColorFromWallpaper=false`, оставлен только `accentColorFromWallpaper=false`
6. `CHANGELOG.md` обновлён (раздел Unreleased)

## Связано

- tasks/2026_11_09_glass-font-wallpaper.md (DONE) — предыдущая карточка по теме `glass`.
