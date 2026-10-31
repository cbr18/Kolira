import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami
import org.kde.kirigami.platform as Platform
import Components 1.0

/*! Theme Inspector — what the current look is made of.
 *
 *  Live scope edits in place (instant apply + undo); profile scopes are
 *  read-only snapshots. Sections are an accordion: one open at a time keeps
 *  the page short even though it exposes ~55 fields.
 */
Kirigami.ScrollablePage {
    id: page

    title: qsTr("Theme")
    readonly property bool glassMode: themeController.isGlass

    property string expandedSection: ""
    property bool showAdvanced: false
    property string searchText: ""
    property var searchResults: []
    property string message: ""
    property bool messageOk: true

    function showMessage(text, ok) {
        page.message = text
        page.messageOk = ok
        messageTimer.restart()
    }

    function runSearch() {
        page.searchResults = page.searchText.length > 0
            ? themeInspector.search(page.searchText)
            : []
    }

    function syncScope() {
        var scopes = themeInspector.scopes
        var index = scopes.indexOf(themeInspector.scope)
        scopeCombo.model = scopes
        scopeCombo.currentIndex = index
    }

    background: Rectangle {
        color: page.glassMode ? ThemeColors.glassPanel() : ThemeColors.pageBg(page.glassMode)
    }

    Timer {
        id: messageTimer
        interval: 6000
        onTriggered: page.message = ""
    }

    Timer {
        id: searchTimer
        interval: 200
        onTriggered: page.runSearch()
    }

    FileDialog {
        id: exportDialog
        title: qsTr("Save theme snapshot")
        fileMode: FileDialog.SaveFile
        defaultSuffix: "json"
        nameFilters: [qsTr("JSON files (*.json)"), qsTr("All files (*)")]
        onAccepted: themeInspector.exportJson(selectedFile)
    }

    Kirigami.Dialog {
        id: configValueDialog
        title: qsTr("Edit config value")
        property string relPath: ""
        property string configKey: ""
        property string oldValue: ""
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        preferredWidth: 480

        function openFor(path, key, value) {
            relPath = path
            configKey = key
            oldValue = value
            newValueField.text = value
            open()
        }

        ColumnLayout {
            spacing: Platform.Units.smallSpacing

            Label {
                Layout.fillWidth: true
                text: configValueDialog.relPath + "  ·  " + configValueDialog.configKey
                font.family: "monospace"
                font.pointSize: TypeScale.caption
                color: ThemeColors.dimColor(page.glassMode)
                wrapMode: Text.WordWrap
            }
            TextField {
                id: newValueField
                Layout.fillWidth: true
            }
            Label {
                // Живое превью «было → станет» (этап C2)
                Layout.fillWidth: true
                text: configValueDialog.oldValue + "  →  " + newValueField.text
                font.family: "monospace"
                font.pointSize: TypeScale.caption
                color: ThemeColors.fgColorSoft(page.glassMode)
                wrapMode: Text.WordWrap
            }
        }

        onAccepted: {
            if (configValueDialog.relPath !== "") {
                themeInspector.setAppConfigValue(
                    configValueDialog.relPath, configValueDialog.configKey,
                    newValueField.text.trim())
            }
        }
    }

    Connections {
        target: themeInspector
        function onToast(text, ok) { page.showMessage(text, ok) }
        function onScopeChanged() { page.syncScope() }
        function onFieldsChanged() {
            if (page.searchText.length > 0)
                page.runSearch()
        }
    }

    Connections {
        target: profileController
        function onProfilesChanged() { page.syncScope() }
    }

    Component.onCompleted: page.syncScope()

    ColumnLayout {
        Layout.margins: Platform.Units.largeSpacing
        spacing: Platform.Units.largeSpacing

        // ─── Header ───────────────────────────────────────────
        RowLayout {
            Layout.fillWidth: true
            spacing: Platform.Units.mediumSpacing

            Item { Layout.fillWidth: true }

            BusyIndicator {
                running: themeInspector.busy
                visible: themeInspector.busy
                Layout.preferredHeight: Platform.Units.iconSizes.small
            }

            ComboBox {
                id: scopeCombo
                Layout.preferredWidth: Platform.Units.gridUnit * 10
                textRole: ""
                displayText: currentIndex >= 0 && currentText === "live"
                    ? qsTr("Live system")
                    : currentText.replace("profile:", qsTr("Profile: "))
                onActivated: function(index) {
                    if (index >= 0)
                        themeInspector.setScope(scopeCombo.model[index])
                }
            }

            ToolButton {
                icon.name: "view-refresh"
                enabled: !themeInspector.busy
                ToolTip.text: qsTr("Re-read the configuration")
                ToolTip.visible: hovered
                onClicked: themeInspector.refresh()
            }
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: page.message !== ""
            text: page.message
            type: page.messageOk ? Kirigami.MessageType.Positive : Kirigami.MessageType.Error
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: themeInspector.readOnly
            text: qsTr("Profile snapshots are read-only. Switch to “Live system” to edit values.")
            type: Kirigami.MessageType.Information
        }

        // ─── Overview ─────────────────────────────────────────
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
                    spacing: Platform.Units.mediumSpacing

                    Rectangle {
                        Layout.preferredWidth: Platform.Units.gridUnit * 4
                        Layout.preferredHeight: Platform.Units.gridUnit * 4
                        radius: Platform.Units.smallSpacing
                        color: themeInspector.accentColor
                        border.width: 1
                        border.color: ThemeColors.subtleBorder(page.glassMode)

                        Label {
                            anchors.centerIn: parent
                            text: themeInspector.accentColor
                            color: "white"
                            style: Text.Outline
                            styleColor: "black"
                            font.pointSize: TypeScale.caption
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2

                        Label {
                            text: themeInspector.scope === "live"
                                ? qsTr("Live system")
                                : qsTr("Snapshot: %1").arg(themeInspector.scope.replace("profile:", ""))
                            color: ThemeColors.fgColor(page.glassMode)
                            font.bold: true
                            font.pointSize: TypeScale.subtitle
                        }

                        Label {
                            text: qsTr("Active profile: %1 · %2 difference(s)")
                                .arg(themeInspector.activeProfile || "—")
                                .arg(themeInspector.modifiedCount)
                            color: ThemeColors.dimColor(page.glassMode)
                            font.pointSize: TypeScale.caption
                        }

                        Label {
                            visible: themeInspector.sessionChanges > 0
                            text: qsTr("%1 change(s) in this session").arg(themeInspector.sessionChanges)
                            color: ThemeColors.accent()
                            font.pointSize: TypeScale.caption
                        }
                    }

                    Image {
                        Layout.preferredWidth: Platform.Units.gridUnit * 8
                        Layout.preferredHeight: Platform.Units.gridUnit * 4.5
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                        visible: wallpaperController.currentWallpaper !== ""
                        source: wallpaperController.currentWallpaper
                        clip: true
                    }

                    ColumnLayout {
                        spacing: Platform.Units.smallSpacing

                        Button {
                            text: qsTr("Undo")
                            icon.name: "edit-undo"
                            enabled: !themeInspector.readOnly && themeInspector.sessionChanges > 0 && !themeInspector.busy
                            onClicked: themeInspector.undo()
                        }

                        Button {
                            text: qsTr("Revert all")
                            icon.name: "document-revert"
                            enabled: !themeInspector.readOnly && themeInspector.sessionChanges > 0 && !themeInspector.busy
                            onClicked: themeInspector.revertAll()
                        }
                    }
                }

                Kirigami.Separator { Layout.fillWidth: true }

                Repeater {
                    model: themeInspector.sections

                    delegate: RowLayout {
                        Layout.fillWidth: true
                        spacing: Platform.Units.mediumSpacing

                        Kirigami.Icon {
                            source: modelData.icon
                            Layout.preferredWidth: Platform.Units.iconSizes.small
                            Layout.preferredHeight: Platform.Units.iconSizes.small
                            color: ThemeColors.accent()
                        }

                        Label {
                            text: modelData.title
                            color: ThemeColors.fgColor(page.glassMode)
                            Layout.preferredWidth: Platform.Units.gridUnit * 7
                            elide: Text.ElideRight
                        }

                        Label {
                            Layout.fillWidth: true
                            text: modelData.summary || "—"
                            color: ThemeColors.dimColor(page.glassMode)
                            elide: Text.ElideRight
                        }

                        Label {
                            visible: (modelData.issueCount || 0) > 0
                            text: qsTr("%1 issue(s)").arg(modelData.issueCount)
                            color: ThemeColors.warning()
                            font.pointSize: TypeScale.caption
                        }
                    }
                }
            }
        }

        // ─── Search ───────────────────────────────────────────
        TextField {
            Layout.fillWidth: true
            placeholderText: qsTr("Search settings… (accent, blur, kitty, шрифт)")
            text: page.searchText
            onTextChanged: {
                page.searchText = text
                searchTimer.restart()
            }
            onAccepted: page.runSearch()
        }

        // ─── Validation ───────────────────────────────────────
        Kirigami.Card {
            Layout.fillWidth: true
            visible: themeInspector.issues.length > 0

            background: Rectangle {
                radius: Platform.Units.smallSpacing
                border.width: 1
                color: ThemeColors.cardBg(page.glassMode)
                border.color: ThemeColors.warning()
            }

            contentItem: ColumnLayout {
                spacing: Platform.Units.smallSpacing

                RowLayout {
                    Layout.fillWidth: true

                    Kirigami.Heading {
                        level: 2
                        text: qsTr("Validation")
                        color: ThemeColors.fgColor(page.glassMode)
                    }
                    Item { Layout.fillWidth: true }
                    Label {
                        text: qsTr("%1 issue(s)").arg(themeInspector.issues.length)
                        color: ThemeColors.warning()
                    }
                }

                Repeater {
                    model: themeInspector.issues

                    delegate: RowLayout {
                        Layout.fillWidth: true
                        spacing: Platform.Units.mediumSpacing

                        Kirigami.Icon {
                            source: modelData.severity === "error"
                                ? "dialog-error" : "dialog-warning"
                            color: modelData.severity === "error"
                                ? ThemeColors.negative() : ThemeColors.warning()
                            Layout.preferredWidth: Platform.Units.iconSizes.small
                            Layout.preferredHeight: Platform.Units.iconSizes.small
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 0

                            Label {
                                Layout.fillWidth: true
                                text: modelData.message
                                color: ThemeColors.fgColor(page.glassMode)
                                wrapMode: Text.WordWrap
                            }
                            Label {
                                Layout.fillWidth: true
                                visible: (modelData.fixHint || "") !== ""
                                text: modelData.fixHint || ""
                                color: ThemeColors.dimColor(page.glassMode)
                                font.pointSize: TypeScale.caption
                                wrapMode: Text.WordWrap
                            }
                        }

                        Button {
                            text: qsTr("Fix")
                            visible: (modelData.fieldId || "") !== ""
                            enabled: !themeInspector.readOnly && !themeInspector.busy
                            onClicked: themeInspector.applyFix(modelData.fieldId)
                        }

                        Button {
                            text: qsTr("Show")
                            visible: (modelData.section || "") !== ""
                            onClicked: page.expandedSection = modelData.section
                        }
                    }
                }
            }
        }

        // ─── Search results ───────────────────────────────────
        ColumnLayout {
            Layout.fillWidth: true
            visible: page.searchText.length > 0
            spacing: Platform.Units.smallSpacing

            Label {
                text: qsTr("%1 result(s) for “%2”")
                    .arg(page.searchResults.length)
                    .arg(page.searchText)
                color: ThemeColors.dimColor(page.glassMode)
                font.bold: true
            }

            Repeater {
                model: page.searchResults

                delegate: ThemeFieldRow {
                    fieldData: modelData
                    showSection: true
                    glassMode: page.glassMode
                }
            }

            Label {
                visible: page.searchResults.length === 0
                text: qsTr("Nothing matches. Try “blur”, “font”, “cursor” or “kitty”.")
                color: ThemeColors.dimColor(page.glassMode)
            }
        }

        // ─── Sections (accordion) ─────────────────────────────
        ColumnLayout {
            Layout.fillWidth: true
            visible: page.searchText.length === 0
            spacing: Platform.Units.smallSpacing

            Repeater {
                model: themeInspector.sections

                delegate: ThemeSectionCard {
                    sectionData: modelData
                    glassMode: page.glassMode
                    showAdvanced: page.showAdvanced
                    expanded: page.expandedSection === modelData.id
                    onToggleRequested: {
                        page.expandedSection = page.expandedSection === modelData.id
                            ? "" : modelData.id
                    }
                }
            }
        }

        // ─── Application configs (read-only, stage C1) ────────
        Kirigami.Card {
            Layout.fillWidth: true
            visible: page.searchText.length === 0 && themeInspector.appConfigs.length > 0

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
                    Kirigami.Heading {
                        level: 2
                        text: qsTr("Application configs")
                        color: ThemeColors.fgColor(page.glassMode)
                    }
                    Item { Layout.fillWidth: true }
                    Label {
                        text: themeInspector.readOnly ? qsTr("read-only") : qsTr("live: editable")
                        font.pointSize: TypeScale.caption
                        color: ThemeColors.dimColor(page.glassMode)
                    }
                }

                Label {
                    Layout.fillWidth: true
                    text: qsTr("What the tracked applications store, file by file. "
                               + "Pencil-edit a value in the live scope (INI/JSON); "
                               + "TOML/user.js arrive in a later stage.")
                    color: ThemeColors.dimColor(page.glassMode)
                    wrapMode: Text.WordWrap
                    font.pointSize: TypeScale.caption
                }

                Repeater {
                    model: themeInspector.appConfigs

                    ColumnLayout {
                        required property var modelData
                        Layout.fillWidth: true
                        spacing: 2

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Platform.Units.smallSpacing
                            Kirigami.Icon {
                                source: modelData.icon
                                Layout.preferredWidth: Platform.Units.iconSizes.small
                                Layout.preferredHeight: Platform.Units.iconSizes.small
                            }
                            Label {
                                text: modelData.name
                                font.bold: true
                                color: ThemeColors.fgColor(page.glassMode)
                            }
                        }

                        Repeater {
                            model: modelData.files

                            ColumnLayout {
                                required property var modelData
                                // Алиас для вложенного Repeater строк: там
                                // modelData перекрывается строкой параметра.
                                property var fileData: modelData
                                Layout.fillWidth: true
                                Layout.leftMargin: Platform.Units.largeSpacing
                                spacing: 1

                                Label {
                                    Layout.fillWidth: true
                                    text: modelData.relPath
                                    font.family: "monospace"
                                    font.pointSize: TypeScale.caption
                                    color: ThemeColors.fgColorSoft(page.glassMode)
                                    elide: Text.ElideMiddle
                                }

                                Repeater {
                                    model: modelData.params

                                    RowLayout {
                                        required property var modelData
                                        Layout.fillWidth: true
                                        Layout.leftMargin: Platform.Units.largeSpacing

                                        Label {
                                            Layout.fillWidth: true
                                            Layout.maximumWidth: 360
                                            text: modelData.key
                                            font.family: "monospace"
                                            font.pointSize: TypeScale.caption
                                            color: ThemeColors.fgColor(page.glassMode)
                                            elide: Text.ElideRight
                                        }
                                        Label {
                                            Layout.fillWidth: true
                                            text: modelData.value
                                            font.family: "monospace"
                                            font.pointSize: TypeScale.caption
                                            color: ThemeColors.dimColor(page.glassMode)
                                            elide: Text.ElideRight
                                        }

                                        ToolButton {
                                            text: qsTr("Edit value")
                                            icon.name: "document-edit"
                                            visible: fileData.editable && !themeInspector.readOnly
                                            onClicked: configValueDialog.openFor(
                                                fileData.relPath, modelData.key, modelData.value)
                                        }
                                    }
                                }

                                Label {
                                    Layout.fillWidth: true
                                    Layout.leftMargin: Platform.Units.largeSpacing
                                    visible: modelData.truncated || modelData.note !== ""
                                    text: (modelData.note !== "" ? modelData.note + " · " : "")
                                          + (modelData.truncated ? qsTr("truncated") : "")
                                    font.pointSize: TypeScale.caption
                                    font.italic: true
                                    color: ThemeColors.warning()
                                }
                            }
                        }
                    }
                }
            }
        }

        // ─── Advanced ─────────────────────────────────────────
        Kirigami.Card {
            Layout.fillWidth: true
            visible: page.searchText.length === 0

            background: Rectangle {
                radius: Platform.Units.smallSpacing
                border.width: 1
                color: ThemeColors.nestedBg(page.glassMode)
                border.color: ThemeColors.cardBorder(page.glassMode)
            }

            contentItem: ColumnLayout {
                spacing: Platform.Units.mediumSpacing

                RowLayout {
                    Layout.fillWidth: true

                    Kirigami.Heading {
                        level: 2
                        text: qsTr("Advanced")
                        color: ThemeColors.fgColor(page.glassMode)
                    }
                    Item { Layout.fillWidth: true }

                    Switch {
                        text: qsTr("Show advanced fields")
                        checked: page.showAdvanced
                        onClicked: page.showAdvanced = checked
                    }
                }

                Label {
                    Layout.fillWidth: true
                    text: qsTr("Deep settings stay in System Settings. These buttons open the right module instead of duplicating it.")
                    color: ThemeColors.dimColor(page.glassMode)
                    wrapMode: Text.WordWrap
                    font.pointSize: TypeScale.caption
                }

                Flow {
                    Layout.fillWidth: true
                    spacing: Platform.Units.smallSpacing

                    Repeater {
                        model: [
                            { label: qsTr("Colors"), module: "kcm_colors" },
                            { label: qsTr("Fonts"), module: "kcm_fonts" },
                            { label: qsTr("Icons"), module: "kcm_icons" },
                            { label: qsTr("Cursors"), module: "kcm_cursortheme" },
                            { label: qsTr("Window effects"), module: "kcm_kwin_effects" },
                            { label: qsTr("Kvantum"), module: "kcm_style" }
                        ]

                        delegate: Button {
                            text: modelData.label
                            onClicked: themeInspector.openSystemSettings(modelData.module)
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true

                    Button {
                        text: qsTr("Open config folder")
                        icon.name: "folder-open"
                        onClicked: themeInspector.openFile(".config")
                    }

                    Item { Layout.fillWidth: true }

                    Button {
                        text: qsTr("Export JSON…")
                        icon.name: "document-save-as"
                        onClicked: exportDialog.open()
                    }

                    Button {
                        text: qsTr("Undo last change")
                        icon.name: "edit-undo"
                        enabled: !themeInspector.readOnly && themeInspector.sessionChanges > 0 && !themeInspector.busy
                        onClicked: themeInspector.undo()
                    }

                    Button {
                        text: qsTr("Revert session")
                        icon.name: "document-revert"
                        enabled: !themeInspector.readOnly && themeInspector.sessionChanges > 0 && !themeInspector.busy
                        onClicked: themeInspector.revertAll()
                    }
                }
            }
        }
    }
}
