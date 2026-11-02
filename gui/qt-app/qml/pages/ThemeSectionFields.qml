import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami
import org.kde.kirigami.platform as Platform
import Components 1.0

/*! All rows of one inspector section (live values or a profile snapshot). */
ColumnLayout {
    id: root

    property string sectionId: ""
    property bool glassMode: false
    property bool showAdvanced: false
    property string filterText: ""
    property string highlightFieldId: ""
    property var fields: []

    readonly property var filteredFields: {
        const query = root.filterText.trim().toLocaleLowerCase()
        const result = []
        for (let i = 0; i < root.fields.length; i++) {
            const field = root.fields[i]
            if (!root.showAdvanced && field.advanced)
                continue
            const haystack = [field.label, field.id, field.key].join(" ").toLocaleLowerCase()
            if (query === "" || haystack.includes(query))
                result.push(field)
        }
        return result
    }

    Layout.fillWidth: true
    spacing: Platform.Units.smallSpacing

    function reload() {
        fields = root.visible ? themeInspector.fieldsFor(sectionId) : []
    }

    Component.onCompleted: reload()
    onVisibleChanged: reload()

    Connections {
        target: themeInspector
        function onFieldsChanged() { root.reload() }
    }

    Repeater {
        model: root.filteredFields

        delegate: Rectangle {
            required property var modelData
            readonly property bool highlighted: root.highlightFieldId !== ""
                && root.highlightFieldId === modelData.id
            Layout.fillWidth: true
            implicitHeight: fieldRow.implicitHeight + 8
            radius: Platform.Units.smallSpacing
            color: highlighted ? ThemeColors.cardHoverBg(root.glassMode) : "transparent"
            border.width: highlighted ? 1 : 0
            border.color: ThemeColors.accent()

            ThemeFieldRow {
                id: fieldRow
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 4
                fieldData: modelData
                glassMode: root.glassMode
            }
        }
    }

    Label {
        Layout.fillWidth: true
        visible: root.filteredFields.length === 0
        text: root.fields.length === 0
            ? qsTr("No fields in this section.")
            : qsTr("No settings match this filter.")
        color: ThemeColors.dimColor(root.glassMode)
    }
}
