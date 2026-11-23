import Components 1.0
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami
import org.kde.kirigami.platform as Platform

Kirigami.ScrollablePage {
    id: page

    readonly property bool glassMode: themeController.isGlass
    property string selectedId: ""
    signal reapplyRequested(string profileName)
    readonly property var selected: historySchedule.history.find(function(row) {
        return row.id === page.selectedId;
    }) || null

    padding: Platform.Units.gridUnit
    objectName: "auditPage"
    title: qsTr("Theme change history")
    Component.onCompleted: historySchedule.refreshHistory()

    function pointTitle(point) {
        return point.name || qsTr("Before Live changes")
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        TabBar {
            id: historyTabs
            objectName: "historyTabs"
            Layout.fillWidth: true
            onCurrentIndexChanged: {
                if (currentIndex === 1)
                    historySchedule.refreshPoints()
            }
            TabButton { text: qsTr("Applications") }
            TabButton { text: qsTr("Restore points") }
        }

        ActionFlow {
            AppButton {
                text: qsTr("Refresh")
                enabled: !historySchedule.busy
                onClicked: historyTabs.currentIndex === 0
                    ? historySchedule.refreshHistory() : historySchedule.refreshPoints()
            }
            AppButton {
                objectName: "createRestorePoint"
                visible: historyTabs.currentIndex === 1
                text: qsTr("Create restore point…")
                icon.name: "list-add"
                enabled: !historySchedule.busy && !profileController.busy
                onClicked: nameDialog.edit("", "")
            }
        }

        BusyIndicator {
            running: historySchedule.busy
            visible: running
        }

        Label {
            color: ThemeColors.fgColor(page.glassMode)
            Layout.fillWidth: true
            text: historySchedule.message
            visible: text !== ""
            wrapMode: Text.Wrap
            textFormat: Text.PlainText
            Accessible.role: Accessible.AlertMessage
        }

        Label {
            color: ThemeColors.fgColor(page.glassMode)
            text: qsTr("No theme changes recorded")
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            visible: historyTabs.currentIndex === 0 && historySchedule.history.length === 0 && !historySchedule.busy
        }

        Repeater {
            model: historyTabs.currentIndex === 0 ? historySchedule.history : []

            delegate: Frame {
                id: entry

                required property var modelData

                Layout.fillWidth: true

                ColumnLayout {
                    anchors.fill: parent

                    Label {
                        color: ThemeColors.fgColor(page.glassMode)
                        Layout.fillWidth: true
                        text: entry.modelData.restored_operation ? qsTr("Restore appearance")
                            : entry.modelData.report?.kind === "live" ? qsTr("Apply Live draft")
                            : (entry.modelData.previous_profile || qsTr("Unknown")) + " → " + entry.modelData.profile
                        font.bold: true
                        wrapMode: Text.Wrap
                        textFormat: Text.PlainText
                    }

                    Label {
                        color: ThemeColors.fgColor(page.glassMode)
                        Layout.fillWidth: true
                        text: new Date(entry.modelData.started).toLocaleString(Qt.locale(), Locale.ShortFormat) + " · " + entry.modelData.sourceText + " · " + entry.modelData.stateText
                        wrapMode: Text.Wrap
                        textFormat: Text.PlainText
                    }

                    Label {
                        Layout.fillWidth: true
                        visible: !!entry.modelData.report?.restore_point_error
                        text: qsTr("An automatic restore point could not be saved.")
                        wrapMode: Text.Wrap
                    }

                    Label {
                        color: ThemeColors.fgColor(page.glassMode)
                        Layout.fillWidth: true
                        text: entry.modelData.errorText || ""
                        visible: text !== ""
                        wrapMode: Text.Wrap
                        textFormat: Text.PlainText
                    }

                    AppButton {
                        text: qsTr("Details")
                        onClicked: page.selectedId = page.selectedId === entry.modelData.id ? "" : entry.modelData.id
                    }

                    ColumnLayout {
                        visible: page.selectedId === entry.modelData.id
                        Layout.fillWidth: true

                        Label {
                            color: ThemeColors.fgColor(page.glassMode)
                            Layout.fillWidth: true
                            text: qsTr("Operation: %1").arg(entry.modelData.id)
                            wrapMode: Text.WrapAnywhere
                            textFormat: Text.PlainText
                        }

                        Label {
                            color: ThemeColors.fgColor(page.glassMode)
                            Layout.fillWidth: true
                            text: qsTr("Backup: %1").arg(entry.modelData.backup || qsTr("Not created"))
                            wrapMode: Text.WrapAnywhere
                            textFormat: Text.PlainText
                        }

                        Label {
                            color: ThemeColors.fgColor(page.glassMode)
                            Layout.fillWidth: true
                            text: entry.modelData.backup_error || ""
                            visible: text !== ""
                            wrapMode: Text.Wrap
                            textFormat: Text.PlainText
                        }

                        TextArea {
                            color: ThemeColors.fgColor(page.glassMode)
                            Layout.fillWidth: true
                            readOnly: true
                            text: JSON.stringify(entry.modelData.report, null, 2) + "\n" + (entry.modelData.recovery_errors || []).join("\n")
                            wrapMode: TextEdit.WrapAnywhere
                            textFormat: TextEdit.PlainText
                            Accessible.name: qsTr("Operation details")
                        }

                        Flow {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.smallSpacing

                            AppButton {
                                text: qsTr("Re-apply")
                                icon.name: "dialog-ok-apply"
                                visible: profileController.profiles.some(function(profile) {
                                    return profile.name === entry.modelData.profile
                                })
                                enabled: !profileController.busy && !historySchedule.busy
                                onClicked: page.reapplyRequested(entry.modelData.profile)
                            }

                            AppButton {
                                text: qsTr("Open backup")
                                enabled: entry.modelData.backup_status === "available"
                                onClicked: historySchedule.openBackup(entry.modelData.id)
                            }

                            AppButton {
                                text: qsTr("Restore…")
                                visible: entry.modelData.interrupted === true
                                enabled: !historySchedule.busy && !profileController.busy
                                onClicked: {
                                    confirmation.operationId = entry.modelData.id;
                                    confirmation.interrupted = entry.modelData.interrupted;
                                    confirmation.open();
                                }
                            }

                        }

                    }

                }

                background: Rectangle {
                    color: ThemeColors.cardBg(page.glassMode)
                    border.color: ThemeColors.cardBorder(page.glassMode)
                    border.width: 1
                    radius: Platform.Units.smallSpacing
                }

            }

        }

        AppButton {
            visible: historyTabs.currentIndex === 0 && historySchedule.moreHistory
            text: qsTr("Load older operations")
            enabled: !historySchedule.busy
            onClicked: historySchedule.loadMoreHistory()
        }

        Label {
            Layout.fillWidth: true
            visible: historyTabs.currentIndex === 1 && historySchedule.points.length === 0 && !historySchedule.busy
            text: qsTr("No restore points yet")
            wrapMode: Text.Wrap
        }

        Repeater {
            model: historyTabs.currentIndex === 1 ? ["manual", "automatic"] : []
            delegate: ColumnLayout {
                id: pointGroup
                required property string modelData
                readonly property var rows: historySchedule.points.filter(point => point.kind === modelData)
                Layout.fillWidth: true
                visible: rows.length > 0
                Kirigami.Heading {
                    Layout.fillWidth: true
                    level: 3
                    text: pointGroup.modelData === "manual" ? qsTr("Manual points") : qsTr("Automatic points")
                    wrapMode: Text.Wrap
                }
                Repeater {
                    model: pointGroup.rows
                    delegate: Frame {
                        id: pointRow
                        required property var modelData
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        ColumnLayout {
                            anchors.fill: parent
                            Label {
                                Layout.fillWidth: true
                                Layout.minimumWidth: 0
                                textFormat: Text.PlainText
                                font.bold: true
                                wrapMode: Text.Wrap
                                text: page.pointTitle(pointRow.modelData)
                            }
                            Label {
                                Layout.fillWidth: true
                                wrapMode: Text.Wrap
                                text: new Date(pointRow.modelData.created).toLocaleString(Qt.locale(), Locale.ShortFormat)
                                    + (pointRow.modelData.pinned ? " · " + qsTr("Pinned") : "")
                            }
                            ActionFlow {
                                AppButton {
                                    objectName: "restorePoint-" + pointRow.modelData.id
                                    text: qsTr("Restore…")
                                    enabled: !historySchedule.busy && !profileController.busy
                                    onClicked: pointConfirmation.ask(pointRow.modelData, true)
                                }
                                AppButton {
                                    text: pointRow.modelData.pinned ? qsTr("Unpin") : qsTr("Pin")
                                    enabled: !historySchedule.busy
                                    onClicked: historySchedule.pinPoint(pointRow.modelData.id, !pointRow.modelData.pinned)
                                }
                                ToolButton {
                                    text: qsTr("More actions")
                                    icon.name: "overflow-menu"
                                    onClicked: pointMenu.open()
                                    Menu {
                                        id: pointMenu
                                        MenuItem {
                                            text: qsTr("Rename…")
                                            enabled: !historySchedule.busy
                                            onTriggered: nameDialog.edit(pointRow.modelData.id, pointRow.modelData.name)
                                        }
                                        MenuItem {
                                            text: qsTr("Delete…")
                                            enabled: !historySchedule.busy
                                            onTriggered: pointConfirmation.ask(pointRow.modelData, false)
                                        }
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
        id: nameDialog
        objectName: "restorePointNameDialog"
        property string pointId: ""
        standardButtons: Dialog.Cancel
        preferredWidth: 420
        title: pointId ? qsTr("Rename restore point") : qsTr("Create restore point")
        function edit(id, name) {
            pointId = id
            pointName.text = name
            open()
            pointName.forceActiveFocus()
        }
        TextField {
            id: pointName
            objectName: "restorePointName"
            Layout.fillWidth: true
            placeholderText: qsTr("Name")
            maximumLength: 120
            onAccepted: if (commitPoint.enabled) commitPoint.trigger()
        }
        customFooterActions: Kirigami.Action {
            id: commitPoint
            text: qsTr("Save")
            enabled: pointName.text.trim() !== "" && !historySchedule.busy
            onTriggered: {
                historySchedule.savePoint(nameDialog.pointId, pointName.text)
                nameDialog.close()
            }
        }
    }

    AppDialog {
        id: pointConfirmation
        property string pointId: ""
        property bool restore: true
        title: restore ? qsTr("Restore appearance?") : qsTr("Delete restore point?")
        standardButtons: Dialog.Cancel
        preferredWidth: 420
        function ask(point, restore) {
            pointId = point.id
            pointTitle.text = page.pointTitle(point)
            pointConfirmation.restore = restore
            open()
        }
        Label {
            id: pointTitle
            Layout.fillWidth: true
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
        }
        customFooterActions: Kirigami.Action {
            text: pointConfirmation.restore ? qsTr("Restore") : qsTr("Delete")
            enabled: !historySchedule.busy && !profileController.busy
            onTriggered: {
                historySchedule.confirmPoint(pointConfirmation.pointId, pointConfirmation.restore)
                pointConfirmation.close()
            }
        }
    }

    AppDialog {
        id: confirmation

        property string operationId: ""
        property bool interrupted: false

        title: qsTr("Restore previous appearance?")
        standardButtons: Dialog.Cancel
        preferredWidth: 560

        Label {
            color: ThemeColors.fgColor(page.glassMode)
            text: confirmation.interrupted ? qsTr("Recover the interrupted operation. Files changed after interruption are protected; conflicts are reported without overwriting them.") : qsTr("Replace current appearance with the state before this operation. A safety backup of the current state will be created first.")
            wrapMode: Text.WordWrap
        }

        customFooterActions: Kirigami.Action {
            text: qsTr("Restore")
            enabled: !historySchedule.busy
            onTriggered: {
                historySchedule.restoreOperation(confirmation.operationId, confirmation.interrupted);
                confirmation.close();
            }
        }

    }

    background: Rectangle {
        color: page.glassMode ? ThemeColors.glassPanel() : ThemeColors.pageBg(page.glassMode)
    }

}
