"""Declarative schema for the Theme Inspector.

One registry drives reading, writing, diffing and validation. Keep it free of
I/O: everything here is static metadata.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Optional


@dataclass(frozen=True, slots=True)
class SectionDef:
    id: str
    title: str
    icon: str
    description: str = ""


@dataclass(frozen=True, slots=True)
class FieldDef:
    """Один параметр темы.

    ``source``:
      - ``ini``    — INI-файл (KConfig, GTK, Kvantum), читаем секцию/ключ;
      - ``kv``     — строки ``key value`` (kitty.conf);
      - ``toml``   — плоский ключ верхнего уровня (alacritty);
      - ``json``   — путь в JSON/JSONC (zed, vscode, obsidian);
      - ``userjs`` — ``user_pref("key", value);`` (Zen);
      - ``kvantum``— файл активной Kvantum-темы (путь вычисляется).

    ``apply``:
      - ``config``    — запись через kwriteconfig6 (KDE) или generic INI;
      - ``ini``/``kv``/``toml``/``json``/``userjs`` — точечная запись файла;
      - ``cmd_config``— записать конфиг, затем выполнить ``command``.
    """

    id: str
    section: str
    label: str
    type: str
    source: str
    file: str = ""
    group: str = ""
    key: str = ""
    path: tuple[str, ...] = ()
    choices: str = ""
    apply: str = "config"
    command: tuple[str, ...] = ()
    hook: str = ""
    badge: str = "live"
    summary: bool = False
    advanced: bool = False
    profile_tracked: bool = True
    description: str = ""
    keywords: tuple[str, ...] = ()
    min: Optional[float] = None
    max: Optional[float] = None
    step: Optional[float] = None
    suffix: str = ""
    confirm: bool = False


SECTIONS: tuple[SectionDef, ...] = (
    SectionDef("colors", "Colors", "preferences-desktop-color",
               "Accent, color scheme, palette roles"),
    SectionDef("typography", "Typography", "preferences-desktop-font",
               "UI, fixed, menu and title fonts"),
    SectionDef("windows", "Windows & Effects", "preferences-system-windows",
               "Decorations, blur, translucency, motion"),
    SectionDef("assets", "Desktop & Assets", "preferences-desktop-theme",
               "Plasma theme, look & feel, icons, cursors, Kvantum"),
    SectionDef("apps", "Applications", "application-x-executable",
               "Terminal, editor and app-level theming"),
)


def F(*args, **kwargs) -> FieldDef:  # noqa: N802 - compact schema DSL
    return FieldDef(*args, **kwargs)


FIELDS: tuple[FieldDef, ...] = (
    # ── Colors ────────────────────────────────────────────────────────────
    F("accent_color", "colors", "Accent color", "color",
      "ini", ".config/kdeglobals", "General", "AccentColor",
      apply="cmd_config",
      command=("plasma-apply-colorscheme", "--accent-color", "{hex}", "{scheme}"),
      summary=True,
      description="Selection/accent color used by Plasma and Qt applications",
      keywords=("accent", "выделение", "цвет", "акцент")),
    F("accent_from_wallpaper", "colors", "Accent from wallpaper", "bool",
      "ini", ".config/kdeglobals", "General", "accentColorFromWallpaper",
      description="Let Plasma derive the accent color from the wallpaper",
      keywords=("wallpaper", "обои", "акцент")),
    F("color_scheme", "colors", "Color scheme", "enum",
      "ini", ".config/kdeglobals", "General", "ColorScheme",
      choices="color_schemes", apply="cmd_config",
      command=("plasma-apply-colorscheme", "{value}"),
      summary=True,
      description="Installed KDE color scheme",
      keywords=("scheme", "схема", "цвета", "палитра")),
    F("window_bg", "colors", "Window background", "color",
      "ini", ".config/kdeglobals", "Colors:Window", "BackgroundNormal",
      summary=True, description="Background of normal windows",
      keywords=("window", "фон", "окно")),
    F("view_bg", "colors", "View background", "color",
      "ini", ".config/kdeglobals", "Colors:View", "BackgroundNormal",
      description="Background of lists, text views and inputs",
      keywords=("view", "список", "поле")),
    F("button_bg", "colors", "Button background", "color",
      "ini", ".config/kdeglobals", "Colors:Button", "BackgroundNormal",
      description="Background of push buttons", keywords=("button", "кнопка")),
    F("text_color", "colors", "Text", "color",
      "ini", ".config/kdeglobals", "Colors:Window", "ForegroundNormal",
      description="Normal foreground text", keywords=("text", "текст")),
    F("dim_text", "colors", "Dim text", "color",
      "ini", ".config/kdeglobals", "Colors:Window", "ForegroundInactive",
      description="Inactive/secondary text", keywords=("dim", "вторичный", "текст")),
    F("selection_bg", "colors", "Selection background", "color",
      "ini", ".config/kdeglobals", "Colors:Selection", "BackgroundNormal",
      description="Highlighted text/selection background",
      keywords=("selection", "выделение")),
    F("selection_text", "colors", "Selection text", "color",
      "ini", ".config/kdeglobals", "Colors:Selection", "ForegroundNormal",
      description="Highlighted text color", keywords=("selection", "выделение", "текст")),
    F("link_color", "colors", "Link", "color",
      "ini", ".config/kdeglobals", "Colors:Window", "ForegroundLink",
      description="Hyperlink color", keywords=("link", "ссылка")),
    F("header_bg", "colors", "Header background", "color",
      "ini", ".config/kdeglobals", "Colors:Header", "BackgroundNormal",
      description="Window header/title area background",
      keywords=("header", "заголовок")),
    F("tooltip_bg", "colors", "Tooltip background", "color",
      "ini", ".config/kdeglobals", "Colors:Tooltip", "BackgroundNormal",
      description="Tooltip background", keywords=("tooltip", "подсказка")),
    F("contrast", "colors", "Contrast", "int",
      "ini", ".config/kdeglobals", "KDE", "contrast",
      min=0, max=7, step=1,
      description="Widget style contrast level", keywords=("contrast", "контраст")),
    F("frame_contrast", "colors", "Frame contrast", "float",
      "ini", ".config/kdeglobals", "KDE", "frameContrast",
      min=0.0, max=1.0, step=0.05,
      description="Contrast of frames and separators",
      keywords=("frame", "контраст", "рамка")),
    F("widget_style", "colors", "Widget style", "enum",
      "ini", ".config/kdeglobals", "KDE", "widgetStyle",
      choices="widget_styles", badge="app",
      description="Qt widget style; Kvantum themes are selected below",
      keywords=("style", "стиль", "kvantum")),

    # ── Typography ────────────────────────────────────────────────────────
    F("ui_font", "typography", "UI font", "font",
      "ini", ".config/kdeglobals", "General", "font",
      summary=True, description="Default font for Plasma and Qt applications",
      keywords=("font", "шрифт", "ui")),
    F("fixed_font", "typography", "Fixed font", "font",
      "ini", ".config/kdeglobals", "General", "fixed",
      description="Monospace font", keywords=("monospace", "моно", "шрифт")),
    F("small_font", "typography", "Small font", "font",
      "ini", ".config/kdeglobals", "General", "smallestReadableFont",
      description="Smallest readable font", keywords=("small", "мелкий", "шрифт")),
    F("menu_font", "typography", "Menu font", "font",
      "ini", ".config/kdeglobals", "General", "menuFont",
      description="Menu font (defaults to the UI font)",
      keywords=("menu", "меню", "шрифт")),
    F("toolbar_font", "typography", "Toolbar font", "font",
      "ini", ".config/kdeglobals", "General", "toolBarFont",
      description="Toolbar font (defaults to the UI font)",
      keywords=("toolbar", "панель", "шрифт")),
    F("title_font", "typography", "Window title font", "font",
      "ini", ".config/kdeglobals", "WM", "activeFont", badge="new",
      description="Active window title font", keywords=("title", "заголовок", "шрифт")),
    F("gtk_font", "typography", "GTK font", "font",
      "ini", ".config/gtk-3.0/settings.ini", "Settings", "gtk-font-name",
      apply="ini", badge="app",
      description="Font used by GTK3/GTK4 applications",
      keywords=("gtk", "шрифт")),

    # ── Windows & Effects ─────────────────────────────────────────────────
    F("decoration_library", "windows", "Decoration library", "enum",
      "ini", ".config/kwinrc", "org.kde.kdecoration2", "library",
      choices="decoration_libraries", hook="kwin", badge="new",
      summary=True, description="Window decoration plugin",
      keywords=("decoration", "декорации", "окна")),
    F("decoration_theme", "windows", "Decoration theme", "enum",
      "ini", ".config/kwinrc", "org.kde.kdecoration2", "theme",
      choices="decoration_themes", hook="kwin", badge="new",
      description="Theme of the selected decoration plugin",
      keywords=("decoration", "тема", "окна")),
    F("border_size", "windows", "Border size", "enum",
      "ini", ".config/kwinrc", "org.kde.kdecoration2", "BorderSize",
      choices="border_sizes", hook="kwin", badge="new",
      description="Window border size", keywords=("border", "рамка")),
    F("klassy_active_opacity", "windows", "Active titlebar opacity", "int",
      "ini", ".config/klassy/klassyrc", "TitleBarOpacity", "ActiveTitleBarOpacity",
      min=0, max=100, step=1, suffix=" %", hook="kwin", badge="new",
      summary=True, description="Klassy active window titlebar opacity",
      keywords=("klassy", "opacity", "прозрачность", "заголовок")),
    F("klassy_inactive_opacity", "windows", "Inactive titlebar opacity", "int",
      "ini", ".config/klassy/klassyrc", "TitleBarOpacity", "InactiveTitleBarOpacity",
      min=0, max=100, step=1, suffix=" %", hook="kwin", badge="new",
      description="Klassy inactive window titlebar opacity",
      keywords=("klassy", "opacity", "прозрачность")),
    F("klassy_radius", "windows", "Corner radius", "int",
      "ini", ".config/klassy/klassyrc", "Windeco", "WindowCornerRadius",
      min=0, max=30, step=1, suffix=" px", hook="kwin", badge="new",
      summary=True, description="Klassy window corner radius",
      keywords=("radius", "углы", "скругление", "klassy")),
    F("klassy_shadow", "windows", "Shadow", "enum",
      "ini", ".config/klassy/klassyrc", "ShadowStyle", "ShadowSizeActive",
      choices="shadow_sizes", hook="kwin", badge="new",
      description="Klassy window shadow size", keywords=("shadow", "тень")),
    F("blur_enabled", "windows", "Better Blur DX", "bool",
      "ini", ".config/kwinrc", "Plugins", "better_blur_dxEnabled",
      hook="effect:better_blur_dx", summary=True,
      description="Enable the Better Blur DX KWin effect",
      keywords=("blur", "размытие", "стекло", "glass")),
    F("blur_strength", "windows", "Blur strength", "int",
      "ini", ".config/kwinrc", "Effect-better-blur-dx", "BlurStrength",
      min=0, max=20, step=1, hook="effect:better_blur_dx",
      summary=True, description="Better Blur DX blur strength",
      keywords=("blur", "размытие")),
    F("blur_contrast", "windows", "Blur contrast", "int",
      "ini", ".config/kwinrc", "Effect-better-blur-dx", "Contrast",
      min=0, max=100, step=1, hook="effect:better_blur_dx",
      description="Better Blur DX contrast", keywords=("blur", "контраст")),
    F("blur_decorations", "windows", "Blur decorations", "bool",
      "ini", ".config/kwinrc", "Effect-better-blur-dx", "BlurDecorations",
      hook="effect:better_blur_dx", description="Blur window decorations",
      keywords=("blur", "декорации")),
    F("blur_docks", "windows", "Blur docks", "bool",
      "ini", ".config/kwinrc", "Effect-better-blur-dx", "BlurDocks",
      hook="effect:better_blur_dx", description="Blur docks and panels",
      keywords=("blur", "док", "панель")),
    F("blur_menus", "windows", "Blur menus", "bool",
      "ini", ".config/kwinrc", "Effect-better-blur-dx", "BlurMenus",
      hook="effect:better_blur_dx", description="Blur menus and popups",
      keywords=("blur", "меню")),
    F("breeze_menu_opacity", "windows", "Breeze menu opacity", "int",
      "ini", ".config/breezerc", "Style", "MenuOpacity",
      min=0, max=100, step=5, suffix=" %",
      description="Opacity of menus with the Breeze widget style (100 = opaque); "
                  "translucent menus are blurred only with Blur menus on. "
                  "Applies to newly started applications",
      keywords=("menu", "меню", "прозрачность", "breeze", "opacity")),
    F("legacy_blur", "windows", "Legacy blur effect", "bool",
      "ini", ".config/kwinrc", "Plugins", "blurEnabled",
      hook="effect:blur", advanced=True,
      description="Classic KWin Blur effect (do not combine with Better Blur DX)",
      keywords=("blur", "размытие")),
    F("contrast_enabled", "windows", "Contrast effect", "bool",
      "ini", ".config/kwinrc", "Plugins", "contrastEnabled",
      hook="effect:contrast", advanced=True,
      description="KWin Contrast effect", keywords=("contrast", "контраст")),
    F("translucency_enabled", "windows", "Translucency effect", "bool",
      "ini", ".config/kwinrc", "Plugins", "translucencyEnabled",
      hook="effect:translucency",
      description="KWin Translucency effect", keywords=("translucency", "прозрачность")),
    F("translucency_inactive", "windows", "Inactive windows", "int",
      "ini", ".config/kwinrc", "Effect-translucency", "Inactive",
      min=0, max=100, step=1, suffix=" %", hook="effect:translucency",
      summary=True, description="Opacity of inactive windows",
      keywords=("translucency", "прозрачность")),
    F("translucency_menus", "windows", "Menus", "int",
      "ini", ".config/kwinrc", "Effect-translucency", "Menus",
      min=0, max=100, step=1, suffix=" %", hook="effect:translucency",
      description="Opacity of menus", keywords=("translucency", "меню")),
    F("translucency_dialogs", "windows", "Dialogs", "int",
      "ini", ".config/kwinrc", "Effect-translucency", "Dialogs",
      min=0, max=100, step=1, suffix=" %", hook="effect:translucency",
      description="Opacity of dialogs", keywords=("translucency", "диалоги")),
    F("translucency_popups", "windows", "Popups", "int",
      "ini", ".config/kwinrc", "Effect-translucency", "PopupMenus",
      min=0, max=100, step=1, suffix=" %", hook="effect:translucency",
      description="Opacity of popup menus", keywords=("translucency", "попрпы")),
    F("animation_factor", "windows", "Animation speed", "float",
      "ini", ".config/kdeglobals", "KDE", "AnimationDurationFactor",
      min=0.0, max=2.0, step=0.1, suffix="×",
      description="Global animation duration multiplier (0 = off)",
      keywords=("animation", "анимация", "скорость")),

    # ── Desktop & Assets ──────────────────────────────────────────────────
    F("plasma_theme", "assets", "Plasma theme", "enum",
      "ini", ".config/plasmarc", "Theme", "name",
      choices="plasma_themes", apply="cmd_config",
      command=("plasma-apply-desktoptheme", "{value}"),
      summary=True, confirm=True,
      description="Plasma desktop theme (widgets, panels)",
      keywords=("plasma", "тема", "плазма")),
    F("look_and_feel", "assets", "Look & Feel", "enum",
      "ini", ".config/kdeglobals", "KDE", "LookAndFeelPackage",
      choices="look_and_feel", apply="cmd_config",
      command=("plasma-apply-lookandfeel", "{value}"),
      summary=True, badge="live", confirm=True,
      description="Global look & feel package (may change colors, wallpaper and layout)",
      keywords=("look", "feel", "l&f", "пакет", "оформление")),
    F("icon_theme", "assets", "Icon theme", "enum",
      "ini", ".config/kdeglobals", "Icons", "Theme",
      choices="icon_themes", hook="kbuildsycoca", badge="app",
      summary=True, confirm=True,
      description="Icon theme for Plasma and Qt apps",
      keywords=("icons", "иконки")),
    F("cursor_theme", "assets", "Cursor theme", "enum",
      "ini", ".config/kcminputrc", "Mouse", "cursorTheme",
      choices="cursor_themes", apply="cmd_config",
      command=("plasma-apply-cursortheme", "{value}"),
      profile_tracked=False, summary=True,
      description="Mouse cursor theme (live only — not stored in profiles)",
      keywords=("cursor", "курсор", "мышь")),
    F("cursor_size", "assets", "Cursor size", "int",
      "ini", ".config/kcminputrc", "Mouse", "cursorSize",
      min=16, max=96, step=1, suffix=" px", apply="cmd_config",
      command=("plasma-apply-cursortheme", "--size", "{value}", "{cursor}"),
      profile_tracked=False,
      description="Mouse cursor size", keywords=("cursor", "курсор", "размер")),
    F("gtk_theme", "assets", "GTK theme", "enum",
      "ini", ".config/gtk-3.0/settings.ini", "Settings", "gtk-theme-name",
      choices="gtk_themes", apply="ini", badge="app",
      description="Theme for GTK3/GTK4 applications",
      keywords=("gtk", "тема")),
    F("gtk_dark", "assets", "GTK prefer dark", "bool",
      "ini", ".config/gtk-3.0/settings.ini", "Settings",
      "gtk-application-prefer-dark-theme", apply="ini", badge="app",
      description="Prefer dark GTK variant", keywords=("gtk", "тёмная")),
    F("kvantum_theme", "assets", "Kvantum theme", "enum",
      "ini", ".config/Kvantum/kvantum.kvconfig", "General", "theme",
      choices="kvantum_themes", apply="cmd_config",
      command=("kvantummanager", "--set", "{value}"),
      badge="app", summary=True,
      description="SVG theme used by the Kvantum Qt style",
      keywords=("kvantum", "тема", "qt")),
    F("kvantum_translucent", "assets", "Kvantum translucent windows", "bool",
      "kvantum", group="%General", key="translucent_windows",
      badge="app", advanced=True, apply="cmd_config",
      command=("kvantummanager", "--set", "{theme}"),
      description="Let Kvantum paint translucent windows",
      keywords=("kvantum", "прозрачность")),
    F("kvantum_blurring", "assets", "Kvantum blurring", "bool",
      "kvantum", group="%General", key="blurring",
      badge="app", advanced=True, apply="cmd_config",
      command=("kvantummanager", "--set", "{theme}"),
      description="Blur behind translucent Kvantum windows",
      keywords=("kvantum", "blur", "размытие")),
    F("kvantum_popup_blurring", "assets", "Kvantum popup blurring", "bool",
      "kvantum", group="%General", key="popup_blurring",
      badge="app", advanced=True, apply="cmd_config",
      command=("kvantummanager", "--set", "{theme}"),
      description="Blur behind Kvantum popups",
      keywords=("kvantum", "blur", "попапы")),

    # ── Applications ──────────────────────────────────────────────────────
    F("kitty_opacity", "apps", "Kitty opacity", "float",
      "kv", ".config/kitty/kitty.conf", key="background_opacity",
      min=0.0, max=1.0, step=0.05, apply="kv", hook="kitty",
      summary=True, description="Kitty background opacity (live reload)",
      keywords=("kitty", "терминал", "прозрачность")),
    F("kitty_blur", "apps", "Kitty blur", "int",
      "kv", ".config/kitty/kitty.conf", key="background_blur",
      min=0, max=64, step=1, apply="kv", hook="kitty",
      description="Kitty background blur (live reload)",
      keywords=("kitty", "blur", "размытие")),
    F("alacritty_opacity", "apps", "Alacritty opacity", "float",
      "toml", ".config/alacritty/alacritty.toml", key="opacity",
      path=("window",),
      min=0.0, max=1.0, step=0.05, apply="toml", badge="app",
      description="Alacritty background opacity",
      keywords=("alacritty", "терминал", "прозрачность")),
    F("alacritty_blur", "apps", "Alacritty blur", "bool",
      "toml", ".config/alacritty/alacritty.toml", key="blur",
      path=("window",),
      apply="toml", badge="app",
      description="Alacritty background blur", keywords=("alacritty", "blur")),
    F("btop_theme", "apps", "btop theme", "enum",
      "ini", ".config/btop/btop.conf", key="color_theme",
      choices="btop_themes", apply="ini", badge="app",
      description="btop color theme", keywords=("btop", "тема")),
    F("btop_rounded", "apps", "btop rounded corners", "bool",
      "ini", ".config/btop/btop.conf", key="rounded_corners",
      apply="ini", badge="app",
      description="Rounded corners in btop", keywords=("btop", "углы")),
    F("zed_mode", "apps", "Zed theme mode", "enum",
      "json", ".config/zed/settings.json", path=("theme", "mode"),
      choices="zed_modes", apply="json",
      description="Zed light/dark/system mode", keywords=("zed", "тема")),
    F("zed_light", "apps", "Zed light theme", "str",
      "json", ".config/zed/settings.json", path=("theme", "light"),
      apply="json", description="Zed theme used in light mode",
      keywords=("zed", "тема", "светлая")),
    F("zed_dark", "apps", "Zed dark theme", "str",
      "json", ".config/zed/settings.json", path=("theme", "dark"),
      apply="json", description="Zed theme used in dark mode",
      keywords=("zed", "тема", "тёмная")),
    F("vscode_theme", "apps", "VS Code theme", "str",
      "json", ".config/Code/User/settings.json",
      path=("workbench.colorTheme",), apply="json",
      description="VS Code color theme", keywords=("vscode", "code", "тема")),
    F("cursor_ide_theme", "apps", "Cursor theme", "str",
      "json", ".config/Cursor/User/settings.json",
      path=("workbench.colorTheme",), apply="json",
      description="Cursor editor color theme", keywords=("cursor", "ide", "тема")),
    F("zen_transparent", "apps", "Zen transparency", "bool",
      "userjs", ".zen/gp0y8wrv.Default (release)/user.js",
      key="browser.tabs.allow_transparent_browser", apply="userjs", badge="app",
      description="Allow transparent browser tabs in Zen",
      keywords=("zen", "прозрачность", "браузер")),
    F("zen_opacity", "apps", "Zen background opacity", "float",
      "userjs", ".zen/gp0y8wrv.Default (release)/user.js",
      key="mod.sameerasw.zen_bg_opacity",
      min=0.0, max=1.0, step=0.05, apply="userjs", badge="app",
      description="Zen background opacity", keywords=("zen", "прозрачность")),
    F("zen_window_scheme", "apps", "Zen window scheme", "enum",
      "userjs", ".zen/gp0y8wrv.Default (release)/user.js",
      key="zen.view.window.scheme", choices="zen_window_schemes",
      apply="userjs", badge="app", summary=True,
      description="Zen Browser window color scheme (dark, light or system)",
      keywords=("zen", "browser", "dark", "light", "тема", "светлая", "тёмная")),
    F("obsidian_theme", "apps", "Obsidian theme", "str",
      "json", "obsidian/vault/.obsidian/appearance.json", path=("theme",),
      apply="json", badge="app",
      description="Obsidian base theme", keywords=("obsidian", "тема")),
)


FIELD_BY_ID: dict[str, FieldDef] = {f.id: f for f in FIELDS}

__all__ = ["SECTIONS", "FIELDS", "FIELD_BY_ID", "FieldDef", "SectionDef"]
