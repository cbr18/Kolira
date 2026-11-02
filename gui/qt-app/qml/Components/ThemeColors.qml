pragma Singleton
import QtQuick
import org.kde.kirigami 2.20 as Kirigami

/*! Single source of truth for the app's colours.
 *
 *  Every accessor takes the current `glass` mode as an argument so the
 *  singleton stays side-effect free; QML binding dependency tracking still
 *  follows the property reads performed inside these functions.
 *
 *  `safeColor()` exists because PySide6's bundled Qt combined with the system
 *  Kirigami plugin can return invalid Kirigami.Theme colours.
 */
QtObject {
    function safeColor(role, fallback) {
        var c = Kirigami.Theme[role]
        return (c && c.valid) ? c : fallback
    }

    // ── Surfaces ────────────────────────────────────────────────
    readonly property color glassPanelColor: Qt.rgba(0.09, 0.10, 0.12, 0.85)

    function glassPanel() { return glassPanelColor }
    function windowBg(glass) { return glass ? "transparent" : safeColor("backgroundColor", "#eff0f3") }
    function pageBg(glass) { return glass ? glassPanelColor : safeColor("backgroundColor", "#eff0f3") }
    function cardBg(glass) { return glass ? Qt.rgba(1, 1, 1, 0.16) : safeColor("viewColor", "#ffffff") }
    function cardHoverBg(glass) { return glass ? Qt.rgba(1, 1, 1, 0.20) : Qt.rgba(0, 0, 0, 0.04) }
    function cardBorder(glass) { return glass ? Qt.rgba(1, 1, 1, 0.18) : Qt.rgba(0.15, 0.15, 0.15, 0.18) }
    function subtleBorder(glass) { return glass ? Qt.rgba(1, 1, 1, 0.14) : Qt.rgba(0.15, 0.15, 0.15, 0.14) }
    function nestedBg(glass) { return glass ? Qt.rgba(1, 1, 1, 0.12) : safeColor("viewColor", "#ffffff") }
    function nestedBorder(glass) { return glass ? Qt.rgba(1, 1, 1, 0.18) : Qt.rgba(0.15, 0.15, 0.15, 0.18) }
    // Dialogs sit above the page in their own popup window, so they need an
    // opaque surface (the page's glass tint is translucent).
    function dialogBg(glass) { return glass ? "#1f2126" : safeColor("backgroundColor", "#eff0f3") }
    function codeBg(glass) { return glass ? Qt.rgba(0, 0, 0, 0.40) : Qt.rgba(0, 0, 0, 0.05) }

    // ── Foreground ──────────────────────────────────────────────
    function fgColor(glass) { return glass ? "#ffffff" : safeColor("textColor", "#1f1f1f") }
    function fgColorSoft(glass) { return glass ? "#f0f0f0" : safeColor("textColor", "#1f1f1f") }
    function dimColor(glass) { return glass ? Qt.rgba(1, 1, 1, 0.80) : Qt.rgba(0.15, 0.15, 0.15, 0.65) }
    function dimColorSoft(glass) { return glass ? Qt.rgba(1, 1, 1, 0.6) : Qt.rgba(0.15, 0.15, 0.15, 0.6) }
    function disabledColor(glass) { return glass ? Qt.rgba(1, 1, 1, 0.45) : safeColor("disabledTextColor", "#888888") }

    // ── Accents ─────────────────────────────────────────────────
    function accent() { return safeColor("highlightColor", "#3daee9") }
    function positive() { return safeColor("positiveTextColor", "#27ae60") }
    function warning() { return safeColor("warningTextColor", "#f39c12") }
    function negative() { return safeColor("negativeTextColor", "#e74c3c") }
}
