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
    id: page

    title: qsTr("Theme")
    property string profileName: ""
    property string selectedScope: "live"
    property string message: ""
    property bool messageOk: true

    readonly property bool glassMode: themeController.isGlass
    readonly property bool compact: width < Platform.Units.gridUnit * 36
    readonly property var scopeChoices: {
        const values = ["live"]
        for (let i = 0; i < themeBundle.profiles.length; ++i)
            values.push(themeBundle.profiles[i].name)
        return values
    }

    function showMessage(text, ok) {
        page.message = text
        page.messageOk = ok
        messageBox.visible = text !== ""
        if (ok)
            messageTimer.restart()
        else
            messageTimer.stop()
    }

    function scopeLabel() {
        if (page.selectedScope === "live")
            return qsTr("Live system")
        return qsTr("Profile: %1").arg(page.selectedScope)
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

    function switchScope(name) {
        page.selectedScope = name
        if (name === "live")
            themeBundle.openLive()
        else
            themeBundle.openProfile(name)
    }

    Component.onCompleted: {
        if (page.profileName !== "")
            page.switchScope(page.profileName)
        else
            page.switchScope("live")
    }

    Connections {
        target: themeBundle
        function onToast(text, ok) { page.showMessage(text, ok) }
        function onScopeChanged() {
            page.selectedScope = themeBundle.scope === "live"
                ? "live" : themeBundle.scope.replace("profile:", "")
        }
    }

    background: Rectangle {
        color: page.glassMode ? ThemeColors.glassPanel() : ThemeColors.pageBg(page.glassMode)
    }

    Timer {
        id: messageTimer
        interval: 6000
        onTriggered: messageBox.visible = false
    }

    Kirigami.Dialog {
        id: saveProfileDialog
        title: qsTr("Save theme bundle as profile")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        preferredWidth: 460

        ColumnLayout {
            Layout.fillWidth: true
            spacing: Platform.Units.smallSpacing
            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: qsTr("The current component choices, application themes and wallpaper will be saved as a v2 theme bundle.")
                color: ThemeColors.fgColor(page.glassMode)
            }
            TextField {
                id: profileNameField
                Layout.fillWidth: true
                placeholderText: qsTr("Profile name")
            }
            TextField {
                id: profileDescriptionField
                Layout.fillWidth: true
                placeholderText: qsTr("Description (optional)")
            }
            Label {
                id: profileNameError
                Layout.fillWidth: true
                visible: false
                wrapMode: Text.WordWrap
                color: ThemeColors.negative()
                text: qsTr("Use letters, digits, dots, dashes or underscores; the first character must not be a dot.")
            }
        }

        onAccepted: {
            const name = profileNameField.text.trim()
            if (!/^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/.test(name)) {
                profileNameError.visible = true
                saveProfileDialog.open()
                return
            }
            themeBundle.saveProfile(name, profileDescriptionField.text.trim())
        }
    }

    Kirigami.Dialog {
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

    Kirigami.Dialog {
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
        Layout.margins: Platform.Units.largeSpacing
        spacing: Platform.Units.largeSpacing

        RowLayout {
            Layout.fillWidth: true
            spacing: Platform.Units.mediumSpacing

            Kirigami.Heading {
                level: 2
                text: page.selectedScope === "live"
                    ? qsTr("Live theme bundle")
                    : qsTr("Saved theme bundle")
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
                currentIndex: page.selectedScope === "live"
                    ? 0 : page.scopeChoices.indexOf(page.selectedScope)
                displayText: currentIndex === 0 ? qsTr("Live system") : currentText
                enabled: !themeBundle.busy
                Layout.preferredWidth: Platform.Units.gridUnit * 11
                onActivated: function(index) {
                    if (index >= 0 && index < page.scopeChoices.length)
                        page.switchScope(page.scopeChoices[index])
                }
                Accessible.name: qsTr("Theme scope")
            }

            Button {
                text: qsTr("Refresh")
                icon.name: "view-refresh"
                enabled: !themeBundle.busy
                onClicked: themeBundle.refresh()
            }
        }

        Kirigami.InlineMessage {
            id: messageBox
            Layout.fillWidth: true
            visible: false
            text: page.message
            type: page.messageOk
                ? Kirigami.MessageType.Positive : Kirigami.MessageType.Error
            showCloseButton: true
            onVisibleChanged: {
                if (!visible)
                    page.message = ""
            }
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

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Platform.Units.smallSpacing

                    Kirigami.Heading {
                        level: 2
                        text: page.scopeLabel()
                        color: ThemeColors.fgColor(page.glassMode)
                    }

                    Kirigami.Chip {
                        text: page.selectedScope === "live"
                            ? qsTr("Active")
                            : qsTr("Read-only")
                        icon.name: page.selectedScope === "live"
                            ? "dialog-ok" : "document-preview"
                        closable: false
                    }

                    Item { Layout.fillWidth: true }

                    Button {
                        text: qsTr("Apply profile")
                        icon.name: "dialog-ok-apply"
                        highlighted: true
                        visible: page.selectedScope !== "live"
                        enabled: !themeBundle.busy
                        onClicked: themeBundle.applyProfile(page.selectedScope)
                    }
                }

                Label {
                    Layout.fillWidth: true
                    text: page.selectedScope === "live"
                        ? qsTr("Active profile: %1 · %2 change(s)").arg(themeBundle.activeProfile || "—").arg(themeBundle.sessionChanges)
                        : qsTr("This profile is a saved theme bundle. Apply it to make it active.")
                    wrapMode: Text.WordWrap
                    color: ThemeColors.dimColor(page.glassMode)
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Platform.Units.smallSpacing

                    Button {
                        text: qsTr("Save as profile…")
                        icon.name: "document-save-as"
                        enabled: page.selectedScope === "live" && !themeBundle.busy
                        onClicked: {
                            profileNameField.text = ""
                            profileDescriptionField.text = ""
                            profileNameError.visible = false
                            saveProfileDialog.open()
                        }
                    }

                    Button {
                        text: qsTr("Create custom theme…")
                        icon.name: "document-new"
                        enabled: page.selectedScope === "live" && !themeBundle.busy
                        onClicked: {
                            customThemeName.text = ""
                            customThemeDialog.open()
                        }
                    }

                    Item { Layout.fillWidth: true }

                    Button {
                        text: qsTr("Apply changes")
                        icon.name: "dialog-ok-apply"
                        highlighted: true
                        visible: themeBundle.sessionChanges > 0
                        enabled: page.selectedScope === "live"
                            && themeBundle.sessionChanges > 0 && !themeBundle.busy
                        onClicked: themeBundle.applyChanges()
                    }

                    Button {
                        text: qsTr("Undo")
                        icon.name: "edit-undo"
                        visible: themeBundle.sessionChanges > 0 || themeBundle.undoDepth > 0
                        enabled: page.selectedScope === "live" && !themeBundle.busy
                            && (themeBundle.sessionChanges > 0 || themeBundle.undoDepth > 0)
                        onClicked: themeBundle.undo()
                    }

                    Button {
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
                    required property var modelData
                    Layout.fillWidth: true
                    activeFocusOnTab: true
                    Accessible.role: Accessible.Button
                    Accessible.name: modelData.label
                    Accessible.description: page.groupSummary(modelData)

                    background: Rectangle {
                        radius: Platform.Units.smallSpacing
                        border.width: groupTile.activeFocus ? 2 : 1
                        color: groupHover.hovered || groupTile.activeFocus
                            ? ThemeColors.cardHoverBg(page.glassMode)
                            : ThemeColors.cardBg(page.glassMode)
                        border.color: groupTile.activeFocus
                            ? ThemeColors.accent()
                            : ThemeColors.cardBorder(page.glassMode)
                    }

                    HoverHandler { id: groupHover }
                    TapHandler {
                        onTapped: pageStack.layers.push(
                            Qt.resolvedUrl("ThemeComponentsPage.qml"), {
                                groupId: modelData.id,
                                groupTitle: modelData.label,
                                groupDescription: modelData.description,
                                components: modelData.components
                            })
                    }
                    Keys.onPressed: function(event) {
                        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter
                                || event.key === Qt.Key_Space) {
                            pageStack.layers.push(
                                Qt.resolvedUrl("ThemeComponentsPage.qml"), {
                                    groupId: modelData.id,
                                    groupTitle: modelData.label,
                                    groupDescription: modelData.description,
                                    components: modelData.components
                                })
                            event.accepted = true
                        }
                    }

                    contentItem: ColumnLayout {
                        id: groupTile
                        anchors.margins: Platform.Units.mediumSpacing
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

                        Button {
                            Layout.alignment: Qt.AlignRight
                            text: qsTr("Open")
                            icon.name: "go-next"
                            onClicked: pageStack.layers.push(
                                Qt.resolvedUrl("ThemeComponentsPage.qml"), {
                                    groupId: modelData.id,
                                    groupTitle: modelData.label,
                                    groupDescription: modelData.description,
                                    components: modelData.components
                                })
                        }
                    }
                }
            }

            Kirigami.Card {
                id: paletteCard
                Layout.fillWidth: true
                activeFocusOnTab: true
                Accessible.role: Accessible.Button
                Accessible.name: qsTr("Palette")
                Accessible.description: qsTr("One base16 palette for the colour scheme, terminals and Kvantum")

                background: Rectangle {
                    radius: Platform.Units.smallSpacing
                    border.width: paletteTile.activeFocus ? 2 : 1
                    color: paletteHover.hovered || paletteTile.activeFocus
                        ? ThemeColors.cardHoverBg(page.glassMode)
                        : ThemeColors.cardBg(page.glassMode)
                    border.color: paletteTile.activeFocus
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
                    anchors.margins: Platform.Units.mediumSpacing
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
                            text: qsTr("Palette")
                            color: ThemeColors.fgColor(page.glassMode)
                        }
                    }

                    RowLayout {
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
                            ? qsTr("%1 · %2").arg(paletteController.palette.name).arg(paletteController.palette.source)
                            : qsTr("No palette yet — gallery or wallpaper")
                        wrapMode: Text.WordWrap
                        color: ThemeColors.dimColor(page.glassMode)
                    }

                    Button {
                        Layout.alignment: Qt.AlignRight
                        text: qsTr("Open palette")
                        icon.name: "go-next"
                        onClicked: paletteCard.openPalette()
                    }
                }
            }

            Kirigami.Card {
                id: appsCard
                Layout.fillWidth: true
                activeFocusOnTab: true
                Accessible.role: Accessible.Button
                Accessible.name: qsTr("Application themes")
                Accessible.description: qsTr("Choose themes for supported applications")

                background: Rectangle {
                    radius: Platform.Units.smallSpacing
                    border.width: appsTile.activeFocus ? 2 : 1
                    color: appsHover.hovered || appsTile.activeFocus
                        ? ThemeColors.cardHoverBg(page.glassMode)
                        : ThemeColors.cardBg(page.glassMode)
                    border.color: appsTile.activeFocus
                        ? ThemeColors.accent()
                        : ThemeColors.cardBorder(page.glassMode)
                }

                HoverHandler { id: appsHover }
                function openApps() {
                    pageStack.layers.push(Qt.resolvedUrl("ThemeApplicationsPage.qml"))
                }
                TapHandler { onTapped: appsCard.openApps() }
                Keys.onPressed: function(event) {
                    if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter
                            || event.key === Qt.Key_Space) {
                        appsCard.openApps()
                        event.accepted = true
                    }
                }

                contentItem: ColumnLayout {
                    id: appsTile
                    anchors.margins: Platform.Units.mediumSpacing
                    spacing: Platform.Units.smallSpacing

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Platform.Units.smallSpacing
                        Kirigami.Icon {
                            source: "applications-other"
                            color: ThemeColors.accent()
                            Layout.preferredWidth: Platform.Units.iconSizes.small
                            Layout.preferredHeight: Platform.Units.iconSizes.small
                        }
                        Kirigami.Heading {
                            Layout.fillWidth: true
                            level: 3
                            text: qsTr("Applications")
                            color: ThemeColors.fgColor(page.glassMode)
                        }
                    }

                    Label {
                        Layout.fillWidth: true
                        text: {
                            const installed = themeBundle.applications.filter(
                                app => app.installed).length
                            return qsTr("%1 supported · %2 installed").arg(themeBundle.applications.length).arg(installed)
                        }
                        wrapMode: Text.WordWrap
                        color: ThemeColors.dimColor(page.glassMode)
                    }

                    Button {
                        Layout.alignment: Qt.AlignRight
                        text: qsTr("Choose app themes")
                        icon.name: "go-next"
                        onClicked: appsCard.openApps()
                    }
                }
            }
        }
    }
}
