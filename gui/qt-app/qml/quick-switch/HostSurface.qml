import QtQuick
import QtQuick.Window
import org.kde.layershell 1.0 as LayerShell

// One layer-shell surface per monitor. The shell is what makes the overlay
// sit above panels and normal windows without decorations.
//
// The attached type is `LayerShell.Window`, so the import must be aliased:
// `import org.kde.layershell 1.0 as LayerShell`. Without the alias the QML
// engine reports "Non-existent attached object".
Window {
    id: surface

    property int screenIndex: 0
    property bool primary: false
    property bool interactive: true

    screen: Qt.application.screens[screenIndex]
    color: "transparent"
    flags: Qt.FramelessWindowHint
    visible: true

    LayerShell.Window.anchors: LayerShell.Window.AnchorTop
                               | LayerShell.Window.AnchorBottom
                               | LayerShell.Window.AnchorLeft
                               | LayerShell.Window.AnchorRight
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.keyboardInteractivity: interactive
        ? LayerShell.Window.KeyboardInteractivityExclusive
        : LayerShell.Window.KeyboardInteractivityNone
    LayerShell.Window.scope: "kolira_quick_switch"
    LayerShell.Window.exclusionZone: 0

    QuickSwitchContent {
        anchors.fill: parent
        primary: surface.primary
        interactive: surface.interactive
    }
}