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
    readonly property var selected: historySchedule.history.find(function(row) {
        return row.id === page.selectedId;
    }) || null

    padding: Platform.Units.gridUnit
    objectName: "auditPage"
    title: qsTr("Theme change history")
    Component.onCompleted: historySchedule.refreshHistory()

    Timer {
        interval: 15000
        running: page.visible
        repeat: true
        onTriggered: historySchedule.refreshHistory()
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        Label {
            color: ThemeColors.fgColor(page.glassMode)
            Layout.fillWidth: true
            text: qsTr("Last 10 completed changes and all interrupted operations. Backups are kept when older history entries are removed.")
            wrapMode: Text.WordWrap
        }

        AppButton {
            text: qsTr("Refresh")
            enabled: !historySchedule.busy
            onClicked: historySchedule.refreshHistory()
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
            visible: historySchedule.history.length === 0
        }

        Repeater {
            model: historySchedule.history

            delegate: Frame {
                id: entry

                required property var modelData

                Layout.fillWidth: true

                ColumnLayout {
                    anchors.fill: parent

                    Label {
                        color: ThemeColors.fgColor(page.glassMode)
                        Layout.fillWidth: true
                        text: (entry.modelData.previous_profile || qsTr("Unknown")) + " → " + entry.modelData.profile
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
                                text: qsTr("Open backup")
                                enabled: entry.modelData.backup_status === "available"
                                onClicked: historySchedule.openBackup(entry.modelData.id)
                            }

                            AppButton {
                                text: qsTr("Restore…")
                                enabled: !historySchedule.busy && !profileController.busy && (entry.modelData.can_restore || (entry.modelData.interrupted && !entry.modelData.backup))
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
