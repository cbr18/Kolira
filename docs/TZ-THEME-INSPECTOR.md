# TZ: Theme Inspector — экран «Тема»

> **Статус**: Phase 0–2 реализованы, Phase 3 — частично (см. §3a)
> **Приоритет**: P1 (после текущих багфиксов)
> **Оценка**: Phase 0 — 1 ч · Phase 1 — 1.5–2 дня · Phase 2 — 2–3 дня · Phase 3 — ~2 дня
> **Дизайн и обоснование**: [THEME_INSPECTOR.md](THEME_INSPECTOR.md)
> **Ветка**: `main` (репозиторий `github.com/cbr18/Kolira`)

---

## 1. Цель

Дать пользователю один экран, который отвечает на вопросы:

1. **Что внутри моей темы?** — accent, схема, Kvantum, blur, прозрачность, шрифты, иконки, курсоры, обои.
2. **Чем профиль отличается от live?** — diff по полям, а не по 4000 строк.
3. **Что сломано?** — несуществующие пакеты/темы/шрифты, конфликты эффектов.
4. **Можно ли поправить здесь?** — да, безопасные поля: пикеры, слайдеры, цвета.

## 2. Границы

**Входит:**
- новая страница `Theme` в drawer;
- read-only инспектор 69 полей в 5 секциях;
- валидация («Theme Doctor»);
- редактирование live-значений с мгновенным применением, Undo и снимком сессии;
- поиск по полям, diff live↔profile, ссылки в System Settings и «Open file».

**Не входит:**
- замена System Settings, редактор панелей/виджетов, полный редактор `.colors`;
- пиксель-точный предпросмотр темы;
- автоудаление бэкапов;
- Rust-реализация инспектора (Phase 3, после стабилизации схемы).

## 3. Результат

| Артефакт | Где |
|---|---|
| Схема полей | `gui/qt-app/src/core/theme_schema.py` |
| Ядро инспектора (read/write/hooks/validate) | `gui/qt-app/src/core/theme_core.py` |
| Контракты | `gui/qt-app/src/core/protocol.py` (`ThemeCore`, `ThemeSection`, `ThemeFieldValue`, `ThemeIssue`, `Choice`) |
| Контроллер | `gui/qt-app/src/controllers/theme_inspector.py` (`ThemeInspectorController`, context property `themeInspector`) |
| UI | `qml/pages/ThemePage.qml`, `qml/pages/ThemeSectionCard.qml`, `qml/pages/ThemeSectionFields.qml`, `qml/pages/ThemeFieldRow.qml` |
| Тесты | `gui/qt-app/tests/test_theme_inspector.py`, `tests/test-scripts.sh` (Section 13) |

---

## 3a. Статус реализации (2026-10-20)

Реализовано:

- **Phase 0**: `apply_live_hooks` читает L&F из профиля правильным вызовом
  `kreadconfig6`; live и профиль `glass` восстановлены (`LookAndFeelPackage=glass`);
  `kolira status` парсит Zed/VS Code/Cursor/Obsidian без обрывков JSON;
  регрессионный тест в `tests/test-scripts.sh` (Section 13). Тесты дополнительно
  изолированы от `XDG_CONFIG_HOME` — раньше `HOME=...` не защищал реальный конфиг.
- **Курсор (T0.4)**: `kcminputrc` содержит не только тему курсора, но и
  скорость/инерцию мыши, поэтому снимок берёт из него только `cursorTheme`
  (политика appearance-only). Поле `cursor_theme` хранится в теме и попадает в
  зависимости; `cursor_size` — личная настройка (`profile_tracked=False`).
- **Phase 1**: схема на 69 полей, 5 секций + карточка Advanced; чтение KConfig,
  JSON/JSONC, TOML, kitty, user.js; choices из файловой системы; diff live↔profile;
  валидация (несуществующие схемы/темы/шрифты, битый L&F, конфликт blur);
  поиск по label/id/ключу/русским ключевым словам; страница Theme в drawer и
  кнопка Inspect на карточке профиля.
- **Phase 2**: запись через `kwriteconfig6` и точечные патчеры (JSONC сохраняет
  комментарии и trailing comma); snapshot в `backups/inspector-<ts>/`; Undo/Revert;
  ColorDialog, слайдеры с дебаунсом, ComboBox, font family (TextField)+size, confirm для
  L&F/Plasma theme/icons; hooks `kwin`, `kwin effect`, `kbuildsycoca`, `kitty`.
- **Phase 3 (частично)**: глобальный поиск, «Restore field from profile», кнопка
  Fix в Validation, экспорт снимка в JSON, ссылки на KCM, per-app поля.
  Rust-паритет не делался: `CoreFactory.theme_core()` всегда Python — это
  задокументировано в докстринге и здесь.

Отличия от первоначального ТЗ:

- Разделы открываются отдельной `SectionPage.qml` в стеке слоёв; на обзорной
  странице и внутри раздела есть поиск.
- «Raw-таблица по файлам» не делалась: вместо неё экспорт JSON и кнопка
  «Open config folder». Сырые файлы остаются в System Settings/редакторе.
- Ctrl+Z, Ctrl+F и Escape реализованы. Проверки mtime перед записью нет:
  внешние изменения подхватываются при следующем Refresh. Это осознанный
  компромисс, чтобы не плодить диалоги в MVP.
- FontDialog заменён на TextField семейства + SpinBox размера: QtQuick.Dialogs
  FontDialog в PySide6/Kirigami выглядит чужеродно, а ComboBox на 800+ семейств
  в Qt 6.12 синхронно строит меню и подвешивает страницу. Текстовое поле
  сохраняет контракт «family + size» и не требует прогрева списка.

---

## 4. Phase 0 — блокирующие багфиксы

### T0.1. Починить `LookAndFeelPackage` в `apply_live_hooks`

`files/bin/kolira`:

```bash
# было (лишний позиционный аргумент → kreadconfig6 печатает --help):
look_and_feel="$(kreadconfig6 --file kdeglobals --group KDE --key LookAndFeelPackage "$source/.config/kdeglobals" ...)"

# стало:
look_and_feel="$(kreadconfig6 --file "$source/.config/kdeglobals" --group KDE --key LookAndFeelPackage ...)"
```

- [ ] Исправить вызов.
- [ ] Проверить, что `kreadconfig6 --file <abs path>` отдаёт значение, а не usage.
- [ ] Добавить регрессионный тест в `tests/test-scripts.sh`: `kolira` не пишет в `kdeglobals` текст, содержащий `Usage:`/`Options:`.

### T0.2. Восстановить испорченное значение

Сейчас в live и в профиле `glass` лежит текст `--help`.

- [ ] Бэкап: `cp ~/.config/kdeglobals ~/.config/kdeglobals.bak-$(date +%F)` (и то же для профиля).
- [ ] Записать `LookAndFeelPackage=glass` в live и в `profiles/glass/.config/kdeglobals`.
- [ ] Проверить: `kreadconfig6 --file kdeglobals --group KDE --key LookAndFeelPackage` → `glass`.
- [ ] Убедиться, что `plasma-apply-lookandfeel --list` содержит `glass`.

### T0.3. Починить `kolira status`

- [ ] `zed_*`: вытаскивать значения без `"light":` / `"mode":` (парсить JSON, а не awk).
- [ ] `code_theme` / `cursor_theme`: только значение в кавычках, без имени ключа.
- [ ] `obsidian_appearance`: если `{}` — писать пусто, а не мусор.
- [ ] Не ломать существующий формат `key=value` (его читают тесты и GUI).

### T0.4. Курсорная тема (решение)

`kcminputrc` не входит в `COMPONENT_PATHS`, поэтому KDE-курсор не сохраняется в профиль.

- [ ] Решить: добавлять компонент `cursor` (меняет состав профилей) или оставить read-only в инспекторе.
- [ ] Если добавляем — обновить `COMPONENT_PATHS` в `kolira`, `TRACKED_APPS` в Python и Rust, тесты паритета.

**Acceptance Phase 0:** `kolira status` больше не содержит `Usage:`, L&F = `glass`, тесты зелёные.

---

## 5. Phase 1 — read-only инспектор

### T1.1. Схема полей

`gui/qt-app/src/core/theme_schema.py`:

```python
ThemeField(
    id="accent_color",
    section="colors",
    label="Accent color",
    source=IniSource(".config/kdeglobals", "General", "AccentColor", type="color"),
    default=None,
    choices=None,
    apply=CmdApply(["plasma-apply-colorscheme", "-a", "$VALUE", "$ColorScheme"]),
    badge="live",
    summary=True,
    description="Selection/accent color used by Plasma and Qt apps",
    keywords=("accent", "выделение", "цвет"),
)
```

- [ ] Dataclasses: `ThemeField`, `IniSource`, `JsonSource`, `TomlSource`, `CssSource`, `UserJsSource`, `CmdApply`, `ConfigApply`.
- [ ] Секции: `colors`, `typography`, `windows`, `assets`, `apps`, `advanced`.
- [ ] Набор полей — по таблицам [THEME_INSPECTOR.md §3.3](THEME_INSPECTOR.md#33-кураторский-набор-полей-mvp) (~60).
- [ ] `summary=True` — для карточки обзора и строки «DNA».

### T1.2. Чтение

`gui/qt-app/src/core/theme_core.py`:

- [ ] INI через `kreadconfig6` (абсолютный путь) с fallback на свой парсер.
- [ ] JSON/JSONC — чтение с вырезанием комментариев.
- [ ] TOML — `tomllib`.
- [ ] CSS — `@define-color`.
- [ ] `user.js` — переиспользовать логику `zen_user_pref`.
- [ ] «Не задано» ≠ пустая строка: `is_default=True` и понятный display.
- [ ] Кэш значений + инвалидация по mtime.

### T1.3. Choices-провайдеры

- [ ] Color schemes: `plasma-apply-colorscheme --list-schemes`, группировка User/System.
- [ ] Plasma theme: `plasma-apply-desktoptheme --list-themes`.
- [ ] L&F / cursor: `--list` / `--list-themes`.
- [ ] Kvantum: `*.kvconfig` (варианты внутри каталогов! `Orchis/OrchisDark.kvconfig`).
- [ ] Icons: каталоги с `index.theme`.
- [ ] GTK themes: `/usr/share/themes`, `~/.themes`, `~/.local/share/themes`.
- [ ] Fonts: `Qt.fontFamilies()`.
- [ ] Если текущее значение не в списке — пункт «Custom: <value>» (не терять его).

### T1.4. Валидация

- [ ] Цветовая схема существует.
- [ ] L&F-пакет существует (ловит текущий баг).
- [ ] Plasma theme / icons / cursor / GTK theme существуют.
- [ ] Kvantum-тема существует (с учётом вариантов).
- [ ] Шрифт есть в системе (`fc-match`).
- [ ] Конфликт: `blurEnabled` и `better_blur_dxEnabled` одновременно.
- [ ] Конфиг configured vs loaded (`org.kde.KWin /Effects loadedEffects`).
- [ ] `ThemeIssue(severity, field_id, message, fix_action)`.

### T1.5. Контракт и контроллер

- [x] `ThemeCore` ABC в `protocol.py`:
  `sections()`, `fields(section)`, `get(field_id, scope)`, `choices(field_id)`, `validate()`, `set_value()`, `reset()`, `undo()`, `revert()`, `session_changes()`.
- [x] `CoreFactory.theme_core()` → Python-реализация (не выбираемое ядро, не зависит от `--core`; отмечено в докстринге и §3a).
- [x] `ThemeInspectorController` (QObject, context property `themeInspector`): `sections`, `issues`, `scope`, `scopes`, `readOnly`, `busy`, `sessionChanges`, `accentColor`, `modifiedCount`; сигналы `sectionsChanged`, `fieldsChanged`, `issuesChanged`, `scopeChanged`, `busyChanged`, `toast(text, ok)`.
- [x] Регистрация в `register_controllers(engine)`; Python-ссылка живёт в `MainController._theme`.
- [x] Async-обёртки: записи/undo/revert идут через `QThreadPool`, GUI не блокируется.

### T1.6. UI

- [ ] `ThemePage.qml`: scope-переключатель `Live | <profile>`, карточка обзора (обои, палитра, 4 факта, issues), 6 карточек секций, Advanced.
- [ ] `ThemeSectionPage.qml`: `Kirigami.FormLayout`, группы, поиск по секции, бейджи, контекстное меню (Reset/Copy/Open file/Raw).
- [ ] `ThemeSectionCard.qml`, `ThemeFieldRow.qml`, `ColorSwatch.qml`, `ValueBadge.qml`.
- [ ] Добавить типы в `qml/Components/qmldir`.
- [ ] Drawer: `Profiles · Theme · Settings`.
- [ ] Карточка профиля: действие «Inspect» + строка DNA (`#808e47 · OrchisDark · blur 11 · r10`).
- [ ] Advanced: поиск по всем полям, Validation report, diff live↔profile по полям.
- [ ] «Open in System Settings»: `kcmshell6 kcm_colors|kcm_fonts|kcm_icons|kcm_cursortheme|kcm_kwindecoration|kcm_kwin_effects|kcm_kwinrules|kcm_style`.
- [ ] «Open file»: `xdg-open` (fallback: показать путь).
- [ ] Glass-режим: страница следует `themeController.isGlass` (как `Main.qml`).
- [ ] Без `ToolTip.text` → `tooltip:` (есть тест).

### T1.7. Тесты

- [ ] `test_theme_inspector.py`: схема (уникальные id, валидные секции, у каждого поля есть source).
- [ ] Golden-паритет с `kolira status` там, где значения пересекаются.
- [ ] Валидация на синтетическом `$HOME` (подмена `Path.home`): битый L&F, отсутствующая схема, Kvantum-вариант.
- [ ] QML: файлы существуют, `qmldir` содержит новые типы, offscreen-рендер страницы без QML-предупреждений.
- [ ] «Не блокирует GUI»: чтение секции не дольше N мс на кэше (мягкий тест).
- [ ] Обновить `ALL_PY_FILES`-совместимость (новые файлы компилируются).

**Acceptance Phase 1:** страница открывается, все поля заполнены, Validation находит испорченный L&F, diff по полям работает, `pytest` зелёный, 0 QML-warning offscreen.

---

## 6. Phase 2 — редактирование

### T2.1. Запись

- [ ] `kwriteconfig6 --notify` (+ `--type bool`) для KDE/Kvantum INI.
- [ ] Точечная запись JSON/TOML/CSS (regex по ключу, без переформатирования файла).
- [ ] Атомарная запись (tmp + rename), сохранение прав.
- [ ] Хуки: `kwin-reconfigure`, `kwin-effect`, `kbuildsycoca`, `kitty-live`, `plasmashell`.
- [ ] Best-effort: ошибка хука → тост «сохранено, перезапустите приложение», запись не откатывается.

### T2.2. Снимок / Undo

- [ ] Снимок затронутых файлов при первой записи в сессии + `backups/inspector-<ts>/`.
- [ ] Стек Undo (до 50): файл, байты до, поле, старое значение.
- [x] `Ctrl+Z`, «Undo», «Revert all» (с подтверждением).
- [ ] Проверка mtime/хеша перед записью: «файл изменён вне приложения — перечитать?».

### T2.3. Контролы

- [ ] ColorDialog (accent, роли палитры), hex + alpha.
- [ ] Слайдеры с дебаунсом 250 мс (blur, contrast, opacity, радиус, анимация).
- [ ] ComboBox из choices + «Custom…».
- [ ] FontDialog: family + size, сохранять weight/style в составной строке.
- [ ] Confirm для тяжёлых: L&F, иконки, рестарт plasmashell.

### T2.4. Набор редактируемых полей

Colors: accent, accentFromWallpaper, scheme, window/view/button/text/selection/link, contrast, frameContrast, widgetStyle.
Typography: UI/fixed/small/menu/toolbar/title, GTK font.
Windows: decoration library/theme/border, Klassy opacity ×2 + radius + shadow, bbdx enabled/strength/contrast/decorations/docks/menus, legacy blur, contrast, translucency + 6 процентов, animation factor.
Assets: Plasma theme, L&F (confirm), icons, cursor + size, splash, GTK theme/dark/icons, Kvantum theme + translucent/blurring/popup blurring.
Apps: kitty opacity/blur (live), alacritty opacity/blur, btop theme/rounded corners, zed mode/light/dark, vscode/cursor colorTheme, zen prefs.

### T2.5. Тесты

- [ ] Roundtrip на temp `$HOME`: для каждого writable-поля `set → value` совпадает.
- [ ] Undo восстанавливает байты файла.
- [ ] Внешнее изменение файла → warning, без потери данных.
- [ ] Хуки вызываются с ожидаемыми аргументами (mock subprocess/D-Bus).
- [ ] Confirm-диалоги не ломают offscreen-тесты.

**Acceptance Phase 2:** accent, Kvantum, radius, blur и opacity меняются из UI; Undo/Revert работают; ни один файл не повреждён; тесты зелёные.

---

## 7. Phase 3 — продвинутое

- [ ] Raw-таблица по файлам + глобальный поиск по ключам.
- [ ] «Restore field from profile» (частичное применение).
- [ ] Validation report UI с Fix-кнопками.
- [ ] Экспорт снимка темы в JSON.
- [ ] Per-app advanced: zed/vscode/zen/obsidian.
- [ ] Rust-паритет инспектора (или явное «inspector is Python-only» в Settings и docs).

---

## 8. Контракты (минимальный вид)

```python
@dataclass(frozen=True)
class ThemeSection:
    id: str; title: str; icon: str; description: str
    summary: str; field_count: int; issue_count: int

@dataclass(frozen=True)
class ThemeFieldValue:
    id: str; section: str; label: str; type: str
    value: object; display: str; default: object | None
    is_default: bool; differs: bool; profile_value: object
    profile_display: str
    badge: str            # live | new | app | shell | warning | error
    choices: list[Choice]
    file: str; group: str; key: str; description: str
    min: float | None; max: float | None; step: float | None
    suffix: str
    tracked_in_profile: bool; advanced: bool; confirm: bool

    # QML получает не dataclass, а dict из to_dict()

@dataclass(frozen=True)
class ThemeIssue:
    severity: str         # error | warning | info
    field_id: str; message: str; fix_hint: str

class ThemeCore(ABC):
    def sections(self, scope: str = "live") -> list[ThemeSection]: ...
    def fields(self, section: str, scope: str = "live") -> list[dict]: ...
    def get(self, field_id: str, scope: str = "live") -> dict | None: ...
    def choices(self, field_id: str) -> list[Choice]: ...
    def font_families(self) -> list[str]: ...
    def validate(self) -> list[ThemeIssue]: ...
    def search(self, query: str, scope: str = "live") -> list[dict]: ...
    def set_value(self, field_id: str, value: object) -> Result: ...
    def reset(self, field_id: str, to: str = "default") -> Result: ...
    def undo(self) -> Result: ...
    def revert(self) -> Result: ...
    def session_changes(self) -> int: ...
```

`scope` = `"live"` или `"profile:<name>"` (для последнего запись отклоняется контроллером: «Profile snapshots are read-only»).

---

## 9. Проверки перед коммитом

```bash
# Python
cd gui/qt-app && QT_QPA_PLATFORM=offscreen .venv/bin/python -m pytest tests/ -q

# Bash
bash tests/test-scripts.sh
shellcheck files/bin/* scripts/*.sh

# Rust (если трогали ядро)
cd gui/qt-app/src/core/rust_core && cargo build --release

# Живой запуск / скриншот
cd gui/qt-app && KTM_SCREENSHOT=/tmp/theme-page.png .venv/bin/kolira
```

Установка и деплой — только из исходников:

```bash
scripts/install-kolira.sh          # CLI + GUI
scripts/install-kolira.sh --only-gui
```

---

## 10. Definition of Done

- [x] Phase 0 смержен, испорченный L&F восстановлен, есть регрессионный тест.
- [x] Theme-страница открывается из drawer и с карточки профиля.
- [x] Read-only инспектор: ≥50 полей, 5 секций + Advanced, поиск, diff live↔profile.
- [x] Validation находит битые значения (проверено на реальном кейсе L&F и на синтетическом `$HOME`).
- [x] Редактирование: accent, Kvantum, Klassy radius/opacity, blur, translucency, kitty opacity, шрифты, Zed/VS Code/Cursor/Obsidian.
- [x] Undo/Revert работают; тесты используют temp HOME и не трогают реальные конфиги.
- [x] `pytest` (146 в GUI-наборе), `test-scripts.sh`, `shellcheck` — зелёные.
- [ ] 0 QML-предупреждений offscreen: остаётся одно предупреждение Kirigami StackView при программном push в dev-хуке (`KTM_OPEN_THEME`); при обычном клике не воспроизводится.
- [x] README + `docs/CURRENT_APP.md` + `docs/ROADMAP.md` обновлены.
- [x] Коммит и пуш в `origin/main` (`39d879c`, `aa219fe`).

---

## 11. Риски

| Риск | Митигация |
|---|---|
| KConfig-экранирование | только `kwriteconfig6`, не свой INI-writer |
| Составные строки шрифтов | менять family/size, остальное сохранять |
| `plasmashell` перезаписывает appletsrc | не редактировать live, только read-only |
| Kvantum-варианты | список по `*.kvconfig`, а не по каталогам |
| Эффекты: configured ≠ loaded | показывать оба, предупреждать |
| Долгие D-Bus/файловые операции | async-контроллер, не блокировать GUI |
| Случайная порча конфигов | снимок + Undo + бэкап в `backups/inspector-*` |
| Расползание в System Settings | глубокое — ссылками `kcmshell6` |

---

## 12. Открытые решения

1. Модель применения: **instant + Undo** (рекомендация) или черновик + Apply.
2. Scope: **только live** (рекомендация) или ещё файлы профилей.
3. Курсор: добавлять в отслеживаемые компоненты или read-only (T0.4).
4. Язык UI: английский (как сейчас) или добавляем русские строки.
