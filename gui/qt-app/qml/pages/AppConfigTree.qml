import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami
import org.kde.kirigami.platform as Platform
import Components 1.0

/*! Application config tree: «приложение → файл → ключ».

 *  Shows what the tracked applications store, file by file, with
 *  pencil-editing of values (staged, applied by the inspector's Apply).
 *  ``appId`` set → only that application; empty → all of them.
 *  Host page owns the scroll surface; this is just the content.
 */
ColumnLayout {
    id: tree

    property string appId: ""
    property bool glassMode: false

    Layout.fillWidth: true
    spacing: 2

    readonly property var entries: {
        if (tree.appId === "")
            return themeInspector.appConfigs
        const filtered = []
        const all = themeInspector.appConfigs
        for (let i = 0; i < all.length; i++) {
            if (all[i].id === tree.appId)
                filtered.push(all[i])
        }
        return filtered
    }

    Repeater {
        model: tree.entries

        ColumnLayout {
            required property var modelData
            Layout.fillWidth: true
            spacing: 2

            RowLayout {
                Layout.fillWidth: true
                spacing: Platform.Units.smallSpacing
                Kirigami.Icon {
                    source: modelData.icon
                    Layout.preferredWidth: Platform.Units.iconSizes.small
                    Layout.preferredHeight: Platform.Units.iconSizes.small
                }
                Label {
                    text: modelData.name
                    font.bold: true
                    color: ThemeColors.fgColor(tree.glassMode)
                }
            }

            Repeater {
                model: modelData.files

                ColumnLayout {
                    required property var modelData
                    // Алиас для вложенного Repeater строк: там
                    // modelData перекрывается строкой параметра.
                    property var fileData: modelData
                    Layout.fillWidth: true
                    Layout.leftMargin: Platform.Units.largeSpacing
                    spacing: 1

                    Label {
                        Layout.fillWidth: true
                        text: modelData.relPath
                        font.family: "monospace"
                        font.pointSize: TypeScale.caption
                        color: ThemeColors.fgColorSoft(tree.glassMode)
                        elide: Text.ElideMiddle
                    }

                    Repeater {
                        model: modelData.params

                        RowLayout {
                            required property var modelData
                            Layout.fillWidth: true
                            Layout.leftMargin: Platform.Units.largeSpacing

                            Label {
                                Layout.fillWidth: true
                                Layout.maximumWidth: 360
                                text: modelData.key
                                font.family: "monospace"
                                font.pointSize: TypeScale.caption
                                color: ThemeColors.fgColor(tree.glassMode)
                                elide: Text.ElideRight
                            }
                            Label {
                                Layout.fillWidth: true
                                text: modelData.value
                                font.family: "monospace"
                                font.pointSize: TypeScale.caption
                                color: ThemeColors.dimColor(tree.glassMode)
                                elide: Text.ElideRight
                            }

                            ToolButton {
                                text: qsTr("Edit value")
                                icon.name: "document-edit"
                                visible: fileData.editable && !themeInspector.readOnly
                                onClicked: configValueDialog.openFor(
                                    fileData.relPath, modelData.key, modelData.value)
                            }
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.leftMargin: Platform.Units.largeSpacing
                        visible: modelData.truncated || modelData.note !== ""

                        Label {
                            Layout.fillWidth: true
                            text: (modelData.note !== "" ? modelData.note + " · " : "")
                                  + (modelData.truncated ? qsTr("truncated") : "")
                            font.pointSize: TypeScale.caption
                            font.italic: true
                            color: ThemeColors.warning()
                        }

                        // Экзотика (CSS и пр.): ключами не правится —
                        // открываем папку файла во внешнем редакторе.
                        ToolButton {
                            text: qsTr("Show in folder")
                            icon.name: "folder-open"
                            visible: modelData.note === "unsupported format"
                            onClicked: themeInspector.openFile(modelData.relPath)
                        }
                    }
                }
            }
        }
    }

    Label {
        Layout.fillWidth: true
        visible: tree.entries.length === 0
        text: qsTr("No config files found for this application.")
        color: ThemeColors.dimColor(tree.glassMode)
        font.pointSize: TypeScale.caption
    }

    Kirigami.Dialog {
        id: configValueDialog
        title: qsTr("Edit config value")
        property string relPath: ""
        property string configKey: ""
        property string oldValue: ""
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        preferredWidth: 480

        function openFor(path, key, value) {
            configValueDialog.relPath = path
            configValueDialog.configKey = key
            configValueDialog.oldValue = value
            newValueField.text = value
            configValueDialog.open()
        }

        ColumnLayout {
            Layout.fillWidth: true
            Label {
                text: configValueDialog.relPath + "  ·  " + configValueDialog.configKey
                color: ThemeColors.fgColor(tree.glassMode)
                font.pointSize: TypeScale.caption
                elide: Text.ElideMiddle
                Layout.fillWidth: true
            }
            TextField {
                id: newValueField
                Layout.fillWidth: true
            }
            Label {
                text: configValueDialog.oldValue + "  →  " + newValueField.text
                color: ThemeColors.dimColor(tree.glassMode)
                font.pointSize: TypeScale.caption
                Layout.fillWidth: true
            }
        }

        onAccepted: {
            if (configValueDialog.relPath !== "") {
                themeInspector.setAppConfigValue(
                    configValueDialog.relPath, configValueDialog.configKey,
                    newValueField.text)
            }
        }
    }
}
