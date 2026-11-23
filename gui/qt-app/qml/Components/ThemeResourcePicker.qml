import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami

ColumnLayout {
    id: picker
    required property var setting
    property bool editable: true
    property bool expanded: false
    signal selected(var value)
    readonly property var previews: themeBundle.resourcePreviews[themeBundle.scope + "/" + setting.id] || ({})
    readonly property var filtered: expanded ? setting.choices.filter(choice =>
        (choice.label || "").toLowerCase().includes(search.text.trim().toLowerCase())) : []
    Layout.fillWidth: true
    Layout.minimumWidth: 0

    function key(value) {
        if (value && typeof value === "object")
            return value.disabled ? "" : (value.theme || "")
        return String(value ?? "")
    }

    function matches(value) {
        const selected = setting.currentValue
        if (value && selected && typeof value === "object" && typeof selected === "object")
            return !!value.disabled === !!selected.disabled && value.library === selected.library
                && value.theme === selected.theme
        return value === selected
    }

    function requestCurrent() {
        if (visible && !expanded && setting.currentValue)
            themeBundle.requestResourcePreviews(setting.id, [key(setting.currentValue)])
    }
    Component.onCompleted: requestCurrent()
    onSettingChanged: requestCurrent()

    RowLayout {
        Layout.fillWidth: true
        Image {
            id: currentIcon
            Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
            Layout.preferredHeight: Layout.preferredWidth
            source: picker.visible ? (picker.previews[picker.key(picker.setting.currentValue)] || "") : ""
            sourceSize: Qt.size(48, 48)
            asynchronous: true
            fillMode: Image.PreserveAspectFit
            visible: status === Image.Ready
        }
        AppButton {
            objectName: "openResources_" + picker.setting.id
            Layout.maximumWidth: picker.width
            text: qsTr("Browse resources")
            icon.name: picker.expanded ? "go-up" : "go-down"
            enabled: picker.editable && picker.setting.choices.length > 0
            onClicked: {
                picker.expanded = !picker.expanded
                if (picker.expanded) {
                    themeBundle.requestResourcePreviews(picker.setting.id, picker.setting.choices.map(choice => picker.key(choice.value)))
                    search.forceActiveFocus()
                }
            }
        }
    }

    TextField {
        id: search
        objectName: "resourceSearch_" + picker.setting.id
        Layout.fillWidth: true
        visible: picker.expanded
        placeholderText: qsTr("Search resources")
        Accessible.name: placeholderText
        Keys.onDownPressed: gallery.forceActiveFocus()
    }

    Label {
        Layout.fillWidth: true
        visible: picker.expanded && !picker.filtered.length
        text: qsTr("No matching resources")
        wrapMode: Text.Wrap
    }

    GridView {
        id: gallery
        objectName: "resourceGallery_" + picker.setting.id
        visible: picker.expanded && picker.filtered.length > 0
        Layout.fillWidth: true
        Layout.preferredHeight: cellHeight * Math.min(3, Math.ceil(picker.filtered.length / columns))
        Layout.minimumWidth: 0
        clip: true
        keyNavigationEnabled: true
        activeFocusOnTab: true
        readonly property int columns: Math.max(1, Math.floor(picker.width / (Kirigami.Units.gridUnit * 7)))
        cellWidth: Math.max(1, Math.floor(width / columns))
        cellHeight: Kirigami.Units.gridUnit * 4
        model: picker.filtered
        Keys.onReturnPressed: {
            if (currentIndex >= 0 && picker.editable)
                picker.selected(picker.filtered[currentIndex].value)
        }
        Keys.onSpacePressed: {
            if (currentIndex >= 0 && picker.editable)
                picker.selected(picker.filtered[currentIndex].value)
        }
        ScrollBar.vertical: ScrollBar {}

        delegate: AppButton {
            id: tile
            required property var modelData
            width: Math.max(1, gallery.cellWidth - Kirigami.Units.smallSpacing)
            height: gallery.cellHeight - Kirigami.Units.smallSpacing
            checkable: true
            checked: picker.matches(modelData.value)
            enabled: picker.editable
            highlighted: GridView.isCurrentItem && gallery.activeFocus
            Accessible.name: modelData.label
            onClicked: picker.selected(modelData.value)
            contentItem: ColumnLayout {
                Image {
                    id: actualImage
                    Layout.alignment: Qt.AlignHCenter
                    Layout.preferredWidth: Kirigami.Units.iconSizes.medium
                    Layout.preferredHeight: Layout.preferredWidth
                    source: picker.previews[picker.key(tile.modelData.value)] || ""
                    sourceSize: Qt.size(64, 64)
                    asynchronous: true
                    fillMode: Image.PreserveAspectFit
                    visible: status === Image.Ready
                }
                Kirigami.Icon {
                    Layout.alignment: Qt.AlignHCenter
                    Layout.preferredWidth: Kirigami.Units.iconSizes.medium
                    Layout.preferredHeight: Layout.preferredWidth
                    source: "image-missing"
                    visible: !actualImage.visible
                }
                Label {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    text: tile.modelData.label
                    textFormat: Text.PlainText
                    horizontalAlignment: Text.AlignHCenter
                    elide: Text.ElideRight
                }
            }
        }
    }
}
