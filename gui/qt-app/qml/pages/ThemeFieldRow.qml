import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami
import org.kde.kirigami.platform as Platform
import Components 1.0

/*! One editable parameter of the theme.
 *
 *  The control is chosen from the schema ``type``; every write goes through
 *  ``themeInspector`` (live only, undoable). Profile scopes render the same
 *  rows but the controller rejects writes.
 */
ColumnLayout {
    id: row

    property var fieldData: ({})
    property bool glassMode: false
    property bool showSection: false
    property string pendingValue: ""

    Layout.fillWidth: true
    spacing: 2

    readonly property string fieldId: fieldData.id || ""
    readonly property string fieldType: fieldData.type || "str"
    readonly property var choices: fieldData.choices || []
    readonly property bool isReadOnly: themeInspector.readOnly
    readonly property color fg: ThemeColors.fgColor(row.glassMode)
    readonly property color dim: ThemeColors.dimColor(row.glassMode)

    function choiceIndex(value) {
        for (var i = 0; i < choices.length; ++i) {
            if (choices[i].value === value)
                return i
        }
        return -1
    }

    function fontFamily(value) {
        var text = String(value || "")
        var parts = text.split(",")
        return parts.length > 0 ? parts[0].trim() : ""
    }

    function fontSize(value) {
        var text = String(value || "")
        var parts = text.split(",")
        if (parts.length < 2)
            return 10
        var size = parseFloat(parts[1])
        return isNaN(size) || size <= 0 ? 10 : size
    }

    function applyFont() {
        var family = fontFamilyField.text.trim()
        if (family.length === 0)
            return
        themeInspector.setFont(row.fieldId, family, fontSize.value)
    }

    function colorHex(value) {
        function channel(part) {
            var text = Math.round(Math.max(0, Math.min(1, part)) * 255).toString(16)
            return text.length < 2 ? "0" + text : text
        }
        return "#" + channel(value.r) + channel(value.g) + channel(value.b)
    }

    // ─── Label line ───────────────────────────────────────────
    RowLayout {
        Layout.fillWidth: true
        spacing: Platform.Units.smallSpacing

        Label {
            text: {
                var label = row.fieldData.label || row.fieldId
                if (row.showSection && row.fieldData.sectionTitle)
                    return row.fieldData.sectionTitle + " · " + label
                return label
            }
            color: row.fg
            font.bold: true
            elide: Text.ElideRight
            Layout.fillWidth: true
        }

        // A filled dot means staged; a ring means different from the profile.
        Rectangle {
            visible: row.fieldData.staged === true
            width: 9
            height: 9
            radius: 4.5
            activeFocusOnTab: true
            color: ThemeColors.warning()
            border.width: activeFocus ? 2 : 0
            border.color: ThemeColors.accent()
            Accessible.role: Accessible.StaticText
            Accessible.name: qsTr("Staged — press Apply to write it")
            ToolTip.text: Accessible.name
            ToolTip.visible: stagedHover.hovered || activeFocus
            HoverHandler { id: stagedHover }
        }

        Rectangle {
            visible: row.fieldData.differs === true
            width: 9
            height: 9
            radius: 4.5
            activeFocusOnTab: true
            color: "transparent"
            border.width: 2
            border.color: ThemeColors.accent()
            Accessible.role: Accessible.StaticText
            Accessible.name: qsTr("Differs from %1").arg(themeInspector.activeProfile || qsTr("profile"))
            ToolTip.text: Accessible.name
            ToolTip.visible: differsHover.hovered || activeFocus
            HoverHandler { id: differsHover }
        }

        Rectangle {
            visible: (row.fieldData.badge || "live") !== "live"
            radius: 3
            color: row.glassMode ? Qt.rgba(1, 1, 1, 0.12) : Qt.rgba(0, 0, 0, 0.06)
            implicitWidth: badgeLabel.implicitWidth + 8
            implicitHeight: badgeLabel.implicitHeight + 2
            Label {
                id: badgeLabel
                anchors.centerIn: parent
                text: row.fieldData.badge === "app" ? qsTr("app restart")
                    : row.fieldData.badge === "new" ? qsTr("relogin")
                    : row.fieldData.badge
                font.pointSize: TypeScale.caption
                color: row.dim
            }
        }

        ToolButton {
            icon.name: "edit-undo"
            visible: row.fieldData.differs === true && !row.isReadOnly
            enabled: !themeInspector.busy
            ToolTip.text: qsTr("Restore the value stored in the profile")
            ToolTip.visible: hovered || activeFocus
            onClicked: themeInspector.resetToProfile(row.fieldId)
        }

        ToolButton {
            icon.name: "dialog-information"
            ToolTip.text: {
                var source = row.fieldData.file || ""
                if (row.fieldData.group)
                    source += " [" + row.fieldData.group + "]"
                if (row.fieldData.key)
                    source += " " + row.fieldData.key
                var text = row.fieldData.description || ""
                if (source)
                    text += (text ? "\n" : "") + source
                if (row.fieldData.profileDisplay)
                    text += "\n" + qsTr("Profile: %1").arg(row.fieldData.profileDisplay)
                return text
            }
            ToolTip.visible: hovered || activeFocus
            onClicked: themeInspector.openFile(row.fieldData.file || "")
        }
    }

    // ─── Control ──────────────────────────────────────────────
    Switch {
        visible: row.fieldType === "bool"
        checked: row.fieldData.value === true
        enabled: !row.isReadOnly && !themeInspector.busy
        text: checked ? qsTr("On") : qsTr("Off")
        onClicked: themeInspector.setBool(row.fieldId, checked)
    }

    RowLayout {
        visible: row.fieldType === "int" || row.fieldType === "float"
        Layout.fillWidth: true
        spacing: Platform.Units.smallSpacing

        Slider {
            id: slider
            Layout.fillWidth: true
            from: row.fieldData.min !== null && row.fieldData.min !== undefined ? row.fieldData.min : 0
            to: row.fieldData.max !== null && row.fieldData.max !== undefined ? row.fieldData.max : 100
            stepSize: row.fieldData.step !== null && row.fieldData.step !== undefined ? row.fieldData.step : 1
            value: Number(row.fieldData.value !== null && row.fieldData.value !== undefined
                          ? row.fieldData.value : from)
            enabled: !row.isReadOnly && !themeInspector.busy
            onMoved: writeTimer.restart()

            Timer {
                id: writeTimer
                interval: 300
                onTriggered: {
                    if (row.fieldType === "int")
                        themeInspector.setInt(row.fieldId, Math.round(slider.value))
                    else
                        themeInspector.setFloat(row.fieldId, slider.value)
                }
            }
        }

        Label {
            text: row.fieldType === "int"
                ? Math.round(slider.value) + (row.fieldData.suffix || "")
                : Number(slider.value).toFixed(2) + (row.fieldData.suffix || "")
            color: row.dim
            horizontalAlignment: Text.AlignRight
            Layout.preferredWidth: Platform.Units.gridUnit * 4
        }
    }

    ComboBox {
        visible: row.fieldType === "enum"
        Layout.fillWidth: true
        model: row.choices
        textRole: "label"
        valueRole: "value"
        currentIndex: row.choiceIndex(row.fieldData.value)
        enabled: !row.isReadOnly && !themeInspector.busy
        onActivated: function(index) {
            if (index < 0 || index >= row.choices.length)
                return
            var value = row.choices[index].value
            if (row.fieldData.confirm === true) {
                row.pendingValue = value
                confirmDialog.text = qsTr("Apply “%1”?").arg(row.fieldData.label || value)
                confirmDialog.informativeText = qsTr("This applies several settings at once and may replace the wallpaper, colors or panel layout.")
                confirmDialog.open()
            } else {
                themeInspector.setText(row.fieldId, value)
            }
        }
    }

    RowLayout {
        visible: row.fieldType === "font"
        Layout.fillWidth: true
        spacing: Platform.Units.smallSpacing

        // A ComboBox over every installed family (800+ entries) makes the
        // default Menu build every item synchronously — that froze the page
        // with Qt 6.12. An editable field keeps the same "family + size"
        // contract and stays instant.
        TextField {
            id: fontFamilyField
            Layout.fillWidth: true
            text: row.fontFamily(row.fieldData.value)
            placeholderText: qsTr("Font family, e.g. Noto Sans")
            enabled: !row.isReadOnly && !themeInspector.busy
            onEditingFinished: row.applyFont()
        }

        SpinBox {
            id: fontSize
            from: 6
            to: 40
            value: row.fontSize(row.fieldData.value)
            enabled: !row.isReadOnly && !themeInspector.busy
            onValueModified: row.applyFont()
        }

        Button {
            text: qsTr("Set")
            enabled: !row.isReadOnly && !themeInspector.busy
            onClicked: row.applyFont()
        }
    }

    RowLayout {
        visible: row.fieldType === "color"
        Layout.fillWidth: true
        spacing: Platform.Units.smallSpacing

        Rectangle {
            id: colorSwatch
            width: Platform.Units.gridUnit * 3
            height: Platform.Units.gridUnit * 1.4
            radius: 4
            activeFocusOnTab: !row.isReadOnly
            color: row.fieldData.display || "transparent"
            border.width: activeFocus ? 2 : 1
            border.color: activeFocus
                ? ThemeColors.accent() : ThemeColors.subtleBorder(row.glassMode)
            Accessible.role: Accessible.Button
            Accessible.name: qsTr("%1: %2").arg(row.fieldData.label || row.fieldId)
                .arg(row.fieldData.display || qsTr("not set"))

            function openPicker() {
                if (row.isReadOnly || themeInspector.busy)
                    return
                colorDialog.selectedColor = row.fieldData.display
                colorDialog.open()
            }

            MouseArea {
                anchors.fill: parent
                enabled: !row.isReadOnly && !themeInspector.busy
                cursorShape: Qt.PointingHandCursor
                onClicked: colorSwatch.openPicker()
            }
            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter
                        || event.key === Qt.Key_Space) {
                    colorSwatch.openPicker()
                    event.accepted = true
                }
            }
        }

        Label {
            text: row.fieldData.display || qsTr("not set")
            color: row.dim
        }

        Item { Layout.fillWidth: true }

        ColorDialog {
            id: colorDialog
            title: qsTr("Choose %1").arg(row.fieldData.label || "")
            onAccepted: themeInspector.setColor(row.fieldId, row.colorHex(selectedColor))
        }
    }

    TextField {
        visible: row.fieldType === "str"
        Layout.fillWidth: true
        text: row.fieldData.value === null || row.fieldData.value === undefined
            ? "" : String(row.fieldData.value)
        placeholderText: qsTr("Not set")
        enabled: !row.isReadOnly && !themeInspector.busy
        onEditingFinished: {
            if (text !== String(row.fieldData.value === null || row.fieldData.value === undefined
                                ? "" : row.fieldData.value))
                themeInspector.setText(row.fieldId, text)
        }
    }

    Label {
        Layout.fillWidth: true
        visible: row.fieldData.differs === true && (row.fieldData.profileDisplay || "") !== ""
        text: qsTr("Profile: %1").arg(row.fieldData.profileDisplay || "")
        color: row.dim
        font.pointSize: TypeScale.caption
        wrapMode: Text.WordWrap
    }

    Label {
        Layout.fillWidth: true
        visible: text !== ""
        text: row.fieldData.description || ""
        color: ThemeColors.dimColorSoft(row.glassMode)
        font.pointSize: TypeScale.caption
        wrapMode: Text.WordWrap
    }

    Kirigami.Separator {
        Layout.fillWidth: true
        visible: !row.isReadOnly
        opacity: 0.4
    }

    MessageDialog {
        id: confirmDialog
        title: qsTr("Apply theme component?")
        buttons: MessageDialog.Ok | MessageDialog.Cancel
        onAccepted: themeInspector.setText(row.fieldId, row.pendingValue)
    }
}
