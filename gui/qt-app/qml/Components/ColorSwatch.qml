import QtQuick
import QtQuick.Controls
import org.kde.kirigami.platform as Platform

AbstractButton {
    id: control
    property color swatchColor: "transparent"
    property bool glassMode: false
    implicitWidth: Platform.Units.gridUnit * 4
    implicitHeight: Platform.Units.gridUnit * 2.2
    focusPolicy: Qt.StrongFocus
    Keys.onReturnPressed: if (enabled) clicked()
    Keys.onEnterPressed: if (enabled) clicked()
    Accessible.name: text
    Accessible.onPressAction: clicked()
    ToolTip.visible: hovered || visualFocus
    ToolTip.text: text
    background: Rectangle {
        color: control.swatchColor
        radius: Platform.Units.smallSpacing
        border.width: control.visualFocus ? 3 : 1
        border.color: control.visualFocus ? ThemeColors.accent() : ThemeColors.cardBorder(control.glassMode)
        Rectangle {
            anchors.fill: parent
            anchors.margins: 3
            visible: control.visualFocus
            color: "transparent"
            radius: Platform.Units.smallSpacing
            border.width: 1
            border.color: ThemeColors.fgColor(control.glassMode)
        }
    }
}
