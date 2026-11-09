import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami.platform as Platform

// What a palette looks like before applying it: a mock window (title bar,
// list with a selection, button, input, link) and a terminal with the 16
// ANSI colours, plus contrast checks (WCAG ratio, 4.5 for text). Colours are
// the palette's own data (base16: base00…base0F + accent).
ColumnLayout {
    id: preview
    property var base: []
    property string accent: "#3daee9"
    property bool glassMode: false
    spacing: Platform.Units.mediumSpacing

    function c(i) { return base[i] || "#000000" }
    function lum(hex) {
        var col = Qt.color(String(hex || "#000000"))
        function ch(v) { return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4) }
        return 0.2126 * ch(col.r) + 0.7152 * ch(col.g) + 0.0722 * ch(col.b)
    }
    function ratio(a, b) {
        var la = lum(a), lb = lum(b)
        return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05)
    }
    // Text on the accent: whichever of black/white reads better.
    function textOn(bg) { return ratio(bg, "#000000") >= ratio(bg, "#ffffff") ? "#000000" : "#ffffff" }
    readonly property string onAccent: textOn(accent)

    RowLayout {
        Layout.fillWidth: true
        spacing: Platform.Units.largeSpacing

        // ── window ──
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: Platform.Units.gridUnit * 11
            radius: Platform.Units.smallSpacing
            color: preview.c(0)
            border.width: 1
            border.color: preview.c(2)
            clip: true
            ColumnLayout {
                anchors.fill: parent
                spacing: 0
                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: Platform.Units.gridUnit * 1.6
                    color: preview.c(1)
                    Label {
                        anchors.centerIn: parent
                        text: qsTr("Window title")
                        color: preview.c(5)
                    }
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.margins: Platform.Units.smallSpacing
                    spacing: 2
                    Repeater {
                        model: [qsTr("Documents"), qsTr("Pictures"), qsTr("Music")]
                        delegate: Rectangle {
                            required property string modelData
                            required property int index
                            Layout.fillWidth: true
                            Layout.preferredHeight: Platform.Units.gridUnit * 1.3
                            radius: 3
                            color: index === 1 ? preview.accent : (index % 2 ? preview.c(1) : preview.c(0))
                            Label {
                                anchors.verticalCenter: parent.verticalCenter
                                x: Platform.Units.smallSpacing
                                text: modelData
                                color: index === 1 ? preview.onAccent : preview.c(5)
                            }
                        }
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        Rectangle {
                            Layout.preferredWidth: Platform.Units.gridUnit * 5
                            Layout.preferredHeight: Platform.Units.gridUnit * 1.4
                            radius: 3
                            color: preview.c(2)
                            Label { anchors.centerIn: parent; text: qsTr("Button"); color: preview.c(5) }
                        }
                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: Platform.Units.gridUnit * 1.4
                            radius: 3
                            color: preview.c(0)
                            border.width: 1
                            border.color: preview.c(3)
                            Label {
                                anchors.verticalCenter: parent.verticalCenter
                                x: Platform.Units.smallSpacing
                                text: qsTr("Search…")
                                color: preview.c(4)
                            }
                        }
                    }
                    Label {
                        text: qsTr("A link")
                        color: preview.c(13)
                    }
                }
            }
        }

        // ── terminal ──
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: Platform.Units.gridUnit * 11
            radius: Platform.Units.smallSpacing
            color: preview.c(0)
            border.width: 1
            border.color: preview.c(2)
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: Platform.Units.smallSpacing
                spacing: 2
                Label { text: "$ ls --color"; color: preview.c(5); font.family: "monospace" }
                Label { text: "src/  docs/  README.md"; color: preview.c(13); font.family: "monospace" }
                Label { Layout.fillWidth: true; elide: Text.ElideRight; text: qsTr("error: missing file"); color: preview.c(8); font.family: "monospace" }
                Label { Layout.fillWidth: true; elide: Text.ElideRight; text: qsTr("ok: 263 passed"); color: preview.c(11); font.family: "monospace" }
                Label { Layout.fillWidth: true; elide: Text.ElideRight; text: qsTr("warning: deprecated"); color: preview.c(10); font.family: "monospace" }
                Label { Layout.fillWidth: true; elide: Text.ElideRight; text: qsTr("# a comment"); color: preview.c(3); font.family: "monospace" }
                Item { Layout.fillHeight: true }
                // ANSI 0–15 in the usual base16 mapping
                Grid {
                    columns: 8
                    spacing: 2
                    Repeater {
                        model: [0, 8, 11, 10, 13, 14, 12, 5, 3, 8, 11, 10, 13, 14, 12, 7]
                        delegate: Rectangle {
                            required property int modelData
                            width: Platform.Units.gridUnit
                            height: Platform.Units.gridUnit * 0.7
                            radius: 2
                            color: preview.c(modelData)
                            border.width: 1
                            border.color: preview.c(2)
                        }
                    }
                }
            }
        }
    }

    // ── contrast ──
    Flow {
        Layout.fillWidth: true
        spacing: Platform.Units.largeSpacing
        Repeater {
            model: [
                { what: qsTr("Text"), fg: preview.c(5), bg: preview.c(0), need: 4.5 },
                { what: qsTr("Dim text"), fg: preview.c(4), bg: preview.c(0), need: 3.0 },
                { what: qsTr("Selection"), fg: preview.textOn(preview.accent), bg: preview.accent, need: 4.5 },
                { what: qsTr("Link"), fg: preview.c(13), bg: preview.c(0), need: 3.0 },
            ]
            delegate: Label {
                required property var modelData
                readonly property real r: preview.ratio(modelData.fg, modelData.bg)
                text: (r >= modelData.need ? "✓ " : "⚠ ") + modelData.what + " " + r.toFixed(1) + ":1"
                color: r >= modelData.need ? ThemeColors.positive() : ThemeColors.warning()
            }
        }
    }
}
