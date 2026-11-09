import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import QtCore
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
    readonly property bool applying: profileController.busy || dependencyController.busy

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
    // Existing profile equal to `name` ignoring case but not exactly ("paper"
    // vs "Paper") — the CLI rejects those; say so before the user saves.
    function caseDuplicateOf(name) {
        const lower = name.toLowerCase()
        const list = profileController.profiles
        for (let i = 0; i < list.length; ++i) {
            const existing = list[i].name || ""
            if (existing !== name && existing.toLowerCase() === lower)
                return existing
        }
        return ""
    }

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
            groupDescription: group.description
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
    property string auditOperationId: ""
    function openAuditPage() {
        pageStack.layers.push(Qt.resolvedUrl("pages/AuditPage.qml"), { selectedId: root.auditOperationId })
    }
    Connections {
        target: historySchedule
        function onErrorFound(id, message) {
            root.auditOperationId = id
            root.operationMessage = message
            applyErrorDialog.open()
        }
    }
    function openDependenciesPage() {
        pageStack.layers.push(Qt.resolvedUrl("pages/DependenciesPage.qml"))
    }

    // Every manual application is reviewed against the live state first.
    function requestApply(profileName, alreadyConfirmed) {
        if (profileName !== "")
            profileController.previewApply(profileName, false)
    }

    function showThemeMessage(message, ok) {
        const current = pageStack.layers.currentItem
        if (current && typeof current.handleThemeMessage === "function" && current.handleThemeMessage(message, ok)) return
        if (ok) root.showPassiveNotification(message)
        else { feedbackDialog.message = message; feedbackDialog.open() }
    }
    Connections { target: themeBundle; function onToast(message, ok) { root.showThemeMessage(message, ok) } }
    Connections { target: themeInspector; function onToast(message, ok) { root.showThemeMessage(message, ok) } }
    AppDialog {
        id: feedbackDialog
        objectName: "themeFeedbackDialog"
        property string message: ""
        title: qsTr("Operation failed")
        preferredWidth: 560
        standardButtons: Kirigami.Dialog.Close
        Label {
            text: feedbackDialog.message
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            Accessible.role: Accessible.AlertMessage
        }
    }

    // Last active profile we observed. Used to distinguish "the user changed
    // the selection" from "the controller just refreshed the active profile".
    property string _lastActive: ""

    // Transient feedback for the last controller operation.
    property string operationMessage: ""
    property string _lastOperation: ""
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
            root._lastOperation = op
            if (op === "apply")
                root.operationMessage = qsTr("Applying profile…")
            else if (op === "save")
                root.operationMessage = qsTr("Saving profile…")
            else if (op === "delete")
                root.operationMessage = qsTr("Deleting profile…")
            else if (op === "export")
                root.operationMessage = qsTr("Exporting theme…")
            else if (op === "import")
                root.operationMessage = qsTr("Importing theme…")
            else if (op === "install")
                root.operationMessage = qsTr("Installing theme parts…")
            else
                root.operationMessage = qsTr("Working…")
        }
        function onOperationFinished(success, message) {
            root._operationSucceeded = success
            root.operationMessage = success && root._lastOperation === "apply" ? "" : message
            if (root._lastOperation === "apply") {
                historySchedule.refreshHistory()
                if (!success) {
                    root.auditOperationId = historySchedule.operationIdFromMessage(message)
                    applyErrorDialog.open()
                }
            }
            if (success) messageTimer.restart()
            else messageTimer.stop()
            // Name taken: let the user pick another name or replace it.
            if (!success && root._lastOperation === "import"
                    && importConflictDialog.fileUrl !== ""
                    && message.indexOf("already exists") >= 0) {
                importConflictDialog.message = message
                importConflictDialog.open()
            } else if (root._lastOperation === "import") {
                importConflictDialog.fileUrl = ""
            }
        }
        function onDependenciesMissing(profile, missing) {
            missingDialog.targetProfile = profile
            missingDialog.missing = missing
            missingDialog.afterImport = root._lastOperation === "import"
            missingDialog.beforeApply = false
            missingDialog.open()
        }
        function onPreviewRequested() {
            root.visible = true
            root.raise()
            root.requestActivate()
            applyDialog.open()
        }
        function onDependenciesBeforeApply(profile, missing, keepWallpaper) {
            missingDialog.targetProfile = profile
            missingDialog.missing = missing
            missingDialog.afterImport = false
            missingDialog.beforeApply = true
            missingDialog.keepWallpaper = keepWallpaper
            missingDialog.open()
        }
        function onDependencyProgress(what) {
            root.showPassiveNotification(qsTr("Downloading: %1").arg(what), 8000, qsTr("Stop"),
                                         function() { profileController.cancelOperation() })
        }
        function onActionsOffered(profile, actions) {
            actionsDialog.targetProfile = profile
            actionsDialog.actions = actions
            // One dialog at a time: after the dependencies dialog, if it is open.
            if (missingDialog.visible)
                actionsDialog.waiting = true
            else
                actionsDialog.open()
        }
    }

    Connections {
        target: dependencyController
        function onFinished(success, message) {
            if (message !== "") {
                root._operationSucceeded = success
                root.operationMessage = message
            }
        }
        function onRetryApply(profile, keepWallpaper) {
            missingDialog.targetProfile = profile
            missingDialog.beforeApply = true
            missingDialog.keepWallpaper = keepWallpaper
            missingDialog.missing = []
            dependencyFailure.open()
        }
    }
    AppDialog {
        id: dependencyFailure
        title: qsTr("Some dependencies could not be installed")
        preferredWidth: 560
        standardButtons: Kirigami.Dialog.Close
        Label {
            text: qsTr("The theme was not applied. Review the results on the Dependencies page, retry the installation, or apply without the missing resources.")
            wrapMode: Text.WordWrap
        }
        customFooterActions: [
            Kirigami.Action {
                text: qsTr("Dependencies")
                onTriggered: {
                    pageStack.layers.push(Qt.resolvedUrl("pages/DependenciesPage.qml"))
                    dependencyFailure.close()
                }
            },
            Kirigami.Action {
                text: qsTr("Apply without them")
                enabled: !root.applying
                onTriggered: {
                    dependencyController.skipAndApply(missingDialog.targetProfile, missingDialog.keepWallpaper)
                    dependencyFailure.close()
                }
            }
        ]
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
                text: qsTr("Dependencies")
                icon.name: "package-x-generic"
                onTriggered: root.openDependenciesPage()
            },
            Kirigami.Action {
                text: qsTr("Theme change history")
                icon.name: "view-history"
                onTriggered: root.openAuditPage()
            },
            Kirigami.Action {
                text: qsTr("Theme schedule")
                icon.name: "appointment-new"
                onTriggered: pageStack.layers.push(Qt.resolvedUrl("pages/SchedulePage.qml"))
            },
            Kirigami.Action {
                text: qsTr("Settings")
                icon.name: "settings-configure"
                onTriggered: pageStack.layers.push(Qt.resolvedUrl("pages/SettingsPage.qml"))
            }
        ]
    }

    AppDialog {
        id: applyErrorDialog
        title: qsTr("Theme change failed")
        preferredWidth: 560
        standardButtons: Dialog.Close
        Label { text: root.operationMessage; wrapMode: Text.WordWrap; textFormat: Text.PlainText }
        customFooterActions: Kirigami.Action {
            text: qsTr("View history")
            onTriggered: { applyErrorDialog.close(); root.openAuditPage() }
        }
    }

    // ──────────────────────────────────────────────────────────────
    // Main page
    // NOTE: ScrollablePage IS the page — its single child must be a
    // layout (children are only anchored left/right, not laid out).
    // ──────────────────────────────────────────────────────────────
    pageStack.initialPage: Kirigami.ScrollablePage {
    padding: Platform.Units.gridUnit
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

            // ─── Header: thin row — active profile + primary actions ───
            ActionFlow {
                Layout.fillWidth: true

                Kirigami.Heading {
                    level: 2
                    text: profileController.activeProfile !== ""
                        ? qsTr("Active: %1").arg(profileController.activeProfile)
                        : qsTr("Theme Profiles")
                    width: Math.min(implicitWidth, parent.width)
                    wrapMode: Text.Wrap
                    color: ThemeColors.fgColor(root.glassMode)
                }
                AppButton {
                    text: qsTr("Edit live theme…")
                    icon.name: "document-edit"
                    onClicked: {
                        themeBundle.openLive()
                        pageStack.layers.push(Qt.resolvedUrl("pages/ThemePage.qml"))
                    }
                }
                BusyIndicator {
                    running: root.applying
                    visible: root.applying
                    Layout.preferredHeight: Platform.Units.iconSizes.small
                }
                AppButton {
                    text: qsTr("Refresh")
                    icon.name: "view-refresh"
                    enabled: !root.applying
                    onClicked: profileController.refresh()
                }
                AppButton {
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

                    ActionFlow {
                        Layout.fillWidth: true

                        Kirigami.Heading {
                            level: 2
                            text: qsTr("Available Profiles")
                            width: Math.min(implicitWidth, parent.width)
                            wrapMode: Text.Wrap
                            color: ThemeColors.fgColor(root.glassMode)
                        }
                                AppButton {
                            text: qsTr("Import theme…")
                            icon.name: "document-import"
                            enabled: !root.applying && !profileController.busy
                            onClicked: importFileDialog.open()
                        }
                        AppButton {
                            text: qsTr("New theme…")
                            icon.name: "document-new"
                            enabled: !root.applying && !profileController.busy
                            onClicked: newThemeDialog.open()
                        }
                        AppButton {
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
                            onExportClicked: function(profileName) {
                                exportFileDialog.profileName = profileName
                                exportFileDialog.selectedFile = StandardPaths.writableLocation(StandardPaths.HomeLocation) + "/" + profileName + ".ktmtheme.zip"
                                exportFileDialog.open()
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

                    ActionFlow {
                        Layout.fillWidth: true

                        Kirigami.Heading {
                            level: 2
                            text: qsTr("Wallpaper")
                            color: ThemeColors.fgColor(root.glassMode)
                        }
                                AppButton {
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
                            objectName: "wallpaperPreview"
                            anchors.fill: parent
                            source: wallpaperController.currentWallpaper
                            fillMode: Image.PreserveAspectCrop
                            // Big JPEGs decode off-thread; the indicator below
                            // covers the gap instead of blocking the page.
                            asynchronous: true
                        }

                        BusyIndicator {
                            objectName: "wallpaperPreviewBusy"
                            anchors.centerIn: parent
                            running: wallpaperPreview.status === Image.Loading
                            visible: running
                        }

                        Label {
                            objectName: "wallpaperPreviewError"
                            anchors.centerIn: parent
                            visible: wallpaperPreview.status === Image.Error
                            text: qsTr("Wallpaper unavailable")
                            color: ThemeColors.dimColor(root.glassMode)
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

                        AppButton {
                            Layout.fillWidth: true
                            text: qsTr("Previous")
                            icon.name: "go-previous-view"
                            enabled: !wallpaperController.busy
                            onClicked: wallpaperController.prevWallpaper()
                        }
                        AppButton {
                            Layout.fillWidth: true
                            text: qsTr("Pick Wallpaper…")
                            icon.name: "image-x-generic"
                            highlighted: true
                            enabled: !wallpaperController.busy
                            onClicked: wallpaperController.pickWallpaper()
                        }
                        AppButton {
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
    ApplyPreviewDialog {
        id: applyDialog
        onDependenciesRequested: function(report) {
            missingDialog.targetProfile = report.name
            missingDialog.missing = report.dependencies || []
            missingDialog.beforeApply = true
            missingDialog.afterImport = false
            missingDialog.keepWallpaper = report.keep_wallpaper || false
            missingDialog.open()
        }
    }

    // A theme needs things this machine lacks: list them with where they come
    // from; store items are installed from the official source on request
    // (search results unticked until the user confirms), packages show the
    // command to run (no sudo from the GUI).
    AppDialog {
        id: missingDialog
        title: qsTr("This Theme Needs")
        preferredWidth: 600
        property string targetProfile: ""
        property var missing: []
        property bool afterImport: false
        // Nothing changed yet: the theme is applied after this dialog.
        property bool beforeApply: false
        property bool keepWallpaper: false
        property var chosen: ({})
        // System packages go to one terminal where the
        // user types their password — the app itself never runs sudo.
        readonly property var installable: ["kde-store", "vscode-marketplace", "open-vsx", "zed", "package", "aur"]
        function selection() {
            var out = []
            for (var i = 0; i < missing.length; i++) {
                var d = missing[i]
                if (d.index >= 0 && installable.indexOf(d.source) >= 0 && chosen[d.index])
                    out.push(d.resource_id)
            }
            return out
        }
        onMissingChanged: {
            var c = {}
            for (var i = 0; i < missing.length; i++)
                c[missing[i].index] = missing[i].exact && installable.indexOf(missing[i].source) >= 0
            chosen = c
            rememberStore.checked = false
            rememberPackages.checked = false
        }
        function completeGroup(group) {
            const list = missing.filter(function(d) {
                const key = d.source === "package" || d.source === "aur" ? "packages" : "store"
                return key === group
            })
            return list.length > 0 && list.every(function(d) { return chosen[d.index] === true && installable.indexOf(d.source) >= 0 })
        }
        function emptyGroup(group) {
            const list = missing.filter(function(d) {
                return (d.source === "package" || d.source === "aur" ? "packages" : "store") === group
            })
            return list.length > 0 && list.every(function(d) { return chosen[d.index] !== true })
        }
        function rememberedGroups(install) {
            const groups = []
            if (rememberStore.checked && (!install || completeGroup("store"))) groups.push("store")
            if (rememberPackages.checked && (!install || completeGroup("packages"))) groups.push("packages")
            return groups
        }
        standardButtons: Kirigami.Dialog.Close
        customFooterActions: [
            Kirigami.Action {
                text: missingDialog.beforeApply ? qsTr("Download and review") : qsTr("Install selected")
                icon.name: "download"
                enabled: !root.applying && missingDialog.selection().length > 0
                onTriggered: {
                    dependencyController.rememberChoice(missingDialog.targetProfile, missingDialog.selection(), missingDialog.rememberedGroups(true), true)
                    dependencyController.installThemeDependencies(missingDialog.targetProfile,
                                                          missingDialog.selection(),
                                                          missingDialog.beforeApply || reapplyBox.checked,
                                                          missingDialog.keepWallpaper)
                    missingDialog.close()
                }
            },
            Kirigami.Action {
                text: qsTr("Apply without them")
                visible: missingDialog.beforeApply
                enabled: !root.applying
                onTriggered: {
                    dependencyController.rememberChoice(missingDialog.targetProfile, [], missingDialog.rememberedGroups(false), false)
                    dependencyController.skipAndApply(missingDialog.targetProfile, missingDialog.keepWallpaper)
                    missingDialog.close()
                }
            }
        ]

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
                text: missingDialog.beforeApply
                      ? qsTr("Theme \"%1\" needs these, which are not installed here. Nothing has changed yet: download what you want, or apply without them (what is missing keeps your current look).").arg(missingDialog.targetProfile)
                      : missingDialog.afterImport
                      ? qsTr("Theme \"%1\" was imported. It needs these, which are not installed here — install them?").arg(missingDialog.targetProfile)
                      : qsTr("Theme \"%1\" was applied without these parts — install them?").arg(missingDialog.targetProfile)
            }
            Repeater {
                model: missingDialog.missing
                delegate: RowLayout {
                    required property var modelData
                    Layout.fillWidth: true
                    readonly property bool canInstall: modelData.index >= 0
                        && missingDialog.installable.indexOf(modelData.source) >= 0
                    CheckBox {
                        enabled: parent.canInstall
                        checked: parent.canInstall && missingDialog.chosen[modelData.index] === true
                        onToggled: {
                            var c = Object.assign({}, missingDialog.chosen)
                            c[modelData.index] = checked
                            missingDialog.chosen = c
                        }
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0
                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            textFormat: Text.StyledText
                            color: ThemeColors.fgColor(root.glassMode)
                            text: "<b>" + (modelData.title || modelData.name) + "</b> · " + modelData.kind
                                  + (modelData.url ? " — <a href=\"" + modelData.url + "\">" + qsTr("source") + "</a>" : "")
                            onLinkActivated: (link) => Qt.openUrlExternally(link)
                        }
                        Label {
                            Layout.fillWidth: true
                            visible: !modelData.exact && parent.parent.canInstall
                            wrapMode: Text.WordWrap
                            font.pointSize: TypeScale.caption
                            color: ThemeColors.dimColor(root.glassMode)
                            text: qsTr("Found by searching for \"%1\" — open the source to check it is the right one").arg(modelData.name)
                        }
                        Label {
                            Layout.fillWidth: true
                            visible: modelData.source === "package" || modelData.source === "aur"
                            wrapMode: Text.WordWrap
                            font.pointSize: TypeScale.caption
                            color: ThemeColors.dimColor(root.glassMode)
                            text: qsTr("System package: installing opens a terminal with the command — you type your password there")
                        }
                        TextField {
                            Layout.fillWidth: true
                            visible: !!modelData.command
                            readOnly: true
                            selectByMouse: true
                            text: modelData.command || ""
                            ToolTip.text: qsTr("The command the terminal runs")
                            ToolTip.visible: hovered
                        }
                    }
                }
            }
            CheckBox {
                id: rememberStore
                text: qsTr("Do not ask again about store downloads")
                enabled: missingDialog.completeGroup("store") || (missingDialog.beforeApply && missingDialog.emptyGroup("store"))
                onEnabledChanged: if (!enabled) checked = false
            }
            CheckBox {
                id: rememberPackages
                text: qsTr("Do not ask again about system packages")
                enabled: missingDialog.completeGroup("packages") || (missingDialog.beforeApply && missingDialog.emptyGroup("packages"))
                onEnabledChanged: if (!enabled) checked = false
            }
            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: qsTr("Remembering applies to a whole group: install all of it automatically, or skip it when applying without dependencies. Partial selections are not remembered. Change this later in Settings → Automation.")
                color: ThemeColors.dimColor(root.glassMode)
            }
            CheckBox {
                id: reapplyBox
                checked: true
                text: qsTr("Apply the theme again after installing")
            }
        }
    }

    // New theme: start from a base — a built-in neutral template (Breeze, no
    // blur, no translucency) or a copy of a theme — then edit the live look
    // and save over it.
    AppDialog {
        id: newThemeDialog
        title: qsTr("New Theme")
        preferredWidth: 480
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        readonly property var bases: {
            var list = [{ value: "neutral-light", text: qsTr("Neutral, light (Breeze, no blur)") },
                        { value: "neutral-dark", text: qsTr("Neutral, dark (Breeze, no blur)") }]
            for (var i = 0; i < profileController.profiles.length; i++) {
                var n = profileController.profiles[i].name
                list.push({ value: n, text: qsTr("Copy of \"%1\"").arg(n) })
            }
            return list
        }
        background: Rectangle {
            color: ThemeColors.dialogBg(root.glassMode)
            radius: 8
            border.width: 1
            border.color: ThemeColors.cardBorder(root.glassMode)
        }
        ColumnLayout {
            Layout.fillWidth: true
            TextField {
                id: newThemeName
                Layout.fillWidth: true
                placeholderText: qsTr("Theme name")
            }
            ComboBox {
                id: newThemeBase
                Layout.fillWidth: true
                textRole: "text"
                valueRole: "value"
                model: newThemeDialog.bases
            }
            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                font.pointSize: TypeScale.caption
                color: ThemeColors.dimColor(root.glassMode)
                text: qsTr("Starting replaces your current look with the base (a backup is made). Then change the look and use Save current… with the same name.")
            }
            Label {
                id: newThemeError
                Layout.fillWidth: true
                visible: false
                wrapMode: Text.WordWrap
                color: ThemeColors.negative()
                text: qsTr("Use letters, digits, dots, dashes or underscores; the first character must not be a dot.")
            }
        }
        onAccepted: {
            var name = newThemeName.text.trim()
            if (!/^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/.test(name) || root.caseDuplicateOf(name) !== "") {
                newThemeError.visible = true
                newThemeDialog.open()
                return
            }
            newThemeError.visible = false
            profileController.newTheme(name, newThemeBase.currentValue)
            newThemeName.text = ""
        }
    }

    // First run: ask the user what applying themes may do by itself
    // (store downloads, system packages, restarting programs). Per user, not
    // per theme; changeable later in Settings → Automation.
    AppDialog {
        id: automationDialog
        title: qsTr("Before You Apply Themes")
        preferredWidth: 640
        closePolicy: Popup.NoAutoClose
        standardButtons: Kirigami.Dialog.NoButton
        customFooterActions: [
            Kirigami.Action {
                text: qsTr("Save")
                icon.name: "dialog-ok"
                onTriggered: {
                    settingsController.confirmAutomation(automationList.pending)
                    automationDialog.close()
                }
            }
        ]
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
                text: qsTr("A theme may need things outside its files. What may the app do by itself? Ask — offer it each time; Always — just do it; Never — skip it. You can change this later in Settings → Automation.")
            }
            AutomationList {
                id: automationList
                Layout.fillWidth: true
                glassMode: root.glassMode
                items: settingsController.automationItems
            }
        }
    }
    Timer {
        // After the window is up (the settings controller is initialised).
        interval: 600
        running: true
        onTriggered: if (!settingsController.automationConfigured) automationDialog.open()
    }

    // After applying: programs to restart, Telegram's
    // one-time choice — each with a warning and its own checkbox; nothing
    // runs without the user (a call or a game may be on).
    AppDialog {
        id: actionsDialog
        title: qsTr("Finish Applying")
        preferredWidth: 600
        property string targetProfile: ""
        property var actions: []
        property var chosen: ({})
        property bool waiting: false
        function selection() {
            var out = []
            for (var i = 0; i < actions.length; i++)
                if (actions[i].kind !== "manual" && chosen[actions[i].id] !== false)
                    out.push(actions[i].id)
            return out
        }
        onActionsChanged: chosen = ({})
        standardButtons: Kirigami.Dialog.Close
        customFooterActions: [
            Kirigami.Action {
                text: qsTr("Do selected")
                icon.name: "run-build"
                enabled: !profileController.busy && actionsDialog.selection().length > 0
                onTriggered: {
                    profileController.runActions(actionsDialog.selection())
                    actionsDialog.close()
                }
            }
        ]
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
                text: qsTr("Theme \"%1\" is applied. These steps finish it — untick what you want to skip.").arg(actionsDialog.targetProfile)
            }
            Repeater {
                model: actionsDialog.actions
                delegate: RowLayout {
                    required property var modelData
                    Layout.fillWidth: true
                    CheckBox {
                        visible: modelData.kind !== "manual"
                        checked: actionsDialog.chosen[modelData.id] !== false
                        onToggled: {
                            var c = Object.assign({}, actionsDialog.chosen)
                            c[modelData.id] = checked
                            actionsDialog.chosen = c
                        }
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0
                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            font.bold: true
                            color: ThemeColors.fgColor(root.glassMode)
                            text: qsTranslate("Core", modelData.title)
                        }
                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            font.pointSize: TypeScale.caption
                            color: ThemeColors.dimColor(root.glassMode)
                            text: qsTranslate("Core", modelData.reason)
                        }
                        Label {
                            Layout.fillWidth: true
                            visible: modelData.warning !== ""
                            wrapMode: Text.WordWrap
                            font.pointSize: TypeScale.caption
                            color: ThemeColors.warning()
                            text: "⚠ " + qsTranslate("Core", modelData.warning)
                        }
                    }
                }
            }
        }
    }
    Connections {
        target: missingDialog
        function onClosed() {
            if (actionsDialog.waiting) {
                actionsDialog.waiting = false
                actionsDialog.open()
            }
        }
    }

    // ── Export / import (.ktmtheme.zip) ──────────────────────────────────
    FileDialog {
        id: exportFileDialog
        property string profileName: ""
        title: qsTr("Export theme \"%1\"").arg(profileName)
        fileMode: FileDialog.SaveFile
        defaultSuffix: "zip"
        nameFilters: [qsTr("Theme archives (*.zip)")]
        onAccepted: exportOptionsDialog.open()
    }

    AppDialog {
        id: exportOptionsDialog
        title: qsTr("Export Theme")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        preferredWidth: 440

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
                text: qsTr("The archive holds the theme's appearance files; your home folder is written as $HOME and desktop icon positions are left out.")
            }
            CheckBox {
                id: exportWallpaperBox
                checked: true
                text: qsTr("Include the wallpaper")
                ToolTip.text: qsTr("Leave it out if you may not share the image")
                ToolTip.visible: hovered
            }
        }

        onAccepted: profileController.exportProfile(exportFileDialog.profileName,
                                                    exportFileDialog.selectedFile.toString(),
                                                    exportWallpaperBox.checked)
    }

    FileDialog {
        id: importFileDialog
        title: qsTr("Import a theme")
        fileMode: FileDialog.OpenFile
        nameFilters: [qsTr("Theme archives (*.zip)"), qsTr("All files (*)")]
        onAccepted: {
            importConflictDialog.fileUrl = selectedFile.toString()
            profileController.importTheme(importConflictDialog.fileUrl, "", false)
        }
    }

    AppDialog {
        id: importConflictDialog
        title: qsTr("Theme Already Exists")
        property string fileUrl: ""
        property string message: ""
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        preferredWidth: 440

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
                text: importConflictDialog.message
            }
            TextField {
                id: importNameField
                Layout.fillWidth: true
                enabled: !importReplaceBox.checked
                placeholderText: qsTr("Import under a new name")
            }
            CheckBox {
                id: importReplaceBox
                text: qsTr("Replace the existing theme (it is backed up first)")
            }
        }

        onAccepted: {
            var replace = importReplaceBox.checked
            var name = replace ? "" : importNameField.text.trim()
            importReplaceBox.checked = false
            importNameField.text = ""
            profileController.importTheme(fileUrl, name, replace)
        }
        onRejected: fileUrl = ""
    }

    AppDialog {
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

    AppDialog {
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
            Label {
                Layout.fillWidth: true
                readonly property string clash: root.caseDuplicateOf(nameField.text.trim())
                visible: clash !== ""
                wrapMode: Text.WordWrap
                color: ThemeColors.negative()
                text: qsTr("Profile \"%1\" already exists — names may not differ only by case.").arg(clash)
            }
            TextField {
                id: descriptionField
                Layout.fillWidth: true
                placeholderText: qsTr("Description (optional)")
            }
            CheckBox {
                id: includeWallpaperBox
                checked: true
                text: qsTr("Include the current wallpaper")
                ToolTip.text: qsTr("Without a wallpaper, applying this theme leaves the wallpaper unchanged")
                ToolTip.visible: hovered
            }
            CheckBox {
                id: makeLookAndFeelBox
                checked: false
                text: qsTr("Create a Look & Feel package for this theme")
                ToolTip.text: qsTr("Colour scheme, widget style, icons, cursor, Plasma style and window decoration as a Global Theme (no panel layout)")
                ToolTip.visible: hovered
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
            if (root.caseDuplicateOf(name) !== "") {
                saveDialog.open()
                return
            }
            if (!/^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/.test(name)) {
                saveError.visible = true
                saveDialog.open()
                return
            }
            profileController.saveProfileWithLookAndFeel(name, descriptionField.text.trim(),
                                                         includeWallpaperBox.checked, makeLookAndFeelBox.checked)
            nameField.text = ""
            descriptionField.text = ""
            includeWallpaperBox.checked = true
            makeLookAndFeelBox.checked = false
        }
    }

    AppDialog {
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

    AppDialog {
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

    AppDialog {
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

            InsetScrollView {
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

    AppDialog {
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
                        text: trackedAppsController.componentName(modelData)
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

            InsetScrollView {
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
