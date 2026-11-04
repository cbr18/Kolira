"""QML controller for the Palette page (base16 + accent, see docs/PALETTE.md)."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Callable, Optional

from PySide6.QtCore import Property, QObject, QUrl, Signal, Slot

from core.palette_core import DEFAULT_TARGETS, PaletteCore
from core.protocol import Result

from .theme_inspector import _run


class PaletteController(QObject):
    """Live palette: show, gallery, import, edit, from wallpaper, apply."""

    paletteChanged = Signal()
    galleryChanged = Signal()
    busyChanged = Signal()
    toast = Signal(str, bool)

    def __init__(self, parent: Optional[QObject] = None, core: Optional[PaletteCore] = None) -> None:
        super().__init__(parent)
        self._core = core or PaletteCore()
        self._palette: dict = {}
        self._gallery: list = []
        self._busy = False
        self._wallpaper = ""
        self.refresh()

    # ── properties ───────────────────────────────────────────────────────

    @Property("QVariantMap", notify=paletteChanged)
    def palette(self) -> dict:
        """{name, variant, base[16], accent, source, warnings, kdeScheme} or {}."""
        return self._palette

    @Property(bool, notify=paletteChanged)
    def hasPalette(self) -> bool:
        return bool(self._palette)

    @Property(str, notify=paletteChanged)
    def wallpaperPath(self) -> str:
        return self._wallpaper

    @Property(str, notify=paletteChanged)
    def wallpaperUrl(self) -> str:
        return QUrl.fromLocalFile(self._wallpaper).toString() if self._wallpaper else ""

    @Property(list, notify=galleryChanged)
    def galleryItems(self) -> list:
        """{id, origin (bundled|cached|online), name, variant}."""
        return self._gallery

    @Property(list, constant=True)
    def defaultTargets(self) -> list:
        return list(DEFAULT_TARGETS)

    @Property(bool, notify=busyChanged)
    def busy(self) -> bool:
        return self._busy

    # ── helpers ──────────────────────────────────────────────────────────

    def _set_busy(self, value: bool) -> None:
        if self._busy != value:
            self._busy = value
            self.busyChanged.emit()

    def _mutate(self, work: Callable[[], Result], *, quiet: bool = False) -> None:
        """Run a palette-changing CLI call off the GUI thread, then reload."""
        if self._busy:
            self.toast.emit(self.tr("Another operation is still running"), False)
            return

        def job() -> tuple[bool, str]:
            result = work()
            return result.success, result.message

        def done(success: bool, message: str) -> None:
            self._set_busy(False)
            if not success or not quiet:
                self.toast.emit(message.splitlines()[0] if success and message else message, success)
            self.refresh()

        self._set_busy(True)
        _run(self, job, done)

    # ── slots ────────────────────────────────────────────────────────────

    @Slot()
    def refresh(self) -> None:
        self._palette = self._core.show() or {}
        self._wallpaper = self._core.current_wallpaper()
        self.paletteChanged.emit()

    @Slot(str, bool)
    def searchGallery(self, query: str, online: bool) -> None:
        holder: dict[str, Any] = {}

        def job() -> tuple[bool, str]:
            result = self._core.gallery(query, online)
            holder["items"] = (result.data or {}).get("items", [])
            return result.success, result.message

        def done(success: bool, message: str) -> None:
            self._gallery = holder.get("items", []) if success else []
            self.galleryChanged.emit()
            if not success:
                self.toast.emit(message, False)

        _run(self, job, done)

    @Slot(str)
    def importGallery(self, scheme_id: str) -> None:
        self._mutate(lambda: self._core.import_gallery(scheme_id), quiet=True)

    @Slot(str)
    def importFile(self, url: str) -> None:
        path = QUrl(url).toLocalFile() if url.startswith("file:") else url
        self._mutate(lambda: self._core.import_file(path), quiet=True)

    @Slot(str, str)
    def setSlot(self, slot: str, color: str) -> None:
        self._mutate(lambda: self._core.set_slots({slot: color}), quiet=True)

    @Slot(list)
    def fromImageColors(self, colors: list) -> None:
        """Swatches from Kirigami.ImageColors (Plasma's own algorithm)."""
        stem = Path(self._wallpaper).stem[:24] if self._wallpaper else ""
        name = f"Wallpaper {stem}".strip()
        source = f"imagecolors:{self._wallpaper}" if self._wallpaper else "imagecolors"
        swatches = [dict(c) for c in colors if isinstance(c, dict) or hasattr(c, "keys")]
        self._mutate(lambda: self._core.from_colors(swatches, name, source), quiet=True)

    @Slot()
    def fromWallpaper(self) -> None:
        """Rust k-means on the current wallpaper (no ImageColors needed)."""
        self._mutate(lambda: self._core.from_wallpaper(self._wallpaper), quiet=True)

    @Slot(list, str)
    def apply(self, targets: list, kvantum_base: str = "") -> None:
        self._mutate(lambda: self._core.apply([str(t) for t in targets], kvantum_base))
