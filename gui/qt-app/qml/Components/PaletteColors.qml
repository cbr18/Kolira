import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami.platform as Platform

ColumnLayout {
    id: colors
    required property var paletteData
    property bool glassMode: false
    signal colorRequested(string slot, color current)

    readonly property var slotHints: [
        qsTr("Background"), qsTr("Lighter background"), qsTr("Selection background"), qsTr("Comments, inactive"),
        qsTr("Dark foreground"), qsTr("Text"), qsTr("Light foreground"), qsTr("Lightest"),
        qsTr("Red"), qsTr("Orange"), qsTr("Yellow"), qsTr("Green"),
        qsTr("Cyan"), qsTr("Blue"), qsTr("Magenta"), qsTr("Brown")
    ]

    spacing: Platform.Units.mediumSpacing

    GridLayout {
        Layout.fillWidth: true
        columns: Math.max(2, Math.min(8, Math.floor(width / (Platform.Units.gridUnit * 5))))
        columnSpacing: Platform.Units.smallSpacing
        rowSpacing: Platform.Units.smallSpacing

        Repeater {
            model: (colors.paletteData.base || []).length
            delegate: ColumnLayout {
                id: swatch
                required property int index
                readonly property string colorValue: colors.paletteData.base[index]
                readonly property string code: index.toString(16).toUpperCase().padStart(2, "0")
                readonly property string slot: "base" + code
                Layout.fillWidth: true
                spacing: 2

                ColorSwatch {
                    objectName: "paletteSwatch_" + swatch.slot
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    swatchColor: swatch.colorValue
                    glassMode: colors.glassMode
                    text: qsTr("%1 (%2): %3").arg(swatch.slot).arg(colors.slotHints[swatch.index]).arg(swatch.colorValue)
                    onClicked: colors.colorRequested(swatch.slot, swatchColor)
                }
                Label {
                    Layout.fillWidth: true
                    text: swatch.code + " " + swatch.colorValue
                    font.family: "monospace"
                    font.pointSize: TypeScale.caption
                    color: ThemeColors.dimColor(colors.glassMode)
                    elide: Text.ElideRight
                }
            }
        }
    }

    RowLayout {
        spacing: Platform.Units.mediumSpacing

        ColorSwatch {
            objectName: "paletteAccent"
            Layout.preferredWidth: Platform.Units.gridUnit * 4
            swatchColor: colors.paletteData.accent || "transparent"
            glassMode: colors.glassMode
            text: qsTr("Accent: %1").arg(colors.paletteData.accent || "")
            onClicked: colors.colorRequested("accent", swatchColor)
        }
        Label {
            Layout.fillWidth: true
            Layout.minimumWidth: 0
            wrapMode: Text.Wrap
            text: qsTr("Accent %1 — selection, focus, active elements").arg(colors.paletteData.accent || "")
            color: ThemeColors.fgColor(colors.glassMode)
        }
    }
}
