# Полная проработка kolira lint (Kvantum + расширение охвата)

**Статус:** DONE (2026-11-24). Реализовано и проверено.
[Контракт lint](../docs/THEMES.md#kolira-lint-name----json),
[модуль](../gui/qt-app/src/core/rust_core/src/lint.rs).

## Проблема

`kolira lint` проверяет только: затенение иконок, контраст цветовых схем KDE
(`Colors:Window/View/Button/Tooltip/Header`), и «bright black» терминалов
**kitty/alacritty/konsole**. За бортом:

- **Kvantum** — стиль виджетов, где живёт оформление окон/меню/тултипов. Реальный
  баг: светлый `tooltip.text.color` в теме Sandstone (Paper) на светлом фоне →
  нечитаемый тултип Dolphin. `lint` его не видит.
- **ghostty / wezterm / foot** — поставляются во всех bundled-темах, но их
  «bright black»/foreground не проверяется.
- **GTK colors.css** — не проверяется, хотя содержит fg/bg пары.

## Что сделать

Расширить `lint_package` новыми проверками (переиспользуя `parse_color`/
`contrast`), не ломая существующие виды предупреждений:

1. **Kvantum (`kind: "kvantum"`)** — в `[GeneralColors]` kvconfig темы:
   - `tooltip.text.color` на `tooltip.base.color` (fallback на `window.color`);
   - `text.color` на `window.color`;
   - `highlight.text.color` на `highlight.color`;
   - `disabled.text.color` не проверять (он всегда низкоконтрастный намеренно).
   Порог: `MIN_TEXT` для основных, отдельный мягкий порог для tooltip/highlight.
2. **Терминалы ghostty/wezterm/foot (`kind: "terminal"`):**
   - ghostty: `background`, `foreground`, `palette = 8=` (bright black);
   - wezterm: `colors.background`, `colors.foreground`, `colors.brights[0]`;
   - foot: `[colors-dark]`/`[colors-light]` `background`/`foreground`/`bright0`
     (bare hex без `#` — расширить `parse_color`).
   Порог: `MIN_TEXT` для foreground, `MIN_DIM` для bright black.
3. **GTK colors.css (`kind: "gtk"`):**
   - `theme_fg_color_breeze` на `theme_bg_color_breeze`;
   - `theme_selected_fg_color_breeze` на `theme_selected_bg_color_breeze`.
   Порог: `MIN_TEXT`. Только если оба ключа есть.

## Приёмка

- [x] `lint` ловит светлый Kvantum-тултип (регрессионный тест на подобие Sandstone).
- [x] ghostty/wezterm/foot bright-black и foreground проверяются (тесты).
- [x] GTK fg/bg пары проверяются (тест).
- [x] Существующие проверки (icons/colors/terminal kitty/alacritty/konsole) не
      сломаны; старые тесты проходят.
- [x] `cargo clippy -D warnings` + `cargo fmt --check` чисты; весь набор тестов зелёный.
- [x] Обновлены `docs/THEMES.md` (описание lint), `CHANGELOG.md`; карточка и
      `tasks/README.md`.
- [x] Прогон `kolira lint` на всех 9 bundled-темах: 0 ложных срабатываний на
      корректных темах (или осознанно задокументированные).

## Границы

- Линт проверяет **контраст данных**, не финальный рендер (Kvantum может иметь
  SVG-художественный фон, который не читается из kvconfig). Пороги консервативные,
  чтобы не плодить ложные срабатывания.
- Проверка ограничена файлами, которые есть в пакете темы; внешние темы (Sandstone
  по имени) линтятся только если их kvconfig попал в снимок.