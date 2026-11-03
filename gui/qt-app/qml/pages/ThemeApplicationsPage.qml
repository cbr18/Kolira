import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami
import org.kde.kirigami.platform as Platform
import Components 1.0

/*! Per-application theme page built on app adapters. */
Kirigami.ScrollablePage {
    id: page

    title: qsTr("Application themes")
    readonly property bool glassMode: themeController.isGlass
    readonly property var apps: themeBundle.applications

    function choiceIndex(app) {
        for (let i = 0; i < app.themes.length; ++i) {
            if (app.themes[i].id === app.currentValue)
                return i
        }
        return -1
    }

    background: Rectangle {
        color: page.glassMode ? ThemeColors.glassPanel() : ThemeColors.pageBg(page.glassMode)
    }

    ColumnLayout {
        Layout.margins: Platform.Units.largeSpacing
        spacing: Platform.Units.largeSpacing

        Label {
            Layout.fillWidth: true
            text: qsTr("Installed themes are detected for each application. If automatic control is not available, open the application's native theme settings.")
            wrapMode: Text.WordWrap
            color: ThemeColors.dimColor(page.glassMode)
        }

        Repeater {
            model: page.apps

            Kirigami.Card {
                required property var modelData
                Layout.fillWidth: true

                background: Rectangle {
                    radius: Platform.Units.smallSpacing
                    border.width: 1
                    color: ThemeColors.cardBg(page.glassMode)
                    border.color: modelData.status === "not_installed"
                        ? ThemeColors.subtleBorder(page.glassMode)
                        : ThemeColors.cardBorder(page.glassMode)
                }

                contentItem: ColumnLayout {
                    spacing: Platform.Units.smallSpacing

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Platform.Units.mediumSpacing

                        Kirigami.Icon {
                            source: modelData.icon || "application-x-executable"
                            color: modelData.installed
                                ? ThemeColors.accent() : ThemeColors.dimColor(page.glassMode)
                            Layout.preferredWidth: Platform.Units.iconSizes.small
                            Layout.preferredHeight: Platform.Units.iconSizes.small
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 0
                            Label {
                                Layout.fillWidth: true
                                text: modelData.name
                                color: ThemeColors.fgColor(page.glassMode)
                                font.bold: true
                            }
                            Label {
                                Layout.fillWidth: true
                                text: !modelData.installed
                                    ? qsTr("Not installed")
                                    : modelData.capabilities.current_theme !== true
                                        ? qsTr("Theme is chosen inside the app")
                                        : (modelData.currentDisplay || qsTr("No theme selected"))
                                color: ThemeColors.dimColor(page.glassMode)
                                elide: Text.ElideRight
                            }
                        }

                        Label {
                            visible: modelData.status !== "ok" && modelData.statusText !== ""
                            text: modelData.statusText
                            color: modelData.status === "not_installed"
                                ? ThemeColors.dimColor(page.glassMode) : ThemeColors.warning()
                            font.pointSize: TypeScale.caption
                            wrapMode: Text.WordWrap
                            Layout.maximumWidth: Platform.Units.gridUnit * 10
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Platform.Units.smallSpacing

                        ComboBox {
                            id: themeChooser
                            Layout.fillWidth: true
                            // A list we can neither read nor apply (Telegram) is
                            // just an empty disabled box — hide it.
                            visible: modelData.capabilities.installed_themes === true
                                && modelData.capabilities.apply_theme === true
                                && modelData.themes.length > 0
                            model: modelData.themes
                            textRole: "label"
                            valueRole: "id"
                            currentIndex: page.choiceIndex(modelData)
                            displayText: currentIndex >= 0 ? currentText : modelData.currentDisplay
                            enabled: modelData.installed
                                && modelData.capabilities.apply_theme === true
                                && modelData.themes.length > 0
                                && !themeBundle.busy
                            onActivated: function(index) {
                                if (index >= 0 && index < modelData.themes.length)
                                    themeBundle.setComponent(modelData.id, modelData.themes[index].id)
                            }
                            Accessible.name: qsTr("%1 theme").arg(modelData.name)
                        }

                        Button {
                            text: qsTr("Open config")
                            icon.name: "document-open"
                            visible: modelData.capabilities.open_config === true
                            enabled: modelData.configPath !== "" && !themeBundle.busy
                            onClicked: themeBundle.openAppConfig(modelData.id)
                        }

                        Button {
                            text: qsTr("Launch")
                            icon.name: "media-playback-start"
                            visible: modelData.capabilities.launch === true
                            enabled: modelData.installed && !themeBundle.busy
                            onClicked: themeBundle.launchApp(modelData.id)
                        }

                        Button {
                            text: qsTr("App theme settings")
                            icon.name: "settings-configure"
                            visible: modelData.capabilities.open_theme_settings === true
                            enabled: modelData.installed && !themeBundle.busy
                            onClicked: themeBundle.openAppThemeSettings(modelData.id)
                        }
                    }

                    Label {
                        Layout.fillWidth: true
                        visible: modelData.configPath !== "" && modelData.integration === "config"
                        text: qsTr("Config: %1").arg(modelData.configDisplay)
                        wrapMode: Text.WordWrap
                        color: ThemeColors.dimColorSoft(page.glassMode)
                        font.pointSize: TypeScale.caption
                    }

                    Label {
                        Layout.fillWidth: true
                        visible: modelData.capabilities.apply_theme !== true && modelData.installed
                        text: modelData.integration === "config"
                            ? qsTr("This application is styled through its config file. Open it in the default editor; profile snapshots still include the file.")
                            : modelData.themes.length > 0
                                ? qsTr("This application can list themes, but automatic applying is not supported yet.")
                                : qsTr("No installed themes were detected for this application.")
                        wrapMode: Text.WordWrap
                        color: ThemeColors.dimColorSoft(page.glassMode)
                        font.pointSize: TypeScale.caption
                    }
                }
            }
        }
    }
}
