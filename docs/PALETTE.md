# Палитра темы (base16 + акцент)

Палитра — единый источник цветов сгенерированной темы. Логика живёт в Rust
(`gui/qt-app/src/core/rust_core/src/palette.rs`), доступ — через
`theme-profile palette …`. План работ: `plans/2026_11_04_09_00_theme-constructor-palette.md`.

## Страница «Палитра» (GUI)

«Тема» → плитка «Палитра» (`qml/pages/PalettePage.qml`, контроллер
`paletteController` = `src/controllers/palette.py` поверх `core/palette_core.py`,
который только вызывает `theme-profile palette …`):

- 16 образцов `base00…base0F` и акцент; клик → выбор цвета → `palette set`;
- предупреждения о контрасте (`palette show --json` → `warnings`);
- **Галерея…** — поиск по встроенным/скачанным схемам, флажок «Онлайн-каталог»;
- **Из обоев** — `Kirigami.ImageColors` на текущих обоях (алгоритм самой Plasma)
  → `palette from-colors`; первое `paletteChanged` после `update()` — сброс
  (пустой список), поэтому ждём непустой, а если за 6 с ответа нет — Rust;
- **Из обоев (подробно)** — `palette from-wallpaper` (k-means);
- **Импорт файла…** — base16 YAML/JSON;
- **Применить к**: схема KDE, kitty, Alacritty (по умолчанию), Kvantum (явно).

Настройка «Сетевые функции» (`network_enabled`) выключает онлайн-каталог и
магазины тем.

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
theme-profile palette import gallery:nord  # схема из галереи
theme-profile palette gallery [ЗАПРОС] [--online] [--refresh] [--json]
theme-profile palette show [--json]        # слоты, акцент, предупреждения о контрасте
theme-profile palette set accent=#a7c080 base0F=#9da9a0
theme-profile palette from-wallpaper [IMAGE] [--variant=dark|light] [--name=…]   # по умолчанию — текущие обои
theme-profile palette from-colors JSON|-   # цвета от Kirigami.ImageColors (GUI)
theme-profile palette generate kde|kitty|alacritty   # напечатать сгенерированное
theme-profile palette apply [--targets=kde,kitty,alacritty,kvantum] [--kvantum-base=THEME]
                                           # по умолчанию kde,kitty,alacritty
```

`apply` пишет `$XDG_DATA_HOME/color-schemes/ThemeProfile<Имя>.colors` (имя с
префиксом — пользовательские схемы вроде `Everforest.colors` не
перезаписываются), выключает `accentColorFromWallpaper` и применяет схему и
акцент. `KTM_THEME_NO_APPLY=1` отключает внешние команды.

**Терминалы.** Как у райсеров: оформление терминала живёт в отдельном файле,
который личный конфиг подключает (`docs/THEMES.md`). kitty: в
`~/.config/kitty/theme-profile.conf` заменяется (или дописывается) блок между
`# BEGIN theme-profile palette …` и `# END theme-profile palette`; в
`kitty.conf` остаются клавиши и прочее личное, а последней строкой —
`include theme-profile.conf` (у kitty побеждает последнее значение). Если в
`kitty.conf` ещё было оформление, оно один раз переносится в
`theme-profile.conf` (оригинал — в `backups/pre-split-kitty-*`). Повторное
применение идемпотентно. Запущенные kitty
перезагружают цвета через `kitty @ set-colors` по всем сокетам
`/tmp/kitty-theme-profile[-<pid>]` (kitty дописывает PID к `listen_on`). Alacritty:
цвета `[colors.*]` пишутся через `toml_edit` в `~/.config/alacritty/theme-profile.toml`
(комментарии сохраняются), `alacritty.toml` получает
`[general] import = ["~/.config/alacritty/theme-profile.toml"]` (или старый
верхнеуровневый `import`, если он уже есть); Alacritty перечитывает файлы сам.
Перед правкой файлы тем копируются в `backups/palette-apply-<время>/`; права
файла сохраняются. Если `kitty.conf`/`alacritty.toml` нет — цель пропускается.

**Konsole** (цель `konsole`, по умолчанию включена, пропускается без Konsole):
генерируется `~/.local/share/konsole/ThemeProfile<Имя>.colorscheme` (те же ANSI-цвета,
что у kitty; прозрачность/размытие прошлой версии схемы сохраняются) и выбирается
в профиле по умолчанию (`konsolerc DefaultProfile`); без профиля создаётся
`Theme Profile.profile` (`Parent=FALLBACK/`) и становится профилем по умолчанию.
Открытые вкладки сохраняют свою схему, новые получают новую.

**Neovim** (цель `nvim`, по умолчанию включена, пропускается без Neovim): как в
дотфайлах (matugen/base16) — `~/.config/nvim/colors/theme-profile.lua` (base16 →
группы подсветки, диагностика, плавающие окна, `terminal_color_0..15`; без
плагинов) и `lua/theme-profile.lua` (выбор схемы; прозрачный фон, если у kitty
темы `background_opacity` < 1); в `init.lua` один раз дописывается последней
строкой `pcall(require, "theme-profile")`. Новые сессии получают схему,
в открытой — `:colorscheme theme-profile`.

**Discord** (цель `discord`, по умолчанию включена; обычный Discord тем не
поддерживает — нужен Vencord или Vesktop): в `themes/` клиента пишется
`theme-profile.theme.css` — цвета палитры поверх популярной темы midnight
(MIT, refact0r; подключается с её сайта, как в шаблоне matugen) — и
включается в `settings/settings.json` `enabledThemes` (точечная правка, прочие
настройки не трогаются). Vencord перечитывает темы сам.

**Kvantum** (цель `kvantum`, включается явно — она переключает стиль виджетов на
`kvantum`). Темы Kvantum — GPL-графика, поэтому в репозитории их нет: при
применении берётся **установленная** базовая тема (по умолчанию `KvArcDark` для
тёмной палитры и `KvArc` для светлой, любую другую — `--kvantum-base=…`), её SVG и
kvconfig копируются в `~/.config/Kvantum/ThemeProfile<Имя>/` с перекраской в
OKLab:

- почти серые цвета сохраняют своё место на шкале «фон → текст» базовой темы и
  переносятся на шкалу палитры (с экстраполяцией: тени и блики не теряют глубину);
- цвета, близкие по тону (±45°) к `highlight.color` базы, становятся `accent`
  с тем же сдвигом яркости и относительной насыщенностью;
- прочие насыщенные (смысловые красный/зелёный) не меняются; альфа сохраняется;
- в `[GeneralColors]` ключи с понятной ролью выставляются явно (окно, списки,
  кнопки, акцент, текст на выделении, ссылки); `text.press/toggle.color`,
  совпадающие с `highlight.text.color` базы, получают текст на выделении.

Затем `kvantummanager --set ThemeProfile<Имя>` и `widgetStyle=kvantum`. Профили
сохраняют **активную** пользовательскую тему Kvantum целиком (каталог из
`~/.config/Kvantum/<theme>`) и восстанавливают её при `apply`.

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

Терминалы (kitty и Alacritty):

| Роль | Источник |
|---|---|
| фон / текст | `base00` / `base05` |
| выделение фон / текст | `accent` / текст на выделении (как в KDE) |
| курсор / текст под курсором | `base05` / `base00` |
| ссылки (kitty `url_color`) | `base0D` |
| рамка активного окна, активная вкладка kitty | `accent` |
| ANSI 0 / 8 (чёрный) | `base02` / `base03` — видны на фоне |
| ANSI 1–6, 9–14 | `base08` `base0B` `base0A` `base0D` `base0E` `base0C` |
| ANSI 7 / 15 (белый) | `base05` / `base07` |

`ColorEffects` берутся из Breeze. Plasma при применении акцента дополнительно
подкрашивает `[Colors:Selection]` своим алгоритмом — это ожидаемо.

## Контраст

`palette show` и генераторы проверяют WCAG-контраст (порог 4.5:1) для текста на
окне, текста на списках и текста на выделении; предупреждения выводятся строками
`warning=…`.

## Палитра из обоев

Модуль `extract.rs`. Два входа, одно правило:

- CLI: картинка (jpeg/png/webp/bmp, крейт `image`, без ImageMagick) уменьшается
  до 96 px, пиксели кластеризуются k-means (k = 8, OKLab, детерминированный
  посев по квантилям яркости);
- GUI: цвета `Kirigami.ImageColors.palette` (тот же алгоритм KDE, что и акцент
  Plasma) передаются в `palette from-colors` как
  `{"colors":[{"color":"#rrggbb","ratio":0.4},…],"variant":…,"name":…,"source":…}`.

Правило раскладки (`palette_from_swatches`):

| Слот | Как получается |
|---|---|
| `variant` | тёмная, если средневзвешенная яркость OKLab < 0.62 (можно задать `--variant`) |
| `base00…base07` | тон преобладающего цвета, насыщенность ×0.35 (≤ 0.035), фиксированные шаги яркости (тёмная: 0.24 → 0.97, светлая — наоборот) |
| `base05` (текст) | дотягивается до контраста ≥ 7:1 к `base00` |
| `accent` | самый заметный насыщенный кластер (вес × насыщенность), не из «семейства фона» (дальше 0.1 OKLab от преобладающего), яркость 0.76/0.55; затем подстраивается, пока текст на выделении не даст ≥ 4.5:1 |
| `base08…base0F` | канонические тона (красный 25°, оранжевый 55°, жёлтый 95°, зелёный 140°, бирюзовый 195°, синий 255°, пурпурный 330°, коричневый 60° темнее), притянутые к тону картинки в пределах ±25° |

Источник записывается в `source` (`wallpaper:<путь>` / `imagecolors…`). Разные
входы (k-means и ImageColors) из одних обоев могут дать немного разные палитры.

## Галерея

`theme-profile palette gallery` (модуль `gallery.rs`) показывает схемы из трёх
источников: `id  origin  variant  name`.

| origin | Откуда | Сеть |
|---|---|---|
| `bundled` | 15 схем, вкомпилированы из `rust_core/data/base16/*.yaml` (Everforest, Gruvbox ×2, Nord, Catppuccin ×2, Tokyo Night, Rosé Pine ×2, Dracula, Solarized ×2, OneDark, One Light, Kanagawa) | нет |
| `cached` | ранее скачанные, `$XDG_CACHE_HOME/theme-profile/base16/<id>.yaml` | нет |
| `online` | каталог tinted-theming (`spec-0.11`, ~360 схем): индекс через GitHub tree API, кэш на 7 дней (`index.json`), `--refresh` — обновить | только с `--online`/`--refresh` |

`palette import gallery:<id>` берёт схему из встроенных → кэша → скачивает
(raw.githubusercontent.com) и кладёт в кэш после проверки разбора. id
проверяется до любого сетевого запроса (`[a-z0-9][a-z0-9._-]*`), ответы
ограничены по размеру. Сеть отключается настройкой `"network_enabled": false` в
`app-preferences.json` (поле `AppSettings.network_enabled`) или `KTM_OFFLINE=1`
(тесты); тогда доступны только встроенные и кэшированные схемы и уже
сохранённый индекс.

Встроенные схемы — копии из tinted-theming (MIT, атрибуция в
`data/base16/README.md`).

## Дальше

P1 закрыт (инструменты MCP — `docs/MCP.md`). Впереди P2 (свой Look & Feel на
профиль, «Начать с…») и P3 (предпросмотр палитры в редакторе) — см. план.
