import QtQuick
import QtQuick.Shapes
import QtQuick.Effects

import "CardGeometry.js" as Geometry

// One card. The same component serves every shape: the geometry decides the
// mask, the border and the hit area; the content is never skewed.
Item {
    id: card

    property var theme: ({})
    property bool active: false
    property string shape: "rounded"
    property real cardHeight: 400
    property real radius: 18
    property real skew: 0.18
    property bool interactive: true
    property int index: 0

    readonly property color accent: (theme && theme.accent) ? theme.accent : "#8a8a94"
    readonly property string outline: Geometry.outline(
        card.shape, card.width, card.height, card.radius, card.skew)

    width: Math.round(cardHeight * Geometry.widthFactor(shape))
    height: cardHeight

    // ── glow (behind the card, active only) ──────────────────────────────────
    Shape {
        anchors.fill: parent
        opacity: card.active ? 0.5 : 0.0
        layer.enabled: true
        layer.effect: MultiEffect {
            blurEnabled: true
            blur: 1.0
            autoPaddingEnabled: true
        }
        antialiasing: true

        ShapePath {
            fillColor: "transparent"
            strokeColor: card.accent
            strokeWidth: 8
            PathSvg { path: card.outline }
        }

        Behavior on opacity { NumberAnimation { duration: 200 } }
    }

    // ── content ──────────────────────────────────────────────────────────────
    Item {
        id: content
        anchors.fill: parent

        Rectangle {
            anchors.fill: parent
            gradient: Gradient {
                GradientStop { position: 0.0; color: card.accent }
                GradientStop { position: 1.0; color: "#0d0d11" }
            }
            opacity: 0.30
        }

        Image {
            id: preview
            anchors.fill: parent
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            cache: true
            smooth: true
            source: (card.theme && card.theme.preview && card.theme.preview.path)
                    ? "file://" + card.theme.preview.path + "?rev=" + card.theme.preview.revision
                    : ""
            visible: status === Image.Ready
        }

        Rectangle {
            anchors.fill: parent
            gradient: Gradient {
                GradientStop { position: 0.0; color: "#00000000" }
                GradientStop { position: 0.45; color: "#00000000" }
                GradientStop { position: 1.0; color: "#d0000000" }
            }
        }

        Column {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.leftMargin: 18
            anchors.rightMargin: 18
            anchors.bottomMargin: 20
            spacing: 10

            Text {
                width: parent.width
                text: (card.theme && card.theme.name) ? card.theme.name : ""
                color: "#f2f2f4"
                font.pixelSize: 20
                font.weight: card.active ? Font.DemiBold : Font.Normal
                elide: Text.ElideRight
                maximumLineCount: 2
                wrapMode: Text.WordWrap
            }

            Row {
                spacing: 6
                Repeater {
                    model: (card.theme && card.theme.swatches)
                           ? card.theme.swatches.slice(0, 6) : []
                    delegate: Rectangle {
                        required property var modelData
                        width: 22
                        height: 6
                        radius: 3
                        color: modelData
                        opacity: 0.9
                    }
                }
            }

            Rectangle {
                width: 64
                height: 3
                radius: 2
                color: card.accent
                opacity: card.active ? 0.95 : 0.35
            }
        }
    }

    // ── geometry mask ────────────────────────────────────────────────────────
    Item {
        id: mask
        anchors.fill: parent
        visible: false

        Shape {
            anchors.fill: parent
            antialiasing: true

            ShapePath {
                fillColor: "white"
                strokeColor: "transparent"
                strokeWidth: 0
                PathSvg { path: card.outline }
            }
        }
    }

    MultiEffect {
        source: content
        maskEnabled: true
        maskSource: mask
        anchors.fill: content
        autoPaddingEnabled: false
    }

    // ── border ───────────────────────────────────────────────────────────────
    Shape {
        anchors.fill: parent
        antialiasing: true

        ShapePath {
            fillColor: "transparent"
            strokeColor: card.active ? card.accent : "#3a3a44"
            strokeWidth: card.active ? 3 : 1
            PathSvg { path: card.outline }
            Behavior on strokeWidth { NumberAnimation { duration: 160 } }
        }
    }

    MouseArea {
        anchors.fill: parent
        enabled: card.interactive
        onClicked: quickSwitch.selectIndex(card.index)
        onDoubleClicked: quickSwitch.confirm()
    }
}