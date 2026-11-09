import QtQuick
import org.kde.kirigami.platform as Platform
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami

Kirigami.Card {
    id: group

    function categoryIcon(cat) {
        const icons = {
            "KDE / Plasma": "plasma",
            "GTK / Qt": "gtk-preferences",
            "Window Decorations": "preferences-system-windows",
            "Terminals": "utilities-terminal",
            "Launchers / Menu": "system-run",
            "Monitors": "utilities-system-monitor",
            "Sysinfo": "preferences-desktop-theme",
            "Editors": "text-editor",
            "Browser": "web-browser",
            "Notes": "x-office-document",
            "System Profiles": "preferences-desktop-theme"
        }
        return icons[cat] || "application-x-executable"
    }

    Layout.fillWidth: true
    property string category: ""
    // Input: the app-wide selection (bound by the parent). Never write to this.
    property var selectedComponents: []
    property var availableComponents: []
    property bool glassMode: false
    property bool expanded: true

    // Local selection state (mirrors selectedComponents but mutable)
    property var _localSelection: []

    // Carries the new full selection; the parent propagates it to the controller
    // (e.g. settingsController.customComponents = components).
    signal selectionChanged(var components)

    onSelectedComponentsChanged: _localSelection = selectedComponents.slice()
    Component.onCompleted: _localSelection = selectedComponents.slice()

    contentItem: ColumnLayout {
        spacing: Platform.Units.smallSpacing

        // Category header
        RowLayout {
            Layout.fillWidth: true

            Kirigami.Icon {
                source: group.categoryIcon(group.category)
                Layout.preferredWidth: Platform.Units.iconSizes.medium
                Layout.preferredHeight: Platform.Units.iconSizes.medium
                color: ThemeColors.accent()
            }

            Kirigami.Heading {
                level: 3
                text: group.category
                color: ThemeColors.fgColor(group.glassMode)
            }

            Item { Layout.fillWidth: true }

            AppButton {
                id: expandButton
                flat: true
                readonly property string actionLabel: group.expanded ? qsTr("Collapse") : qsTr("Expand")
                icon.name: group.expanded ? "go-up" : "go-down"
                Accessible.name: actionLabel
                ToolTip.text: actionLabel
                ToolTip.visible: hovered
                onClicked: group.expanded = !group.expanded
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 1
            color: ThemeColors.subtleBorder(group.glassMode)
        }

        // Components list (collapsible)
        ColumnLayout {
            id: componentsLayout
            Layout.fillWidth: true
            clip: true
            spacing: Platform.Units.smallSpacing
            Layout.preferredHeight: group.expanded ? implicitHeight : 0
            // Keep it visible while it collapses as well, so the animation is
            // actually painted (a pure `Layout.preferredHeight > 0` test can
            // deadlock against the implicitHeight the layout computes).
            visible: group.expanded || Layout.preferredHeight > 0

            Behavior on Layout.preferredHeight {
                NumberAnimation { duration: 200; easing.type: Easing.OutCubic }
            }

            Repeater {
                model: {
                    // Establishes a dependency on the async registry refresh
                    // so the model re-evaluates once components are loaded.
                    var _revision = trackedAppsController.componentsRevision
                    return trackedAppsController.getComponentsByCategory(group.category)
                }
                delegate: ComponentCheckbox {
                    glassMode: group.glassMode
                    // `getComponentsByCategory()` returns a list of maps, which
                    // Qt 6.12 only exposes via `modelData` (not `model.<key>`).
                    componentId: modelData.id
                    componentName: modelData.name
                    componentStatus: modelData.status
                    componentStatusText: modelData.statusText
                    componentDescription: modelData.description
                    componentIcon: modelData.icon
                    componentSupportsLiveReload: modelData.supportsLiveReload
                    isSelected: group._localSelection.indexOf(modelData.id) !== -1
                    isAvailable: group.availableComponents.indexOf(modelData.id) !== -1
                    onComponentToggled: function(checked, componentId) {
                        var next = group._localSelection.slice()
                        var idx = next.indexOf(componentId)
                        if (checked && idx === -1) {
                            next.push(componentId)
                        } else if (!checked && idx !== -1) {
                            next.splice(idx, 1)
                        }
                        // Reassign instead of mutating in place: an in-place
                        // array mutation does not notify, so the delegates'
                        // `isSelected` bindings (and therefore their checked
                        // state) would never re-evaluate.
                        group._localSelection = next
                        group.selectionChanged(next.slice())
                    }
                }
            }
        }
    }

    background: Rectangle {
        radius: 6
        border.width: 1
        color: ThemeColors.cardBg(group.glassMode)
        border.color: ThemeColors.cardBorder(group.glassMode)
    }
}
