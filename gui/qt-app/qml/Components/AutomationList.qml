import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami.platform as Platform

// The user's automation choices (kolira automation): one row per
// kind of step a theme may need — ask / always / never — with what it does
// and what to know before choosing "always". Emits chosen(key, policy).
ColumnLayout {
    id: list
    property var items: []
    property bool glassMode: false
    // Choices made here but not stored yet (the first-run dialog keeps them).
    property var pending: ({})
    signal chosen(string key, string policy)

    readonly property var policies: ["ask", "always", "never"]
    function label(policy) {
        return policy === "always" ? qsTr("Always")
             : policy === "never" ? qsTr("Never")
             : qsTr("Ask")
    }

    spacing: Platform.Units.mediumSpacing

    Repeater {
        model: list.items
        delegate: RowLayout {
            required property var modelData
            Layout.fillWidth: true
            spacing: Platform.Units.largeSpacing
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0
                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: ThemeColors.fgColor(list.glassMode)
                    text: qsTranslate("Core", modelData.what)
                }
                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    font.pointSize: TypeScale.caption
                    color: ThemeColors.dimColor(list.glassMode)
                    text: qsTranslate("Core", modelData.note)
                }
            }
            ComboBox {
                Layout.preferredWidth: Platform.Units.gridUnit * 8
                model: list.policies.map(p => list.label(p))
                currentIndex: Math.max(0, list.policies.indexOf(list.pending[modelData.key] || modelData.policy))
                onActivated: (index) => {
                    var p = Object.assign({}, list.pending)
                    p[modelData.key] = list.policies[index]
                    list.pending = p
                    list.chosen(modelData.key, list.policies[index])
                }
            }
        }
    }
}
