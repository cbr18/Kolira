import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami
import org.kde.kirigami.platform as Platform
import Components 1.0

// Страница «Приложения»: реестр отслеживаемых компонентов.
// Встроенные — галочку можно снять, удалить нельзя; пользовательские —
// галочка + изменить + удалить. Галочка = участие в save/apply
// (id попадает в AppSettings.disabled_components, файлы не трогаются).
Kirigami.ScrollablePage {
    padding: Platform.Units.gridUnit
    id: page

    title: qsTr("Applications")

    readonly property bool glassMode: themeController.isGlass

    // Grouped view of trackedAppsController.components (list of dicts).
    readonly property var grouped: {
        const groups = []
        const byCategory = {}
        const comps = trackedAppsController.components
        for (let i = 0; i < comps.length; i++) {
            const c = comps[i]
            if (!byCategory[c.category]) {
                byCategory[c.category] = []
                groups.push({ category: c.category, items: byCategory[c.category] })
            }
            byCategory[c.category].push(c)
        }
        return groups
    }

    // Одно раскрытое дерево конфигов за раз — страница остаётся компактной.
    property string expandedConfigs: ""

    readonly property var configByApp: {
        const map = {}
        const list = themeInspector.appConfigs
        for (let i = 0; i < list.length; i++)
            map[list[i].id] = list[i]
        return map
    }

    background: Rectangle {
        color: page.glassMode ? ThemeColors.glassPanel() : ThemeColors.pageBg(page.glassMode)
    }

    Component.onCompleted: trackedAppsController.refresh()

    ColumnLayout {
        spacing: Platform.Units.largeSpacing

        ActionFlow {
            Layout.fillWidth: true

            Kirigami.Heading {
                level: 2
                text: qsTr("Tracked applications")
                width: Math.min(implicitWidth, parent.width)
                wrapMode: Text.Wrap
                color: ThemeColors.fgColor(page.glassMode)
                Layout.fillWidth: true
            }

            AppButton {
                text: qsTr("Apply")
                icon.name: "dialog-ok-apply"
                highlighted: true
                enabled: !themeInspector.readOnly
                    && themeInspector.sessionChanges > 0
                    && !themeInspector.busy
                onClicked: themeInspector.applyChanges()
            }

            AppButton {
                text: qsTr("Undo")
                icon.name: "edit-undo"
                enabled: !themeInspector.readOnly
                    && (themeInspector.sessionChanges > 0 || themeInspector.undoDepth > 0)
                    && !themeInspector.busy
                onClicked: themeInspector.undo()
            }

            AppButton {
                text: qsTr("Add application…")
                icon.name: "list-add"
                onClicked: appEditor.openNew()
            }
        }

        Label {
            text: qsTr("Checked applications take part in saving and applying profiles. "
                       + "Built-in ones can be disabled but not removed; your own can also "
                       + "be edited and deleted.")
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
            color: ThemeColors.dimColor(page.glassMode)
            font.pointSize: TypeScale.caption
        }

        Label {
            id: errorLabel
            visible: text !== ""
            text: ""
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
            color: ThemeColors.negative()
        }

        Repeater {
            model: page.grouped

            Kirigami.Card {
                required property var modelData
                Layout.fillWidth: true

                background: Rectangle {
                    radius: Platform.Units.smallSpacing
                    border.width: 1
                    color: ThemeColors.cardBg(page.glassMode)
                    border.color: ThemeColors.cardBorder(page.glassMode)
                }

                contentItem: ColumnLayout {
                    spacing: Platform.Units.smallSpacing

                    Kirigami.Heading {
                        level: 3
                        text: modelData.category
                        color: ThemeColors.fgColor(page.glassMode)
                    }

                    Repeater {
                        model: modelData.items

                        ColumnLayout {
                            required property var modelData

                            Layout.fillWidth: true
                            spacing: 0

                            Rectangle {

                            Layout.fillWidth: true
                            implicitHeight: rowLayout.implicitHeight + Platform.Units.smallSpacing * 2
                            radius: 4
                            color: ThemeColors.nestedBg(page.glassMode)

                            RowLayout {
                                id: rowLayout
                                anchors.fill: parent
                                anchors.margins: Platform.Units.smallSpacing
                                spacing: Platform.Units.mediumSpacing

                                CheckBox {
                                    id: trackedBox
                                    checked: modelData.enabled
                                    // QQC2 writes to `checked` on toggle,
                                    // destroying the binding — re-arm it.
                                    Binding {
                                        target: trackedBox
                                        property: "checked"
                                        value: modelData.enabled
                                        restoreMode: Binding.RestoreBindingOrValue
                                    }
                                    onToggled: trackedAppsController.setComponentTracked(
                                        modelData.id, trackedBox.checked)
                                    Accessible.name: modelData.name
                                    palette.windowText: ThemeColors.fgColor(page.glassMode)
                                    palette.text: ThemeColors.fgColor(page.glassMode)
                                }

                                Kirigami.Icon {
                                    source: modelData.icon
                                    Layout.preferredWidth: Platform.Units.iconSizes.medium
                                    Layout.preferredHeight: Platform.Units.iconSizes.medium
                                }

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 2

                                    RowLayout {
                                        Label {
                                            text: modelData.name
                                            font.bold: true
                                            color: ThemeColors.fgColor(page.glassMode)
                                        }
                                        Label {
                                            text: modelData.statusText
                                            font.pointSize: TypeScale.caption
                                            visible: modelData.status !== "available"
                                            color: modelData.status === "partial"
                                                   ? ThemeColors.warning()
                                                   : ThemeColors.negative()
                                        }
                                        Label {
                                            text: qsTr("reloads on apply")
                                            font.pointSize: TypeScale.caption
                                            color: ThemeColors.accent()
                                            visible: modelData.isCustom && modelData.reloadCommand !== ""
                                        }
                                    }

                                    Label {
                                        text: modelData.description
                                            + (modelData.description !== "" && modelData.isCustom
                                               && modelData.configPaths.length > 0 ? " · " : "")
                                            + (modelData.isCustom ? modelData.configPaths.join(", ") : "")
                                        font.pointSize: TypeScale.caption
                                        color: ThemeColors.dimColor(page.glassMode)
                                        wrapMode: Text.WordWrap
                                        Layout.fillWidth: true
                                        visible: text !== ""
                                    }
                                }

                                ToolButton {
                                    text: qsTr("Edit")
                                    icon.name: "document-edit"
                                    visible: modelData.isCustom
                                    onClicked: appEditor.openExisting(modelData)
                                }

                                ToolButton {
                                    text: qsTr("Delete")
                                    icon.name: "edit-delete"
                                    visible: modelData.isCustom
                                    onClicked: confirmDelete.ask(modelData.id, modelData.name)
                                }

                                // Раскрытие дерева конфигов под приложением (M4):
                                // файлы и ключи, перенесённые со страницы Theme.
                                ToolButton {
                                    text: qsTr("Configs")
                                    icon.name: "view-list-details"
                                    checkable: true
                                    checked: page.expandedConfigs === modelData.id
                                    visible: page.configByApp[modelData.id] !== undefined
                                    ToolTip.text: qsTr("Show the config files this application stores")
                                    ToolTip.visible: hovered || activeFocus
                                    onClicked: page.expandedConfigs =
                                        page.expandedConfigs === modelData.id ? "" : modelData.id
                                }
                            }
                            }

                            Loader {
                                Layout.fillWidth: true
                                Layout.leftMargin: Platform.Units.largeSpacing
                                active: page.expandedConfigs === modelData.id
                                visible: active
                                sourceComponent: Component {
                                    AppConfigTree {
                                        appId: modelData.id
                                        glassMode: page.glassMode
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    AppDialog {
        id: appEditor
        objectName: "appEditor"

        property string editingId: ""
        property string validationError: ""

        title: editingId === "" ? qsTr("Add application") : qsTr("Edit application")

        function openNew() {
            editingId = ""
            nameField.text = ""
            categoryField.text = ""
            iconField.text = ""
            descriptionField.text = ""
            pathsArea.text = ""
            reloadField.text = ""
            appEditor.validationError = ""
            open()
        }

        function openExisting(c) {
            editingId = c.id
            nameField.text = c.name
            categoryField.text = c.category
            iconField.text = c.icon
            descriptionField.text = c.description
            pathsArea.text = c.configPaths.join("\n")
            reloadField.text = c.reloadCommand
            appEditor.validationError = ""
            open()
        }

        function collect() {
            return {
                "id": editingId,
                "name": nameField.text,
                "category": categoryField.text,
                "icon": iconField.text,
                "description": descriptionField.text,
                "configPaths": pathsArea.text.split("\n"),
                "reloadCommand": reloadField.text
            }
        }

        function save() {
            const fields = collect()
            const error = editingId === ""
                ? trackedAppsController.addCustomComponent(fields)
                : trackedAppsController.updateCustomComponent(fields)
            if (error === "") {
                close()
            } else {
                appEditor.validationError = error
            }
        }

        ColumnLayout {
            Label {
                objectName: "appEditorError"
                Layout.fillWidth: true
                visible: text !== ""
                text: appEditor.validationError
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: ThemeColors.negative()
                Accessible.role: Accessible.AlertMessage
            }
            Kirigami.FormLayout {
                Layout.fillWidth: true

                Label {
                    Kirigami.FormData.label: qsTr("Id")
                    text: appEditor.editingId === ""
                          ? qsTr("(generated from the name)")
                          : appEditor.editingId
                    color: ThemeColors.dimColor(page.glassMode)
                    visible: appEditor.editingId !== ""
                }

                TextField {
                    id: nameField
                    onTextEdited: appEditor.validationError = ""
                    Kirigami.FormData.label: qsTr("Name")
                    Layout.fillWidth: true
                }

                TextField {
                    id: categoryField
                    onTextEdited: appEditor.validationError = ""
                    Kirigami.FormData.label: qsTr("Category")
                    placeholderText: qsTr("Custom")
                    Layout.fillWidth: true
                }

                TextArea {
                    id: pathsArea
                    onTextChanged: appEditor.validationError = ""
                    wrapMode: TextEdit.Wrap
                    Kirigami.FormData.label: qsTr("Config paths")
                    placeholderText: ".config/myapp/settings.json\n.config/myapp/themes/"
                    Layout.fillWidth: true
                    Layout.minimumHeight: Platform.Units.gridUnit * 4
                }

                TextField {
                    id: reloadField
                    onTextEdited: appEditor.validationError = ""
                    Kirigami.FormData.label: qsTr("Reload command")
                    placeholderText: qsTr("optional, e.g. pkill -HUP myapp")
                    Layout.fillWidth: true
                }

                TextField {
                    id: descriptionField
                    onTextEdited: appEditor.validationError = ""
                    Kirigami.FormData.label: qsTr("Description")
                    Layout.fillWidth: true
                }

                TextField {
                    id: iconField
                    onTextEdited: appEditor.validationError = ""
                    Kirigami.FormData.label: qsTr("Icon")
                    placeholderText: "application-x-executable"
                    Layout.fillWidth: true
                }
            }
        }

        customFooterActions: [
            Kirigami.Action {
                text: qsTr("Save")
                icon.name: "dialog-ok"
                onTriggered: appEditor.save()
            },
            Kirigami.Action {
                text: qsTr("Cancel")
                icon.name: "dialog-cancel"
                onTriggered: appEditor.close()
            }
        ]
    }

    AppDialog {
        id: confirmDelete

        property string targetId: ""
        property string targetName: ""

        function ask(id, name) {
            targetId = id
            targetName = name
            open()
        }

        title: qsTr("Delete application")

        Label {
            text: qsTr("Delete '%1'? Its id is removed from the component selection; "
                       + "config files and profile snapshots are not touched.")
                   .arg(confirmDelete.targetName)
            wrapMode: Text.WordWrap
        }

        customFooterActions: [
            Kirigami.Action {
                text: qsTr("Delete")
                icon.name: "edit-delete"
                onTriggered: {
                    const error = trackedAppsController.removeCustomComponent(confirmDelete.targetId)
                    errorLabel.text = error
                    confirmDelete.close()
                }
            },
            Kirigami.Action {
                text: qsTr("Cancel")
                icon.name: "dialog-cancel"
                onTriggered: confirmDelete.close()
            }
        ]
    }
}
