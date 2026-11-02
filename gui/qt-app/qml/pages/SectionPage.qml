import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami 2.20 as Kirigami
import org.kde.kirigami.platform as Platform
import Components 1.0

/*! One theme section drilled in from the Theme overview.
 *
 *  Pushed onto pageStack.layers; Kirigami shows the back button on its own.
 *  Field edits stage as usual — Apply/Undo/Revert are repeated here so the
 *  user does not have to go back to commit a change.
 */
Kirigami.ScrollablePage {
    id: page

    title: page.sectionTitle
    property string sectionId: ""
    property string sectionTitle: ""
    property bool showAdvanced: false

    readonly property bool glassMode: themeController.isGlass

    background: Rectangle {
        color: page.glassMode
            ? ThemeColors.glassPanel()
            : ThemeColors.pageBg(page.glassMode)
    }

    ColumnLayout {
        Layout.margins: Platform.Units.largeSpacing
        spacing: Platform.Units.largeSpacing

        ThemeSectionFields {
            Layout.fillWidth: true
            sectionId: page.sectionId
            glassMode: page.glassMode
            showAdvanced: page.showAdvanced
        }

        RowLayout {
            Layout.fillWidth: true

            Item { Layout.fillWidth: true }

            Button {
                text: qsTr("Apply")
                icon.name: "dialog-ok-apply"
                highlighted: true
                enabled: !themeInspector.readOnly
                    && themeInspector.sessionChanges > 0
                    && !themeInspector.busy
                onClicked: themeInspector.applyChanges()
            }

            Button {
                text: qsTr("Undo")
                icon.name: "edit-undo"
                enabled: !themeInspector.readOnly
                    && themeInspector.sessionChanges > 0
                    && !themeInspector.busy
                onClicked: themeInspector.undo()
            }

            Button {
                text: qsTr("Revert")
                icon.name: "document-revert"
                enabled: !themeInspector.readOnly
                    && themeInspector.sessionChanges > 0
                    && !themeInspector.busy
                onClicked: themeInspector.revertAll()
            }
        }
    }
}
