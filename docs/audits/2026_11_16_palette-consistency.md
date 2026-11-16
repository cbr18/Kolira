# Палитра и настройки приложений: восемь тем

Дата: 2026-11-16. Задача: [исправление согласованности](../../tasks/2026_11_16_palette-consistency.md).
Проверяются Paper, dracula, everforest, frutiger-aero, frutiger-aero-light, glass,
material-you, minecraft-overworld-dark; repo и подготовленные локальные пакеты.

## Найдено

1. Everforest GTK3/4 `colors.css` содержал чужие красно-розовые Breeze definitions:
   `theme_selected_bg_color_breeze=#943147`, header=#97354b, base=#1f1115.
   Это не `palette.accent=#a7c080`. Аналогичные stale определения были в других
   пакетах. Firefox system theme мог наследовать их.
2. Firefox snapshot задавал day/night, а не акцент. Zen задавал toolbar/icon
   variables через CSS. Прежние проверки контраста/возврата не проверяли
   соответствие native accent палитре; исправлены реализация и сами проверки.
3. Alacritty/Ghostty palette resources обновлялись не во всех пакетах. Часть
   literal цветов и inline colour tags OMP оставалась от прежней схемы.
4. KDE values могли расходиться с palette.accent (например Paper), а
   accent-from-wallpaper мог переопределить акцент уже рассчитанной палитры.

## Исправление

Maintenance-пример обновляет named GTK/Breeze definitions, активный KDE colour
scheme/colour fields, Alacritty/Ghostty, OMP и обе партии app resources. Alpha
значения GTK и произвольные widget rules не заменяются. KDE profiles используют
собственную сгенерированную схему. Для комплектных пакетов акцент явно берётся
из палитры; повторная генерация из текущих обоев при Apply выключена.

Браузеры получают native `ui.accentcolor`, `ui.accentcolortext` и day/night,
Zen также `zen.theme.accent-color`. Toolbar, кнопки, надписи, icon fill/stroke,
workspace gradients, личный CSS не переписываются. CSS support не включается.
`prefs.js` не редактируется; reset отсутствующего исходного native override
выполняется через user.js и проверяется настоящими браузерами.

## Матрица проверок

Каждая строка ниже проверяется для **всех восьми пакетов**, а не только Everforest.
`rust_core/tests/first_ten.rs` проверяет конечные конфиги после импорта/Apply и
возврат к теме без этих конфигов. Личные размеры шрифта, команды, permissions и
прочие исходные настройки восстанавливаются отдельно от совпадения цветов.

| Приложение / ресурс | Проверяется |
|---|---|
| Firefox, Zen | Реальные computed AccentColor/AccentColorText, native day/night, штатный icon contrast, disabled CSS, baseline → 8 тем → neutral |
| GTK3/GTK4 | Named selection accent соответствует палитре, stale красный отсутствует; отдельная регрессия alpha и сохранения widget rules |
| KDE | Активные AccentColor/Selection совпадают с palette.accent; собственный ресурс схемы |
| Cursor / VS Code snapshot | focusBorder и editor colours; для Cursor отдельно cache/registry/возврат |
| Zed | Собственная JSON theme, text.accent, выбор темы и сохранение личных шрифтов |
| Neovim | Палитра JSON, фиксированный loader, colorscheme и сохранение init |
| Obsidian | Собственный snippet, палитра/акцент; сохранение размеров шрифта и личных snippets |
| Discord/Vesktop | Собственный CSS accent и его выбор, сохранение themeLinks/личных настроек |
| Kitty | Active border из акцента, ANSI/resource/include, сохранение шрифта и bindings |
| Konsole/Yakuake | Выбор схемы из default profile и её цвет фона; открытые сессии не переключаются целиком |
| Alacritty | colors.selection.background из акцента; host/font сохраняются |
| Ghostty | Palette resource/include, акцент, личный host сохраняется |
| fish / Pure | Цветовые переменные из base16, без замены функций/prompt |
| Oh My Posh | Literal/inline palette colours из палитры; контрастные чёрный/белый разрешены; структура/текст сохраняются |
| btop | Собственная native theme, цвета из палитры; polling не меняется |
| Cava | foreground/gradient из палитры; input/audio сохраняются |
| Yazi | Native styles из палитры; icon/filetype rules сохраняются |
| Wofi | Отдельные colour CSS/import; без потери поздней геометрии |
| fzf | --color из палитры, pointer=accent; personal options/binds сохраняются |
| lazygit | gui.theme.activeBorderColor=accent; остальной YAML сохраняется |
| Zellij | KDL resource/selectors из палитры, keybindings/plugins сохраняются |
| OpenCode V2 | Native system/ANSI mode: palette терминала + day/night, без отдельного custom theme JSON |
| Claude Code | Native ANSI/day-night, сохранение accessibility; не полномасштабная перекраска TUI |
| Codex | Native tmTheme syntax highlighting + selector; не палитра всего TUI |

## Границы

Нативные AI/ANSI режимы наследуют палитру терминала; они не объявляются отдельными
полными GUI skins. Firefox theme extension и личные Zen gradients могут иметь
собственные цвета: выбор расширений/workspaces не меняется.

Kvantum/Plasma/decorations иконки — отдельные явно выбранные style/library assets.
Например Paper использует Sandstone, glass/material-you — OrchisDark; их SVG и
формы не являются base16-шаблонами. KDE/GTK palette values проверяются отдельно;
этот аудит не объявляет побайтного совпадения всех цветов стороннего SVG с base16.
Fastfetch/Steam сохраняют свои исключения; остальные строки очереди не становятся
реализованными от присутствия в реестре. Программы, отсутствующие на машине,
не устанавливаются и не получают новые личные конфиги только ради проверки.

Проверка файлов/селекторов остальных приложений не объявляется визуальной
проверкой каждого работающего окна. Личные Firefox/Zen профили и вкладки не
использовались в native tests. Для чтения user.js работающему браузеру нужен
штатный перезапуск. Итоговые логи/счётчики ведутся в карточке задачи.


## Инвентаризация всех 41 записей реестра

Наличие snapshot — проверка ресурса в пакете, а не обещание visual/live reload.
«—» означает отсутствие отдельного снимка: личный конфиг сохраняется; программа
может наследовать системный стиль. Obsidian генерируется для выбранного vault.

| Запись | Paper | dracula | everforest | frutiger-aero | frutiger-aero-light | glass | material-you | minecraft-overworld-dark |
|---|---|---|---|---|---|---|---|---|
| KWin | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| Plasma Shell | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| KDE Globals | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| GTK3 | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| GTK4 | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| Qt5/Qt6 Style | снимок | снимок | снимок | снимок | снимок | снимок | снимок | — |
| Breeze style | снимок | — | снимок | — | — | — | — | — |
| Klassy | — | снимок | снимок | — | — | снимок | снимок | снимок |
| Kvantum | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| Kitty | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| Alacritty | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| Neovim | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| Wofi | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| btop | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| Zed | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| VS Code | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| Cursor | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| Zen Browser | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| Discord (Vencord / Vesktop) | — | — | — | снимок | снимок | — | — | — |
| Spotify (Spicetify) | — | — | — | — | — | — | — | — |
| oh-my-posh (prompt) | снимок | снимок | снимок | снимок | снимок | снимок | снимок | — |
| fish colours | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| Pure colours | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| fzf | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| lazygit | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| Zellij | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| OpenCode | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| Claude Code | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| Codex | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| Ghostty | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| cava | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| yazi | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| bat | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| Konsole / Yakuake | снимок | снимок | снимок | снимок | снимок | снимок | снимок | — |
| Firefox | снимок | снимок | снимок | снимок | снимок | снимок | снимок | снимок |
| Obsidian | vault | vault | vault | vault | vault | vault | vault | vault |
| Color Schemes | — | — | снимок | — | — | — | — | — |
| Palette | палитра | палитра | палитра | палитра | палитра | палитра | палитра | палитра |
| Look & Feel | — | снимок | снимок | — | — | снимок | снимок | — |


Итог: 263 Rust / 345 Qt / 260 shell, clippy/линтеры PASS; real Firefox/Zen по
8 темам + возврат PASS, native GTK 19 CSS / 76 contrast pairs (min 5.51:1) PASS.
Docker 8 тем + GUI ALL PASSED. Локальные восемь пакетов обновлены с backup,
Everforest применён, native accent/GTK/KDE #a7c080 подтверждён на живых файлах.
Браузеры не закрывались: работающим процессам может понадобиться перезапуск.
