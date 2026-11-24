import QtQuick

import "CardGeometry.js" as Geometry

// The whole overlay: dimmed desktop, card carousel, closing wave, black
// apply screen with the real stages, and the reveal.
Item {
    id: root

    property bool primary: true
    property bool interactive: true

    readonly property string phase: quickSwitch.phase
    readonly property real cardScale: {
        var value = quickSwitch.settings.card_scale
        return (typeof value === "number" && value > 0) ? value : 1.0
    }
    readonly property real cardHeight: Math.round(root.height * 0.60 * root.cardScale)
    readonly property real cardWidth: Math.round(root.cardHeight * Geometry.widthFactor(
        quickSwitch.settings.card_shape))
    readonly property real spacing: Math.round(root.cardHeight * 0.07)
    // Space before the first card so it can reach the centre, and after the
    // last one for the same reason.
    readonly property real edgeInset: Math.max(
        0, Math.round((root.width - root.cardWidth) / 2 - root.spacing / 2))
    readonly property real dimOpacity: {
        var value = quickSwitch.settings.dim
        return (typeof value === "number") ? Math.max(0, Math.min(1, value)) : 0.72
    }
    readonly property string shape: quickSwitch.settings.card_shape || "rounded"
    readonly property var current: quickSwitch.current
    readonly property color accent: (current && current.accent) ? current.accent : "#8a8a94"
    readonly property real progressTrackWidth: Math.min(Math.round(root.width * 0.48), 420)

    // Cards the user may see right now: a search filter over the full list.
    // A spacer at both ends keeps the first and last card able to reach the
    // centre — without them the ends stop at the screen edge.
    readonly property var visibleThemes: {
        var query = root.searchText.toLowerCase()
        var cards = []
        var themes = quickSwitch.themes || []
        for (var i = 0; i < themes.length; i++) {
            var theme = themes[i]
            if (theme && theme.name && theme.name.toLowerCase().indexOf(query) !== -1) {
                cards.push({"theme": theme, "index": i})
            }
        }
        return [{"spacer": true}].concat(cards, [{"spacer": true}])
    }
    readonly property int activePosition: {
        for (var i = 0; i < root.visibleThemes.length; i++) {
            var item = root.visibleThemes[i]
            if (item && item.index === quickSwitch.index) return i
        }
        return 1
    }

    property string searchText: ""

    focus: true

    // ── desktop dimming ──────────────────────────────────────────────────────
    Rectangle {
        anchors.fill: parent
        color: "#000000"
        opacity: root.dimOpacity
        Behavior on opacity { NumberAnimation { duration: 220 } }
    }

    Rectangle {
        anchors.fill: parent
        opacity: 0.45
        gradient: Gradient {
            GradientStop { position: 0.0; color: "#000000" }
            GradientStop { position: 0.5; color: "#00000000" }
            GradientStop { position: 1.0; color: "#000000" }
        }
    }

    // ── carousel ─────────────────────────────────────────────────────────────
    ListView {
        id: carousel
        visible: root.primary
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.verticalCenter: parent.verticalCenter
        anchors.verticalCenterOffset: -Math.round(root.height * 0.02)
        width: root.width
        height: Math.round(root.cardHeight * 1.5)
        orientation: ListView.Horizontal
        snapMode: ListView.SnapOneItem
        highlightRangeMode: ListView.StrictlyEnforceRange
        preferredHighlightBegin: Math.round((root.width - root.cardWidth) / 2)
        preferredHighlightEnd: Math.round((root.width - root.cardWidth) / 2)
        highlightMoveDuration: 260
        highlightMoveVelocity: 600
        boundsBehavior: Flickable.StopAtBounds
        clip: false
        model: root.visibleThemes
        currentIndex: 0

        delegate: Item {
            id: slot
            width: (slot.modelData && slot.modelData.spacer)
                   ? root.edgeInset
                   : root.cardWidth + root.spacing
            height: carousel.height
            required property var modelData
            required property int index

            Loader {
                anchors.centerIn: parent
                active: !(slot.modelData && slot.modelData.spacer)

                sourceComponent: ThemeCard {
                    theme: slot.modelData ? slot.modelData.theme : ({})
                    index: slot.modelData ? slot.modelData.index : 0
                    shape: root.shape
                    cardHeight: root.cardHeight
                    radius: Math.round(root.cardHeight * 0.045)
                    active: slot.modelData ? quickSwitch.index === slot.modelData.index : false
                    interactive: root.phase === "select" && root.primary

                    opacity: active ? 1.0 : 0.55
                    scale: active ? 1.0 : 0.94
                    z: active ? 2 : 1

                    Behavior on opacity { NumberAnimation { duration: 220 } }
                    Behavior on scale { NumberAnimation { duration: 220; easing.type: Easing.OutCubic } }
                }
            }
        }

        onMovingChanged: if (!moving) root.commitCarousel()
    }

    Connections {
        target: quickSwitch
        function onIndexChanged() { root.syncCarousel() }
    }

    Connections {
        target: root
        function onVisibleThemesChanged() { root.syncCarousel() }
    }

    // Keep the selected card in the centre. `currentIndex` is also written by
    // flicking, so it is never bound — it is only re-applied here.
    function syncCarousel() {
        if (carousel.count === 0) return
        // Slots 0 and count-1 are the spacers.
        var last = Math.max(1, carousel.count - 2)
        var position = Math.max(1, Math.min(root.activePosition, last))
        if (carousel.currentIndex !== position) carousel.currentIndex = position
    }

    function commitCarousel() {
        var item = root.visibleThemes[carousel.currentIndex]
        if (item && !item.spacer && item.index !== quickSwitch.index) {
            quickSwitch.selectIndex(item.index)
        }
    }

    // ── footer ───────────────────────────────────────────────────────────────
    Column {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Math.round(root.height * 0.06)
        spacing: 8
        visible: root.phase === "select" && root.primary

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: root.searchText.length > 0
                  ? qsTr("Поиск: %1").arg(root.searchText)
                  : ((root.current && root.current.name) ? root.current.name : "")
            color: "#f2f2f4"
            font.pixelSize: 18
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: quickSwitch.previewOnly
                  ? qsTr("Предпросмотр — тема не применяется")
                  : qsTr("← → выбор · Enter применить · Esc закрыть · печатайте для поиска")
            color: "#9a9aa4"
            font.pixelSize: 13
        }
    }

    // ── closing wave + blackout ──────────────────────────────────────────────
    Rectangle {
        id: wave
        anchors.centerIn: parent
        width: root.height
        height: root.height
        radius: width / 2
        color: root.accent
        opacity: 0
        scale: 0
        visible: false
    }

    Rectangle {
        id: blackout
        anchors.fill: parent
        color: "#000000"
        opacity: 0
    }

    // ── apply screen ─────────────────────────────────────────────────────────
    ApplyScreen {
        id: applyScreen
        anchors.fill: parent
        opacity: 0
        visible: opacity > 0.01

        current: root.current
        accent: root.accent
        swatches: (root.current && root.current.swatches) ? root.current.swatches : []
        running: root.phase === "applying"
                 && quickSwitch.settings.apply_animation !== false
                 && !quickSwitch.reducedMotion
        stage: quickSwitch.stage
        statusText: quickSwitch.status.length > 0 ? quickSwitch.status : root.stageText
        trackWidth: root.progressTrackWidth

        Behavior on opacity { NumberAnimation { duration: 180 } }
    }

    readonly property string stageText: {
        if (quickSwitch.stage === 0) return qsTr("Подготовка…")
        if (quickSwitch.stage === 1) return qsTr("Применение…")
        return qsTr("Готово")
    }

    // ── attention / error panel ──────────────────────────────────────────────
    Rectangle {
        id: panel
        anchors.centerIn: parent
        width: Math.min(Math.round(root.width * 0.5), 560)
        implicitHeight: panelColumn.implicitHeight + 48
        radius: 14
        color: "#15151bf0"
        border.color: "#3a3a44"
        border.width: 1
        visible: root.phase === "attention" || root.phase === "error"

        Column {
            id: panelColumn
            anchors.centerIn: parent
            width: parent.width - 48
            spacing: 14

            Text {
                width: parent.width
                text: root.phase === "error" ? qsTr("Не удалось применить") : qsTr("Тема требует внимания")
                color: "#f2f2f4"
                font.pixelSize: 20
                wrapMode: Text.WordWrap
            }

            Text {
                width: parent.width
                text: quickSwitch.message
                color: "#f0a0a0"
                font.pixelSize: 14
                wrapMode: Text.WordWrap
                visible: quickSwitch.message.length > 0
            }

            Repeater {
                model: quickSwitch.attention
                delegate: Text {
                    required property var modelData
                    width: panelColumn.width
                    text: "• " + modelData
                    color: "#c8c8d0"
                    font.pixelSize: 13
                    wrapMode: Text.WordWrap
                }
            }

            Row {
                spacing: 12
                Rectangle {
                    radius: 8
                    color: "#2a2a33"
                    implicitWidth: closeLabel.implicitWidth + 28
                    implicitHeight: 36
                    Text {
                        id: closeLabel
                        anchors.centerIn: parent
                        text: qsTr("Закрыть")
                        color: "#f2f2f4"
                        font.pixelSize: 14
                    }
                    MouseArea { anchors.fill: parent; onClicked: quickSwitch.dismiss() }
                }
                Rectangle {
                    radius: 8
                    color: "#2a2a33"
                    implicitWidth: koliraLabel.implicitWidth + 28
                    implicitHeight: 36
                    Text {
                        id: koliraLabel
                        anchors.centerIn: parent
                        text: qsTr("Открыть Kolira")
                        color: "#f2f2f4"
                        font.pixelSize: 14
                    }
                    MouseArea { anchors.fill: parent; onClicked: quickSwitch.openInKolira() }
                }
            }
        }
    }

    // ── phase plumbing ───────────────────────────────────────────────────────
    onPhaseChanged: root.runPhase(root.phase)

    function runPhase(phase) {
        if (phase === "closing") {
            // `closeAnimation` is an id, not a property: `root.closeAnimation`
            // is undefined and the animation would never start.
            closeAnimation.start()
            return
        }
        if (phase === "revealing") {
            applyScreen.opacity = 1
            revealAnimation.start()
            return
        }
        if (phase === "applying") {
            // The closing animation already reached full black; keep it.
            return
        }
        // attention, error and select: drop the blackout.
        wave.visible = false
        blackout.opacity = 0
        applyScreen.opacity = 0
    }

    // Closing: the wave spreads and the screen reaches full black before the
    // Rust transaction starts.
    SequentialAnimation {
        id: closeAnimation
        ScriptAction {
            script: {
                wave.visible = true
                wave.opacity = 0.9
                wave.scale = 0
            }
        }
        ParallelAnimation {
            NumberAnimation {
                target: wave
                property: "scale"
                from: 0
                to: Math.max(2, root.width / root.height)
                duration: 250
                easing.type: Easing.InCubic
            }
            NumberAnimation { target: wave; property: "opacity"; from: 0.9; to: 1; duration: 250 }
            NumberAnimation { target: blackout; property: "opacity"; from: 0; to: 1; duration: 250 }
        }
        ScriptAction { script: quickSwitch.beginApply() }
    }

    // Reveal: the black opens and the new desktop appears.
    SequentialAnimation {
        id: revealAnimation
        ParallelAnimation {
            NumberAnimation {
                target: blackout
                property: "opacity"
                from: 1
                to: 0
                duration: 250
                easing.type: Easing.OutCubic
            }
            NumberAnimation {
                target: applyScreen
                property: "opacity"
                from: 1
                to: 0
                duration: 250
            }
        }
        ScriptAction { script: quickSwitch.finishReveal() }
    }

    // ── input ────────────────────────────────────────────────────────────────
    function handleKey(event) {
        var phase = root.phase
        if (event.key === Qt.Key_Escape) {
            if (phase === "applying") {
                // A running transaction is never aborted by Esc (Q9).
                return
            }
            quickSwitch.dismiss()
            event.accepted = true
            return
        }
        if (phase !== "select") return
        if (event.key === Qt.Key_Left) {
            quickSwitch.step(-1)
            event.accepted = true
        } else if (event.key === Qt.Key_Right) {
            quickSwitch.step(1)
            event.accepted = true
        } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            quickSwitch.confirm()
            event.accepted = true
        } else if (event.key === Qt.Key_Backspace) {
            root.searchText = root.searchText.slice(0, -1)
            event.accepted = true
        } else if (event.text && event.text.length === 1 && !event.modifiers) {
            root.searchText += event.text
            event.accepted = true
        }
    }

    Keys.onPressed: function(event) { handleKey(event) }

    WheelHandler {
        target: carousel
        acceptedDevices: PointerDevice.Mouse
        onWheel: function(event) {
            if (root.phase !== "select") return
            if (event.angleDelta.y > 0) quickSwitch.step(-1)
            else if (event.angleDelta.y < 0) quickSwitch.step(1)
        }
    }

    Component.onCompleted: root.forceActiveFocus()
}