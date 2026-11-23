import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami
import org.kde.kirigami.platform as Platform
import Components 1.0

/*! Controls shared by the Live and saved-theme editors. */
Kirigami.ScrollablePage {
    padding: Platform.Units.gridUnit
    id: page
    objectName: "themeComponentsPage"

    title: page.groupTitle
    property string groupId: ""
    property string groupTitle: ""
    property string groupDescription: ""
    readonly property var components: {
        const groups = themeBundle.groups
        for (let i = 0; i < groups.length; ++i) {
            if (groups[i].id === page.groupId)
                return groups[i].components
        }
        return []
    }
    property string colorComponentId: ""
    property string paletteSlot: ""

    readonly property bool glassMode: themeController.isGlass
    readonly property bool readOnly: themeBundle.readOnly
    readonly property bool editableProfileGroup: components.some(component => component.profileEditable === true)
    readonly property var profileFontFamilies: readOnly && components.some(component => component.profileEditable === true && component.kind === "font")
        ? themeInspector.fontFamilies() : []

    function choiceIndex(component) {
        if (component.currentChoiceIndex !== undefined)
            return component.currentChoiceIndex;
        for (let i = 0; i < component.choices.length; ++i) {
            if (component.choices[i].value === component.currentValue)
                return i
        }
        return -1
    }

    ColorDialog {
        id: componentColorDialog
        objectName: "componentColorDialog"
        onAccepted: {
            if (page.paletteSlot !== "")
                themeBundle.setProfilePaletteColor(page.paletteSlot, selectedColor.toString())
            else
                themeBundle.setComponent(page.colorComponentId, selectedColor.toString())
        }
    }

    background: Rectangle {
        color: page.glassMode ? ThemeColors.glassPanel() : ThemeColors.pageBg(page.glassMode)
    }

    FileDialog {
        id: wallpaperDialog
        objectName: "liveWallpaperDialog"
        title: qsTr("Choose wallpaper")
        fileMode: FileDialog.OpenFile
        nameFilters: [qsTr("Images (*.png *.jpg *.jpeg *.webp *.bmp)"), qsTr("All files (*)")]
        onAccepted: themeBundle.stageWallpaper(selectedFile)
    }

    ColumnLayout {
        spacing: Platform.Units.largeSpacing

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: page.readOnly && !page.editableProfileGroup
            type: Kirigami.MessageType.Information
            text: qsTr("This is a saved profile. Switch to Live system to edit components.")
        }

        Label {
            Layout.fillWidth: true
            visible: page.groupDescription !== ""
            text: page.groupDescription
            wrapMode: Text.WordWrap
            color: ThemeColors.dimColor(page.glassMode)
        }

        Repeater {
            model: page.components

            Kirigami.Card {
                id: componentCard
                required property var modelData
                readonly property bool visualResource: modelData.kind === "choice"
                    && ["icon_theme", "cursor_theme", "plasma_global", "plasma_style",
                        "decoration_theme", "gtk_theme"].includes(modelData.id)
                readonly property bool editable: (!page.readOnly || modelData.profileEditable === true) && !themeBundle.busy
                readonly property bool fontUnavailable: page.readOnly && modelData.profileEditable === true
                    && modelData.kind === "font" && !!modelData.currentValue
                    && !page.profileFontFamilies.includes(modelData.currentValue)
                Layout.fillWidth: true

                background: Rectangle {
                    radius: Platform.Units.smallSpacing
                    border.width: 1
                    color: ThemeColors.cardBg(page.glassMode)
                    border.color: modelData.status === "warning"
                        ? ThemeColors.warning() : ThemeColors.cardBorder(page.glassMode)
                }

                contentItem: ColumnLayout {
                    spacing: Platform.Units.smallSpacing

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Platform.Units.mediumSpacing

                        Kirigami.Icon {
                            source: modelData.icon || "preferences-desktop-theme"
                            color: ThemeColors.accent()
                            Layout.preferredWidth: Platform.Units.iconSizes.small
                            Layout.preferredHeight: Platform.Units.iconSizes.small
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 0

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: Platform.Units.smallSpacing
                                Label {
                                    Layout.fillWidth: true
                                    text: modelData.label
                                    color: ThemeColors.fgColor(page.glassMode)
                                    font.bold: true
                                }
                                Rectangle {
                                    visible: modelData.staged === true
                                    width: 9
                                    height: 9
                                    radius: 4.5
                                    color: ThemeColors.warning()
                                    ToolTip.text: page.readOnly ? qsTr("Unsaved theme change") : qsTr("Staged — press Apply to write it")
                                    ToolTip.visible: stageHover.hovered || activeFocus
                                    HoverHandler { id: stageHover }
                                    activeFocusOnTab: true
                                }
                                Label {
                                    visible: modelData.status === "warning" || componentCard.fontUnavailable
                                    text: componentCard.fontUnavailable ? qsTr("Font is not installed") : modelData.statusText
                                    color: ThemeColors.warning()
                                    font.pointSize: TypeScale.caption
                                    wrapMode: Text.WordWrap
                                    Layout.maximumWidth: Platform.Units.gridUnit * 12
                                }
                            }

                            Label {
                                Layout.fillWidth: true
                                text: modelData.currentDisplay || qsTr("Not set")
                                color: ThemeColors.dimColor(page.glassMode)
                                elide: Text.ElideRight
                            }
                        }

                        ComboBox {
                            objectName: "componentChoice_" + modelData.id
                            visible: modelData.kind === "choice" && !componentCard.visualResource
                            model: modelData.choices
                            textRole: "label"
                            valueRole: "value"
                            currentIndex: page.choiceIndex(modelData)
                            displayText: currentIndex >= 0 ? currentText : (modelData.currentDisplay || qsTr("Not set"))
                            enabled: componentCard.editable && modelData.supported
                            onActivated: function(index) {
                                if (index >= 0 && index < modelData.choices.length)
                                    themeBundle.setComponent(modelData.id, modelData.choices[index].value)
                            }
                            Accessible.name: modelData.label
                        }

                        ComboBox {
                            objectName: "profileFontCombo_" + modelData.id
                            visible: page.readOnly && modelData.profileEditable === true && modelData.kind === "font"
                            Layout.maximumWidth: Platform.Units.gridUnit * 14
                            enabled: componentCard.editable
                            editable: true
                            model: visible ? page.profileFontFamilies : []
                            currentIndex: visible ? page.profileFontFamilies.indexOf(modelData.currentValue) : -1
                            editText: visible ? (modelData.currentValue || "") : ""
                            onAccepted: {
                                const family = editText.trim()
                                if (family)
                                    themeBundle.setComponent(modelData.id, family)
                            }
                            onActivated: function(index) {
                                if (index >= 0)
                                    themeBundle.setComponent(modelData.id, page.profileFontFamilies[index])
                            }
                            Accessible.name: qsTr("Interface font family")
                        }

                        Switch {
                            objectName: "componentSwitch_" + modelData.id
                            visible: modelData.kind === "bool"
                            checked: String(modelData.currentValue).toLowerCase() === "true"
                            enabled: componentCard.editable
                            text: checked ? qsTr("On") : qsTr("Off")
                            onClicked: themeBundle.setComponent(modelData.id, checked)
                            Accessible.name: modelData.label
                        }

                        Rectangle {
                            visible: modelData.kind === "color" && modelData.currentDisplay !== ""
                            Layout.preferredWidth: Platform.Units.iconSizes.small
                            Layout.preferredHeight: Layout.preferredWidth
                            radius: Platform.Units.smallSpacing
                            color: visible ? modelData.currentDisplay : "transparent"
                            border.color: page.palette.mid
                            border.width: 1
                        }

                        AppButton {
                            objectName: "componentColor_" + modelData.id
                            visible: modelData.kind === "color"
                            text: qsTr("Choose color…")
                            icon.name: "color-picker"
                            enabled: componentCard.editable
                            Accessible.name: qsTr("Choose %1").arg(modelData.label)
                            onClicked: {
                                page.paletteSlot = ""
                                page.colorComponentId = modelData.id
                                componentColorDialog.title = Accessible.name
                                componentColorDialog.selectedColor = modelData.currentDisplay || palette.highlight
                                componentColorDialog.open()
                            }
                        }

                        RowLayout {
                            visible: modelData.kind === "int" || modelData.kind === "float"
                            spacing: Platform.Units.smallSpacing
                            Layout.preferredWidth: Platform.Units.gridUnit * 16

                            Slider {
                                id: valueSlider
                                objectName: "componentSlider_" + modelData.id
                                property bool movedWhilePressed: false
                                Layout.fillWidth: true
                                from: modelData.minimum !== undefined && modelData.minimum !== null
                                      ? Number(modelData.minimum) : 0
                                to: modelData.maximum !== undefined && modelData.maximum !== null
                                    ? Number(modelData.maximum) : 100
                                stepSize: modelData.step !== undefined && modelData.step !== null
                                          ? Number(modelData.step) : 1
                                value: Number(modelData.currentValue !== undefined
                                             && modelData.currentValue !== null
                                             && modelData.currentValue !== ""
                                             ? modelData.currentValue : from)
                                enabled: componentCard.editable
                                function stageValue() {
                                    themeBundle.setComponent(modelData.id, modelData.kind === "int" ? Math.round(value) : value)
                                }
                                onMoved: {
                                    if (page.readOnly && pressed)
                                        movedWhilePressed = true
                                    else
                                        stageValue()
                                }
                                onPressedChanged: {
                                    if (!pressed && movedWhilePressed) {
                                        movedWhilePressed = false
                                        stageValue()
                                    }
                                }
                                Accessible.name: modelData.label
                            }
                            Label {
                                text: (modelData.kind === "int"
                                       ? Math.round(valueSlider.value)
                                       : Number(valueSlider.value).toFixed(2))
                                      + (modelData.suffix || "")
                                color: ThemeColors.dimColor(page.glassMode)
                                horizontalAlignment: Text.AlignRight
                                Layout.preferredWidth: Platform.Units.gridUnit * 4
                            }
                        }

                        AppButton {
                            visible: modelData.kind === "wallpaper"
                            text: qsTr("Choose…")
                            icon.name: "document-open-folder"
                            enabled: !page.readOnly && !themeBundle.busy
                            onClicked: {
                                wallpaperDialog.open()
                            }
                        }
                    }

                    Loader {
                        Layout.fillWidth: true
                        active: componentCard.visualResource
                        visible: active
                        sourceComponent: ThemeResourcePicker {
                            setting: componentCard.modelData
                            editable: componentCard.editable
                            onSelected: value => themeBundle.setComponent(componentCard.modelData.id, value)
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Platform.Units.smallSpacing

                        Label {
                            Layout.fillWidth: true
                            text: modelData.description || ""
                            wrapMode: Text.WordWrap
                            color: ThemeColors.dimColorSoft(page.glassMode)
                            font.pointSize: TypeScale.caption
                            visible: text !== ""
                        }

                        AppButton {
                            text: qsTr("Include in theme")
                            icon.name: "list-add"
                            visible: modelData.profileEditable === true && modelData.included === false
                                && (modelData.kind === "int" || modelData.kind === "float" || modelData.kind === "bool")
                            enabled: componentCard.editable
                            onClicked: {
                                if (modelData.kind === "bool")
                                    themeBundle.setComponent(modelData.id, false)
                                else
                                    valueSlider.stageValue()
                            }
                        }

                        AppButton {
                            text: qsTr("Exclude from theme")
                            icon.name: "list-remove"
                            visible: modelData.profileEditable === true && modelData.included === true
                            enabled: componentCard.editable
                            onClicked: themeBundle.excludeProfileComponent(modelData.id)
                        }

                        AppButton {
                            text: qsTr("KDE settings")
                            icon.name: "settings-configure"
                            visible: !page.readOnly && modelData.externalModule !== ""
                            onClicked: themeBundle.openSystemSettings(modelData.externalModule)
                        }

                        AppButton {
                            text: qsTr("Customize")
                            icon.name: "document-edit"
                            visible: !page.readOnly && modelData.legacySection !== ""
                                && (modelData.kind === "font" || modelData.kind === "int"
                                    || modelData.kind === "float")
                            onClicked: pageStack.layers.push(
                                Qt.resolvedUrl("SectionPage.qml"), {
                                    sectionId: modelData.legacySection,
                                    sectionTitle: modelData.label,
                                    showAdvanced: true
                                })
                        }
                    }
                }
            }
        }

        Kirigami.Card {
            Layout.fillWidth: true
            visible: page.groupId === "palette" && page.readOnly && themeBundle.canEditProfile

            contentItem: ColumnLayout {
                id: base16
                readonly property var paletteData: themeBundle.profilePalette
                readonly property bool hasPalette: (paletteData.base || []).length > 0
                spacing: Platform.Units.mediumSpacing

                Kirigami.Heading {
                    Layout.fillWidth: true
                    level: 2
                    text: qsTr("Base16 palette")
                }
                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    text: base16.hasPalette ? base16.paletteData.name : qsTr("No Base16 palette in this theme")
                }
                Label {
                    Layout.fillWidth: true
                    visible: themeBundle.profilePaletteChanged
                    text: qsTr("Unsaved theme change")
                }
                ActionFlow {
                    Layout.fillWidth: true
                    ComboBox {
                        objectName: "profilePalettePreset"
                        model: themeBundle.profilePalettePresets
                        textRole: "name"
                        currentIndex: -1
                        displayText: qsTr("Choose a preset…")
                        enabled: !themeBundle.busy && count > 0
                        onActivated: function(index) {
                            themeBundle.chooseProfilePalette(index)
                            currentIndex = -1
                        }
                    }
                    AppButton {
                        objectName: "excludeProfilePalette"
                        text: qsTr("Exclude from theme")
                        icon.name: "list-remove"
                        visible: base16.hasPalette
                        enabled: !themeBundle.busy
                        onClicked: themeBundle.excludeProfileComponent("base16_palette")
                    }
                }
                PaletteColors {
                    Layout.fillWidth: true
                    visible: base16.hasPalette
                    enabled: !themeBundle.busy
                    paletteData: base16.paletteData
                    glassMode: page.glassMode
                    onColorRequested: (slot, current) => {
                        page.paletteSlot = slot
                        componentColorDialog.title = qsTr("Choose %1").arg(slot === "accent" ? qsTr("Accent color") : slot)
                        componentColorDialog.selectedColor = current
                        componentColorDialog.open()
                    }
                }
                PalettePreview {
                    Layout.fillWidth: true
                    visible: base16.hasPalette
                    base: base16.paletteData.base || []
                    accent: base16.paletteData.accent || "transparent"
                    glassMode: page.glassMode
                }
                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    text: qsTr("Saving updates colors in resources that match the previous palette.")
                    visible: base16.hasPalette
                }
                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    text: qsTr("Automatic color generation stops. Components stored in the theme still apply.")
                    visible: !base16.hasPalette && themeBundle.profilePaletteChanged
                }
            }
        }

        ActionFlow {
            Layout.fillWidth: true
            visible: page.groupId === "applications" && !page.readOnly
            AppButton {
                objectName: "openLiveApplicationThemes"
                text: qsTr("Application themes")
                icon.name: "go-next"
                onClicked: pageStack.layers.push(Qt.resolvedUrl("ThemeApplicationsPage.qml"))
            }
        }

        Kirigami.Card {
            Layout.fillWidth: true
            visible: page.groupId === "applications" && page.readOnly && themeBundle.canEditProfile

            contentItem: ColumnLayout {
                id: appColors
                readonly property bool hasPalette: (themeBundle.profilePalette.base || []).length > 0
                spacing: Platform.Units.smallSpacing

                Button {
                    id: appTargetToggle
                    objectName: "profileAppTargetToggle"
                    Layout.fillWidth: true
                    text: qsTr("Generate application colors from Base16")
                    enabled: appColors.hasPalette && !themeBundle.busy
                    checkable: true
                    icon.name: checked ? "go-down" : "go-next"
                }
                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    text: appColors.hasPalette
                          ? qsTr("%n application(s) selected", "", themeBundle.profileSelectedAppTargets.length)
                          : qsTr("Choose a Base16 palette to enable color generation.")
                }
                Loader {
                    Layout.fillWidth: true
                    active: appColors.hasPalette && appTargetToggle.checked
                    sourceComponent: ColumnLayout {
                        id: targetList
                        readonly property var results: {
                            const query = targetSearch.text.trim().toLowerCase()
                            return themeBundle.profileAppTargets.filter(app => app.name.toLowerCase().includes(query)
                                                                                 || app.id.includes(query))
                        }

                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.Wrap
                            text: qsTr("Colors are generated when you apply the theme. Kolira uses saved custom themes where the application supports them.")
                        }
                        TextField {
                            id: targetSearch
                            objectName: "profileAppTargetSearch"
                            Layout.fillWidth: true
                            placeholderText: qsTr("Search applications…")
                            Accessible.name: placeholderText
                        }
                        Repeater {
                            model: targetList.results
                            CheckBox {
                                required property var modelData
                                objectName: "profileAppTarget_" + modelData.id
                                Layout.fillWidth: true
                                text: modelData.name
                                enabled: !themeBundle.busy
                                checked: themeBundle.profileSelectedAppTargets.includes(modelData.id)
                                onToggled: themeBundle.setProfileAppTarget(modelData.id, checked)
                            }
                        }
                        Label {
                            Layout.fillWidth: true
                            visible: targetList.results.length === 0
                            text: qsTr("No applications match the search.")
                        }
                    }
                }
            }
        }

        ActionFlow {
            Layout.fillWidth: true
            visible: page.readOnly && themeBundle.profileDraftSize > 0

            AppButton {
                objectName: "saveThemeDraft"
                text: qsTr("Save theme changes")
                icon.name: "document-save"
                enabled: !themeBundle.busy
                onClicked: themeBundle.saveProfileDraft()
            }
            AppButton {
                objectName: "discardProfileCategory"
                text: qsTr("Discard category changes")
                icon.name: "edit-undo"
                visible: themeBundle.profileDraftGroups.includes(page.groupId)
                enabled: !themeBundle.busy
                onClicked: themeBundle.discardProfileGroup(page.groupId)
            }
        }

        Label {
            Layout.fillWidth: true
            visible: page.components.length === 0
            text: qsTr("No components in this group.")
            color: ThemeColors.dimColor(page.glassMode)
        }
    }
}
