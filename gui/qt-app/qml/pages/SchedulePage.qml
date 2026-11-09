import Components 1.0
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami
import org.kde.kirigami.platform as Platform

Kirigami.ScrollablePage {
    id: page

    readonly property bool glassMode: themeController.isGlass
    property string editingId: ""
    property var chosenDays: [1, 2, 3, 4, 5, 6, 7]

    readonly property var dayNames: [qsTr("Mon"), qsTr("Tue"), qsTr("Wed"), qsTr("Thu"), qsTr("Fri"), qsTr("Sat"), qsTr("Sun")]
    function newRule() {
        editingId = ""; chosenDays = [1,2,3,4,5,6,7]
        time.text = "08:00"; theme.currentIndex = theme.count > 0 ? 0 : -1
        ruleEnabled.checked = true; keep.checked = false
    }
    function edit(rule) {
        editingId = rule.id;
        theme.currentIndex = theme.model.indexOf(rule.theme);
        time.text = rule.time;
        chosenDays = rule.days.slice();
        ruleEnabled.checked = rule.enabled;
        keep.checked = rule.keep_wallpaper;
    }

    padding: Platform.Units.gridUnit
    objectName: "schedulePage"
    title: qsTr("Theme schedule")
    Component.onCompleted: historySchedule.refreshSchedule()

    Timer {
        interval: 30000
        running: page.visible
        repeat: true
        onTriggered: historySchedule.refreshSchedule()
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        CheckBox {
            text: qsTr("Enable scheduled theme changes")
            checked: historySchedule.schedule.enabled || false
            enabled: !historySchedule.busy
            onClicked: historySchedule.enableSchedule(checked)
        }

        Label {
            color: ThemeColors.fgColor(page.glassMode)
            Layout.fillWidth: true
            text: historySchedule.schedule.running ? qsTr("Scheduler is running") : qsTr("Scheduler is stopped")
            wrapMode: Text.WordWrap
        }

        Label {
            color: ThemeColors.fgColor(page.glassMode)
            Layout.fillWidth: true
            text: qsTr("Uses local time in your Plasma session, including when this window is closed. Missed times are skipped. Missing dependencies require manual installation. Simultaneous rules run in order of their identifiers; only the first applies.")
            wrapMode: Text.WordWrap
        }

        Label {
            color: ThemeColors.fgColor(page.glassMode)
            Layout.fillWidth: true
            text: historySchedule.message
            visible: text !== ""
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            Accessible.role: Accessible.AlertMessage
        }

        BusyIndicator {
            running: historySchedule.busy
            visible: running
        }

        Repeater {
            model: historySchedule.schedule.rules || []

            delegate: Frame {
                id: entry

                required property var modelData

                Layout.fillWidth: true

                ColumnLayout {
                    anchors.fill: parent

                    Label {
                        color: ThemeColors.fgColor(page.glassMode)
                        Layout.fillWidth: true
                        text: entry.modelData.theme + " · " + entry.modelData.time + " · " + entry.modelData.days.map(function(day) { return page.dayNames[day - 1] }).join(", ") + (entry.modelData.enabled ? "" : " · " + qsTr("Disabled"))
                        wrapMode: Text.Wrap
                        textFormat: Text.PlainText
                    }

                    Label {
                        color: ThemeColors.fgColor(page.glassMode)
                        Layout.fillWidth: true
                        text: qsTr("Next: %1").arg(entry.modelData.next ? new Date(entry.modelData.next).toLocaleString(Qt.locale(), Locale.ShortFormat) : qsTr("None"))
                        wrapMode: Text.Wrap
                    }

                    Label {
                        color: ThemeColors.fgColor(page.glassMode)
                        Layout.fillWidth: true
                        text: entry.modelData.last ? entry.modelData.last.stateText + ": " + entry.modelData.last.messageText : qsTr("Not run yet")
                        wrapMode: Text.Wrap
                        textFormat: Text.PlainText
                    }

                    Flow {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing

                        AppButton {
                            text: qsTr("Edit")
                            enabled: !historySchedule.busy
                            onClicked: page.edit(entry.modelData)
                        }

                        AppButton {
                            text: qsTr("Delete")
                            enabled: !historySchedule.busy
                            onClicked: historySchedule.deleteRule(entry.modelData.id)
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

        Label {
            color: ThemeColors.fgColor(page.glassMode)
            text: page.editingId ? qsTr("Edit rule") : qsTr("New rule")
            font.bold: true
        }

        ComboBox {
            id: theme

            Layout.fillWidth: true
            model: profileController.profiles.map(function(p) {
                return p.name;
            })
            Accessible.name: qsTr("Theme")
        }

        TextField {
            id: time

            text: "08:00"
            placeholderText: qsTr("Time (HH:MM)")
            Accessible.name: placeholderText

            validator: RegularExpressionValidator {
                regularExpression: /([01][0-9]|2[0-3]):[0-5][0-9]/
            }

        }

        Flow {
            Layout.fillWidth: true

            Repeater {
                model: page.dayNames

                delegate: CheckBox {
                    id: day

                    required property int index
                    required property string modelData

                    text: modelData
                    checked: page.chosenDays.indexOf(index + 1) >= 0
                    onClicked: {
                        let days = page.chosenDays.slice();
                        if (checked)
                            days.push(index + 1);
                        else
                            days = days.filter(function(d) {
                            return d !== day.index + 1;
                        });
                        page.chosenDays = days;
                    }
                }

            }

        }

        CheckBox {
            id: ruleEnabled

            text: qsTr("Rule enabled")
            checked: true
        }

        CheckBox {
            id: keep

            text: qsTr("Keep current wallpaper")
        }

        Flow {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing

            AppButton {
                text: qsTr("Save rule")
                enabled: !historySchedule.busy && theme.currentIndex >= 0 && time.acceptableInput && page.chosenDays.length > 0
                onClicked: {
                    historySchedule.saveRule({
                        "id": page.editingId || "rule-" + Date.now(),
                        "theme": theme.currentText,
                        "time": time.text,
                        "days": page.chosenDays,
                        "enabled": ruleEnabled.checked,
                        "keep_wallpaper": keep.checked
                    });
                    page.editingId = "";
                }
            }

            AppButton {
                text: qsTr("New rule")
                onClicked: page.newRule()
            }

        }

    }

    background: Rectangle {
        color: page.glassMode ? ThemeColors.glassPanel() : ThemeColors.pageBg(page.glassMode)
    }

}
