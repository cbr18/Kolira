import Components 1.0
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami
import org.kde.kirigami.platform as Platform

AppDialog {
    id: dialog

    readonly property var report: profileController.applyPreview || ({
    })
    readonly property bool working: profileController.previewBusy || profileController.busy
    readonly property bool ready: !working && !!report.token
    readonly property bool glassMode: themeController.isGlass

    signal dependenciesRequested(var report)

    title: qsTr("Review theme changes")
    objectName: "applyPreviewDialog"
    width: Math.min(760, absoluteMaximumWidth)
    height: Math.min(680, absoluteMaximumHeight)
    standardButtons: Kirigami.Dialog.Close
    onAboutToHide: profileController.cancelPreview()
    customFooterActions: [
        Kirigami.Action {
            text: (dialog.report.dependencies || []).length > 0 ? qsTr("Apply without installing") : qsTr("Apply")
            icon.name: "dialog-ok-apply"
            enabled: dialog.ready
            onTriggered: {
                profileController.confirmPreview(dialog.report.token);
                dialog.close();
            }
        }
    ]

    background: Rectangle {
        color: ThemeColors.dialogBg(dialog.glassMode)
        radius: 8
        border.width: 1
        border.color: ThemeColors.cardBorder(dialog.glassMode)
    }

    contentItem: Item {
        ListView {
            id: fileList

            objectName: "applyFiles"
            anchors.fill: parent
            anchors.rightMargin: fileBar.implicitWidth + Platform.Units.smallSpacing
            clip: true
            spacing: Platform.Units.smallSpacing
            boundsBehavior: Flickable.StopAtBounds
            model: dialog.report.files || []

            ScrollBar.vertical: ScrollBar {
                id: fileBar

                objectName: "applyFilesVerticalBar"
                parent: fileList.parent
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                anchors.right: parent.right
            }

            header: ColumnLayout {
                width: fileList.width
                spacing: Platform.Units.largeSpacing

                Label {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    text: qsTr("Theme: %1").arg(dialog.report.name || "")
                    textFormat: Text.PlainText
                    font.bold: true
                    color: ThemeColors.fgColor(dialog.glassMode)
                }

                RowLayout {
                    BusyIndicator {
                        running: dialog.working
                        visible: running
                        implicitWidth: 28
                        implicitHeight: 28
                    }

                    Label {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: dialog.working ? qsTr("Calculating changes…") : qsTr("%n files will change", "", dialog.report.file_count || 0)
                        color: ThemeColors.fgColor(dialog.glassMode)
                    }

                }

                Label {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    visible: !!dialog.report.error
                    text: dialog.report.error || ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: ThemeColors.negative()
                }

                Label {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    text: qsTr("Personal settings in supported apps, the launcher and desktop layout are preserved. Plasma will restart; a recovery backup is created first.")
                    wrapMode: Text.Wrap
                    color: ThemeColors.dimColor(dialog.glassMode)
                }

                Label {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    visible: dialog.report.stale || false
                    text: qsTr("Settings changed. Review this updated preview before applying.")
                    wrapMode: Text.Wrap
                    color: ThemeColors.warning()
                }

                Label {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    visible: (dialog.report.custom_components || []).length > 0
                    text: qsTr("Custom components restore whole files: %1").arg((dialog.report.custom_components || []).join(", "))
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: ThemeColors.warning()
                }

                CheckBox {
                    text: qsTr("Keep the current wallpaper")
                    checked: dialog.report.keep_wallpaper || false
                    enabled: !profileController.busy
                    onToggled: profileController.previewApply(dialog.report.name, checked)
                }

            }

            delegate: Frame {
                id: fileRow

                required property var modelData
                property bool expanded: false

                width: fileList.width

                contentItem: ColumnLayout {
                    RowLayout {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0

                        ToolButton {
                            text: fileRow.expanded ? "−" : "+"
                            Accessible.name: qsTr("Show changes for %1").arg(fileRow.modelData.path)
                            onClicked: fileRow.expanded = !fileRow.expanded
                        }

                        Label {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            text: fileRow.modelData.path
                            textFormat: Text.PlainText
                            wrapMode: Text.WrapAnywhere
                            color: ThemeColors.fgColor(dialog.glassMode)
                        }

                        Label {
                            text: fileRow.modelData.operation === "delete" ? qsTr("Delete") : fileRow.modelData.operation === "create" ? qsTr("Create") : qsTr("Modify")
                            color: ThemeColors.dimColor(dialog.glassMode)
                        }

                    }

                    Label {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        visible: (fileRow.modelData.reasons || []).indexOf("reset") >= 0 || (fileRow.modelData.reasons || []).indexOf("previous_theme") >= 0
                        text: qsTr("Includes resetting settings from the previous theme")
                        wrapMode: Text.Wrap
                        color: ThemeColors.dimColor(dialog.glassMode)
                    }

                    Label {
                        visible: fileRow.expanded && !!fileRow.modelData.before_mode && !!fileRow.modelData.after_mode && fileRow.modelData.before_mode !== fileRow.modelData.after_mode
                        text: qsTr("Permissions: %1 → %2").arg(fileRow.modelData.before_mode || "—").arg(fileRow.modelData.after_mode || "—")
                        color: ThemeColors.dimColor(dialog.glassMode)
                    }

                    InsetScrollView {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        Layout.preferredHeight: 220
                        visible: fileRow.expanded
                        clip: true

                        TextArea {
                            readOnly: true
                            selectByMouse: true
                            textFormat: TextEdit.PlainText
                            font.family: "monospace"
                            text: fileRow.modelData.diff || ((fileRow.modelData.before_link || fileRow.modelData.after_link) ? qsTr("Link target: %1 → %2").arg(fileRow.modelData.before_link || "—").arg(fileRow.modelData.after_link || "—") : fileRow.modelData.before_sha256 === fileRow.modelData.after_sha256 ? qsTr("File content is unchanged.") : qsTr("Binary or large file: %1 → %2 bytes").arg(fileRow.modelData.before_bytes).arg(fileRow.modelData.after_bytes))
                            color: ThemeColors.fgColor(dialog.glassMode)
                        }

                    }

                    Label {
                        visible: fileRow.expanded && fileRow.modelData.truncated
                        text: qsTr("Preview shortened; the confirmation covers the complete file.")
                        wrapMode: Text.Wrap
                        color: ThemeColors.dimColor(dialog.glassMode)
                    }

                }

            }

            footer: ColumnLayout {
                width: fileList.width
                spacing: Platform.Units.largeSpacing

                Label {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    visible: (dialog.report.missing || []).length > 0
                    text: qsTr("Unavailable components keep their current appearance where supported:") + "\n" + (dialog.report.missing || []).map(function(item) {
                        return "• " + (item.name || item.field);
                    }).join("\n")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: ThemeColors.warning()
                }

                RowLayout {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    visible: (dialog.report.dependencies || []).length > 0

                    Label {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: qsTr("Missing dependencies: %1").arg((dialog.report.dependencies || []).length) + "\n" + (dialog.report.dependencies || []).map(function(item) {
                            return "• " + (item.title || item.name);
                        }).join("\n")
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: ThemeColors.fgColor(dialog.glassMode)
                    }

                    AppButton {
                        text: qsTr("Dependencies…")
                        enabled: dialog.ready
                        onClicked: {
                            const report = dialog.report;
                            dialog.close();
                            dialog.dependenciesRequested(report);
                        }
                    }

                }

                Label {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    visible: (dialog.report.actions || []).length > 0
                    text: qsTr("Follow-up actions:") + "\n" + (dialog.report.actions || []).map(function(item) {
                        return "• " + qsTranslate("Core", item.title);
                    }).join("\n")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: ThemeColors.fgColor(dialog.glassMode)
                }

                Label {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    visible: (dialog.report.runtime_changes || []).length > 0
                    text: qsTr("Desktop settings:") + "\n" + (dialog.report.runtime_changes || []).map(function(item) {
                        const names = {
                            "gtk-theme": qsTr("GTK theme"),
                            "color-scheme": qsTr("Colour preference"),
                            "icon-theme": qsTr("Icon theme"),
                            "cursor-theme": qsTr("Cursor theme")
                        };
                        return "• " + (names[item.key] || item.key) + ": " + (item.before || qsTr("Unknown")) + " → " + item.after;
                    }).join("\n")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: ThemeColors.fgColor(dialog.glassMode)
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0

                    TextField {
                        id: saveName

                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        placeholderText: qsTr("Name for the current appearance")
                        enabled: dialog.ready
                    }

                    AppButton {
                        text: qsTr("Save current first")
                        icon.name: "document-save"
                        enabled: dialog.ready && saveName.text.trim().length > 0
                        onClicked: profileController.savePreviewCurrent(saveName.text.trim())
                    }

                }

                Label {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    visible: !!dialog.report.saved_as
                    text: qsTr("Current appearance saved as %1").arg(dialog.report.saved_as || "")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: ThemeColors.positive()
                }

            }

        }

    }

}
