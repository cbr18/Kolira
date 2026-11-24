import QtQuick

// The apply screen: black, one procedural shader fed by the theme palette,
// the theme name and the real stage scale. No progress is invented here.
Item {
    id: root

    property var current: ({})
    property color accent: "#8a8a94"
    property var swatches: []
    property bool running: false
    property int stage: 0
    property string statusText: ""
    property real trackWidth: 420

    readonly property string name: (current && current.name) ? current.name : ""
    readonly property real intensity: 1.0

    Rectangle {
        anchors.fill: parent
        color: "black"
    }

    // The shader is the only decoration; a failure must not leave a black
    // screen with no feedback, so the text and the bar are separate items.
    ShaderEffect {
        id: shader
        anchors.fill: parent
        fragmentShader: "shaders/apply.qsb"
        opacity: root.running ? 1.0 : 0.0
        Behavior on opacity { NumberAnimation { duration: 200 } }

        property real u_time: 0
        property vector2d u_resolution: Qt.vector2d(root.width, root.height)
        property vector3d u_accent: Qt.vector3d(root.accent.r, root.accent.g, root.accent.b)
        property vector3d u_c0: Qt.vector3d(colourAt(0).r, colourAt(0).g, colourAt(0).b)
        property vector3d u_c1: Qt.vector3d(colourAt(1).r, colourAt(1).g, colourAt(1).b)
        property vector3d u_c2: Qt.vector3d(colourAt(2).r, colourAt(2).g, colourAt(2).b)
        property real u_intensity: root.intensity

        SequentialAnimation on u_time {
            running: root.running
            loops: Animation.Infinite
            NumberAnimation { from: 0; to: 400; duration: 40000 }
        }
    }

    function colourAt(index) {
        var list = root.swatches || []
        if (list.length > index) {
            var value = Qt.color(list[index])
            if (value.valid) return value
        }
        return index === 0 ? "#101014" : (index === 1 ? root.accent : "#1a1a22")
    }

    Column {
        anchors.centerIn: parent
        spacing: 18
        width: Math.min(root.width * 0.8, 640)

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: root.name
            color: "#f4f4f6"
            font.pixelSize: 28
        }

        Rectangle {
            id: progressTrack
            anchors.horizontalCenter: parent.horizontalCenter
            width: root.trackWidth
            height: 4
            radius: 2
            color: "#1c1c22"

            Rectangle {
                id: progressBar
                width: Math.max(24, progressTrack.width * 0.28)
                height: parent.height
                radius: 2
                color: root.accent
                y: 0
                x: 0
                SequentialAnimation on x {
                    running: root.running
                    loops: Animation.Infinite
                    NumberAnimation {
                        from: 0
                        to: Math.max(1, progressTrack.width - progressBar.width)
                        duration: 900
                    }
                    NumberAnimation {
                        from: Math.max(1, progressTrack.width - progressBar.width)
                        to: 0
                        duration: 900
                    }
                }
            }
        }

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: root.statusText
            color: "#9a9aa4"
            font.pixelSize: 14
        }
    }
}