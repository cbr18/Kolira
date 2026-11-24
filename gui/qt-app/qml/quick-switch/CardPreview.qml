import QtQuick

// One card's face: wallpaper, a small mock window carrying the theme's real
// colours, fonts and opacity, and the name with the palette. The shape is
// applied by ThemeCard; here it only decides how much room is safe.
Item {
    id: root

    property var theme: ({})
    property bool active: false
    property string shape: "rounded"
    property real skew: 0.18

    readonly property var colors: (theme && theme.previewColors) ? theme.previewColors : ({})
    readonly property var appearance: (theme && theme.previewStyle) ? theme.previewStyle : ({})
    readonly property var uiFont: appearance.font || ({})
    readonly property var titleFont: appearance.titleFont || uiFont
    readonly property real windowOpacity: {
        var value = appearance.windowOpacity
        return (typeof value === "number") ? Math.max(0.15, Math.min(1, value)) : 1
    }
    readonly property real terminalOpacity: {
        var value = appearance.terminalOpacity
        return (typeof value === "number") ? Math.max(0.15, Math.min(1, value)) : 1
    }
    readonly property real cornerRadius: {
        var value = appearance.radius
        return (typeof value === "number") ? Math.max(0, Math.min(value, 40)) : 4
    }
    readonly property color surface: colors.window || "#1b1b21"
    readonly property color view: colors.view || surface
    readonly property color accent: colors.accent || (theme && theme.accent) || "#8a8a94"
    readonly property color text: colors.text || "#f2f2f4"

    // A parallelogram is a sheared rectangle: its width is constant, but it
    // slides sideways, so an axis-aligned block must stay inside
    // [skew·w, (1-skew)·w] to survive the mask at every height.
    readonly property real insetX: shape === "parallelogram"
        ? Math.round(root.width * root.skew) : 0
    readonly property real safeWidth: Math.max(1, root.width - root.insetX * 2)

    // The mock window is about a third of the card and sits a third from the
    // top, not in the middle.
    readonly property real mockHeight: Math.round(root.height * 0.34)

    // Cached, downscaled wallpaper when we have one; the original otherwise.
    readonly property string previewSource: {
        if (root.theme && root.theme.thumbPath) return "file://" + root.theme.thumbPath
        if (root.theme && root.theme.preview && root.theme.preview.path)
            return "file://" + root.theme.preview.path + "?rev=" + root.theme.preview.revision
        return ""
    }

    function sampleSize(points, fallback) {
        if (typeof points !== "number") return fallback
        return Math.max(6, Math.min(13, Math.round(points * 0.55)))
    }

    function translucent(color, opacity) {
        var value = Qt.color(color)
        return Qt.rgba(value.r, value.g, value.b, Math.max(0.0, Math.min(1.0, opacity)))
    }

    // ── wallpaper ────────────────────────────────────────────────────────────
    Rectangle {
        anchors.fill: parent
        gradient: Gradient {
            GradientStop { position: 0.0; color: root.surface }
            GradientStop { position: 1.0; color: root.view }
        }
        opacity: 0.6
    }

    Image {
        id: wallpaper
        anchors.fill: parent
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
        cache: true
        smooth: true
        source: root.previewSource
        // Decode at the size we paint, and fade in: a card that pops from an
        // empty rectangle looks like a loading glitch.
        sourceSize: Qt.size(
            Math.min(720, Math.max(1, Math.ceil(root.width * Screen.devicePixelRatio))),
            Math.min(900, Math.max(1, Math.ceil(root.height * Screen.devicePixelRatio))))
        opacity: status === Image.Ready ? 1.0 : 0.0
        Behavior on opacity { NumberAnimation { duration: 220 } }
    }

    Rectangle {
        anchors.fill: parent
        gradient: Gradient {
            GradientStop { position: 0.0; color: "#00000000" }
            GradientStop { position: 0.40; color: "#00000000" }
            GradientStop { position: 0.72; color: "#8a000000" }
            GradientStop { position: 1.0; color: "#e0000000" }
        }
    }

    // ── mock window ──────────────────────────────────────────────────────────
    Item {
        id: mock
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.verticalCenter: parent.verticalCenter
        anchors.verticalCenterOffset: -Math.round(root.height / 6)
        width: Math.round(root.safeWidth * 0.88)
        height: root.mockHeight

        Rectangle {
            anchors.fill: parent
            radius: Math.min(root.cornerRadius * 0.6, width / 6, height / 4)
            color: root.translucent(root.surface, root.windowOpacity)
            border.color: Qt.rgba(1, 1, 1, 0.18)
            border.width: 1

            Column {
                anchors.fill: parent
                anchors.margins: Math.max(5, Math.round(root.mockHeight * 0.08))
                spacing: Math.max(3, Math.round(root.mockHeight * 0.05))

                Row {
                    width: parent.width
                    height: titleLabel.height
                    spacing: 6

                    Text {
                        id: titleLabel
                        width: parent.width - dot.width - 6
                        text: root.theme && root.theme.paletteName
                              ? root.theme.paletteName : "Window"
                        color: root.text
                        font.family: (root.titleFont.family && Qt.fontFamilies().indexOf(root.titleFont.family) >= 0)
                                     ? root.titleFont.family : Qt.application.font.family
                        font.pixelSize: root.sampleSize(root.titleFont.points, 9)
                        font.bold: true
                        elide: Text.ElideRight
                        verticalAlignment: Text.AlignVCenter
                    }

                    Rectangle {
                        id: dot
                        width: Math.max(6, Math.round(root.mockHeight * 0.07))
                        height: width
                        radius: width / 2
                        color: root.accent
                        anchors.verticalCenter: parent.verticalCenter
                    }
                }

                Rectangle {
                    width: parent.width
                    height: parent.height - titleLabel.height - parent.anchors.margins * 2
                    radius: Math.max(2, Math.min(root.cornerRadius * 0.4, 6))
                    color: root.translucent(root.view, Math.min(root.windowOpacity, root.terminalOpacity))

                    Row {
                        anchors.fill: parent
                        anchors.margins: Math.max(4, Math.round(root.mockHeight * 0.06))
                        spacing: 6

                        Rectangle {
                            id: iconBox
                            width: contentLabel.height
                            height: width
                            radius: 3
                            color: "transparent"
                            border.color: Qt.rgba(root.text.r, root.text.g, root.text.b, 0.35)
                            border.width: 1
                            visible: !(root.theme && root.theme.iconTheme)
                        }

                        Text {
                            id: contentLabel
                            width: parent.width - (root.theme && root.theme.iconTheme ? 0 : iconBox.width + 6)
                            text: "Appearance"
                            color: root.text
                            font.family: (root.uiFont.family && Qt.fontFamilies().indexOf(root.uiFont.family) >= 0)
                                         ? root.uiFont.family : Qt.application.font.family
                            font.pixelSize: root.sampleSize(root.uiFont.points, 8)
                            elide: Text.ElideRight
                            verticalAlignment: Text.AlignVCenter
                        }
                    }
                }
            }
        }
    }

    // ── name and palette ─────────────────────────────────────────────────────
    Column {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.leftMargin: Math.round(root.width * 0.09)
        anchors.rightMargin: Math.round(root.width * 0.09)
        anchors.bottomMargin: Math.round(root.height * 0.08)
        spacing: 8

        Text {
            width: parent.width
            text: (root.theme && root.theme.name) ? root.theme.name : ""
            color: "#f4f4f6"
            font.pixelSize: Math.max(12, Math.min(22, Math.round(root.height * 0.036)))
            font.weight: root.active ? Font.DemiBold : Font.Normal
            elide: Text.ElideRight
            maximumLineCount: 1
        }

        Row {
            spacing: 5
            Repeater {
                model: (root.theme && root.theme.swatches) ? root.theme.swatches.slice(0, 6) : []
                delegate: Rectangle {
                    required property var modelData
                    width: Math.max(14, Math.round(root.height * 0.045))
                    height: 5
                    radius: 2
                    color: modelData
                    opacity: 0.9
                }
            }
        }

        Rectangle {
            width: Math.round(root.height * 0.12)
            height: 3
            radius: 2
            color: root.accent
            opacity: root.active ? 0.95 : 0.35
        }
    }
}