import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami
import org.kde.kirigami.platform as Platform
import Components 1.0

/*! Theme bundle overview: installed KDE/Plasma assets, desktop parameters,
 *  Kvantum/GTK choices and application theme adapters.
 */
Kirigami.ScrollablePage {
    padding: Platform.Units.gridUnit
    id: page

    title: qsTr("Theme")
    property string profileName: ""
    readonly property string selectedScope: themeBundle.scope
    readonly property string selectedProfile: selectedScope === "live" ? "" : selectedScope.slice(8)

    readonly property bool glassMode: themeController.isGlass
    readonly property bool compact: width < Platform.Units.gridUnit * 36
    readonly property var scopeChoices: {
        const values = [{ scope: "live", label: qsTr("Live system") }]
        for (let i = 0; i < themeBundle.profiles.length; ++i)
            values.push({ scope: "profile:" + themeBundle.profiles[i].name, label: themeBundle.profiles[i].name })
        return values
    }



    function openGroup(group) {
        pageStack.layers.push(Qt.resolvedUrl("ThemeComponentsPage.qml"), {
            groupId: group.id, groupTitle: group.label, groupDescription: group.description
        })
    }

    function scopeLabel() {
        if (page.selectedScope === "live")
            return qsTr("Live system")
        return qsTr("Profile: %1").arg(page.selectedProfile)
    }

    function groupSummary(group) {
        const values = []
        for (let i = 0; i < group.components.length && values.length < 3; ++i) {
            const text = group.components[i].currentDisplay
            if (text && text !== "—")
                values.push(text)
        }
        return values.join(" · ")
    }

    function switchScope(scope) {
        if (scope === "live")
            themeBundle.openLive()
        else
            themeBundle.openProfile(scope.slice(8))
    }

    Component.onCompleted: {
        if (page.profileName !== "")
            page.switchScope("profile:" + page.profileName)
        else
            page.switchScope("live")
    }

    background: Rectangle {
        color: page.glassMode ? ThemeColors.glassPanel() : ThemeColors.pageBg(page.glassMode)
    }



    AppDialog {
        id: draftDialog
        title: qsTr("Draft changes")
        standardButtons: Kirigami.Dialog.Close
        preferredWidth: 520
        preferredHeight: 420

        InsetScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            ColumnLayout {
                width: draftDialog.availableWidth
                Repeater {
                    model: page.selectedScope === "live" ? themeInspector.draft : themeBundle.profileDraft
                    delegate: Label {
                        required property var modelData
                        Layout.fillWidth: true
                        text: {
                            const heading = modelData.source
                                ? modelData.label + " · " + modelData.source : modelData.label
                            const before = modelData.before ?? qsTr("Not set")
                            const after = modelData.after ?? qsTr("Not set")
                            return heading + "\n" + String(before) + " → " + String(after)
                        }
                        textFormat: Text.PlainText
                        wrapMode: Text.WrapAnywhere
                    }
                }
            }
        }
    }

    AppDialog {
        id: discardDraftDialog
        title: qsTr("Discard the Live draft?")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: themeInspector.discardDraft()
        Label {
            text: qsTr("Discard all unapplied changes in this draft.")
            wrapMode: Text.WordWrap
        }
    }

    AppDialog {
        id: customThemeDialog
        title: qsTr("Create custom derived theme")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        preferredWidth: 460

        ColumnLayout {
            Layout.fillWidth: true
            spacing: Platform.Units.smallSpacing
            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: qsTr("Creates user-owned color and Kvantum themes from the current base themes without changing system themes.")
                color: ThemeColors.fgColor(page.glassMode)
            }
            TextField {
                id: customThemeName
                Layout.fillWidth: true
                placeholderText: qsTr("Custom theme name")
            }
        }

        onAccepted: {
            const name = customThemeName.text.trim()
            if (name !== "")
                themeBundle.createCustomTheme(name)
        }
    }

    AppDialog {
        id: revertDialog
        title: qsTr("Revert theme changes?")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        preferredWidth: 420
        onAccepted: themeBundle.revert()

        Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: qsTr("Discard staged edits and restore applied changes from this session?")
            color: ThemeColors.fgColor(page.glassMode)
        }
    }

    ColumnLayout {
        spacing: Platform.Units.largeSpacing

        ActionFlow {
            Layout.fillWidth: true
            spacing: Platform.Units.mediumSpacing

            Kirigami.Heading {
                level: 2
                text: page.selectedScope === "live"
                    ? qsTr("Live theme bundle")
                    : qsTr("Saved theme bundle")
                width: Math.min(implicitWidth, parent.width)
                wrapMode: Text.Wrap
                color: ThemeColors.fgColor(page.glassMode)
                Layout.fillWidth: true
            }

            BusyIndicator {
                running: themeBundle.busy
                visible: running
                Layout.preferredHeight: Platform.Units.iconSizes.small
            }

            ComboBox {
                id: scopeCombo
                model: page.scopeChoices
                textRole: "label"
                valueRole: "scope"
                currentIndex: page.scopeChoices.findIndex(choice => choice.scope === page.selectedScope)
                displayText: page.selectedScope === "live" ? qsTr("Live system") : page.selectedProfile
                enabled: !themeBundle.busy
                width: Math.min(implicitWidth, parent.width)
                onActivated: function(index) {
                    if (index >= 0 && index < page.scopeChoices.length) {
                        page.switchScope(page.scopeChoices[index].scope)
                        if (currentValue !== page.selectedScope)
                            currentIndex = Qt.binding(() => page.scopeChoices.findIndex(choice => choice.scope === page.selectedScope))
                    }
                }
                Accessible.name: qsTr("Theme scope")
            }

            AppButton {
                text: page.selectedScope === "live" ? qsTr("Refresh Live") : qsTr("Refresh")
                icon.name: "view-refresh"
                enabled: !themeBundle.busy && !themeInspector.busy && !profileController.busy
                onClicked: {
                    if (page.selectedScope === "live")
                        themeInspector.refreshDraft()
                    else
                        themeBundle.refresh()
                }
            }
        }

        AppButton {
            text: qsTr("System Settings")
            icon.name: "configure"
            enabled: !themeBundle.busy
            onClicked: themeBundle.openSystemSettings()
            ToolTip.visible: hovered
            ToolTip.text: qsTr("Open KDE System Settings (all system modules)")
        }


        Kirigami.Card {
            Layout.fillWidth: true
            background: Rectangle {
                radius: Platform.Units.smallSpacing
                border.width: 1
                color: ThemeColors.cardBg(page.glassMode)
                border.color: ThemeColors.cardBorder(page.glassMode)
            }

            contentItem: ColumnLayout {
                spacing: Platform.Units.mediumSpacing

                ActionFlow {
                    Layout.fillWidth: true
                    spacing: Platform.Units.smallSpacing

                    Kirigami.Heading {
                        level: 2
                        text: page.scopeLabel()
                        width: Math.min(implicitWidth, parent.width)
                        wrapMode: Text.Wrap
                        color: ThemeColors.fgColor(page.glassMode)
                    }

                    Kirigami.Chip {
                        text: page.selectedScope === "live"
                            ? qsTr("Active")
                            : themeBundle.canEditProfile ? qsTr("Saved theme") : qsTr("Read-only")
                        icon.name: page.selectedScope === "live"
                            ? "dialog-ok" : "document-preview"
                        closable: false
                    }


                    AppButton {
                        text: qsTr("Apply profile")
                        icon.name: "dialog-ok-apply"
                        highlighted: true
                        visible: page.selectedScope !== "live"
                        enabled: !themeBundle.busy && themeBundle.profileDraftSize === 0
                        onClicked: profileController.previewApply(page.selectedProfile, false)
                    }
                }

                Label {
                    Layout.fillWidth: true
                    text: page.selectedScope === "live"
                        ? qsTr("Active profile: %1 · %n change(s)", "", themeBundle.sessionChanges).arg(themeBundle.activeProfile || "—")
                        : themeBundle.canEditProfile
                            ? qsTr("Save changes to this theme, then apply it when you are ready.")
                            : themeBundle.profileBundle.legacy
                                ? qsTr("Convert this snapshot to edit it. Kolira keeps a copy of the original.")
                                : qsTr("This profile is a saved theme bundle. Apply it to make it active.")
                    wrapMode: Text.WordWrap
                    color: ThemeColors.dimColor(page.glassMode)
                }

                ActionFlow {
                    Layout.fillWidth: true
                    spacing: Platform.Units.smallSpacing

                    AppButton {
                        objectName: "convertLegacyTheme"
                        text: qsTr("Convert theme")
                        icon.name: "document-save"
                        visible: page.selectedScope !== "live" && !!themeBundle.profileBundle.legacy
                        enabled: !themeBundle.busy && !profileController.busy && !profileController.previewBusy
                        onClicked: themeBundle.migrateProfile()
                    }

                    AppButton {
                        text: qsTr("Save theme changes")
                        icon.name: "document-save"
                        visible: themeBundle.profileDraftSize > 0
                        enabled: !themeBundle.busy
                        onClicked: themeBundle.saveProfileDraft()
                    }

                    AppButton {
                        text: qsTr("Review draft")
                        icon.name: "document-preview"
                        visible: themeBundle.profileDraftSize > 0
                        onClicked: draftDialog.open()
                    }

                    AppButton {
                        text: qsTr("Discard theme changes")
                        icon.name: "edit-clear"
                        visible: themeBundle.profileDraftSize > 0
                        enabled: !themeBundle.busy
                        onClicked: themeBundle.discardProfileDraft()
                    }

                    AppButton {
                        text: qsTr("Save as profile…")
                        icon.name: "document-save-as"
                        enabled: page.selectedScope === "live" && !themeBundle.busy
                            && !profileController.busy
                        onClicked: applicationWindow().openSaveThemeDialog()
                    }

                    AppButton {
                        text: qsTr("Create custom theme…")
                        icon.name: "document-new"
                        enabled: page.selectedScope === "live" && !themeBundle.busy
                        onClicked: {
                            customThemeName.text = ""
                            customThemeDialog.open()
                        }
                    }


                    AppButton {
                        text: qsTr("Apply changes")
                        icon.name: "dialog-ok-apply"
                        highlighted: true
                        visible: themeBundle.sessionChanges > 0
                        enabled: page.selectedScope === "live"
                            && themeBundle.sessionChanges > 0 && !themeBundle.busy && !themeInspector.busy
                        onClicked: themeBundle.applyChanges()
                    }

                    AppButton {
                        text: qsTr("Review draft")
                        icon.name: "document-preview"
                        visible: page.selectedScope === "live" && themeInspector.sessionChanges > 0
                        onClicked: draftDialog.open()
                    }
                    AppButton {
                        text: qsTr("Discard draft…")
                        icon.name: "edit-clear"
                        visible: page.selectedScope === "live" && themeInspector.sessionChanges > 0
                        enabled: !themeBundle.busy && !themeInspector.busy
                        onClicked: discardDraftDialog.open()
                    }

                    AppButton {
                        text: qsTr("Undo")
                        icon.name: "edit-undo"
                        visible: themeBundle.sessionChanges > 0 || themeBundle.undoDepth > 0
                        enabled: page.selectedScope === "live" && !themeBundle.busy
                            && (themeBundle.sessionChanges > 0 || themeBundle.undoDepth > 0)
                        onClicked: themeBundle.undo()
                    }

                    AppButton {
                        text: qsTr("Revert")
                        icon.name: "document-revert"
                        visible: themeBundle.sessionChanges > 0 || themeBundle.undoDepth > 0
                        enabled: page.selectedScope === "live" && !themeBundle.busy
                            && (themeBundle.sessionChanges > 0 || themeBundle.undoDepth > 0)
                        onClicked: revertDialog.open()
                    }
                }
            }
        }

        GridLayout {
            Layout.fillWidth: true
            columns: page.compact ? 1 : 2
            columnSpacing: Platform.Units.smallSpacing
            rowSpacing: Platform.Units.smallSpacing

            Repeater {
                model: themeBundle.groups

                Kirigami.Card {
                    id: groupCard
                    objectName: "themeGroup_" + modelData.id
                    function openGroup() { page.openGroup(modelData) }
                    required property var modelData
                    Layout.fillWidth: true
                    activeFocusOnTab: true
                    Accessible.role: Accessible.Button
                    Accessible.name: modelData.label
                    Accessible.description: page.groupSummary(modelData)
                    Accessible.onPressAction: groupCard.openGroup()

                    background: Rectangle {
                        radius: Platform.Units.smallSpacing
                        border.width: groupCard.activeFocus ? 2 : 1
                        color: groupHover.hovered || groupCard.activeFocus
                            ? ThemeColors.cardHoverBg(page.glassMode)
                            : ThemeColors.cardBg(page.glassMode)
                        border.color: groupCard.activeFocus
                            ? ThemeColors.accent()
                            : ThemeColors.cardBorder(page.glassMode)
                    }

                    HoverHandler { id: groupHover }
                    TapHandler {
                        onTapped: groupCard.openGroup()
                    }
                    Keys.onPressed: function(event) {
                        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter
                                || event.key === Qt.Key_Space) {
                            groupCard.openGroup()
                            event.accepted = true
                        }
                    }

                    contentItem: ColumnLayout {
                        id: groupTile
                        spacing: Platform.Units.smallSpacing

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Platform.Units.smallSpacing
                            Kirigami.Icon {
                                source: modelData.icon
                                color: ThemeColors.accent()
                                Layout.preferredWidth: Platform.Units.iconSizes.small
                                Layout.preferredHeight: Platform.Units.iconSizes.small
                            }
                            Kirigami.Heading {
                                Layout.fillWidth: true
                                level: 3
                                text: modelData.label
                                color: ThemeColors.fgColor(page.glassMode)
                            }
                        }

                        Label {
                            Layout.fillWidth: true
                            text: page.groupSummary(modelData)
                            color: ThemeColors.dimColor(page.glassMode)
                            // Text.Wrap (not WordWrap): long wallpaper file names
                            // have no spaces and would otherwise overflow the tile.
                            wrapMode: Text.Wrap
                        }

                        AppButton {
                            objectName: "openGroup_" + modelData.id
                            Layout.alignment: Qt.AlignRight
                            text: qsTr("Open")
                            icon.name: "go-next"
                            onClicked: groupCard.openGroup()
                        }
                    }
                }
            }

            Kirigami.Card {
                id: paletteCard
                visible: !themeBundle.readOnly
                Layout.fillWidth: true
                activeFocusOnTab: true
                Accessible.role: Accessible.Button
                Accessible.name: qsTr("Base16 palette")
                Accessible.description: qsTr("One base16 palette for the colour scheme, terminals and Kvantum")
                Accessible.onPressAction: paletteCard.openPalette()

                background: Rectangle {
                    radius: Platform.Units.smallSpacing
                    border.width: paletteCard.activeFocus ? 2 : 1
                    color: paletteHover.hovered || paletteCard.activeFocus
                        ? ThemeColors.cardHoverBg(page.glassMode)
                        : ThemeColors.cardBg(page.glassMode)
                    border.color: paletteCard.activeFocus
                        ? ThemeColors.accent()
                        : ThemeColors.cardBorder(page.glassMode)
                }

                HoverHandler { id: paletteHover }
                function openPalette() {
                    pageStack.layers.push(Qt.resolvedUrl("PalettePage.qml"))
                }
                TapHandler { onTapped: paletteCard.openPalette() }
                Keys.onPressed: function(event) {
                    if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter
                            || event.key === Qt.Key_Space) {
                        paletteCard.openPalette()
                        event.accepted = true
                    }
                }

                contentItem: ColumnLayout {
                    id: paletteTile
                    spacing: Platform.Units.smallSpacing

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Platform.Units.smallSpacing
                        Kirigami.Icon {
                            source: "color-management"
                            color: ThemeColors.accent()
                            Layout.preferredWidth: Platform.Units.iconSizes.small
                            Layout.preferredHeight: Platform.Units.iconSizes.small
                        }
                        Kirigami.Heading {
                            Layout.fillWidth: true
                            level: 3
                            text: qsTr("Base16 palette")
                            color: ThemeColors.fgColor(page.glassMode)
                        }
                    }

                    Flow {
                        Layout.fillWidth: true
                        spacing: 2
                        visible: paletteController.hasPalette
                        Repeater {
                            model: (paletteController.palette.base || []).concat([paletteController.palette.accent || "#000000"])
                            delegate: Rectangle {
                                required property string modelData
                                implicitWidth: Platform.Units.gridUnit
                                implicitHeight: Platform.Units.gridUnit
                                radius: 3
                                color: modelData
                                border.width: 1
                                border.color: ThemeColors.cardBorder(page.glassMode)
                            }
                        }
                    }

                    Label {
                        Layout.fillWidth: true
                        text: paletteController.hasPalette
                            ? qsTr("%1 · %2").arg(paletteController.palette.name).arg(paletteController.palette.sourceText)
                            : qsTr("No palette yet — gallery or wallpaper")
                        wrapMode: Text.WordWrap
                        color: ThemeColors.dimColor(page.glassMode)
                    }

                    AppButton {
                        Layout.alignment: Qt.AlignRight
                        text: qsTr("Open palette")
                        icon.name: "go-next"
                        onClicked: paletteCard.openPalette()
                    }
                }
            }
        }
    }
}
