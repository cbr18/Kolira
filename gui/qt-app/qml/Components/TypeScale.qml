pragma Singleton
import QtQuick
import org.kde.kirigami.platform as Platform

/*! Typography scale.
 *
 *  Body text used to be bound to Platform.Units.smallSpacing / mediumSpacing
 *  (4 pt / 6 pt in this environment), which rendered it unreadably small.
 *  Derive the scale from the platform font instead, so a user with a larger
 *  system font gets proportionally larger text everywhere.
 */
QtObject {
    // Platform.Theme.defaultFont tracks the application font (and the KDE
    // platform theme when it is in use). Fall back to a sane 10 pt, because a
    // font configured by pixelSize reports -1 for pointSize.
    readonly property real base: {
        var s = Platform.Theme.defaultFont.pointSize
        return (s && s > 0) ? s : 10
    }
    readonly property real body: base
    readonly property real subtitle: base + 1
    readonly property real title: base + 3
    readonly property real caption: Math.max(base > 1 ? base - 1 : base, 9)
    readonly property real monospace: base
}
