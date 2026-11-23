import QtQuick
import org.kde.kirigami.platform as Platform
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami

Kirigami.AbstractCard {
    id: card

    Layout.fillWidth: true
    Layout.minimumWidth: 0

    // Explicit data (avoids relying on QML model-role semantics)
    property string profileName: ""
    property bool profileIsActive: false
    property bool profileLastApplied: false
    property bool profileMatchesLive: false
    property bool loadPreviewAssets: false
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
    readonly property var summary: profileController.summaries[profileName] || ({})

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
        return date.toLocaleDateString(Qt.locale(), Locale.ShortFormat)
    }

    function formatSize(bytes) {
        if (!bytes || bytes <= 0) return ""
        if (bytes >= 1024 * 1024) return qsTr("%1 MiB").arg(Number(bytes / (1024 * 1024)).toLocaleString(Qt.locale(), "f", 1))
        return qsTr("%1 KiB").arg(Number(Math.max(1, Math.round(bytes / 1024))).toLocaleString(Qt.locale(), "f", 0))
    }

    // Selecting is a separate, cheap action. Applying stays an explicit
    // button because switching profiles touches a lot of files.
    TapHandler {
        onTapped: card.selectedClicked(card.profileName)
    }

    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: profileName
    Accessible.onPressAction: selectedClicked(profileName)
    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
            selectedClicked(profileName)
            event.accepted = true
        }
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

                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    spacing: Platform.Units.smallSpacing

                    Label {
                        objectName: "themeCardName"
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        elide: Text.ElideRight
                        text: card.displayName
                        ToolTip.visible: nameHover.hovered || card.activeFocus
                        ToolTip.text: card.profileName
                        HoverHandler { id: nameHover }
                        font.bold: true
                        font.pointSize: TypeScale.subtitle
                        color: ThemeColors.fgColor(card.glassMode)
                    }

                    Label {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        visible: card.profileLastApplied || card.profileMatchesLive
                        text: card.profileMatchesLive ? qsTr("Matches Live") : qsTr("Last applied")
                        wrapMode: Text.Wrap
                        font.pointSize: TypeScale.caption
                        color: card.profileMatchesLive ? ThemeColors.positive() : ThemeColors.dimColor(card.glassMode)
                    }
                }

                Label {
                    objectName: "themeCardMetadata"
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    text: card.metaLine
                    font.pointSize: TypeScale.caption
                    color: ThemeColors.dimColor(card.glassMode)
                    elide: Text.ElideRight
                }

                Label {
                    objectName: "themeCardDescription"
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    text: card.profileDescription
                    textFormat: Text.PlainText
                    font.pointSize: TypeScale.caption
                    font.italic: true
                    color: ThemeColors.fgColorSoft(card.glassMode)
                    wrapMode: Text.Wrap
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

        ThemeAppearancePreview {
            Layout.fillWidth: true
            summary: card.summary
            loadAssets: card.loadPreviewAssets
        }

        Label {
            Layout.fillWidth: true
            visible: (card.summary.unavailableResources || []).length > 0
            text: qsTr("Some resources are unavailable")
            color: ThemeColors.warning()
            wrapMode: Text.WordWrap
        }

        // ─── Palette + theme fingerprint ───
        ColumnLayout {
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
                Layout.minimumWidth: 0
                spacing: 1

                Label {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    text: card.paletteLine
                    font.family: "monospace"
                    font.pointSize: TypeScale.caption
                    color: ThemeColors.fgColorSoft(card.glassMode)
                    elide: Text.ElideRight
                }

                Label {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    text: card.detailLine
                    font.pointSize: TypeScale.caption
                    color: ThemeColors.dimColor(card.glassMode)
                    elide: Text.ElideRight
                }
            }
        }

        ActionFlow {
            Layout.topMargin: Platform.Units.smallSpacing
            spacing: Platform.Units.smallSpacing

            AppButton {
                objectName: "themeCardApply-" + card.profileName
                text: qsTr("Apply")
                icon.name: "dialog-ok-apply"
                highlighted: true
                enabled: !profileController.busy
                onClicked: card.applyClicked(card.profileName)
            }

            ToolButton {
                id: overflowButton
                text: qsTr("More actions")
                icon.name: "overflow-menu"
                Accessible.name: text
                ToolTip.text: text
                ToolTip.visible: hovered
                enabled: !profileController.busy
                onClicked: overflowMenu.popup(overflowButton, 0, overflowButton.height)
            }

            Menu {
                id: overflowMenu

                MenuItem {
                    text: qsTr("Inspect")
                    icon.name: "preferences-desktop-theme"
                    onTriggered: card.inspectClicked(card.profileName)
                }
                MenuItem {
                    text: qsTr("Preview")
                    icon.name: "document-preview"
                    visible: !card.profileIsActive
                    onTriggered: card.previewClicked(card.profileName)
                }
                MenuItem {
                    text: qsTr("Duplicate")
                    icon.name: "edit-copy"
                    onTriggered: card.duplicateClicked(card.profileName)
                }
                MenuItem {
                    text: qsTr("Diff with active")
                    icon.name: "document-properties"
                    visible: !card.profileIsActive
                    onTriggered: card.diffClicked(card.profileName)
                }
                MenuItem {
                    text: qsTr("Export…")
                    icon.name: "document-export"
                    onTriggered: card.exportClicked(card.profileName)
                }
                MenuItem {
                    text: qsTr("Edit description")
                    icon.name: "document-edit"
                    onTriggered: card.editDescriptionClicked(
                        card.profileName, card.profileDescription)
                }
                MenuSeparator {}
                MenuItem {
                    text: qsTr("Delete")
                    icon.name: "edit-delete"
                    visible: !card.profileIsActive
                    onTriggered: card.deleteClicked(card.profileName)
                }
            }
        }
    }

    background: Rectangle {
        radius: 8
        border.width: card.activeFocus || card.profileSelected ? 2 : 1
        color: card.profileSelected
            ? ThemeColors.cardHoverBg(card.glassMode)
            : (hover.hovered ? ThemeColors.cardHoverBg(card.glassMode)
                             : ThemeColors.cardBg(card.glassMode))
        border.color: card.activeFocus ? ThemeColors.accent() : card.profileSelected
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
    signal duplicateClicked(string profileName)
    signal exportClicked(string profileName)
    signal editDescriptionClicked(string profileName, string description)
}
