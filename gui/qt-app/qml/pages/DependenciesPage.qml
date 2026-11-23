import Components 1.0
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami
import org.kde.kirigami.platform as Platform

Kirigami.Page {
    id: page

    readonly property bool glassMode: themeController.isGlass
    property var chosen: ({
    })
    property string themeFilter: ""
    property var reviewOptions: null
    signal reviewRequested(var options)
    readonly property var visibleRows: dependencyController.rows.filter(function(row) {
        return (!missingOnly.checked || !row.installed) && (scope.currentIndex !== 1 || row.application) && (themeFilter === "" || row.themes.indexOf(themeFilter) >= 0) && (search.text === "" || (row.dependency.title + " " + row.dependency.name).toLowerCase().indexOf(search.text.toLowerCase()) >= 0);
    })

    readonly property var groups: {
        const groups = []
        const common = visibleRows.filter(row => row.application)
        if (themeFilter === "" && common.length)
            groups.push({title: qsTr("Common dependencies"), rows: common})
        if (scope.currentIndex === 1)
            return groups
        const themes = new Map()
        const unused = []
        for (const row of visibleRows) {
            if (themeFilter === "" && row.application)
                continue
            const names = themeFilter ? [themeFilter] : row.themes
            if (!names.length) {
                unused.push(row)
                continue
            }
            for (const name of names) {
                if (!themes.has(name))
                    themes.set(name, [])
                themes.get(name).push(row)
            }
        }
        for (const name of [...themes.keys()].sort((a, b) => a.localeCompare(b)))
            groups.push({title: name, rows: themes.get(name)})
        if (unused.length)
            groups.push({title: qsTr("Unused dependencies"), rows: unused})
        return groups
    }

    function selection() {
        return visibleRows.filter(row => chosen[row.id] === true);
    }

    function installSelection() {
        return selection().filter(row => !row.installed && row.can_install)
    }

    function eligible(action) {
        const selected = selection()
        return selected.length > 0 && selected.every(row => action === "remove"
            ? row.can_remove : row.can_install && row.can_remove)
    }

    function select(id, checked) {
        const next = Object.assign({
        }, chosen);
        next[id] = checked;
        chosen = next;
    }

    function category(kind) {
        const labels = {
            "icons": qsTr("Icons"),
            "cursors": qsTr("Cursors"),
            "gtk": qsTr("GTK theme"),
            "kvantum": qsTr("Kvantum"),
            "plasma-theme": qsTr("Plasma style"),
            "look-and-feel": qsTr("Global theme"),
            "color-scheme": qsTr("Colours"),
            "aurorae": qsTr("Window decoration"),
            "decoration": qsTr("Window decoration"),
            "plasmoid": qsTr("Widget"),
            "kwin-effect": qsTr("KWin effect"),
            "widget-style": qsTr("Widget style"),
            "zed": qsTr("Zed theme"),
            "vscode": qsTr("VS Code theme"),
            "cursor": qsTr("Cursor theme"),
            "python": qsTr("Application runtime"),
            "pyside6": qsTr("Application runtime"),
            "command": qsTr("System tool"),
            "qml-module": qsTr("QML module")
        };
        return labels[kind] || kind;
    }

    function sourceLabel(source) {
        const labels = {
            "runtime": qsTr("Application"),
            "package": qsTr("System package"),
            "aur": "AUR",
            "kde-store": "KDE Store",
            "zed": "Zed",
            "vscode-marketplace": "VS Code Marketplace",
            "open-vsx": "Open VSX",
            "none": qsTr("Manual installation")
        };
        return labels[source] || source;
    }

    function removalReason(reason) {
        const labels = {
            "Required by the application": qsTr("Maintained by the application installer"),
            "No verified manager-owned files": qsTr("No verified manager-owned files"),
            "Files changed or location is unsafe": qsTr("Files changed or the location is protected")
        };
        return labels[reason] || reason;
    }

    function askRemoval(action) {
        removal.action = action;
        removal.ids = selection().map(function(row) {
            return row.id;
        });
        removal.names = selection().map(function(row) {
            return row.dependency.title || row.dependency.name;
        }).join(", ");
        let themes = [];
        selection().forEach(function(row) {
            row.themes.forEach(function(name) {
                if (themes.indexOf(name) < 0)
                    themes.push(name);

            });
        });
        removal.themes = themes.join(", ");
        removal.open();
    }

    objectName: "dependenciesPage"
    title: qsTr("Dependencies")
    Component.onCompleted: dependencyController.refresh()

    ColumnLayout {
        anchors.fill: parent
        spacing: Platform.Units.mediumSpacing

        Flow {
            Layout.fillWidth: true
            spacing: Platform.Units.smallSpacing

            AppButton {
                text: qsTr("Review theme changes")
                icon.name: "dialog-ok-apply"
                visible: page.reviewOptions !== null
                enabled: !dependencyController.busy && !profileController.busy
                onClicked: page.reviewRequested(page.reviewOptions)
            }

            AppButton {
                text: qsTr("Install selected")
                icon.name: "download"
                enabled: !dependencyController.busy && !profileController.busy && page.installSelection().length > 0
                onClicked: dependencyController.operate(page.installSelection().map(function(row) {
                    return row.id;
                }), "install", false)
            }

            AppButton {
                objectName: "deleteSelectedDependencies"
                text: qsTr("Delete selected")
                icon.name: "edit-delete"
                enabled: !dependencyController.busy && !profileController.busy && page.eligible("remove")
                onClicked: page.askRemoval("remove")
            }

            AppButton {
                text: qsTr("Reinstall")
                icon.name: "view-refresh"
                enabled: !dependencyController.busy && !profileController.busy && page.eligible("reinstall")
                onClicked: page.askRemoval("reinstall")
            }

            AppButton {
                text: qsTr("Check status")
                icon.name: "view-refresh"
                enabled: !dependencyController.busy && !profileController.busy
                onClicked: dependencyController.refresh()
            }

            AppButton {
                text: qsTr("Stop")
                icon.name: "process-stop"
                enabled: dependencyController.canCancel
                onClicked: dependencyController.cancel()
            }

        }

        Flow {
            id: filterBar

            Layout.fillWidth: true
            spacing: Platform.Units.smallSpacing

            ComboBox {
                id: scope

                width: Math.min(filterBar.width, Math.max(implicitWidth, Platform.Units.gridUnit * 11))
                model: [qsTr("All dependencies"), qsTr("Application"), qsTr("Theme")]
                Component.onCompleted: {
                    if (page.themeFilter !== "")
                        currentIndex = 2
                }
                onActivated: page.themeFilter = currentIndex === 2 ? themeChoice.currentText : ""
                Accessible.name: qsTr("Show dependencies for")
            }

            ComboBox {
                id: themeChoice

                width: Math.min(filterBar.width, Math.max(implicitWidth, Platform.Units.gridUnit * 11))
                visible: scope.currentIndex === 2
                model: profileController.profiles.map(function(p) {
                    return p.name;
                })
                currentIndex: page.themeFilter === "" ? 0 : find(page.themeFilter)
                onActivated: page.themeFilter = currentText
                Accessible.name: qsTr("Theme")
            }

            CheckBox {
                id: missingOnly

                text: qsTr("Missing only")
            }

            TextField {
                id: search

                width: filterBar.width
                placeholderText: qsTr("Search dependencies")
                Accessible.name: placeholderText
            }

        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: false
            visible: dependencyController.busy || dependencyController.message !== ""

            BusyIndicator {
                running: dependencyController.busy
                visible: running
                Layout.preferredWidth: Platform.Units.iconSizes.smallMedium
                Layout.preferredHeight: Layout.preferredWidth
            }

            Label {
                Layout.fillWidth: true
                text: dependencyController.message
                textFormat: Text.PlainText
                wrapMode: Text.WordWrap
                visible: text !== ""
                color: ThemeColors.fgColor(page.glassMode)
                Accessible.role: Accessible.AlertMessage
            }

        }

        InsetScrollView {
            id: table
            objectName: "dependenciesTable"
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumWidth: 0
            contentWidth: availableWidth
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
            ScrollBar.vertical: ScrollBar {
                objectName: "dependenciesVerticalBar"
                parent: table
                anchors.top: table.top
                anchors.right: table.right
                anchors.bottom: table.bottom
                policy: ScrollBar.AsNeeded
            }

            ColumnLayout {
                width: table.availableWidth
                spacing: Kirigami.Units.largeSpacing

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    visible: page.groups.length === 0 && !dependencyController.busy
                    text: qsTr("No dependencies to show.")
                }
                Repeater {
                    model: page.groups
                    delegate: ColumnLayout {
                        id: dependencyGroup
                        required property var modelData
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        spacing: Kirigami.Units.smallSpacing

                        Kirigami.Heading {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            level: 3
                            text: dependencyGroup.modelData.title
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                        }
                        Repeater {
                            model: dependencyGroup.modelData.rows
                            delegate: RowLayout {
                                id: resourceRow
                                required property var modelData
                                Layout.fillWidth: true
                                Layout.minimumWidth: 0

                                CheckBox {
                                    objectName: "dependencySelect_" + resourceRow.modelData.id
                                    checked: page.chosen[resourceRow.modelData.id] === true
                                    onToggled: page.select(resourceRow.modelData.id, checked)
                                    Accessible.name: qsTr("Select %1").arg(resourceRow.modelData.dependency.name)
                                }
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    Layout.minimumWidth: 0
                                    spacing: 0
                                    Label {
                                        Layout.fillWidth: true
                                        Layout.minimumWidth: 0
                                        textFormat: Text.PlainText
                                        wrapMode: Text.Wrap
                                        text: resourceRow.modelData.dependency.title || resourceRow.modelData.dependency.name
                                    }
                                    Label {
                                        Layout.fillWidth: true
                                        Layout.minimumWidth: 0
                                        wrapMode: Text.Wrap
                                        textFormat: Text.PlainText
                                        color: ThemeColors.dimColor(page.glassMode)
                                        text: {
                                            const row = resourceRow.modelData
                                            const parts = [row.installed ? qsTr("Installed") : qsTr("Missing"),
                                                           page.sourceLabel(row.dependency.source)]
                                            if (row.dependency.id)
                                                parts.push(row.dependency.id)
                                            if (row.version)
                                                parts.push(row.version)
                                            return parts.filter(part => part).join(" · ")
                                        }
                                    }
                                }
                                ToolButton {
                                    text: qsTr("Details")
                                    icon.name: "dialog-information"
                                    Accessible.name: text
                                    ToolTip.text: text
                                    ToolTip.visible: hovered
                                    onClicked: {
                                        details.resource = resourceRow.modelData
                                        details.open()
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        InsetScrollView {
            id: operationLog

            objectName: "dependenciesOperationLog"
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(120, logText.implicitHeight)
            Layout.maximumHeight: 120
            visible: dependencyController.log.length > 0
            clip: true
            contentWidth: availableWidth
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

            Label {
                id: logText

                width: operationLog.availableWidth
                text: dependencyController.log.map(function(item) {
                    return (item.ok ? "✓ " : "✗ ") + item.message;
                }).join("\n")
                textFormat: Text.PlainText
                wrapMode: Text.WordWrap
                color: ThemeColors.fgColor(page.glassMode)
            }

        }

    }

    AppDialog {
        id: removal

        property string action: "remove"
        property var ids: []
        property string names: ""
        property string themes: ""

        title: action === "remove" ? qsTr("Delete selected dependencies?") : qsTr("Reinstall selected dependencies?")
        preferredWidth: 560
        standardButtons: Kirigami.Dialog.Cancel

        Label {
            text: removal.names + (removal.themes ? "\n\n" + qsTr("Affected themes: %1").arg(removal.themes) : "")
            wrapMode: Text.WordWrap
            textFormat: Text.PlainText
            color: ThemeColors.fgColor(page.glassMode)
        }

        customFooterActions: Kirigami.Action {
            text: removal.action === "remove" ? qsTr("Delete") : qsTr("Reinstall")
            icon.name: "edit-delete"
            enabled: !dependencyController.busy
            onTriggered: {
                dependencyController.operate(removal.ids, removal.action, true);
                page.chosen = ({
                });
                removal.close();
            }
        }

    }

    AppDialog {
        id: details

        property var resource: null

        title: resource ? resource.dependency.name : ""
        preferredWidth: 560
        standardButtons: Kirigami.Dialog.Close

        ColumnLayout {
            Label {
                Layout.fillWidth: true
                text: details.resource ? [page.category(details.resource.dependency.kind),
                    page.sourceLabel(details.resource.dependency.source), details.resource.dependency.id,
                    details.resource.version].filter(part => part).join(" · ") : ""
                wrapMode: Text.Wrap
                textFormat: Text.PlainText
            }
            Label {
                Layout.fillWidth: true
                visible: text !== ""
                text: details.resource ? page.removalReason(details.resource.removal_reason) : ""
                wrapMode: Text.WordWrap
                textFormat: Text.PlainText
            }

            Repeater {
                model: details.resource ? details.resource.instances : []

                Label {
                    required property var modelData

                    Layout.fillWidth: true
                    text: (modelData.ownership === "managed" ? qsTr("Installed by the manager") : qsTr("External resource")) + "\n" + modelData.path
                    textFormat: Text.PlainText
                    wrapMode: Text.WrapAnywhere
                }

            }

            Label {
                Layout.fillWidth: true
                text: details.resource ? details.resource.last_error : ""
                textFormat: Text.PlainText
                wrapMode: Text.WordWrap
                visible: text !== ""
            }

        }

    }

    background: Rectangle {
        color: page.glassMode ? ThemeColors.glassPanel() : ThemeColors.pageBg(page.glassMode)
    }

}
