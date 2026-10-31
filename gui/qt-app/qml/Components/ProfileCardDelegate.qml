import QtQuick
import org.kde.kirigami.platform as Platform
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami

Kirigami.AbstractCard {
    id: card

    Layout.fillWidth: true

    // Explicit data (avoids relying on QML model-role semantics)
    property string profileName: ""
    property bool profileIsActive: false
    property bool profileSelected: false
    property int profileComponentCount: 0
    property string profileCreatedAt: ""
    property string profileDescription: ""
    // Glass mode is passed down because QML components cannot see the
    // root window's properties across component-file scopes.
    property bool glassMode: false

    // Fingerprint of the saved profile (palette, scheme, assets, effects).
    // Component chips were removed: every profile tracks the same component
    // set, so they said nothing about the theme.
    property var summary: ({})
    property string _summaryLoadedFor: ""

    readonly property string displayName: profileName === "glass" ? qsTr("Glass")
        : profileName === "paper" ? qsTr("Paper")
        : profileName

    readonly property var swatches: summary.swatches ? summary.swatches : []

    readonly property string paletteLine: {
        var parts = []
        if (summary.accent) parts.push(summary.accent)
        if (summary.scheme) parts.push(summary.scheme)
        if (summary.kvantum) parts.push(summary.kvantum)
        return parts.join("  ·  ")
    }

    readonly property string detailLine: {
        var parts = []
        if (summary.lookAndFeel) parts.push(qsTr("L&F %1").arg(summary.lookAndFeel))
        if (summary.radius) parts.push(qsTr("Corners %1").arg(summary.radius))
        if (summary.blurEnabled && summary.blur) parts.push(qsTr("Blur %1").arg(summary.blur))
        if (summary.kitty) {
            var opacity = parseFloat(summary.kitty)
            if (!isNaN(opacity)) parts.push(qsTr("Kitty %1%").arg(Math.round(opacity * 100)))
        }
        return parts.join("  ·  ")
    }

    readonly property string metaLine: {
        var parts = []
        if (profileCreatedAt) parts.push(qsTr("Saved %1").arg(formatSaved(profileCreatedAt)))
        if (summary.sizeBytes) parts.push(formatSize(summary.sizeBytes))
        if (profileComponentCount) parts.push(qsTr("%n components", "", profileComponentCount))
        return parts.join("  ·  ")
    }

    function formatSaved(iso) {
        if (!iso) return ""
        var date = new Date(iso)
        if (isNaN(date.getTime())) return iso.substring(0, 10)
        return Qt.formatDate(date, "dd MMM yyyy")
    }

    function formatSize(bytes) {
        if (!bytes || bytes <= 0) return ""
        if (bytes >= 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + " MB"
        return Math.max(1, Math.round(bytes / 1024)) + " KB"
    }

    function reloadSummary(force) {
        if (!profileName) {
            summary = ({})
            _summaryLoadedFor = ""
            return
        }
        if (!force && _summaryLoadedFor === profileName) return
        _summaryLoadedFor = profileName
        summary = themeInspector.profileSummary(profileName)
    }

    Component.onCompleted: reloadSummary(false)
    onProfileNameChanged: reloadSummary(true)

    Connections {
        target: profileController
        function onProfilesChanged() { card.reloadSummary(true) }
    }

    // Selecting is a separate, cheap action. Applying stays an explicit
    // button because switching profiles touches a lot of files.
    TapHandler {
        onTapped: card.selectedClicked(card.profileName)
    }

    HoverHandler { id: hover }

    contentItem: ColumnLayout {
        spacing: Platform.Units.smallSpacing

        // ─── Header ───
        RowLayout {
            Layout.fillWidth: true
            spacing: Platform.Units.mediumSpacing

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2

                RowLayout {
                    spacing: Platform.Units.smallSpacing

                    Label {
                        text: card.displayName
                        font.bold: true
                        font.pointSize: TypeScale.subtitle
                        color: ThemeColors.fgColor(card.glassMode)
                    }

                    RowLayout {
                        visible: card.profileIsActive
                        spacing: 4

                        Rectangle {
                            Layout.alignment: Qt.AlignVCenter
                            width: 8
                            height: 8
                            radius: 4
                            color: ThemeColors.positive()
                        }

                        Label {
                            text: qsTr("Active")
                            font.pointSize: TypeScale.caption
                            font.bold: true
                            color: ThemeColors.positive()
                        }
                    }
                }

                Label {
                    Layout.fillWidth: true
                    text: card.metaLine
                    font.pointSize: TypeScale.caption
                    color: ThemeColors.dimColor(card.glassMode)
                    elide: Text.ElideRight
                }

                Label {
                    Layout.fillWidth: true
                    text: card.profileDescription
                    font.pointSize: TypeScale.caption
                    font.italic: true
                    color: ThemeColors.fgColorSoft(card.glassMode)
                    wrapMode: Text.WordWrap
                    maximumLineCount: 2
                    elide: Text.ElideRight
                    visible: card.profileDescription !== ""
                }
            }

            ToolButton {
                text: qsTr("Edit description")
                icon.name: "document-edit"
                Layout.alignment: Qt.AlignTop
                visible: hover.hovered || card.profileDescription === ""
                onClicked: card.editDescriptionClicked(card.profileName, card.profileDescription)
            }
        }

        // ─── Palette + theme fingerprint ───
        RowLayout {
            Layout.fillWidth: true
            Layout.topMargin: Platform.Units.smallSpacing
            spacing: Platform.Units.mediumSpacing
            visible: card.swatches.length > 0

            Row {
                Layout.alignment: Qt.AlignTop
                spacing: 4

                Repeater {
                    model: card.swatches

                    delegate: Rectangle {
                        width: 24
                        height: 24
                        radius: 4
                        color: modelData
                        border.width: 1
                        border.color: ThemeColors.cardBorder(card.glassMode)

                        MouseArea {
                            id: swatchArea
                            anchors.fill: parent
                            hoverEnabled: true
                            acceptedButtons: Qt.NoButton
                        }

                        ToolTip.text: modelData
                        ToolTip.visible: swatchArea.containsMouse
                    }
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 1

                Label {
                    Layout.fillWidth: true
                    text: card.paletteLine
                    font.family: "monospace"
                    font.pointSize: TypeScale.caption
                    color: ThemeColors.fgColorSoft(card.glassMode)
                    elide: Text.ElideRight
                }

                Label {
                    Layout.fillWidth: true
                    text: card.detailLine
                    font.pointSize: TypeScale.caption
                    color: ThemeColors.dimColor(card.glassMode)
                    elide: Text.ElideRight
                }
            }
        }

        // ─── Actions ───
        RowLayout {
            Layout.fillWidth: true
            Layout.topMargin: Platform.Units.smallSpacing
            spacing: Platform.Units.smallSpacing

            Button {
                text: card.profileIsActive ? qsTr("Re-apply") : qsTr("Apply")
                icon.name: card.profileIsActive ? "view-refresh" : "dialog-ok-apply"
                highlighted: !card.profileIsActive
                enabled: !profileController.busy
                onClicked: card.applyClicked(card.profileName)
            }

            Button {
                text: qsTr("Preview")
                icon.name: "document-preview"
                visible: !card.profileIsActive
                enabled: !profileController.busy
                onClicked: card.previewClicked(card.profileName)
            }

            Button {
                text: qsTr("Diff")
                icon.name: "document-properties"
                visible: !card.profileIsActive
                enabled: !profileController.busy
                onClicked: card.diffClicked(card.profileName)
            }

            Button {
                text: qsTr("Inspect")
                icon.name: "preferences-desktop-theme"
                enabled: !profileController.busy
                onClicked: card.inspectClicked(card.profileName)
            }

            Item { Layout.fillWidth: true }

            Button {
                text: qsTr("Delete")
                icon.name: "edit-delete"
                flat: true
                visible: !card.profileIsActive
                enabled: !profileController.busy
                palette.buttonText: ThemeColors.negative()
                onClicked: card.deleteClicked(card.profileName)
            }
        }
    }

    background: Rectangle {
        radius: 8
        border.width: card.profileSelected ? 2 : 1
        color: card.profileSelected
            ? ThemeColors.cardHoverBg(card.glassMode)
            : (hover.hovered ? ThemeColors.cardHoverBg(card.glassMode)
                             : ThemeColors.cardBg(card.glassMode))
        border.color: card.profileSelected
            ? (card.summary.accent ? card.summary.accent : ThemeColors.accent())
            : ThemeColors.cardBorder(card.glassMode)
    }

    // Signals
    signal selectedClicked(string profileName)
    signal applyClicked(string profileName)
    signal previewClicked(string profileName)
    signal diffClicked(string profileName)
    signal inspectClicked(string profileName)
    signal deleteClicked(string profileName)
    signal editDescriptionClicked(string profileName, string description)
}
