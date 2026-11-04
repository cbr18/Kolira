import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami
import org.kde.kirigami.platform as Platform
import Components 1.0

Kirigami.ApplicationWindow {
    id: root

    title: qsTr("Theme Profile Manager")
    width: 820
    height: 680
    minimumWidth: 640
    minimumHeight: 480

    // Dogfooding: follow the active profile (glass → translucent).
    readonly property bool glassMode: themeController.isGlass

    // True while the controller is applying/saving/deleting a profile.
    readonly property bool applying: profileController.busy

    // Close → hide to the tray when the user asked for it (Settings →
    // "Minimize to system tray"). Only when the tray icon really exists,
    // otherwise the window would become unreachable; the tray menu's Quit
    // action is the way out in that mode.
    onClosing: function(close) {
        if (typeof trayController !== "undefined" && trayController
                && trayController.minimizeToTray && trayController.trayIconVisible) {
            close.accepted = false
            root.hide()
        }
    }

    // The window surface itself stays transparent in glass mode: the page
    // paints the translucent glass panel on top (see mainPage.background).
    color: ThemeColors.windowBg(root.glassMode)

    // Profile the user picked in the UI. Applying is explicit (Apply button)
    // because switching profiles is expensive — the user may want to look
    // around before committing.
    property string selectedProfile: ""

    // Deep links used by the dev screenshot harness (and available to the tray).
    function openThemePage() {
        pageStack.layers.push(Qt.resolvedUrl("pages/ThemePage.qml"))
    }

    function openThemeComponentsPage(groupId) {
        const groups = themeBundle.groups
        let group = groups.length > 0 ? groups[0] : null
        for (let i = 0; i < groups.length; ++i) {
            if (groups[i].id === groupId) {
                group = groups[i]
                break
            }
        }
        if (group === null)
            return
        pageStack.layers.push(Qt.resolvedUrl("pages/ThemeComponentsPage.qml"), {
            groupId: group.id,
            groupTitle: group.label,
            groupDescription: group.description,
            components: group.components
        })
    }

    function openThemeComponentsPageDefault() {
        openThemeComponentsPage("")
    }

    function openThemeApplicationsPage() {
        pageStack.layers.push(Qt.resolvedUrl("pages/ThemeApplicationsPage.qml"))
    }

    function openPalettePage() {
        pageStack.layers.push(Qt.resolvedUrl("pages/PalettePage.qml"))
    }

    // Applying a profile can restart Plasma components, so honour the
    // "Confirm before applying" preference instead of hard-wiring a click to
    // the controller.  The preview dialog already acts as a confirmation.
    function requestApply(profileName, alreadyConfirmed) {
        if (profileName === "")
            return
        if (!alreadyConfirmed
                && typeof settingsController !== "undefined"
                && settingsController.confirmBeforeApply) {
            applyDialog.targetProfile = profileName
            applyDialog.open()
            return
        }
        profileController.applyProfile(profileName)
    }

    // Last active profile we observed. Used to distinguish "the user changed
    // the selection" from "the controller just refreshed the active profile".
    property string _lastActive: ""

    // Transient feedback for the last controller operation.
    property string operationMessage: ""
    property bool _operationSucceeded: true

    Timer {
        id: messageTimer
        interval: 6000
        onTriggered: root.operationMessage = ""
    }

    // Track real active-profile changes without clobbering a user's in-progress
    // selection. The initial value is seeded in Component.onCompleted because
    // activeProfileChanged already fired during MainController.initialize().
    Connections {
        target: profileController
        function onActiveProfileChanged() {
            var active = profileController.activeProfile
            if (root.selectedProfile === "" || root.selectedProfile === root._lastActive) {
                root.selectedProfile = active
            }
            root._lastActive = active
        }
    }

    Connections {
        target: profileController
        function onOperationStarted(op) {
            if (op === "apply")
                root.operationMessage = qsTr("Applying profile…")
            else if (op === "save")
                root.operationMessage = qsTr("Saving profile…")
            else if (op === "delete")
                root.operationMessage = qsTr("Deleting profile…")
            else
                root.operationMessage = qsTr("Working…")
        }
        function onOperationFinished(success, message) {
            root._operationSucceeded = success
            root.operationMessage = message
            messageTimer.restart()
        }
    }

    Component.onCompleted: {
        // The Python side (main_controller.initialize()) already loaded every
        // controller before engine.load(), so we only need to hand the window
        // to the theme controller and seed the selection.
        themeController.set_window(root)
        root._lastActive = profileController.activeProfile
        root.selectedProfile = profileController.activeProfile
    }

    globalDrawer: Kirigami.GlobalDrawer {
        title: qsTr("Theme Profile Manager")
        titleIcon: "preferences-desktop-theme"
        // modal (the default) keeps the drawer behind the hamburger button.
        // A non-modal GlobalDrawer is a *permanent* sidebar on desktop, which
        // eats ~220 px of width and overlaps the page header.
        modal: true
        actions: [
            Kirigami.Action {
                text: qsTr("Profiles")
                icon.name: "preferences-desktop-theme"
                onTriggered: pageStack.layers.clear()
            },
            Kirigami.Action {
                text: qsTr("Theme")
                icon.name: "preferences-desktop-color"
                onTriggered: {
                    themeBundle.openLive()
                    pageStack.layers.push(Qt.resolvedUrl("pages/ThemePage.qml"))
                }
            },
            Kirigami.Action {
                text: qsTr("Applications")
                icon.name: "preferences-desktop-apps"
                onTriggered: pageStack.layers.push(Qt.resolvedUrl("pages/ApplicationsPage.qml"))
            },
            Kirigami.Action {
                text: qsTr("Settings")
                icon.name: "settings-configure"
                onTriggered: pageStack.layers.push(Qt.resolvedUrl("pages/SettingsPage.qml"))
            }
        ]
    }

    // ──────────────────────────────────────────────────────────────
    // Main page
    // NOTE: ScrollablePage IS the page — its single child must be a
    // layout (children are only anchored left/right, not laid out).
    // ──────────────────────────────────────────────────────────────
    pageStack.initialPage: Kirigami.ScrollablePage {
        id: mainPage
        title: qsTr("Theme Profiles")

        // Kirigami.Page paints Platform.Theme.backgroundColor behind its
        // content. In glass mode we paint a translucent dark "glass panel"
        // ourselves so the window tint is guaranteed regardless of whether
        // the compositor applies blur/alpha to the surface.
        background: Rectangle {
            color: root.glassMode
                ? ThemeColors.glassPanel()
                : ThemeColors.pageBg(root.glassMode)
        }

        ColumnLayout {
            spacing: Platform.Units.largeSpacing
            Layout.margins: Platform.Units.largeSpacing

            // ─── Header: thin row — active profile + primary actions ───
            RowLayout {
                Layout.fillWidth: true

                Kirigami.Heading {
                    level: 2
                    text: profileController.activeProfile !== ""
                        ? qsTr("Active: %1").arg(profileController.activeProfile)
                        : qsTr("Theme Profiles")
                    color: ThemeColors.fgColor(root.glassMode)
                }
                Button {
                    text: qsTr("Edit live theme…")
                    icon.name: "document-edit"
                    onClicked: {
                        themeBundle.openLive()
                        pageStack.layers.push(Qt.resolvedUrl("pages/ThemePage.qml"))
                    }
                }
                Item { Layout.fillWidth: true }
                BusyIndicator {
                    running: root.applying
                    visible: root.applying
                    Layout.preferredHeight: Platform.Units.iconSizes.small
                }
                Button {
                    text: qsTr("Refresh")
                    icon.name: "view-refresh"
                    enabled: !root.applying
                    onClicked: profileController.refresh()
                }
                Button {
                    text: qsTr("Apply")
                    icon.name: "dialog-ok-apply"
                    highlighted: true
                    enabled: !root.applying && root.selectedProfile !== ""
                        && root.selectedProfile !== profileController.activeProfile
                    onClicked: root.requestApply(root.selectedProfile)
                }
            }

            Kirigami.InlineMessage {
                Layout.fillWidth: true
                visible: root.operationMessage !== ""
                text: root.operationMessage
                type: root._operationSucceeded
                    ? Kirigami.MessageType.Positive
                    : Kirigami.MessageType.Error
            }

            // Selection hint. Hidden while the selection is the active
            // profile: the header already says "Active: <name>".
            Label {
                Layout.fillWidth: true
                visible: text !== ""
                wrapMode: Text.WordWrap
                color: ThemeColors.dimColor(root.glassMode)
                font.pointSize: TypeScale.caption
                text: {
                    if (root.selectedProfile === "")
                        return qsTr("Click a profile card to select it, then press Apply.")
                    if (root.selectedProfile === profileController.activeProfile)
                        return ""
                    return qsTr("Selected \"%1\" (not applied yet). Active: %2")
                        .arg(root.selectedProfile)
                        .arg(profileController.activeProfile)
                }
            }

            // ─── Available Profiles ───
            Kirigami.Card {
                Layout.fillWidth: true

                background: Rectangle {
                    color: ThemeColors.cardBg(root.glassMode)
                    radius: Platform.Units.smallSpacing
                    border.width: 1
                    border.color: ThemeColors.cardBorder(root.glassMode)
                }

                contentItem: ColumnLayout {
                    spacing: Platform.Units.mediumSpacing

                    RowLayout {
                        Layout.fillWidth: true

                        Kirigami.Heading {
                            level: 2
                            text: qsTr("Available Profiles")
                            color: ThemeColors.fgColor(root.glassMode)
                        }
                        Item { Layout.fillWidth: true }
                        Button {
                            text: qsTr("Save current…")
                            icon.name: "list-add"
                            enabled: !root.applying
                            onClicked: saveDialog.open()
                        }
                    }

                    Repeater {
                        model: profileController.profiles.length

                        delegate: ProfileCardDelegate {
                            readonly property var entry: profileController.profiles[index]
                            glassMode: root.glassMode
                            profileName: entry ? entry.name : ""
                            profileIsActive: entry ? entry.isActive : false
                            profileSelected: entry ? root.selectedProfile === entry.name : false
                            profileComponentCount: entry ? entry.componentCount : 0
                            profileCreatedAt: entry ? entry.createdAt : ""
                            profileDescription: entry ? entry.description : ""

                            onSelectedClicked: function(profileName) {
                                root.selectedProfile = profileName
                            }
                            onApplyClicked: function(profileName) {
                                root.requestApply(profileName)
                            }
                            onPreviewClicked: function(profileName) {
                                previewDialog.previewProfile = profileName
                                previewDialog.open()
                            }
                            onDiffClicked: function(profileName) {
                                diffDialog.profileA = profileName
                                diffDialog.profileB = profileController.activeProfile
                                diffDialog.open()
                            }
                            onInspectClicked: function(profileName) {
                                pageStack.layers.push(
                                    Qt.resolvedUrl("pages/ThemePage.qml"),
                                    { profileName: profileName })
                            }
                            onDeleteClicked: function(profileName) {
                                confirmDialog.targetProfile = profileName
                                confirmDialog.open()
                            }
                            onDuplicateClicked: function(profileName) {
                                duplicateDialog.sourceProfile = profileName
                                duplicateNameField.text = profileName + "-copy"
                                duplicateError.visible = false
                                duplicateDialog.open()
                            }
                            onEditDescriptionClicked: function(profileName, description) {
                                descriptionDialog.targetProfile = profileName
                                descriptionEdit.text = description
                                descriptionDialog.open()
                            }
                        }
                    }

                    Label {
                        Layout.fillWidth: true
                        visible: profileController.profiles.length === 0
                        text: qsTr("No profiles found. Save the current theme to create one.")
                        color: ThemeColors.dimColor(root.glassMode)
                        wrapMode: Text.WordWrap
                    }
                }
            }

            // ─── Wallpaper ───
            Kirigami.Card {
                Layout.fillWidth: true

                background: Rectangle {
                    color: ThemeColors.cardBg(root.glassMode)
                    radius: Platform.Units.smallSpacing
                    border.width: 1
                    border.color: ThemeColors.cardBorder(root.glassMode)
                }

                contentItem: ColumnLayout {
                    spacing: Platform.Units.mediumSpacing

                    RowLayout {
                        Layout.fillWidth: true

                        Kirigami.Heading {
                            level: 2
                            text: qsTr("Wallpaper")
                            color: ThemeColors.fgColor(root.glassMode)
                        }
                        Item { Layout.fillWidth: true }
                        Button {
                            text: qsTr("Refresh")
                            icon.name: "view-refresh"
                            onClicked: wallpaperController.refresh()
                        }
                    }

                    Item {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 160
                        visible: wallpaperController.currentWallpaper !== ""

                        Image {
                            id: wallpaperPreview
                            anchors.fill: parent
                            source: wallpaperController.currentWallpaper
                            fillMode: Image.PreserveAspectCrop
                            // Big JPEGs decode off-thread; the indicator below
                            // covers the gap instead of blocking the page.
                            asynchronous: true
                        }

                        BusyIndicator {
                            anchors.centerIn: parent
                            running: wallpaperPreview.status !== Image.Ready
                            visible: running
                        }

                        Rectangle {
                            anchors.fill: parent
                            color: "transparent"
                            radius: 8
                            border.width: 1
                            border.color: ThemeColors.cardBorder(root.glassMode)
                        }
                    }

                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        visible: wallpaperController.currentWallpaper === ""
                        text: qsTr("No wallpaper information available.")
                        color: ThemeColors.dimColor(root.glassMode)
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Platform.Units.mediumSpacing

                        Button {
                            Layout.fillWidth: true
                            text: qsTr("Previous")
                            icon.name: "go-previous-view"
                            enabled: !wallpaperController.busy
                            onClicked: wallpaperController.prevWallpaper()
                        }
                        Button {
                            Layout.fillWidth: true
                            text: qsTr("Pick Wallpaper…")
                            icon.name: "image-x-generic"
                            highlighted: true
                            enabled: !wallpaperController.busy
                            onClicked: wallpaperController.pickWallpaper()
                        }
                        Button {
                            Layout.fillWidth: true
                            text: qsTr("Next")
                            icon.name: "go-next-view"
                            enabled: !wallpaperController.busy
                            onClicked: wallpaperController.nextWallpaper()
                        }
                    }
                }
            }
        }
    }

    // ──────────────────────────────────────────────────────────────
    // Dialogs
    // ──────────────────────────────────────────────────────────────
    Kirigami.Dialog {
        id: applyDialog
        title: qsTr("Apply Profile")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        preferredWidth: 420
        property string targetProfile: ""

        background: Rectangle {
            color: ThemeColors.dialogBg(root.glassMode)
            radius: 8
            border.width: 1
            border.color: ThemeColors.cardBorder(root.glassMode)
        }

        ColumnLayout {
            Layout.fillWidth: true
            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: ThemeColors.fgColor(root.glassMode)
                text: qsTr("Apply profile \"%1\"? This can restart Plasma components and change the current desktop appearance.").arg(applyDialog.targetProfile)
            }
        }

        onAccepted: {
            if (applyDialog.targetProfile !== "")
                profileController.applyProfile(applyDialog.targetProfile)
        }
    }

    Kirigami.Dialog {
        id: confirmDialog
        title: qsTr("Delete Profile")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        preferredWidth: 380
        property string targetProfile: ""

        background: Rectangle {
            color: ThemeColors.dialogBg(root.glassMode)
            radius: 8
            border.width: 1
            border.color: ThemeColors.cardBorder(root.glassMode)
        }

        ColumnLayout {
            Layout.fillWidth: true
            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: ThemeColors.fgColor(root.glassMode)
                text: qsTr("Delete profile \"%1\"? This cannot be undone.").arg(confirmDialog.targetProfile)
            }
        }

        onAccepted: {
            if (confirmDialog.targetProfile !== "") {
                profileController.deleteProfile(confirmDialog.targetProfile)
            }
        }
    }

    Kirigami.Dialog {
        id: saveDialog
        title: qsTr("Save Profile")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        preferredWidth: 420

        background: Rectangle {
            color: ThemeColors.dialogBg(root.glassMode)
            radius: 8
            border.width: 1
            border.color: ThemeColors.cardBorder(root.glassMode)
        }

        ColumnLayout {
            Layout.fillWidth: true
            Label {
                text: qsTr("Save the current theme as a new profile:")
                color: ThemeColors.fgColor(root.glassMode)
            }
            TextField {
                id: nameField
                Layout.fillWidth: true
                placeholderText: qsTr("Profile name")
                onTextChanged: saveError.visible = false
            }
            TextField {
                id: descriptionField
                Layout.fillWidth: true
                placeholderText: qsTr("Description (optional)")
            }
            Label {
                id: saveError
                Layout.fillWidth: true
                visible: false
                wrapMode: Text.WordWrap
                color: ThemeColors.negative()
                text: qsTr("Use letters, digits, dots, dashes or underscores; the first character must not be a dot.")
            }
        }

        onAccepted: {
            var name = nameField.text.trim()
            if (!/^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/.test(name)) {
                saveError.visible = true
                saveDialog.open()
                return
            }
            profileController.saveProfile(name, descriptionField.text.trim())
            nameField.text = ""
            descriptionField.text = ""
        }
    }

    Kirigami.Dialog {
        id: duplicateDialog
        title: qsTr("Duplicate Profile")
        property string sourceProfile: ""
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        preferredWidth: 420

        background: Rectangle {
            color: ThemeColors.dialogBg(root.glassMode)
            radius: 8
            border.width: 1
            border.color: ThemeColors.cardBorder(root.glassMode)
        }

        ColumnLayout {
            Layout.fillWidth: true
            Label {
                text: qsTr("Create a copy of \"%1\" under a new name:").arg(duplicateDialog.sourceProfile)
                color: ThemeColors.fgColor(root.glassMode)
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
            TextField {
                id: duplicateNameField
                Layout.fillWidth: true
                placeholderText: qsTr("Profile name")
                onTextChanged: duplicateError.visible = false
            }
            Label {
                id: duplicateError
                Layout.fillWidth: true
                visible: false
                wrapMode: Text.WordWrap
                color: ThemeColors.negative()
                text: qsTr("Use letters, digits, dots, dashes or underscores; the first character must not be a dot.")
            }
        }

        onAccepted: {
            var name = duplicateNameField.text.trim()
            if (!/^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/.test(name)) {
                duplicateError.visible = true
                duplicateDialog.open()
                return
            }
            profileController.duplicateProfile(duplicateDialog.sourceProfile, name)
        }
    }

    Kirigami.Dialog {
        id: descriptionDialog
        title: qsTr("Profile Description")
        property string targetProfile: ""
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        preferredWidth: 420

        background: Rectangle {
            color: ThemeColors.dialogBg(root.glassMode)
            radius: 8
            border.width: 1
            border.color: ThemeColors.cardBorder(root.glassMode)
        }

        ColumnLayout {
            Layout.fillWidth: true
            Label {
                text: qsTr("A note shown on the profile card (stored in the profile's manifest.json):")
                color: ThemeColors.fgColor(root.glassMode)
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
            TextArea {
                id: descriptionEdit
                Layout.fillWidth: true
                Layout.minimumHeight: Platform.Units.gridUnit * 3
                placeholderText: qsTr("Optional description")
            }
        }

        onAccepted: {
            if (descriptionDialog.targetProfile !== "") {
                profileController.setProfileDescription(
                    descriptionDialog.targetProfile, descriptionEdit.text.trim())
            }
        }
    }

    Kirigami.Dialog {
        id: diffDialog
        title: qsTr("Profile Diff")
        standardButtons: Kirigami.Dialog.Close
        preferredWidth: 640
        preferredHeight: 480
        property string profileA: ""
        property string profileB: ""

        background: Rectangle {
            color: ThemeColors.dialogBg(root.glassMode)
            radius: 8
            border.width: 1
            border.color: ThemeColors.cardBorder(root.glassMode)
        }

        function updateDiff() {
            if (profileA !== "" && profileB !== "" && profileA !== profileB) {
                diffText.text = qsTr("Comparing…")
                profileController.requestDiff(profileA, profileB)
            } else if (profileA === profileB) {
                diffText.text = qsTr("No differences: both profiles are \"%1\".").arg(profileA)
            } else {
                diffText.text = qsTr("No active profile to compare with.")
            }
        }

        // The diff can be large (recursive comparison of config trees), so it
        // is computed off the GUI thread; requestDiff emits diffReady with the
        // requested pair so a stale result cannot overwrite a newer request.
        Connections {
            target: profileController
            function onDiffReady(a, b, text) {
                if (a === diffDialog.profileA && b === diffDialog.profileB) {
                    diffText.text = text
                }
            }
        }

        // Same lazy-compute pattern as previewDialog: the dialog content only
        // exists once the popup is opened, so build the diff there.
        onOpened: updateDiff()

        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true

            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: ThemeColors.fgColor(root.glassMode)
                text: qsTr("Difference between \"%1\" and \"%2\":").arg(diffDialog.profileA).arg(diffDialog.profileB)
                font.bold: true
            }

            ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.preferredHeight: 360
                // Kirigami.Dialog sizes itself as
                // max(preferredWidth, content implicit width), so a wide diff
                // would blow the dialog up to the width of its longest line.
                // Reporting 0 keeps the dialog at preferredWidth while the
                // (unwrapped) TextEdit still gets a real horizontal scrollbar.
                implicitWidth: 0
                clip: true

                background: Rectangle {
                    color: ThemeColors.codeBg(root.glassMode)
                    radius: 6
                    border.width: 1
                    border.color: ThemeColors.subtleBorder(root.glassMode)
                }

                TextEdit {
                    id: diffText
                    readOnly: true
                    font.family: "monospace"
                    wrapMode: TextEdit.NoWrap
                    color: ThemeColors.fgColor(root.glassMode)
                    selectionColor: ThemeColors.accent()
                    selectedTextColor: ThemeColors.fgColor(root.glassMode)
                    leftPadding: 10
                    topPadding: 8
                    rightPadding: 10
                    bottomPadding: 8
                    palette.base: "transparent"
                    palette.text: color
                    palette.highlightedText: selectedTextColor
                }
            }
        }
    }

    Kirigami.Dialog {
        id: previewDialog
        title: qsTr("Preview Profile Changes")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        preferredWidth: 640
        preferredHeight: 520
        property string previewProfile: ""
        property string activeProfile: ""
        property var componentsToApply: []
        property string diffContent: ""

        background: Rectangle {
            color: ThemeColors.dialogBg(root.glassMode)
            radius: 8
            border.width: 1
            border.color: ThemeColors.cardBorder(root.glassMode)
        }

        // Computed lazily when the dialog is actually opened: the active
        // profile is read *now* instead of relying on a property-changed
        // handler that may have fired before activeProfile was assigned.
        function updatePreview() {
            var active = profileController.activeProfile
            previewDialog.activeProfile = active

            if (previewDialog.previewProfile === "") {
                previewDialog.componentsToApply = []
                previewDialog.diffContent = ""
                return
            }

            var comps = profileController.getProfileComponents(previewDialog.previewProfile)
            var list = []
            for (var i = 0; i < comps.length; i++) {
                list.push(comps[i])
            }
            previewDialog.componentsToApply = list

            if (active === "") {
                previewDialog.diffContent = qsTr("No active profile to compare with.")
            } else if (active === previewDialog.previewProfile) {
                previewDialog.diffContent = qsTr("\"%1\" is already the active profile.").arg(previewDialog.previewProfile)
            } else {
                previewDialog.diffContent = qsTr("Comparing…")
                profileController.requestDiff(active, previewDialog.previewProfile)
            }
        }

        Connections {
            target: profileController
            function onDiffReady(a, b, text) {
                if (a === previewDialog.activeProfile && b === previewDialog.previewProfile) {
                    previewDialog.diffContent = text
                }
            }
        }

        onOpened: updatePreview()

        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: Platform.Units.mediumSpacing

            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: ThemeColors.fgColor(root.glassMode)
                text: qsTr("Applying \"%1\" will change the following components:").arg(previewDialog.previewProfile)
            }

            Flow {
                Layout.fillWidth: true
                spacing: 4
                Repeater {
                    model: previewDialog.componentsToApply
                    delegate: Kirigami.Chip {
                        text: modelData
                        closable: false
                    }
                }
            }

            Label {
                Layout.fillWidth: true
                text: qsTr("Diff against active profile:")
                font.bold: true
                color: ThemeColors.fgColor(root.glassMode)
            }

            ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.preferredHeight: 280
                implicitWidth: 0   // see diffDialog: keep the dialog at preferredWidth
                clip: true

                background: Rectangle {
                    color: ThemeColors.codeBg(root.glassMode)
                    radius: 6
                    border.width: 1
                    border.color: ThemeColors.subtleBorder(root.glassMode)
                }

                TextEdit {
                    readOnly: true
                    font.family: "monospace"
                    wrapMode: TextEdit.NoWrap
                    text: previewDialog.diffContent
                    color: ThemeColors.fgColor(root.glassMode)
                    selectionColor: ThemeColors.accent()
                    selectedTextColor: ThemeColors.fgColor(root.glassMode)
                    leftPadding: 10
                    topPadding: 8
                    rightPadding: 10
                    bottomPadding: 8
                    palette.base: "transparent"
                    palette.text: color
                    palette.highlightedText: selectedTextColor
                }
            }
        }

        onAccepted: root.requestApply(previewDialog.previewProfile, true)
    }
}
