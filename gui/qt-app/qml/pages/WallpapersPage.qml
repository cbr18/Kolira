import Components 1.0
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami

Kirigami.ScrollablePage {
    id: page

    objectName: "wallpapersPage"
    title: qsTr("Wallpapers")
    padding: Kirigami.Units.gridUnit
    property string operationMessage: ""
    property bool operationSucceeded: true

    Connections {
        target: wallpaperController
        function onOperationFinished(success, message) {
            page.operationSucceeded = success
            page.operationMessage = message
        }
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        ActionFlow {
            Layout.fillWidth: true

            Button {
                text: qsTr("Previous")
                icon.name: "go-previous-view"
                enabled: !wallpaperController.busy
                onClicked: wallpaperController.prevWallpaper()
            }
            Button {
                text: qsTr("Pick Wallpaper…")
                icon.name: "image-x-generic"
                highlighted: true
                enabled: !wallpaperController.busy
                onClicked: wallpaperController.pickWallpaper()
            }
            Button {
                text: qsTr("Next")
                icon.name: "go-next-view"
                enabled: !wallpaperController.busy
                onClicked: wallpaperController.nextWallpaper()
            }
            Button {
                text: qsTr("Refresh")
                icon.name: "view-refresh"
                enabled: !wallpaperController.busy && !wallpaperController.refreshing
                onClicked: wallpaperController.refresh()
            }
            BusyIndicator {
                running: wallpaperController.busy || wallpaperController.refreshing
                visible: running
            }
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: page.operationMessage !== ""
            text: page.operationMessage
            type: page.operationSucceeded ? Kirigami.MessageType.Positive : Kirigami.MessageType.Error
        }

        Item {
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(Kirigami.Units.gridUnit * 24, width * 0.5625)
            visible: wallpaperController.currentWallpaper !== ""

            Image {
                id: preview
                objectName: "wallpaperPreview"
                anchors.fill: parent
                source: wallpaperController.currentWallpaper
                sourceSize.width: Kirigami.Units.gridUnit * 72
                sourceSize.height: Kirigami.Units.gridUnit * 48
                fillMode: Image.PreserveAspectFit
                asynchronous: true
                Accessible.role: Accessible.Graphic
                Accessible.name: qsTr("Current wallpaper")
            }
            BusyIndicator {
                objectName: "wallpaperPreviewBusy"
                anchors.centerIn: parent
                running: preview.status === Image.Loading
                visible: running
            }
            Label {
                objectName: "wallpaperPreviewError"
                anchors.centerIn: parent
                width: parent.width
                visible: preview.status === Image.Error
                text: qsTr("Wallpaper unavailable")
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
            }
        }

        Label {
            Layout.fillWidth: true
            visible: !wallpaperController.refreshing && wallpaperController.currentWallpaper === ""
            text: qsTr("No wallpaper information available.")
            wrapMode: Text.WordWrap
        }
    }
}
