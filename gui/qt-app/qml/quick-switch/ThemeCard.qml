import QtQuick
import QtQuick.Shapes
import QtQuick.Effects
import Qt5Compat.GraphicalEffects

import "CardGeometry.js" as Geometry

// One card. The same component serves every shape: the geometry decides the
// mask, the border and the hit area; the content is never skewed. The face
// itself lives in CardPreview.
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
        layer.samples: 4
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
    // Clipped by the geometry, so it must be a texture provider and must not
    // draw itself: `layer.enabled` + invisible. A visible source would paint
    // unmasked and the preview would spill outside the outline.
    Item {
        id: content
        anchors.fill: parent
        visible: false
        layer.enabled: true
        layer.samples: 4

        CardPreview {
            anchors.fill: parent
            theme: card.theme
            active: card.active
            shape: card.shape
            skew: card.skew
        }
    }

    // ── geometry mask ────────────────────────────────────────────────────────
    // maskSource samples this item's alpha; it is never drawn itself.
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

    OpacityMask {
        anchors.fill: content
        source: content
        maskSource: mask
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