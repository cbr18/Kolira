import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami

Item {
    id: preview
    required property var summary
    property bool loadAssets: true
    implicitHeight: Kirigami.Units.gridUnit * 6
    implicitWidth: Kirigami.Units.gridUnit * 12
    Accessible.role: Accessible.Graphic
    Accessible.name: qsTr("Approximate appearance preview")

    readonly property var colors: summary.previewColors || ({})
    readonly property color surface: colors.view || palette.base
    readonly property color accent: colors.accent || (summary.accentFromWallpaper !== true && summary.accent) || palette.highlight

    readonly property var appearance: summary.previewStyle || ({})
    readonly property var interfaceFont: appearance.font || ({})
    readonly property var headingFont: appearance.titleFont || interfaceFont
    readonly property bool interfaceFontAvailable: !interfaceFont.family || Qt.fontFamilies().includes(interfaceFont.family)
    readonly property bool headingFontAvailable: !headingFont.family || Qt.fontFamilies().includes(headingFont.family)
    readonly property bool iconAvailable: loadAssets && !!summary.iconPreviewUrl
    readonly property real windowOpacity: appearance.windowOpacity ?? 1
    readonly property real terminalOpacity: appearance.terminalOpacity ?? 1

    function sampleSize(points, fallback) {
        return Math.max(7, Math.min(17, (points || fallback) * 0.78))
    }

    function surfaceColor(base, opacity) {
        const value = Qt.tint(base, "transparent")
        return Qt.rgba(value.r, value.g, value.b, Math.max(0, Math.min(1, opacity)))
    }

    SystemPalette { id: palette }

    Rectangle {
        anchors.fill: parent
        color: preview.surface
        border.color: palette.mid
        clip: true

        Image {
            objectName: "themeWallpaperPreview"
            anchors.fill: parent
            source: preview.loadAssets ? (preview.summary.wallpaperPreviewUrl || "") : ""
            sourceSize.width: Math.min(640, Math.ceil(width * Screen.devicePixelRatio))
            sourceSize.height: Math.min(400, Math.ceil(height * Screen.devicePixelRatio))
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            cache: true
        }

        Rectangle {
            id: sample
            objectName: "themePreviewWindow"
            anchors.centerIn: parent
            width: Math.floor(parent.width * 0.74)
            height: Math.floor(parent.height * 0.72)
            radius: Math.min(appearance.radius ?? 3, width / 8, height / 5)
            color: preview.surfaceColor(preview.colors.window || palette.window, preview.windowOpacity)
            border.color: palette.mid

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: Math.max(4, Math.min(12, sample.radius * 0.6))
                spacing: Kirigami.Units.smallSpacing

                RowLayout {
                    Layout.fillWidth: true
                    Label {
                        objectName: "themePreviewTitle"
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: qsTr("Window")
                        color: preview.colors.text || palette.text
                        elide: Text.ElideRight
                        font.bold: true
                        font.family: preview.headingFontAvailable && preview.headingFont.family
                            ? preview.headingFont.family : Qt.application.font.family
                        font.pointSize: preview.sampleSize(preview.headingFont.points, TypeScale.subtitle)
                    }
                    Rectangle {
                        width: Kirigami.Units.smallSpacing * 2
                        height: width
                        radius: Math.min(width / 2, sample.radius)
                        color: preview.accent
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    radius: Math.min(sample.radius, 5)
                    color: preview.surfaceColor(preview.colors.view || palette.alternateBase,
                                                Math.min(preview.windowOpacity, preview.terminalOpacity))
                    clip: true
                    RowLayout {
                        anchors.fill: parent
                        anchors.margins: Math.min(6, parent.height / 5)
                        Item {
                            Layout.preferredWidth: Kirigami.Units.iconSizes.small
                            Layout.preferredHeight: Kirigami.Units.iconSizes.small
                            visible: !!preview.summary.iconTheme

                            Image {
                                id: iconSample
                                objectName: "themeIconPreview"
                                anchors.fill: parent
                                source: preview.iconAvailable ? preview.summary.iconPreviewUrl : ""
                                sourceSize: Qt.size(32, 32)
                                fillMode: Image.PreserveAspectFit
                                asynchronous: true
                                visible: status === Image.Ready
                            }

                            Rectangle {
                                anchors.centerIn: parent
                                width: Math.round(parent.width * 0.7)
                                height: width
                                radius: Math.min(2, sample.radius)
                                visible: !iconSample.visible
                                color: "transparent"
                                border.color: palette.mid
                            }
                        }
                        Label {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            text: qsTr("Appearance")
                            color: preview.colors.text || palette.text
                            elide: Text.ElideRight
                            font.family: preview.interfaceFontAvailable && preview.interfaceFont.family
                                ? preview.interfaceFont.family : Qt.application.font.family
                            font.pointSize: preview.sampleSize(preview.interfaceFont.points, TypeScale.body)
                        }
                    }
                }
            }
        }
    }

    ToolTip.visible: pointer.hovered
    ToolTip.text: {
        const missing = [!interfaceFontAvailable ? interfaceFont.family : "",
                         !headingFontAvailable ? headingFont.family : ""].filter(name => name)
        return qsTr("Approximate appearance preview") + (missing.length
            ? "\n" + qsTr("System font used in place of %1").arg([...new Set(missing)].join(", ")) : "")
    }
    HoverHandler { id: pointer }
}
