import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami
import Components 1.0

Kirigami.ScrollablePage {
    id: page
    objectName: "savedThemeDetails"
    required property string profileName
    property string description
    property string savedAt
    property bool allowDelete: true
    readonly property var summary: profileController.summaries[profileName] || ({})
    readonly property var fields: summary.detailFields || ({})
    readonly property string matchState: (profileController.liveAppearance.profiles || {})[profileName] || ""
    signal applyRequested(string name)
    signal editRequested(string name)
    signal dependenciesRequested(string name)
    signal duplicateRequested(string name)
    signal exportRequested(string name)
    signal deleteRequested(string name)
    signal returnFocus
    StackView.onRemoved: page.returnFocus()

    title: profileName

    function displayValue(field) {
        if (typeof field.value === "boolean")
            return field.value ? qsTr("On") : qsTr("Off")
        if (field.unit === "font")
            return String(field.value).split(",")[0]
        return String(field.value)
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        ActionFlow {
            Layout.fillWidth: true
            Button {
                objectName: "savedThemeApply"
                text: qsTr("Apply")
                icon.name: "dialog-ok-apply"
                enabled: !profileController.busy
                onClicked: page.applyRequested(page.profileName)
            }
            Button {
                text: qsTr("Open in editor")
                icon.name: "document-edit"
                enabled: !profileController.busy
                onClicked: page.editRequested(page.profileName)
            }
            ToolButton {
                text: qsTr("More actions")
                icon.name: "overflow-menu"
                Accessible.name: text
                onClicked: extraActions.open()
                Menu {
                    id: extraActions
                    MenuItem { text: qsTr("Duplicate"); onTriggered: page.duplicateRequested(page.profileName) }
                    MenuItem { text: qsTr("Export…"); onTriggered: page.exportRequested(page.profileName) }
                    MenuItem {
                        text: qsTr("Delete")
                        visible: page.allowDelete
                        enabled: !profileController.busy
                        onTriggered: page.deleteRequested(page.profileName)
                    }
                }
            }
        }

        Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            visible: text !== ""
            text: page.matchState === "match" ? qsTr("Matches Live")
                : page.profileName === historySchedule.lastProfileApply
                    ? (page.matchState === "different" ? qsTr("Live has changed since application") : qsTr("Last applied"))
                    : ""
        }

        ThemeAppearancePreview {
            Layout.fillWidth: true
            Layout.preferredHeight: Kirigami.Units.gridUnit * 14
            summary: page.summary
        }

        Label {
            Layout.fillWidth: true
            text: qsTr("Approximate appearance preview")
            wrapMode: Text.Wrap
        }

        Label {
            Layout.fillWidth: true
            visible: !!page.summary.author
            text: visible ? qsTr("Author: %1").arg(page.summary.author) : ""
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
        }

        Label {
            Layout.fillWidth: true
            visible: page.description.length > 0
            text: page.description
            textFormat: Text.PlainText
            wrapMode: Text.WordWrap
        }

        Label {
            Layout.fillWidth: true
            visible: text.length > 0
            text: {
                if (!page.savedAt)
                    return ""
                const date = new Date(page.savedAt)
                return isNaN(date.getTime()) ? "" : qsTr("Saved %1")
                    .arg(date.toLocaleDateString(Qt.locale(), Locale.ShortFormat))
            }
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: !!page.summary.error
            type: Kirigami.MessageType.Warning
            text: qsTr("Some theme details could not be read.")
        }

        ColumnLayout {
            Layout.fillWidth: true
            Label {
                Layout.fillWidth: true
                text: (page.summary.unavailableResources || []).length > 0
                    ? qsTr("Some resources are unavailable")
                    : page.summary.unavailableResources ? qsTr("No missing resources detected") : ""
                visible: text !== ""
                color: ThemeColors.warning()
                wrapMode: Text.WordWrap
            }
            Button {
                text: qsTr("Dependencies")
                icon.name: "package-x-generic"
                onClicked: page.dependenciesRequested(page.profileName)
            }
        }

        Repeater {
            model: [
                { title: qsTr("KDE and Plasma"), fields: page.fields.desktop || [] },
                { title: qsTr("Windows and effects"), fields: page.fields.windows || [] },
                { title: qsTr("Qt and GTK"), fields: page.fields.toolkits || [] },
                { title: qsTr("Palette"), fields: (page.fields.palette || []).concat(page.summary.base16Name
                    ? [{name: qsTr("Base16 palette"), value: page.summary.base16Name}] : []) },
                { title: qsTr("Applications"), fields: (page.fields.applications || []).concat((page.summary.detailApps || []).map(app => ({name: app}))) },
                { title: qsTr("Wallpapers"), fields: page.summary.wallpaperName
                    ? [{name: qsTr("Wallpaper"), value: page.summary.wallpaperName}] : [] },
            ]

            delegate: ColumnLayout {
                id: group
                required property var modelData
                readonly property var fields: modelData.fields
                visible: fields.length > 0
                Layout.fillWidth: true

                Button {
                    id: disclosure
                    Layout.fillWidth: true
                    text: group.modelData.title
                    checkable: true
                    checked: false
                    icon.name: checked ? "go-down" : "go-next"
                }

                Repeater {
                    model: disclosure.checked ? group.fields : []
                    delegate: Label {
                        required property var modelData
                        Layout.fillWidth: true
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        text: {
                            const name = modelData.name || qsTranslate("ThemeFields", modelData.label)
                            return modelData.value === undefined ? name : qsTr("%1: %2").arg(name).arg(page.displayValue(modelData))
                        }
                    }
                }
            }
        }
    }
}
