# Theme Inspector — экран «Тема»: UX-анализ и архитектура

> **Статус**: реализовано (Phases 0–3) в `gui/qt-app/` — страница **Theme**,
> read-only скопы профилей, live-правки с Undo/Revert, экспорт JSON.
> **Связано**: [ROADMAP.md](ROADMAP.md), [CURRENT_APP.md](CURRENT_APP.md), [TZ-QT-APP.md](TZ-QT-APP.md)
> **Вопрос**: как показать, что внутри темы, и дать править это прямо в приложении.

---

## 1. Короткий вывод

1. **Отдельный экран нужен** — но не «таблица всех ключей», а **обзор из 6 смысловых групп + углубление в раздел + Advanced для сырых файлов**. (M3: углубление — отдельная страница в стеке слоёв, а не аккордеон.)
2. **Редактировать — да, но только «Live»** (текущую систему), а профили остаются снимками. Профиль можно смотреть и сравнивать, но не «править вслепую».
3. **Стейджинг + Apply + Undo**: правки видны сразу, записываются одним батчем; для тяжёлых операций (L&F, Kvantum, иконки) — подтверждение и бейдж «нужен рестарт».
4. **Главная ценность — не редактирование, а видимость и валидация**: сейчас в системе уже два неверных значения (см. §14), и приложение их не видит. «Theme Doctor» на этом и строится.
5. **MVP = read-only инспектор** (69 полей, поиск, diff live↔profile, ссылки в системные настройки). Редактирование — вторая фаза.

---

## 2. Что есть сейчас (аудит)

| Поверхность | Что показывает | Чего не хватает |
|---|---|---|
| Карточка профиля | имя, свотчи палитры, отпечаток темы (accent/Kvantum/blur/радиус), дата сохранения, размер, число компонентов, кнопки Apply/Preview/Diff/Inspect/Delete | что *внутри* по каждому полю (есть в Theme) |
| `Preview` | список компонентов + сырой `diff -ru` на 4000+ строк | человекочитаемые поля «было → станет» |
| `theme-profile status` | ~40 плоских `key=value` | значения с типами, дефолты, валидация; часть строк битая (JSON/zed) |
| Settings | настройки *приложения* | настройки *темы* |
| System Settings | всё, но по разным kcm | единого взгляда «что такое моя тема» |

Размер материала: в профиле `glass` — **91 файл, ~3.4 MB**, в `paper` — 92 файла; `kdeglobals` — 187 строк, `kwinrc` — 232. Показывать всё списком бессмысленно: нужна **кураторская модель + поиск**.

---

## 3. Что «внутри темы»

### 3.1. Карта источников (20 компонентов)

| Компонент | Файлы | Что там реально лежит |
|---|---|---|
| `kdeglobals` | `~/.config/kdeglobals` | палитра (Window/View/Button/Selection/Header/Tooltip), accent, ColorScheme, LookAndFeelPackage, widgetStyle, иконки, шрифты, AnimationDurationFactor |
| `kwin` | `kwinrc`, `kwinrulesrc` | включённые эффекты, параметры blur/contrast/translucency, декорации, правила окон |
| `plasma_shell` | `plasmarc`, `plasmashellrc`, `...appletsrc` | тема Plasma, панели, виджеты, обои |
| `klassy` | `klassy/klassyrc`, `windecopresetsrc` | прозрачность заголовка, радиус углов, тени, кнопки |
| `kvantum` | `Kvantum/kvantum.kvconfig`, `Kvantum/<Theme>/` | активная тема + десятки флагов (blurring, translucent_windows, отступы) |
| `gtk3` / `gtk4` | `gtk-3.0/settings.ini`, `gtk.css`, `colors.css` | тема, иконки, курсор, шрифт, CSS-переопределения |
| `qt5ct` | `Trolltech.conf` | палитра Qt-приложений (`QtProject.conf` — состояние диалога файлов, не отслеживается) |
| `kitty` / `alacritty` | `kitty/theme-profile.conf`, `alacritty/theme-profile.toml` (подключаются из `kitty.conf` / `alacritty.toml`) + themes | прозрачность, blur, цвета, шрифт |
| `wofi` | `wofi/style.css`, `wallpaper.css` | цвета меню через `@define-color` |
| `btop` | `btop.conf`, `themes/*.theme` | тема, скругления |
| `zed` / `vscode` / `cursor` | `settings.json` | active theme (light/dark/mode) |
| `zen` | `user.js`, `userChrome.css`, `userContent.css` | prefs прозрачности/темы, включая `zen.view.window.scheme` (0 dark, 1 light, 2 system) |
| `obsidian` | `appearance.json`, `Preferences` | тема, cssTheme |
| `fastfetch` | `config.jsonc`, `logo` | логотип/конфиг |
| `color_schemes` | `~/.local/share/color-schemes/*.colors` | сами схемы |
| `look_and_feel` | `~/.local/share/plasma/look-and-feel/*` | пакеты L&F |

### 3.2. Семь групп для UI

Группировка не по приложениям, а по **тому, что пользователь хочет изменить**:

1. **Colors** — accent, схема, роли палитры, contrast, widget style.
2. **Typography** — UI/fixed/small/menu/title шрифты, GTK-шрифт.
3. **Windows & Effects** — декорации, Klassy (радиус/опасити), blur/contrast/translucency, анимации.
4. **Desktop & Assets** — тема Plasma, L&F, иконки, курсоры, обои, splash.
5. **Transparency** — сводка «где ещё есть прозрачность»: kitty, alacritty, Kvantum, Zen, Obsidian, btop.
6. **Applications** — 20 компонентов: статус, 1–3 ключевых значения, открыть конфиг.
7. **Advanced** — сырые ключи, файлы, поиск по всему, валидация, diff live↔profile.

### 3.3. Кураторский набор полей (MVP)

Тип: `color | bool | int | float | enum | font | path | string`.
«Рестарт»: `—` live, `new` новые окна/приложения, `app` перезапуск приложения, `shell` plasmashell.

**Colors**

| Поле | Источник | Тип | Применение | Рестарт |
|---|---|---|---|---|
| Accent color | `kdeglobals [General] AccentColor` | color | `plasma-apply-colorscheme -a '#rrggbb' <scheme>` | — |
| Accent from wallpaper | `[General] accentColorFromWallpaper` | bool | `kwriteconfig6 --notify` | — |
| Color scheme | `[General] ColorScheme` | enum | `plasma-apply-colorscheme <name>` | — |
| Window background | `[Colors:Window] BackgroundNormal` | color | `kwriteconfig6 --notify` | — |
| View background | `[Colors:View] BackgroundNormal` | color | то же | — |
| Button background | `[Colors:Button] BackgroundNormal` | color | то же | — |
| Text | `[Colors:Window] ForegroundNormal` | color | то же | — |
| Dim text | `[Colors:Window] ForegroundInactive` | color | то же | — |
| Selection bg / text | `[Colors:Selection] BackgroundNormal / ForegroundNormal` | color pair | то же | — |
| Link | `[Colors:*] ForegroundLink` | color | то же (с «синхронизировать во всех группах») | — |
| Negative / Neutral / Positive | `[Colors:*] Foreground*` | color | advanced | — |
| Header background | `[Colors:Header] BackgroundNormal` | color | то же | — |
| Tooltip bg / text | `[Colors:Tooltip]` | color pair | то же | — |
| Contrast | `[KDE] contrast` | int 0–7 | то же | — |
| Frame contrast | `[KDE] frameContrast` | float 0–1 | то же | — |
| Widget style | `[KDE] widgetStyle` | enum (`Breeze`, `kvantum`, `Fusion`…) | `kwriteconfig6 --notify` | app |
| Inactive color effect | `[ColorEffects:Inactive]` | bool + color | advanced | — |

**Typography**

| Поле | Источник | Тип | Применение | Рестарт |
|---|---|---|---|---|
| UI font | `[General] font` | font | `kwriteconfig6 --notify` | — / app |
| Fixed font | `[General] fixed` | font | то же | — |
| Small font | `[General] smallestReadableFont` | font | то же | — |
| Menu font | `[General] menuFont` | font | то же | — |
| Toolbar font | `[General] toolBarFont` | font | то же | — |
| Window title font | `[WM] activeFont` | font | то же | new |
| GTK font | `gtk-3.0/settings.ini gtk-font-name` (+ gtk4) | font | atomic write | app |

Шрифт в KDE — составная строка (`Family,10,-1,5,50,...`). Редактируем **family + size**, остальные поля сохраняем из текущего значения (см. §10).

**Windows & Effects**

| Поле | Источник | Тип | Применение | Рестарт |
|---|---|---|---|---|
| Decoration library | `kwinrc [org.kde.kdecoration2] library` | enum | config + `qdbus6 … reconfigure` | new |
| Decoration theme | `… theme` | enum (зависит от library) | то же | new |
| Border size | `… BorderSize` | enum (`None/Tiny/Normal/Large/Huge`) | то же | new |
| Klassy active titlebar opacity | `klassyrc [TitleBarOpacity] ActiveTitleBarOpacity` | int 0–100 | config + reconfigure | new |
| Klassy inactive titlebar opacity | `… InactiveTitleBarOpacity` | int 0–100 | то же | new |
| Klassy corner radius | `klassyrc [Windeco] WindowCornerRadius` | int 0–30 | то же | new |
| Klassy shadow | `klassyrc [ShadowStyle] ShadowSizeActive` | enum | то же | new |
| Blur (better_blur_dx) | `kwinrc [Plugins] better_blur_dxEnabled` | bool | `loadEffect` / `unloadEffect` | — |
| Blur strength | `[Effect-better-blur-dx] BlurStrength` | int 0–20 | `reconfigureEffect better_blur_dx` | — |
| Blur contrast | `Contrast` | int | то же | — |
| Blur decorations / docks / menus | bool ×3 | то же | — |
| Blur matching / non-matching | bool ×2 | advanced | — |
| Refraction strength / edge / pow | int ×3 | advanced | — |
| Blur window classes | string, только чтение | — | — |
| Legacy blur | `[Plugins] blurEnabled` | bool + конфликт-варнинг | load/unload | — |
| Contrast effect | `[Plugins] contrastEnabled` | bool | то же | — |
| Translucency effect | `[Plugins] translucencyEnabled` | bool | то же | — |
| Translucency: inactive / menus / dialogs / popups / combobox / moveresize | `[Effect-translucency]` | int 0–100 ×6 | config + reconfigureEffect | — |
| Animation speed | `kdeglobals [KDE] AnimationDurationFactor` | float 0–2 | `kwriteconfig6 --notify` | — |
| Window rules | `kwinrulesrc` | count + список | только чтение + `kcmshell6 kcm_kwinrules` | — |

**Desktop & Assets**

| Поле | Источник | Тип | Применение | Рестарт |
|---|---|---|---|---|
| Plasma theme | `plasmarc [Theme] name` | enum | `plasma-apply-desktoptheme <name>` | — |
| Look & Feel | `kdeglobals [KDE] LookAndFeelPackage` | enum | `plasma-apply-lookandfeel -a <name>` (confirm!) | — |
| Icon theme | `[Icons] Theme` | enum | `kwriteconfig6 --notify` + `kbuildsycoca6` | app |
| Cursor theme | `kcminputrc [Mouse] cursorTheme` | enum | `plasma-apply-cursortheme <name>` | — |
| Cursor size | `kcminputrc [Mouse] cursorSize` | int 16–96 | `plasma-apply-cursortheme --size` / config | — |
| Wallpaper | Plasma config | path | существующий `wallpaper-*` | — |
| Splash screen | `ksplashrc [KSplash] Theme` | enum | config | shell |
| GTK theme | `gtk-3.0/settings.ini gtk-theme-name` | enum | atomic write | app |
| GTK prefer dark | `gtk-application-prefer-dark-theme` | bool | то же | app |
| GTK icons | `gtk-icon-theme-name` | enum | то же (или «синхронизировать с KDE») | app |

GTK-тема, «prefer dark», тема иконок и тема курсора при записи зеркалируются в
оба файла — `gtk-3.0/settings.ini` и `gtk-4.0/settings.ini` (`gtk-theme-name`,
`gtk-application-prefer-dark-theme`, `gtk-icon-theme-name`,
`gtk-cursor-theme-name`): модуль KDE gtkconfig синхронизирует GTK только при
изменениях из «Параметров системы». GTK-шрифт не зеркалируется — размер личный.
| Kvantum theme | `Kvantum/kvantum.kvconfig [General] theme` | enum c вариантами | `kvantummanager --set <theme>` | app |
| Kvantum translucent windows | `<Theme>.kvconfig translucent_windows` | bool | atomic write | app |
| Kvantum blurring / popup blurring | bool ×2 | то же | — |
| Kvantum window / menu transparency | `reduce_window_opacity` / `reduce_menu_opacity` | int 0–90 | то же | app |
| Kvantum see-through file views | `[Hacks] transparent_dolphin_view` | bool | то же | app |
| Kvantum: no window texture | `no_window_pattern` | bool | то же | app |
| Kvantum composite / animate / spacing / margins / shadows | advanced | app |

**Applications** (ключевое на компонент)

| Компонент | Что показываем/правим | Применение |
|---|---|---|
| kitty | `background_opacity` 0–1, `background_blur` 0–64, `font_size`, include-тема | `kitty @ load-config` + `set-background-opacity` (уже есть) |
| alacritty | `opacity`, `blur`, тема | restart |
| btop | `theme` (enum из `btop/themes`), `rounded_corners` | restart |
| zed | `theme.mode`, `theme.light`, `theme.dark` | live (Zed перечитывает) |
| VS Code / Cursor | `workbench.colorTheme` | live |
| Zen | prefs из `user.js`: transparency, opacity, color, `zen.view.window.scheme` | restart |
| Obsidian | `theme`, `cssTheme` | reload |
| wofi | цвета `@define-color` | при следующем открытии |
| fastfetch | `logo`, путь конфига | при следующем запуске |
| plasma_shell | только чтение: панели/виджеты/обои | — |
| qt5ct | `style`, `color_scheme_path` | restart |

**Transparency (сводка)**

Отдельного хранилища нет — это агрегат: blur/translucency из KWin, Klassy opacity, kitty/alacritty opacity, Kvantum `translucent_windows`, Zen `bg_opacity`, btop. Показываем одну карточку «где прозрачность» + быстрые ссылки на поля. Это отвечает на вопрос «почему одно окно прозрачное, а другое нет».

---

## 4. Принципы UX

1. **Сначала ответ, потом ключи.** Сверху — «какая у меня тема» (палитра, обои, 4 факта), ниже — разделы. Сырые ключи — последний уровень.
2. **Progressive disclosure.** Обзор → раздел → поле → «показать все ключи файла». Никогда не вываливать 4600 строк.
3. **Осмысленные контролы, не текст.** Enum — из реально установленных значений; цвет — палитра; число — слайдер с единицами; шрифт — поле семейства + SpinBox размера (ComboBox на сотнях семейств подвешивает Qt 6.12).
4. **Live — это холст, профиль — снимок.** Не смешивать: редактируем только live; профиль можно смотреть, диффить и применять. Иначе непонятно, что произойдёт.
5. **Стейджинг + Apply + Undo.** Правка сразу видна в поле, но записывается батчем по Apply (слайдеры — с дебаунсом 300 мс). Undo работает и после Apply, «Revert all» после подтверждения возвращает снимок сессии.
6. **Видимость цены.** Бейджи: «live», «new windows», «restart app», «restart shell», «требует L&F-пакет». Пользователь заранее знает, что не применится мгновенно.
7. **Валидация — фича.** Если значение ссылается на несуществующий пакет/тему/шрифт — оранжевый бейдж и кнопка «Fix». Это ловит битые конфиги (см. §14).
8. **Всегда есть выход.** В каждом разделе — «Открыть в System Settings» (`kcmshell6 kcm_…`) и «Открыть файл». Мы не обязаны переизобретать все kcm.

---

## 5. Информационная архитектура

### 5.1. Навигация

```
Drawer
├── Profiles      — переключение (как сейчас)
├── Theme         — NEW: обзор + разделы
└── Settings      — настройки приложения
```

Сквозные точки входа:
- Карточка профиля → **Inspect** (иконка глаза) → Theme в scope `profile:glass`.
- Карточка профиля → строка «DNA» под именем (см. §5.2).
- Тост после Apply → «Посмотреть, что изменилось» → Theme, diff.

### 5.2. Profiles: строка «DNA» на карточке

Дёшево и сразу отвечает «что внутри»:

```
Glass                                    ✓ active
#808e47 · OrchisDark · blur 11 · r10 · 0.5
[saved 2026-01-15 · 3.4 MB · 24 components]   [Apply] [Preview] [Diff] [Inspect] [Delete]
```

Считается из тех же полей схемы (accent, kvantum, blur, radius, kitty opacity). Если поле недоступно — пропускаем. Дублирующие чипы компонентов убраны: все профили отслеживают один и тот же набор, поэтому чипы ничего не сообщали.

### 5.3. Theme — обзор

```
┌ Theme ─────────────────────────────── [Live ▾] [glass] [paper] ──┐
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │ [обои thumbnail]   Theme: glass                              │  │
│  │                    Accent ■ #808e47   Kvantum  OrchisDark    │  │
│  │                    Blur 11 · Contrast 80   Radius 10         │  │
│  │                    ⚠ 1 issue: Look & Feel package not found  │  │
│  │  palette: ■■■■■■■■■■■■                                       │  │
│  └─────────────────────────────────────────────────────────────┘  │
│  ┌ Colors ────────────┐  ┌ Typography ────────┐                  │
│  │ Breeze1 · #808e47  │  │ Noto Sans 10       │                  │
│  │ 12 roles           │  │ fixed 9 · title 10 │                  │
│  └────────────────────┘  └────────────────────┘                  │
│  ┌ Windows & Effects ─┐  ┌ Desktop & Assets ──┐                  │
│  │ Klassy · r10       │  │ L&F glass ⚠        │                  │
│  │ blur 11 · transl 97│  │ YAMIS · Bibata     │                  │
│  └────────────────────┘  └────────────────────┘                  │
│  ┌ Applications ──────┐  ┌ Transparency ──────┐                  │
│  │ 14 themed / 20     │  │ kitty .5 · Kvantum │                  │
│  └────────────────────┘  └────────────────────┘                  │
│  ┌ Advanced ──────────────────────────────────────────────────┐   │
│  │ Search all fields…  · Validation report · Raw files         │   │
│  └────────────────────────────────────────────────────────────┘   │
└───────────────────────────────────────────────────────────────────┘
```

Scope-переключатель:
- **Live** — текущая система (редактируемая).
- **Profile: glass** — снимок (read-only), с бейджами «отличается от live» на полях.

### 5.4. Theme — раздел (пример: Windows & Effects)

```
← Windows & Effects                         [Search in section…]
Decoration
  Library            [Klassy ▾]                     [open kcm]
  Border size         [None ▾]
  Titlebar opacity    ●──────── 50 %   (active)
                      ●──────── 48 %   (inactive)   [restart windows]
  Corner radius       ───●────── 10 px
  Shadow              [Small ▾]
Blur
  Better Blur DX      [✓]  live
  Strength            ───●────── 11
  Contrast            ────●───── 80
  Decorations [✓]  Docks [ ]  Menus [ ]
  ⚠ Legacy "Blur" effect is also enabled — two providers may fight.
Translucency
  Effect              [✓]
  Inactive            ───●────── 97 %
  Menus               ───●────── 96 %
  …
Motion
  Animation speed     ───●────── 0.5×
────────────────────────────────────────────────────────────────────
[Undo last] [Revert all]                      ⓘ 2 changes this session
```

Каждая строка: label, контрол, бейдж источника (`default` / `modified` / `restart` / `warning`), контекстное меню (⋮): Reset to default, Reset to profile, Copy key, Open file, Show in raw view.

### 5.5. Advanced

- **Поиск по всему**: label, id, ключ, файл, значение. Результат — сгруппированные строки, клик ведёт к полю.
- **Validation report** («Theme Doctor»): список проблем с severity и кнопкой Fix.
- **Raw files**: компонент → файлы (размер, mtime) → таблица `key | value | source` → «Open in editor».
- **Diff live ↔ profile**: по полям, а не по строкам: `blur: 11 → 14`, `accent: #808e47 → #b875dc`. Плюс «Restore this field from profile» (частичное применение).
- **Экспорт снимка** в JSON (для багрепорта/бэкапа) — позже.

---

## 6. Модель данных и ядро

### 6.1. Декларативная схема

Один реестр, как `TRACKED_APPS`, — источник правды для UI, чтения, записи и валидации:

```python
ThemeField(
    id="accent_color",
    section="colors",
    label="Accent color",
    source=IniSource(".config/kdeglobals", "General", "AccentColor", type="color"),
    default=None,                      # None = «не задано, действует схема»
    apply=CmdApply(["plasma-apply-colorscheme", "-a", "$VALUE", "$ColorScheme"]),
    badge="live", summary=True,
    description="Selection/accent color used by Plasma and Qt apps",
)
```

Поля: `id, section, label, source, type, default, choices, apply, badge, advanced, keywords, docs_url, validate`.

`choices` — провайдеры:
- color schemes → `plasma-apply-colorscheme --list-schemes` (парсить `* name (current)`);
- L&F / Plasma theme / cursor → `plasma-apply-* --list*`;
- icon themes → каталоги с `index.theme`;
- Kvantum → `<dir>/*.kvconfig` (не только каталоги! вариант `OrchisDark` лежит внутри `Orchis/`);
- GTK themes → `/usr/share/themes/*`, `~/.themes/*`, `~/.local/share/themes/*`;
- fonts → `Qt.fontFamilies()` / `fc-list`;
- decorations → `kwinrc` + список плагинов KWin.

### 6.2. `ThemeCore` (protocol.py)

```python
class ThemeCore(ABC):
    def sections(self) -> list[ThemeSection]
    def fields(self, section: str) -> list[ThemeFieldInfo]
    def value(self, field_id: str, scope: str = "live") -> ThemeValue
    def set_value(self, field_id: str, value: Any) -> Result
    def reset(self, field_id: str, to: str = "default") -> Result
    def choices(self, field_id: str) -> list[Choice]
    def validate(self) -> list[ThemeIssue]
    def snapshot(self) -> Result        # снимок сессии для Undo/Revert
    def revert(self) -> Result
```

`ThemeValue`: `raw, display, is_default, is_modified_vs_profile, issue, badge`.
`scope`: `"live"` или `"profile:<name>"`.

**Ядра.** Инспектор — Python-реализация и **не** является частью выбираемого
ядра `--core`: это чтение/запись конфигов, где Rust сейчас ничего
не ускорит. `CoreFactory.theme_core()` возвращает Python-реализацию независимо
от `--core`; это документируется в Settings. Порт инспектора на Rust не
планируется: второе (Rust-библиотечное) ядро убрано, ядро теперь одно.
Мутирующие операции уже делегируются Rust CLI `theme-profile`, так что
прецедент есть.

### 6.3. Чтение

- INI/KConfig → `kreadconfig6` (уже используется) или свой парсер; писать — `kwriteconfig6` (экранирование, `--type bool`, `--notify`).
- JSON/JSONC → `json` + стрип комментариев; **запись только точечная** (regex по ключу), чтобы не переформатировать файл.
- TOML → `tomllib` для чтения; запись — точечная замена строки.
- CSS (`wofi`) → `@define-color` regex.
- `user.js` → существующий `zen_user_pref`-подход.
- Значения по умолчанию: `kreadconfig6 --default` там, где есть; иначе — «не задано» (не выдумывать).

Кэш: читать при открытии страницы и после каждой записи; файловые mtime — для детекта внешних изменений («файл изменён вне приложения, перечитать?»).

### 6.4. Запись и хуки

| Hook | Команда |
|---|---|
| `kwin-reconfigure` | `qdbus6 org.kde.KWin /KWin reconfigure` |
| `kwin-effect` | `… /Effects loadEffect\|unloadEffect <name>` + `reconfigureEffect <name>` |
| `kbuildsycoca` | `kbuildsycoca6` |
| `kitty-live` | `kitty @ --to unix:/tmp/kitty-theme-profile load-config` + `set-background-opacity`/`set-colors` |
| `plasmashell` | `systemctl --user restart plasma-plasmashell` (только панели/темы, с подтверждением) |
| `none` | приложение само перечитает конфиг |

Все хуки — best-effort: ошибка хука не откатывает запись, но показывается в тосте («сохранено, применить на лету не удалось — перезапустите приложение»).

### 6.5. Контроллер

`ThemeController` (QObject) по образцу существующих: `@Slot`-методы, `Property` для секций, сигналы `busyChanged`, `valueChanged(fieldId)`, `toastRequested(text)`. Асинхронность — как в `ProfileController._start_operation`, чтобы не блокировать GUI на D-Bus/файлах.

---

## 7. Семантика редактирования

**Модель применения: стейджинг + Apply (смена решения, M2).**

Раньше было instant + Undo: каждая правка сразу писалась и дергала
reload-хуки. Практика показала, что промежуточные состояния (слайдер,
меняющий opacity от 0.3 до 0.9) порождают серию перезагрузок приложений.
Теперь: set_value/set_config_value только валидируют и копят правки;
`apply()` применяет батч одним проходом — файлы, команды и уникальные
хуки. `revert()` сбрасывает стейдж и откатывает применённую часть сессии
по снимкам за одно действие. Staged-значение сразу видно в поле (флаг
`staged` в модели, закрашенная точка), кнопка Apply подсвечена при непустом
стейдже. Отмена поштучная — undo() сначала снимает стейдж (LIFO), затем
откатывает применённые записи. Undo и Revert остаются доступны после Apply:
контроллер отдельно предоставляет `sessionChanges` и `undoDepth`.

- Первая запись в сессии → снимок затронутых файлов в памяти + `backups/inspector-<ts>/` на диске.
- Каждая запись → в стек Undo `(file, bytes_before, field, old_value)`; `Ctrl+Z` и кнопка «Undo».
- «Revert all» → диалог подтверждения, затем сброс стейджа, восстановление всех файлов снимка и хуки.
- Дебаунс слайдеров 300 мс; во время применения контрол disabled, индикатор Busy подписан «Applying…», UI не блокируется.
- Тяжёлые операции (L&F, смена иконочной темы, рестарт plasmashell) — диалог подтверждения с объяснением последствий.
- Внешние изменения: перед записью сравнить mtime/хеш; если файл изменился — «Перечитать / Всё равно записать».

**Почему не «черновик → Save»**: стейджинг отделяет промежуточные значения слайдеров от записи; явный Apply показывает, когда именно меняются файлы и запускаются хуки.

**Профили**: read-only. Кнопки: «Apply», «Diff vs live», «Restore field from profile» (Phase 3), «Save current as profile…».

---

## 8. Сценарии

1. **«Что у меня сейчас?»** Открыть Theme → обзор: палитра, обои, Kvantum, blur, радиус, 1 предупреждение. 5 секунд.
2. **«Хочу другой accent»** Theme → Colors → swatch → палитра → Apply; затем при необходимости Undo или «Save as profile…».
3. **«Почему терминал прозрачный, а окна нет?»** Transparency → kitty 0.5/blur 30, Klassy 50%, translucency 97%, Kvantum `translucent_windows=false`. Видно, кто за что отвечает.
4. **«Сравнить glass и paper»** Profiles → Inspect у paper → scope `profile:paper`, `diff vs live` по полям: `blur 11 → 0`, `accent → …`, `Kvantum OrchisDark → …`.
5. **«Починить тему»** Validation → Show подсвечивает поле; Fix выбирает безопасное значение и применяет его. (Это реальный баг, см. §14.)
6. **«Тонкая настройка без System Settings»** Windows → Effects → blur strength 11 → 14 → Apply; не понравилось — Undo.

---

## 9. Состояния, ошибки, доступность

- **Loading**: skeleton-строки; D-Bus/файлы не блокируют UI.
- **Empty**: «используется значение по умолчанию» (курсив, бейдж `default`), не пустая строка.
- **Parse error**: строка с бейджем `error`, кнопка «Open file»; остальные поля раздела работают.
- **Component not installed**: скрыт под «Показать все» (фильтр по статусу).
- **Busy**: подписанный глобальный индикатор + disabled контролы; успешные сообщения скрываются автоматически, ошибки остаются до явного закрытия.
- **Доступность**: staged — закрашенная точка, отличие от профиля — кольцо и текст «Profile: …»; у плиток и свотчей есть фокус, `Accessible.name`, активация Enter/Space. `Ctrl+F` фокусирует общий или внутрисекционный поиск, `Ctrl+Z` отменяет, `Esc` возвращает назад; tooltip доступен и по фокусу.
- **Узкое окно (640 px)**: один столбец плиток; обзорная карточка скрывает вторичное превью обоев, а действия переносит горизонтальным рядом под текст; широкое — 2 колонки.
- **Большие конфиги**: по умолчанию показываются первые 300 параметров; «Show all» загружает остальные по требованию.

---

## 10. Риски и подводные камни

1. **KConfig-экранирование** (`;`, `,`, `\`, переводы строк) — только через `kwriteconfig6`, не самописным INI-writer'ом.
2. **Шрифты** — составная строка; менять family/size, сохраняя остальные поля, иначе потеряются weight/style. Для веса/начертания — «Open Fonts kcm».
3. **`plasmashell` перезаписывает `plasma-org.kde.plasma.desktop-appletsrc`** — этот файл не редактируем на лету; только чтение и «открыть System Settings».
4. **Kvantum-варианты**: тема может быть файлом внутри чужого каталога (`Orchis/OrchisDark.kvconfig`). Список — по `*.kvconfig`, не по каталогам.
5. **Kvantum/GTK/иконки** не применяются к уже запущенным приложениям — честный бейдж `restart app`.
6. **Эффекты KWin**: конфиг и фактическое состояние расходятся (`loadedEffects` vs `[Plugins]`). Показывать оба, предупреждать о двух blur-провайдерах.
7. **`kwriteconfig6 --notify` не гарантирует live-перечитывание** — для цветов использовать `plasma-apply-colorscheme`, для эффектов D-Bus. После apply профиля схема переприменяется принудительно; wallpaper-accent переводится в конкретный цвет через `magick`, чтобы уже запущенная Plasma обновила палитру.
8. **Wayland/D-Bus API** может отличаться по версии KWin — все вызовы best-effort, ошибки видимы.
9. **52 системные + 16 пользовательских color scheme** — группировать «User / System», иначе комбобокс нечитаем.
10. **Flatpak GTK** не видит пользовательские темы — предупреждение в GTK-разделе.
11. **Правка чужого файла во время работы приложения** — mtime-проверка и «перечитать».
12. **Не превратиться во второй System Settings**: глубокие вещи (правила окон, панели, полный редактор схем) — ссылками, не копией.

---

## 11. План внедрения

### Phase 0 — починка (≈1 ч)
- [ ] `apply_live_hooks`: `kreadconfig6 --file "$source/.config/kdeglobals"` (сейчас лишний позиционный аргумент → в `LookAndFeelPackage` пишется `--help`).
- [ ] Восстановить `LookAndFeelPackage` в live и в профиле `glass` (`glass`), после фикса — пере-сохранить профиль.
- [ ] Починить парсинг `status`: zed (`"mode": "dark",`), vscode (`workbench.colorTheme`), obsidian (`{}`).

### Phase 1 — read-only инспектор (1.5–2 дня)
- [ ] `ThemeField`-реестр: 69 полей, 5 секций + Advanced, `summary`-флаги.
- [ ] Readers (INI/JSON/TOML/CSS/user.js), defaults, choices-провайдеры.
- [ ] `validate()`: несуществующие scheme/L&F/Plasma theme/иконки/курсоры/шрифт; конфликт blur.
- [ ] `ThemeCore` (Python), `ThemeController`, QML: `ThemePage`, `ThemeSectionPage`, `ThemeFieldRow`, `ColorSwatch`, `ValueBadge`.
- [ ] Поиск по полям, «Open in System Settings», «Open file», diff live↔profile по полям.
- [ ] Тесты: значения против `theme-profile status` (golden), валидация на синтетических конфигах, offscreen-рендер страницы.

### Phase 2 — редактирование (2–3 дня)
- [ ] Writers + hooks; snapshot/undo/revert.
- [ ] Контролы: ColorDialog, слайдеры с дебаунсом, ComboBox из choices, FontDialog, confirm для тяжёлых.
- [ ] Editable: accent, scheme, widget style, icons, cursor, Plasma theme, L&F, Kvantum theme+options, Klassy, effects, translucency, animation, fonts, kitty, alacritty, btop, GTK theme.
- [ ] Тесты на temp `$HOME`: write→read roundtrip, undo, «чужое изменение файла», ошибки хуков.

### Phase 3 — продвинутое (2 дня)
- [ ] Raw-таблица по файлам + глобальный поиск по ключам.
- [ ] «Restore field from profile» (частичное применение).
- [ ] Validation report UI, экспорт снимка в JSON, история изменений сессии.
- [ ] Per-app advanced (zed/vscode/zen/obsidian).
- [x] Rust-паритет — не нужен: второе ядро (Rust-библиотека) убрано, ядро одно,
      инспектор остаётся Python-only.

---

## 12. Метрики успеха

- Ответ «какой у меня blur strength?» — < 5 с без терминала.
- Смена accent — < 10 с, с Undo.
- 0 невалидных enum-значений после правок (roundtrip-тесты).
- Ни один сценарий не требует ручного редактирования файла.
- Все найденные «битые» значения видны в Validation report.

---

## 13. Non-goals

- Замена System Settings целиком.
- Редактор панелей/виджетов Plasma.
- Полный редактор `.colors` (50+ ключей) — только палитра и роли.
- Пиксель-точный предпросмотр темы (мокап окна — приблизительный, с подписью).
- Автоматическое удаление бэкапов (политика ретенции — отдельно).

---

## 14. Найденные баги (повод для Phase 0)

1. **`LookAndFeelPackage` = текст `--help`.** ✅ **Исправлено.** `apply_live_hooks`
   теперь читает значение из профиля без лишнего позиционного аргумента;
   `tests/test-scripts.sh` (section 13) проверяет, что в `kdeglobals` не
   попадает текст usage.
2. **`status` для zed/vscode/obsidian** отдавал обрывки JSON (`"mode": "dark",`,
   `"workbench.colorTheme": …`, `{}`). ✅ **Исправлено в 0.5.0**: `status` стал
   нативным Rust-модулем (`rust_core/src/status.rs`) с точными по ключу
   JSONC/INI-читалками (`config_parser.rs`) — только строковые значения по
   точным ключам, никаких обрывков; obsidian теперь отдаёт `obsidian_theme` и
   `obsidian_css_theme` вместо сырого JSON.
3. **`kwin_glass` пуст** — ключа `glassEnabled` в Plasma 6 нет. ✅ **Исправлено
   в 0.5.0**: `kwin_glass=true|false` выводится из реального
   `better_blur_dxEnabled`; рядом остаются `kwin_blur`, `kwin_contrast`.
4. **Курсорная тема не отслеживается.** Решение: курсор остаётся **live-only**
   (`kcminputrc` не входит в профиль), чтобы не менять состав существующих
   снимков; Theme Inspector показывает и меняет его в live-скопе.

---

## 15. Принятые решения (бывшие открытые вопросы)

1. **Модель применения**: ~~instant + Undo~~ → стейджинг + Apply (M2, решение пользователя). Реализовано.
2. **Scope**: редактируется только Live; профили — read-only снимки.
3. **Порядок**: read-only MVP, затем live-правки. Реализовано.
4. **Место**: отдельная страница «Theme». Реализовано.
5. **Язык UI**: английский (текущее состояние).
