import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami
import org.kde.kirigami.platform as Platform
import Components 1.0

/*! One high-level theme component group (KDE/Plasma, Desktop, Qt/Kvantum, GTK). */
Kirigami.ScrollablePage {
    id: page

    title: page.groupTitle
    property string groupId: ""
    property string groupTitle: ""
    property string groupDescription: ""
    property var components: []
    property string wallpaperComponentId: ""

    readonly property bool glassMode: themeController.isGlass
    readonly property bool readOnly: themeBundle.readOnly

    function choiceIndex(component) {
        for (let i = 0; i < component.choices.length; ++i) {
            if (component.choices[i].value === component.currentValue)
                return i
        }
        return -1
    }

    background: Rectangle {
        color: page.glassMode ? ThemeColors.glassPanel() : ThemeColors.pageBg(page.glassMode)
    }

    FileDialog {
        id: wallpaperDialog
        title: qsTr("Choose wallpaper")
        fileMode: FileDialog.OpenFile
        nameFilters: [qsTr("Images (*.png *.jpg *.jpeg *.webp *.bmp)"), qsTr("All files (*)")]
        onAccepted: {
            if (page.wallpaperComponentId !== "")
                themeBundle.setComponent(page.wallpaperComponentId, selectedFile)
        }
    }

    ColumnLayout {
        Layout.margins: Platform.Units.largeSpacing
        spacing: Platform.Units.largeSpacing

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: page.readOnly
            type: Kirigami.MessageType.Information
            text: qsTr("This is a saved profile. Switch to Live system to edit components.")
        }

        Label {
            Layout.fillWidth: true
            visible: page.groupDescription !== ""
            text: page.groupDescription
            wrapMode: Text.WordWrap
            color: ThemeColors.dimColor(page.glassMode)
        }

        Repeater {
            model: page.components

            Kirigami.Card {
                required property var modelData
                Layout.fillWidth: true

                background: Rectangle {
                    radius: Platform.Units.smallSpacing
                    border.width: 1
                    color: ThemeColors.cardBg(page.glassMode)
                    border.color: modelData.status === "warning"
                        ? ThemeColors.warning() : ThemeColors.cardBorder(page.glassMode)
                }

                contentItem: ColumnLayout {
                    spacing: Platform.Units.smallSpacing

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Platform.Units.mediumSpacing

                        Kirigami.Icon {
                            source: modelData.icon || "preferences-desktop-theme"
                            color: ThemeColors.accent()
                            Layout.preferredWidth: Platform.Units.iconSizes.small
                            Layout.preferredHeight: Platform.Units.iconSizes.small
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 0

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: Platform.Units.smallSpacing
                                Label {
                                    Layout.fillWidth: true
                                    text: modelData.label
                                    color: ThemeColors.fgColor(page.glassMode)
                                    font.bold: true
                                }
                                Rectangle {
                                    visible: modelData.staged === true
                                    width: 9
                                    height: 9
                                    radius: 4.5
                                    color: ThemeColors.warning()
                                    ToolTip.text: qsTr("Staged — press Apply to write it")
                                    ToolTip.visible: stageHover.hovered || activeFocus
                                    HoverHandler { id: stageHover }
                                    activeFocusOnTab: true
                                }
                                Label {
                                    visible: modelData.status === "warning"
                                    text: modelData.statusText
                                    color: ThemeColors.warning()
                                    font.pointSize: TypeScale.caption
                                    wrapMode: Text.WordWrap
                                    Layout.maximumWidth: Platform.Units.gridUnit * 12
                                }
                            }

                            Label {
                                Layout.fillWidth: true
                                text: modelData.currentDisplay || qsTr("Not set")
                                color: ThemeColors.dimColor(page.glassMode)
                                elide: Text.ElideRight
                            }
                        }

                        ComboBox {
                            visible: modelData.kind === "choice"
                            model: modelData.choices
                            textRole: "label"
                            valueRole: "value"
                            currentIndex: page.choiceIndex(modelData)
                            displayText: currentIndex >= 0 ? currentText : modelData.currentDisplay
                            enabled: !page.readOnly && !themeBundle.busy && modelData.supported
                            onActivated: function(index) {
                                if (index >= 0 && index < modelData.choices.length)
                                    themeBundle.setComponent(modelData.id, modelData.choices[index].value)
                            }
                            Accessible.name: modelData.label
                        }

                        Switch {
                            visible: modelData.kind === "bool"
                            checked: modelData.currentValue === true
                                || modelData.currentValue === "true"
                            enabled: !page.readOnly && !themeBundle.busy
                            text: checked ? qsTr("On") : qsTr("Off")
                            onClicked: themeBundle.setComponent(modelData.id, checked)
                            Accessible.name: modelData.label
                        }

                        RowLayout {
                            visible: modelData.kind === "int" || modelData.kind === "float"
                            spacing: Platform.Units.smallSpacing
                            Layout.preferredWidth: Platform.Units.gridUnit * 16

                            Slider {
                                id: valueSlider
                                Layout.fillWidth: true
                                from: modelData.minimum !== undefined && modelData.minimum !== null
                                      ? Number(modelData.minimum) : 0
                                to: modelData.maximum !== undefined && modelData.maximum !== null
                                    ? Number(modelData.maximum) : 100
                                stepSize: modelData.step !== undefined && modelData.step !== null
                                          ? Number(modelData.step) : 1
                                value: Number(modelData.currentValue !== undefined
                                             && modelData.currentValue !== null
                                             && modelData.currentValue !== ""
                                             ? modelData.currentValue : from)
                                enabled: !page.readOnly && !themeBundle.busy
                                onMoved: themeBundle.setComponent(
                                    modelData.id,
                                    modelData.kind === "int" ? Math.round(value) : value
                                )
                                Accessible.name: modelData.label
                            }
                            Label {
                                text: (modelData.kind === "int"
                                       ? Math.round(valueSlider.value)
                                       : Number(valueSlider.value).toFixed(2))
                                      + (modelData.suffix || "")
                                color: ThemeColors.dimColor(page.glassMode)
                                horizontalAlignment: Text.AlignRight
                                Layout.preferredWidth: Platform.Units.gridUnit * 4
                            }
                        }

                        Button {
                            visible: modelData.kind === "wallpaper"
                            text: qsTr("Choose…")
                            icon.name: "document-open-folder"
                            enabled: !page.readOnly && !themeBundle.busy
                            onClicked: {
                                page.wallpaperComponentId = modelData.id
                                wallpaperDialog.open()
                            }
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Platform.Units.smallSpacing

                        Label {
                            Layout.fillWidth: true
                            text: modelData.description || ""
                            wrapMode: Text.WordWrap
                            color: ThemeColors.dimColorSoft(page.glassMode)
                            font.pointSize: TypeScale.caption
                            visible: text !== ""
                        }

                        Button {
                            text: qsTr("KDE settings")
                            icon.name: "settings-configure"
                            visible: modelData.externalModule !== ""
                            onClicked: themeBundle.openSystemSettings(modelData.externalModule)
                        }

                        Button {
                            text: qsTr("Customize")
                            icon.name: "document-edit"
                            visible: !page.readOnly && modelData.legacySection !== ""
                                && (modelData.kind === "font" || modelData.kind === "int"
                                    || modelData.kind === "float")
                            onClicked: pageStack.layers.push(
                                Qt.resolvedUrl("SectionPage.qml"), {
                                    sectionId: modelData.legacySection,
                                    sectionTitle: modelData.label,
                                    showAdvanced: true
                                })
                        }
                    }
                }
            }
        }

        Label {
            Layout.fillWidth: true
            visible: page.components.length === 0
            text: qsTr("No components in this group.")
            color: ThemeColors.dimColor(page.glassMode)
        }
    }
}
