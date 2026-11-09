import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami
import org.kde.kirigami.platform as Platform
import Components 1.0

/*! Palette page: base16 + accent → KDE colours, terminals, Kvantum.
 *
 *  Every action is the Rust CLI behind paletteController (docs/PALETTE.md);
 *  "From wallpaper" uses Kirigami.ImageColors — Plasma's own algorithm, the
 *  same one behind "accent colour from wallpaper".
 */
Kirigami.ScrollablePage {
    padding: Platform.Units.gridUnit
    id: page

    title: qsTr("Palette")
    readonly property bool glassMode: themeController.isGlass
    readonly property var pal: paletteController.palette
    readonly property var slots: ["base00", "base01", "base02", "base03", "base04", "base05", "base06", "base07",
                                  "base08", "base09", "base0A", "base0B", "base0C", "base0D", "base0E", "base0F"]
    readonly property var slotHints: [
        qsTr("Background"), qsTr("Lighter background"), qsTr("Selection background"), qsTr("Comments, inactive"),
        qsTr("Dark foreground"), qsTr("Text"), qsTr("Light foreground"), qsTr("Lightest"),
        qsTr("Red"), qsTr("Orange"), qsTr("Yellow"), qsTr("Green"),
        qsTr("Cyan"), qsTr("Blue"), qsTr("Magenta"), qsTr("Brown")
    ]
    property string editingSlot: ""
    property string message: ""
    property bool messageOk: true
    property bool waitingForImageColors: false

    function hex(c) {
        const v = [c.r, c.g, c.b].map(x => Math.round(x * 255).toString(16).padStart(2, "0"))
        return "#" + v.join("")
    }

    function editSlot(slot, current) {
        page.editingSlot = slot
        colorDialog.selectedColor = current
        colorDialog.open()
    }

    function selectedTargets() {
        const t = []
        if (kdeTarget.checked) t.push("kde")
        if (kittyTarget.checked) t.push("kitty")
        if (alacrittyTarget.checked) t.push("alacritty")
        if (konsoleTarget.checked) t.push("konsole")
        if (discordTarget.checked) t.push("discord")
        if (spicetifyTarget.checked) t.push("spicetify")
        if (ghosttyTarget.checked) t.push("ghostty")
        if (cavaTarget.checked) t.push("cava")
        if (kvantumTarget.checked) t.push("kvantum")
        return t
    }

    function swatchesOf(palette) {
        const swatches = []
        for (let i = 0; i < palette.length; ++i)
            swatches.push({ "color": page.hex(palette[i].color), "ratio": palette[i].ratio })
        return swatches
    }

    function fromImageColors() {
        if (paletteController.wallpaperUrl === "") {
            paletteController.fromWallpaper()
            return
        }
        // Already computed for this wallpaper → use it; otherwise ask
        // ImageColors and fall back to the Rust extractor if it stays silent.
        if (imageColors.palette.length > 0 && !page.waitingForImageColors) {
            paletteController.fromImageColors(page.swatchesOf(imageColors.palette))
            return
        }
        page.waitingForImageColors = true
        imageColorsFallback.restart()
        imageColors.update()
    }

    background: Rectangle {
        color: page.glassMode ? ThemeColors.glassPanel() : ThemeColors.pageBg(page.glassMode)
    }

    Connections {
        target: paletteController
        function onToast(message, success) {
            page.message = message
            page.messageOk = success
            if (success)
                messageTimer.restart()
            else
                messageTimer.stop()
        }
    }

    Timer {
        id: messageTimer
        interval: 5000
        onTriggered: page.message = ""
    }

    Kirigami.ImageColors {
        id: imageColors
        source: paletteController.wallpaperUrl
        // The first change after update() is a reset to an empty list; the
        // real swatches arrive with the next one.
        onPaletteChanged: {
            if (!page.waitingForImageColors || palette.length === 0)
                return
            page.waitingForImageColors = false
            imageColorsFallback.stop()
            paletteController.fromImageColors(page.swatchesOf(palette))
        }
    }

    Timer {
        id: imageColorsFallback
        interval: 6000
        onTriggered: {
            if (!page.waitingForImageColors)
                return
            page.waitingForImageColors = false
            paletteController.fromWallpaper()
        }
    }

    ColorDialog {
        id: colorDialog
        title: qsTr("Choose %1").arg(page.editingSlot)
        onAccepted: paletteController.setSlot(page.editingSlot, page.hex(selectedColor))
    }

    FileDialog {
        id: fileDialog
        title: qsTr("Import a base16 scheme")
        nameFilters: [qsTr("base16 schemes (*.yaml *.yml *.json)"), qsTr("All files (*)")]
        onAccepted: paletteController.importFile(selectedFile.toString())
    }

    ColumnLayout {
        spacing: Platform.Units.largeSpacing

        Label {
            Layout.fillWidth: true
            text: qsTr("One palette colours the whole theme: KDE colour scheme, terminals and, on request, Kvantum. Pick a ready scheme, take it from the wallpaper or edit the colours, then apply.")
            wrapMode: Text.WordWrap
            color: ThemeColors.dimColor(page.glassMode)
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: page.message !== ""
            text: page.message
            type: page.messageOk ? Kirigami.MessageType.Positive : Kirigami.MessageType.Error
            showCloseButton: true
            onVisibleChanged: if (!visible) page.message = ""
        }

        // ── Sources ───────────────────────────────────────────────────
        Flow {
            Layout.fillWidth: true
            spacing: Platform.Units.smallSpacing
            AppButton {
                text: qsTr("Gallery…")
                icon.name: "view-list-icons"
                enabled: !paletteController.busy
                onClicked: galleryDialog.openGallery()
            }
            AppButton {
                text: qsTr("From wallpaper")
                icon.name: "preferences-desktop-wallpaper"
                enabled: !paletteController.busy
                onClicked: page.fromImageColors()
                ToolTip.visible: hovered
                ToolTip.text: qsTr("Plasma's own colour extraction (Kirigami.ImageColors) on the current wallpaper")
            }
            AppButton {
                text: qsTr("From wallpaper (detailed)")
                icon.name: "color-picker"
                enabled: !paletteController.busy
                onClicked: paletteController.fromWallpaper()
                ToolTip.visible: hovered
                ToolTip.text: qsTr("Cluster the wallpaper's pixels (k-means) — more colours to choose from")
            }
            AppButton {
                text: qsTr("Import file…")
                icon.name: "document-import"
                enabled: !paletteController.busy
                onClicked: fileDialog.open()
            }
            BusyIndicator {
                running: paletteController.busy
                visible: running
                implicitHeight: Platform.Units.iconSizes.medium
            }
        }

        Kirigami.PlaceholderMessage {
            Layout.fillWidth: true
            visible: !paletteController.hasPalette
            icon.name: "color-management"
            text: qsTr("No palette yet")
            explanation: qsTr("Choose a scheme in the gallery or build one from the wallpaper.")
        }

        // ── Current palette ───────────────────────────────────────────
        Kirigami.Card {
            Layout.fillWidth: true
            visible: paletteController.hasPalette
            background: Rectangle {
                color: ThemeColors.cardBg(page.glassMode)
                radius: Platform.Units.smallSpacing
                border.width: 1
                border.color: ThemeColors.cardBorder(page.glassMode)
            }

            contentItem: ColumnLayout {
                spacing: Platform.Units.mediumSpacing

                RowLayout {
                    Layout.fillWidth: true
                    Kirigami.Heading {
                        Layout.fillWidth: true
                        level: 2
                        text: page.pal.name || ""
                        color: ThemeColors.fgColor(page.glassMode)
                        elide: Text.ElideRight
                    }
                    Label {
                        text: page.pal.variant === "light" ? qsTr("Light") : qsTr("Dark")
                        color: ThemeColors.dimColor(page.glassMode)
                    }
                }
                Label {
                    Layout.fillWidth: true
                    text: qsTr("Source: %1 · KDE scheme: %2").arg(page.pal.sourceText || "—").arg(page.pal.kdeScheme || "—")
                    color: ThemeColors.dimColor(page.glassMode)
                    font.pointSize: TypeScale.caption
                    elide: Text.ElideMiddle
                }

                Kirigami.InlineMessage {
                    Layout.fillWidth: true
                    type: Kirigami.MessageType.Warning
                    visible: (page.pal.warnings || []).length > 0
                    text: qsTr("Readability: %1").arg((page.pal.warnings || []).join("; "))
                }

                GridLayout {
                    Layout.fillWidth: true
                    columns: Math.max(2, Math.min(8, Math.floor(width / (Platform.Units.gridUnit * 5))))
                    columnSpacing: Platform.Units.smallSpacing
                    rowSpacing: Platform.Units.smallSpacing

                    Repeater {
                        model: page.slots
                        delegate: ColumnLayout {
                            required property int index
                            required property string modelData
                            readonly property string colorValue: (page.pal.base || [])[index] || "#000000"
                            Layout.fillWidth: true
                            spacing: 2

                            ColorSwatch {
                                objectName: "paletteSwatch_" + modelData
                                Layout.fillWidth: true
                                Layout.minimumWidth: 0
                                swatchColor: parent.colorValue
                                glassMode: page.glassMode
                                text: qsTr("%1 (%2): %3").arg(modelData).arg(page.slotHints[index]).arg(parent.colorValue)
                                enabled: !paletteController.busy
                                onClicked: page.editSlot(modelData, parent.colorValue)
                            }
                            Label {
                                Layout.fillWidth: true
                                text: modelData.slice(4) + " " + parent.colorValue
                                font.family: "monospace"
                                font.pointSize: TypeScale.caption
                                color: ThemeColors.dimColor(page.glassMode)
                                elide: Text.ElideRight
                            }
                        }
                    }
                }

                RowLayout {
                    spacing: Platform.Units.mediumSpacing
                    ColorSwatch {
                        objectName: "paletteAccent"
                        Layout.preferredWidth: Platform.Units.gridUnit * 4
                        swatchColor: page.pal.accent || "#000000"
                        glassMode: page.glassMode
                        text: qsTr("Accent: %1").arg(page.pal.accent || "")
                        enabled: !paletteController.busy
                        onClicked: page.editSlot("accent", page.pal.accent)
                    }
                    Label {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        wrapMode: Text.Wrap
                        text: qsTr("Accent %1 — selection, focus, active elements").arg(page.pal.accent || "")
                        color: ThemeColors.fgColor(page.glassMode)
                    }
                }
            }
        }

        // ── Preview ───────────────────────────────────────────────────
        Kirigami.Card {
            Layout.fillWidth: true
            visible: paletteController.hasPalette
            background: Rectangle {
                color: ThemeColors.cardBg(page.glassMode)
                radius: Platform.Units.smallSpacing
                border.width: 1
                border.color: ThemeColors.cardBorder(page.glassMode)
            }
            contentItem: ColumnLayout {
                spacing: Platform.Units.smallSpacing
                Kirigami.Heading {
                    level: 3
                    text: qsTr("Preview")
                    color: ThemeColors.fgColor(page.glassMode)
                }
                PalettePreview {
                    Layout.fillWidth: true
                    base: page.pal.base || []
                    accent: page.pal.accent || "#3daee9"
                    glassMode: page.glassMode
                }
            }
        }

        // ── Apply ─────────────────────────────────────────────────────
        Kirigami.Card {
            Layout.fillWidth: true
            visible: paletteController.hasPalette
            background: Rectangle {
                color: ThemeColors.cardBg(page.glassMode)
                radius: Platform.Units.smallSpacing
                border.width: 1
                border.color: ThemeColors.cardBorder(page.glassMode)
            }
            contentItem: ColumnLayout {
                spacing: Platform.Units.smallSpacing
                Kirigami.Heading {
                    level: 3
                    text: qsTr("Apply to")
                    color: ThemeColors.fgColor(page.glassMode)
                }
                Flow {
                    Layout.fillWidth: true
                    spacing: Platform.Units.largeSpacing
                    CheckBox { id: kdeTarget; text: qsTr("KDE colour scheme"); checked: true }
                    CheckBox { id: kittyTarget; text: qsTr("kitty"); checked: true }
                    CheckBox { id: alacrittyTarget; text: qsTr("Alacritty"); checked: true }
                    CheckBox { id: konsoleTarget; text: qsTr("Konsole"); checked: true }
                    CheckBox { id: discordTarget; text: qsTr("Discord (Vencord / Vesktop)"); checked: true }
                    CheckBox { id: spicetifyTarget; text: qsTr("Spotify (Spicetify)"); checked: true }
                    CheckBox { id: ghosttyTarget; text: qsTr("Ghostty"); checked: true }
                    CheckBox { id: cavaTarget; text: qsTr("cava"); checked: true }
                    CheckBox {
                        id: kvantumTarget
                        text: qsTr("Kvantum (switches the widget style)")
                        checked: false
                    }
                }
                Label {
                    Layout.fillWidth: true
                    text: qsTr("Terminal configs are backed up first; only their colour block changes. Kvantum recolours an installed base theme (KvArcDark / KvArc).")
                    wrapMode: Text.WordWrap
                    color: ThemeColors.dimColorSoft(page.glassMode)
                    font.pointSize: TypeScale.caption
                }
                AppButton {
                    Layout.alignment: Qt.AlignRight
                    text: qsTr("Apply palette")
                    icon.name: "dialog-ok-apply"
                    highlighted: true
                    enabled: !paletteController.busy && page.selectedTargets().length > 0
                    onClicked: paletteController.apply(page.selectedTargets(), "")
                }
            }
        }
    }

    // ── Gallery ───────────────────────────────────────────────────────
    AppDialog {
        id: galleryDialog
        title: qsTr("Palette gallery")
        preferredWidth: Platform.Units.gridUnit * 30
        standardButtons: Kirigami.Dialog.Close

        function openGallery() {
            open()
            paletteController.searchGallery(searchField.text, onlineBox.checked)
            searchField.forceActiveFocus()
        }

        ColumnLayout {
            spacing: Platform.Units.smallSpacing
            RowLayout {
                Layout.fillWidth: true
                TextField {
                    id: searchField
                    Layout.fillWidth: true
                    placeholderText: qsTr("Search, e.g. gruvbox")
                    onTextChanged: searchTimer.restart()
                }
                CheckBox {
                    id: onlineBox
                    text: qsTr("Online catalogue")
                    onToggled: paletteController.searchGallery(searchField.text, checked)
                    ToolTip.visible: hovered
                    ToolTip.text: qsTr("~360 schemes from tinted-theming; needs network (can be disabled in settings)")
                }
            }
            Timer {
                id: searchTimer
                interval: 300
                onTriggered: paletteController.searchGallery(searchField.text, onlineBox.checked)
            }
            Repeater {
                model: paletteController.galleryItems
                delegate: ItemDelegate {
                    required property var modelData
                    Layout.fillWidth: true
                    text: (modelData.name || modelData.id)
                    enabled: !paletteController.busy
                    contentItem: RowLayout {
                        Label {
                            Layout.fillWidth: true
                            text: modelData.name || modelData.id
                            elide: Text.ElideRight
                        }
                        Label {
                            text: modelData.variant === "light" ? qsTr("light")
                                : modelData.variant === "dark" ? qsTr("dark") : ""
                            color: ThemeColors.dimColor(page.glassMode)
                        }
                        Label {
                            text: modelData.origin === "bundled" ? qsTr("built-in")
                                : modelData.origin === "cached" ? qsTr("downloaded") : qsTr("online")
                            color: ThemeColors.dimColorSoft(page.glassMode)
                            font.pointSize: TypeScale.caption
                        }
                    }
                    onClicked: {
                        paletteController.importGallery(modelData.id)
                        galleryDialog.close()
                    }
                }
            }
        }
    }
}
