"""
Theme Controller — Dogfooding: применяет активный профиль к самому приложению.

Поддерживает: glass (прозрачность + KWin blur), paper, dark, light, system.
"""

import os
import subprocess
import sys
from typing import Optional
from PySide6.QtCore import QObject, Signal, Slot, Property, QTimer, Qt
from PySide6.QtGui import QGuiApplication, QWindow, QColor, QPalette
from PySide6.QtWidgets import QApplication

from core.paths import live_path


class ThemeController(QObject):
    """Singleton для управления темой приложения."""

    themeChanged = Signal(str)
    glassEffectChanged = Signal(bool)
    windowBlurChanged = Signal(bool)
    colorsChanged = Signal()

    _instance = None

    def __new__(cls):
        if cls._instance is None:
            cls._instance = super().__new__(cls)
            cls._instance._initialized = False
        return cls._instance

    def __init__(self):
        if self._initialized:
            return
        super().__init__()
        self._initialized = True
        self._current_theme = "system"  # system, glass, paper, dark, light
        self._is_glass = False
        self._blur_enabled = False
        self._colors: dict = {}
        self._window = None
        self._update_timer = QTimer(self)
        self._update_timer.setSingleShot(True)
        self._update_timer.timeout.connect(self._apply_theme)

        # "System" must follow live changes instead of being resolved once at
        # startup.  colorSchemeChanged exists on Qt 6.5+.
        try:
            hints = QGuiApplication.styleHints()
            if hints is not None and hasattr(hints, "colorSchemeChanged"):
                hints.colorSchemeChanged.connect(self._on_system_color_scheme_changed)
        except Exception:
            pass

    @classmethod
    def instance(cls) -> "ThemeController":
        if cls._instance is None:
            cls._instance = cls()
        return cls._instance

    @Slot(QWindow)
    def set_window(self, window: QWindow) -> None:
        """Установить главное окно для применения эффектов."""
        self._window = window

    @Property(str, notify=themeChanged)
    def currentTheme(self) -> str:
        return self._current_theme

    @currentTheme.setter
    def currentTheme(self, value: str) -> None:
        if self._current_theme != value:
            self._current_theme = value
            self.themeChanged.emit(value)
            self._schedule_apply()

    @Property(bool, notify=glassEffectChanged)
    def isGlass(self) -> bool:
        return self._is_glass

    @Property("QVariantMap", notify=colorsChanged)
    def colors(self) -> dict:
        """Current non-glass palette for QML (``ThemeColors``): hex strings plus
        ``textRgb`` (0..1 floats) for translucent tones. Empty until applied."""
        return self._colors

    @Property(bool, notify=windowBlurChanged)
    def blurEnabled(self) -> bool:
        return self._blur_enabled

    def _schedule_apply(self) -> None:
        self._update_timer.start(50)

    def reload_desktop_look(self) -> None:
        """Pick up a theme just applied by the CLI, in another process.

        Controls are drawn by the widget style, and Kvantum reads its theme once,
        when the style is created: a changed ``kvantum.kvconfig`` or a new palette
        leaves the buttons as they were. So the style is created anew, then the
        colours are read again (the same light/dark scheme sends no signal).
        ``QT_STYLE_OVERRIDE`` means the user fixed the style for this process.
        """
        app = QApplication.instance()
        if isinstance(app, QApplication) and not os.environ.get("QT_STYLE_OVERRIDE"):
            app.setStyle(_kde_widget_style())
        self._apply_theme()

    @Slot()
    def _on_system_color_scheme_changed(self) -> None:
        if self._current_theme == "system":
            self._schedule_apply()

    @Slot()
    def _apply_theme(self) -> None:
        """Применить тему к приложению (ровно один проход, без двойных emit)."""
        if self._current_theme == "system":
            # System follows the real KDE scheme colours (see _apply_system_theme);
            # resolving it to plain "dark"/"light" would lose them.
            self._apply_system_theme()
            return

        effective_theme = self._resolve_theme()

        if effective_theme == "glass":
            self._apply_glass_theme()
        elif effective_theme == "paper":
            self._apply_paper_theme()
        elif effective_theme == "dark":
            self._apply_dark_theme()
        elif effective_theme == "light":
            self._apply_light_theme()
        else:  # system
            self._apply_system_theme()

    def _apply_flags(self, glass: bool, dark: bool, colors: Optional[dict] = None) -> None:
        """Единственная точка применения: флаги + палитра + сигналы/эффекты.

        Каждый _apply_*_theme helper делегирует сюда, поэтому палитра
        применяется один раз, а сигналы эмитятся только при реальном
        изменении значения.
        """
        previous_glass = self._is_glass
        previous_blur = self._blur_enabled

        self._is_glass = glass
        self._blur_enabled = glass

        self._apply_palette(dark=dark, colors=colors)

        if previous_glass != glass:
            self.glassEffectChanged.emit(glass)
        if previous_blur != self._blur_enabled:
            self.windowBlurChanged.emit(self._blur_enabled)

        if glass:
            if self._window:
                self._enable_window_blur()
        else:
            self._disable_window_blur()

    def _system_is_dark(self) -> bool:
        """Detected system color scheme (True = dark)."""
        hints = QGuiApplication.styleHints()
        return hints is not None and hints.colorScheme() == Qt.ColorMode.Dark

    def _resolve_theme(self) -> str:
        """Разрешить тему с учётом системной."""
        if self._current_theme != "system":
            return self._current_theme

        kde = _kde_scheme_colors()
        if kde is not None:
            return "dark" if _luminance(kde["window"]) < 0.5 else "light"

        palette = QGuiApplication.palette()
        bg = palette.color(QPalette.ColorRole.Window)
        if not bg.isValid():
            # Fallback: detect from platform theme hint
            theme = QGuiApplication.styleHints().colorScheme()
            return "dark" if theme == Qt.ColorMode.Dark else "light"
        luminance = (0.299 * bg.red() + 0.587 * bg.green() + 0.114 * bg.blue()) / 255
        return "dark" if luminance < 0.5 else "light"

    def _apply_palette(self, dark: bool, colors: Optional[dict] = None) -> None:
        """Switch the whole application palette (drives QQC2 + Kirigami.Theme).

        Without this, glass mode ends up "half dark": our explicit QML colors
        turn light while the QQC2 controls keep following the light palette.
        """
        app = QGuiApplication.instance()
        if not isinstance(app, QGuiApplication):  # None or a bare QCoreApplication
            return

        if dark:
            window = QColor(32, 33, 36)
            base = QColor(24, 25, 28)
            alternate = QColor(41, 43, 47)
            button = QColor(45, 47, 52)
            text = QColor(240, 240, 240)
            highlight = QColor(61, 174, 233)
            highlighted_text = QColor(0, 0, 0)
            disabled = QColor(150, 150, 150)
        else:
            window = QColor(239, 240, 243)
            base = QColor(255, 255, 255)
            alternate = QColor(245, 245, 245)
            button = QColor(239, 240, 243)
            text = QColor(31, 31, 31)
            highlight = QColor(41, 128, 185)
            highlighted_text = QColor(255, 255, 255)
            disabled = QColor(136, 136, 136)

        if colors:
            # Real KDE colour scheme (System mode): the app looks like the
            # rest of the desktop instead of a generic light/dark palette.
            window = colors["window"]
            base = colors["view"]
            alternate = colors["view_alt"]
            button = colors["button"]
            text = colors["text"]
            highlight = colors["highlight"]
            highlighted_text = colors["highlighted_text"]
            disabled = colors["disabled"]

        self._colors = {
            "dark": dark,
            "window": window.name(),
            "view": base.name(),
            "text": text.name(),
            "textRgb": [text.redF(), text.greenF(), text.blueF()],
            "highlight": highlight.name(),
            "disabled": disabled.name(),
        }
        self.colorsChanged.emit()

        placeholder = QColor(text)
        placeholder.setAlpha(128)

        palette = app.palette()
        mapping = {
            QPalette.ColorRole.Window: window,
            QPalette.ColorRole.WindowText: text,
            QPalette.ColorRole.Base: base,
            QPalette.ColorRole.AlternateBase: alternate,
            QPalette.ColorRole.ToolTipBase: base,
            QPalette.ColorRole.ToolTipText: text,
            QPalette.ColorRole.Text: text,
            QPalette.ColorRole.PlaceholderText: placeholder,
            QPalette.ColorRole.Button: button,
            QPalette.ColorRole.ButtonText: text,
            QPalette.ColorRole.BrightText: QColor(255, 0, 0),
            QPalette.ColorRole.Light: alternate,
            QPalette.ColorRole.Midlight: alternate,
            QPalette.ColorRole.Highlight: highlight,
            QPalette.ColorRole.HighlightedText: highlighted_text,
            QPalette.ColorRole.Link: highlight,
        }
        for role, color in mapping.items():
            palette.setColor(QPalette.ColorGroup.All, role, color)
            palette.setColor(QPalette.ColorGroup.Disabled, role, disabled)
            if role == QPalette.ColorRole.Window:
                palette.setColor(QPalette.ColorGroup.Disabled, QPalette.ColorRole.WindowText, disabled)

        app.setPalette(palette)

    def _apply_glass_theme(self) -> None:
        """Glass theme: прозрачность окна + KWin blur + тёмная палитра."""
        self._apply_flags(glass=True, dark=True)

    def _enable_window_blur(self) -> None:
        """Request KWin blur for the (translucent) window.

        The window colour/transparency itself is owned by QML (the QML
        `color` binding) — we must not call QWindow.setColor() here or it
        would fight (and win over) the QML binding.
        """
        if not self._window:
            return

        # Для Wayland: запрашиваем blur через KWin (non-blocking)
        QTimer.singleShot(100, self._request_kwin_blur)

    def _request_kwin_blur(self) -> None:
        """Запросить KWin blur через D-Bus (works on Wayland)."""
        if os.environ.get("KTM_THEME_NO_APPLY") == "1":
            return
        try:
            subprocess.run([
                "qdbus6", "org.kde.KWin", "/Effects",
                "org.kde.kwin.Effects.loadEffect", "blur"
            ], capture_output=True, timeout=2)
        except (OSError, subprocess.SubprocessError):
            pass

    def _apply_paper_theme(self) -> None:
        """Paper theme: светлая, без прозрачности."""
        self._apply_flags(glass=False, dark=False)

    def _disable_window_blur(self) -> None:
        """No-op: window transparency is fully driven by QML (see above)."""
        return

    def _apply_dark_theme(self) -> None:
        """Тёмная тема."""
        self._apply_flags(glass=False, dark=True)

    def _apply_light_theme(self) -> None:
        """Светлая тема."""
        self._apply_flags(glass=False, dark=False)

    def _apply_system_theme(self) -> None:
        """Системная тема (follow system)."""
        kde = _kde_scheme_colors()
        if kde is not None:
            self._apply_flags(glass=False, dark=_luminance(kde["window"]) < 0.5, colors=kde)
            return
        self._apply_flags(glass=False, dark=self._system_is_dark())

    @staticmethod
    def _theme_for_profile(profile_name: str) -> str:
        normalized = (profile_name or "").strip().casefold()
        if normalized == "glass":
            return "glass"
        if normalized == "paper":
            return "paper"
        return ""

    @Slot(str)
    def sync_with_active_profile(self, profile_name: str) -> None:
        """Синхронизировать тему приложения с активным профилем."""
        theme = self._theme_for_profile(profile_name)
        if theme:
            self.currentTheme = theme
        else:
            self._detect_from_kdeglobals()

    def _detect_from_kdeglobals(self) -> None:
        """Определить тему из kdeglobals."""
        try:
            import configparser

            kdeglobals = live_path(".config/kdeglobals")
            if kdeglobals.exists():
                config = configparser.ConfigParser()
                config.read(kdeglobals)
                color_scheme = config.get("General", "ColorScheme", fallback="").lower()
                if "paper" in color_scheme:
                    self.currentTheme = "paper"
                elif "glass" in color_scheme:
                    self.currentTheme = "glass"
                elif "dark" in color_scheme:
                    self.currentTheme = "dark"
                elif "light" in color_scheme:
                    self.currentTheme = "light"
                else:
                    # Any other scheme (Everforest, Nordic, …): follow the
                    # scheme's own colours rather than keep a stale mode.
                    self.currentTheme = "system"
        except Exception:
            pass

    @Slot()
    def toggle_glass(self) -> None:
        """Переключить glass эффект."""
        if self._is_glass:
            self.currentTheme = "paper"
        else:
            self.currentTheme = "glass"


def _luminance(color: QColor) -> float:
    return (0.299 * color.red() + 0.587 * color.green() + 0.114 * color.blue()) / 255


def _kde_widget_style() -> str:
    """``[KDE] widgetStyle`` of the live kdeglobals (KDE's default: Breeze)."""
    import configparser

    config = configparser.ConfigParser(interpolation=None, strict=False)
    try:
        config.read(live_path(".config/kdeglobals"), encoding="utf-8")
    except (OSError, configparser.Error):
        return "Breeze"
    return config.get("KDE", "widgetStyle", fallback="").strip() or "Breeze"


def reload_app_look() -> None:
    """Best-effort :meth:`ThemeController.reload_desktop_look` for controllers."""
    try:
        ThemeController.instance().reload_desktop_look()
    except Exception as exc:
        print(f"Could not reload the app's look: {exc}", file=sys.stderr)


def _kde_scheme_colors() -> Optional[dict]:
    """Palette roles from the live kdeglobals ``[Colors:*]`` groups, or None."""
    import configparser

    def rgb(group: str, key: str) -> Optional[QColor]:
        raw = config.get(group, key, fallback="")
        parts = [p.strip() for p in raw.split(",")]
        if len(parts) < 3 or not all(p.isdigit() for p in parts[:3]):
            return None
        return QColor(*(min(255, int(p)) for p in parts[:3]))

    try:
        config = configparser.ConfigParser(interpolation=None, strict=False)
        config.read(live_path(".config/kdeglobals"), encoding="utf-8")
    except (OSError, configparser.Error):
        return None
    colors = {
        "window": rgb("Colors:Window", "BackgroundNormal"),
        "view": rgb("Colors:View", "BackgroundNormal"),
        "view_alt": rgb("Colors:View", "BackgroundAlternate"),
        "button": rgb("Colors:Button", "BackgroundNormal"),
        "text": rgb("Colors:Window", "ForegroundNormal"),
        "highlight": rgb("Colors:Selection", "BackgroundNormal"),
        "highlighted_text": rgb("Colors:Selection", "ForegroundNormal"),
        "disabled": rgb("Colors:Window", "ForegroundInactive"),
    }
    if any(value is None for value in colors.values()):
        return None
    return colors  # type: ignore[return-value]


def register_theme_controller(engine) -> ThemeController:
    """Зарегистрировать ThemeController в QML."""
    controller = ThemeController.instance()
    engine.rootContext().setContextProperty("themeController", controller)
    return controller