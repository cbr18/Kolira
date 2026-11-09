import Components 1.0
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami
import org.kde.kirigami.platform as Platform

Kirigami.Page {
    id: page

    readonly property bool glassMode: themeController.isGlass
    readonly property real rowPadding: Platform.Units.largeSpacing
    readonly property real selectWidth: Platform.Units.gridUnit * 2.2
    readonly property real nameWidth: Platform.Units.gridUnit * 12
    readonly property real categoryWidth: Platform.Units.gridUnit * 8
    readonly property real statusWidth: Platform.Units.gridUnit * 7
    readonly property real sourceWidth: Platform.Units.gridUnit * 9
    readonly property real usedWidth: Platform.Units.gridUnit * 11
    property var chosen: ({
    })
    property string themeFilter: ""
    readonly property var visibleRows: dependencyController.rows.filter(function(row) {
        return (!missingOnly.checked || !row.installed) && (scope.currentIndex !== 1 || row.application) && (themeFilter === "" || row.themes.indexOf(themeFilter) >= 0) && (search.text === "" || (row.dependency.title + " " + row.dependency.name).toLowerCase().indexOf(search.text.toLowerCase()) >= 0);
    })

    function selection() {
        return visibleRows.filter(function(row) {
            return chosen[row.id] === true;
        });
    }

    function eligible(action) {
        const selected = selection();
        return selected.length > 0 && selected.every(function(row) {
            return action === "install" ? row.can_install : action === "remove" ? row.can_remove : row.can_install && row.can_remove;
        });
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
                text: qsTr("Install selected")
                icon.name: "download"
                enabled: !dependencyController.busy && !profileController.busy && page.eligible("install")
                onClicked: dependencyController.operate(page.selection().map(function(row) {
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

        Label {
            Layout.fillWidth: true
            text: qsTr("Select resources with the checkboxes. Existing external resources and modified files are protected from deletion.")
            wrapMode: Text.WordWrap
            color: ThemeColors.dimColor(page.glassMode)
        }

        Item {
            id: tableViewport

            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumWidth: 0

            Flickable {
                id: table

                objectName: "dependenciesTable"
                anchors.fill: parent
                anchors.rightMargin: tableBar.implicitWidth + Platform.Units.smallSpacing
                contentWidth: Math.max(width, page.selectWidth + page.nameWidth + page.categoryWidth + page.statusWidth + page.sourceWidth + page.usedWidth + 5 * Platform.Units.smallSpacing + 2 * page.rowPadding)
                contentHeight: height
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                flickableDirection: Flickable.HorizontalFlick

                ColumnLayout {
                    width: table.contentWidth
                    height: Math.max(0, table.height - (horizontalBar.visible ? horizontalBar.implicitHeight + Platform.Units.smallSpacing : 0))
                    spacing: Platform.Units.smallSpacing

                    RowLayout {
                        id: tableHeader

                        Layout.fillWidth: true
                        Layout.leftMargin: page.rowPadding
                        Layout.rightMargin: page.rowPadding
                        Layout.fillHeight: false
                        spacing: Platform.Units.smallSpacing

                        CheckBox {
                            id: selectAll

                            objectName: "selectAllDependencies"
                            Layout.preferredWidth: page.selectWidth
                            Layout.minimumWidth: page.selectWidth
                            Layout.maximumWidth: page.selectWidth
                            tristate: true
                            checkState: page.selection().length === 0 ? Qt.Unchecked : page.selection().length === page.visibleRows.length ? Qt.Checked : Qt.PartiallyChecked
                            nextCheckState: function() {
                                return checkState === Qt.Checked ? Qt.Unchecked : Qt.Checked;
                            }
                            onClicked: {
                                const next = Object.assign({
                                }, page.chosen);
                                const check = page.selection().length !== page.visibleRows.length;
                                page.visibleRows.forEach(function(row) {
                                    next[row.id] = check;
                                });
                                page.chosen = next;
                            }
                            Accessible.name: qsTr("Select all visible dependencies")

                            Binding {
                                target: selectAll
                                property: "checkState"
                                value: page.selection().length === 0 ? Qt.Unchecked : page.selection().length === page.visibleRows.length ? Qt.Checked : Qt.PartiallyChecked
                            }

                        }

                        Label {
                            wrapMode: Text.Wrap
                            text: qsTr("Name")
                            Layout.preferredWidth: page.nameWidth
                            Layout.minimumWidth: page.nameWidth
                            Layout.maximumWidth: page.nameWidth
                            color: ThemeColors.fgColor(page.glassMode)
                        }

                        Label {
                            wrapMode: Text.Wrap
                            text: qsTr("Category")
                            Layout.preferredWidth: page.categoryWidth
                            Layout.minimumWidth: page.categoryWidth
                            Layout.maximumWidth: page.categoryWidth
                            color: ThemeColors.fgColor(page.glassMode)
                        }

                        Label {
                            wrapMode: Text.Wrap
                            text: qsTr("Status")
                            Layout.preferredWidth: page.statusWidth
                            Layout.minimumWidth: page.statusWidth
                            Layout.maximumWidth: page.statusWidth
                            color: ThemeColors.fgColor(page.glassMode)
                        }

                        Label {
                            wrapMode: Text.Wrap
                            text: qsTr("Source")
                            Layout.preferredWidth: page.sourceWidth
                            Layout.minimumWidth: page.sourceWidth
                            Layout.maximumWidth: page.sourceWidth
                            color: ThemeColors.fgColor(page.glassMode)
                        }

                        Label {
                            wrapMode: Text.Wrap
                            text: qsTr("Used by")
                            Layout.fillWidth: true
                            color: ThemeColors.fgColor(page.glassMode)
                        }

                    }

                    ListView {
                        id: list

                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        model: page.visibleRows
                        clip: true
                        spacing: Platform.Units.smallSpacing

                        Label {
                            anchors.centerIn: parent
                            visible: list.count === 0 && !dependencyController.busy
                            text: qsTr("No dependencies match these filters")
                            color: ThemeColors.dimColor(page.glassMode)
                        }

                        ScrollBar.vertical: ScrollBar {
                            id: tableBar

                            objectName: "dependenciesVerticalBar"
                            parent: tableViewport
                            anchors.right: parent.right
                            anchors.top: parent.top
                            anchors.topMargin: tableHeader.height + Platform.Units.smallSpacing
                            anchors.bottom: parent.bottom
                            anchors.bottomMargin: horizontalBar.visible ? horizontalBar.implicitHeight + Platform.Units.smallSpacing : 0
                        }

                        delegate: Rectangle {
                            id: row

                            required property var modelData

                            width: list.width
                            implicitHeight: rowContent.implicitHeight + 2 * Platform.Units.smallSpacing
                            color: ThemeColors.cardBg(page.glassMode)
                            radius: Platform.Units.smallSpacing

                            RowLayout {
                                id: rowContent

                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.verticalCenter: parent.verticalCenter
                                anchors.leftMargin: page.rowPadding
                                anchors.rightMargin: page.rowPadding
                                spacing: Platform.Units.smallSpacing

                                CheckBox {
                                    id: selected

                                    objectName: "dependencySelect_" + row.modelData.id
                                    Layout.preferredWidth: page.selectWidth
                                    Layout.minimumWidth: page.selectWidth
                                    Layout.maximumWidth: page.selectWidth
                                    checked: page.chosen[row.modelData.id] === true
                                    onToggled: page.select(row.modelData.id, checked)
                                    Accessible.name: qsTr("Select %1").arg(row.modelData.dependency.title || row.modelData.dependency.name)

                                    Binding {
                                        target: selected
                                        property: "checked"
                                        value: page.chosen[row.modelData.id] === true
                                    }

                                }

                                ColumnLayout {
                                    Layout.preferredWidth: page.nameWidth
                                    Layout.minimumWidth: page.nameWidth
                                    Layout.maximumWidth: page.nameWidth
                                    Layout.fillWidth: false

                                    Label {
                                        text: row.modelData.dependency.title || row.modelData.dependency.name
                                        Layout.fillWidth: true
                                        wrapMode: Text.Wrap
                                        textFormat: Text.PlainText
                                        color: ThemeColors.fgColor(page.glassMode)
                                    }

                                    Label {
                                        text: row.modelData.dependency.name
                                        visible: text !== (row.modelData.dependency.title || text)
                                        Layout.fillWidth: true
                                        wrapMode: Text.Wrap
                                        textFormat: Text.PlainText
                                        color: ThemeColors.dimColor(page.glassMode)
                                    }

                                    AppButton {
                                        text: qsTr("Details")
                                        flat: true
                                        onClicked: {
                                            details.resource = row.modelData;
                                            details.open();
                                        }
                                    }

                                }

                                Label {
                                    Layout.preferredWidth: page.categoryWidth
                                    Layout.minimumWidth: page.categoryWidth
                                    Layout.maximumWidth: page.categoryWidth
                                    wrapMode: Text.Wrap
                                    text: page.category(row.modelData.dependency.kind)
                                    color: ThemeColors.fgColor(page.glassMode)
                                }

                                Label {
                                    Layout.preferredWidth: page.statusWidth
                                    Layout.minimumWidth: page.statusWidth
                                    Layout.maximumWidth: page.statusWidth
                                    wrapMode: Text.Wrap
                                    text: row.modelData.installed ? qsTr("Installed") : qsTr("Missing")
                                    color: ThemeColors.fgColor(page.glassMode)
                                }

                                Label {
                                    Layout.preferredWidth: page.sourceWidth
                                    Layout.minimumWidth: page.sourceWidth
                                    Layout.maximumWidth: page.sourceWidth
                                    wrapMode: Text.Wrap
                                    textFormat: Text.PlainText
                                    text: page.sourceLabel(row.modelData.dependency.source)
                                    color: ThemeColors.fgColor(page.glassMode)
                                }

                                Label {
                                    Layout.fillWidth: true
                                    wrapMode: Text.Wrap
                                    textFormat: Text.PlainText
                                    text: (row.modelData.application ? [qsTr("Application")] : []).concat(row.modelData.themes).join(", ")
                                    color: ThemeColors.fgColor(page.glassMode)
                                }

                            }

                        }

                    }

                }

                ScrollBar.horizontal: ScrollBar {
                    id: horizontalBar

                    parent: tableViewport
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.rightMargin: tableBar.implicitWidth + Platform.Units.smallSpacing
                    anchors.bottom: parent.bottom
                    policy: table.contentWidth > table.width ? ScrollBar.AlwaysOn : ScrollBar.AlwaysOff
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
