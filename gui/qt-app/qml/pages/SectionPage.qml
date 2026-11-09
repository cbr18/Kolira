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
    padding: Platform.Units.gridUnit
    id: page

    title: page.sectionTitle
    property string sectionId: ""
    property string sectionTitle: ""
    property bool showAdvanced: false
    property string highlightFieldId: ""

    readonly property bool glassMode: themeController.isGlass

    background: Rectangle {
        color: page.glassMode
            ? ThemeColors.glassPanel()
            : ThemeColors.pageBg(page.glassMode)
    }

    Shortcut {
        sequence: "Ctrl+Z"
        enabled: !themeInspector.readOnly && !themeInspector.busy
            && (themeInspector.sessionChanges > 0 || themeInspector.undoDepth > 0)
        onActivated: themeInspector.undo()
    }

    Shortcut {
        sequence: "Ctrl+F"
        onActivated: {
            sectionSearch.forceActiveFocus()
            sectionSearch.selectAll()
        }
    }

    Shortcut {
        sequence: "Escape"
        onActivated: pageStack.layers.pop()
    }

    AppDialog {
        id: revertDialog
        title: qsTr("Revert session changes?")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        preferredWidth: 420
        onAccepted: themeInspector.revertAll()

        Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: qsTr("Discard staged edits and restore all applied changes from this session?")
            color: ThemeColors.fgColor(page.glassMode)
        }
    }

    ColumnLayout {
        spacing: Platform.Units.largeSpacing

        TextField {
            id: sectionSearch
            Layout.fillWidth: true
            placeholderText: qsTr("Search in %1…").arg(page.sectionTitle)
        }

        ThemeSectionFields {
            Layout.fillWidth: true
            sectionId: page.sectionId
            glassMode: page.glassMode
            showAdvanced: page.showAdvanced
            filterText: sectionSearch.text
            highlightFieldId: page.highlightFieldId
        }

        ActionFlow {
            Layout.fillWidth: true


            AppButton {
                text: qsTr("Apply")
                icon.name: "dialog-ok-apply"
                highlighted: true
                enabled: !themeInspector.readOnly
                    && themeInspector.sessionChanges > 0
                    && !themeInspector.busy
                onClicked: themeInspector.applyChanges()
            }

            AppButton {
                text: qsTr("Undo")
                icon.name: "edit-undo"
                enabled: !themeInspector.readOnly
                    && (themeInspector.sessionChanges > 0 || themeInspector.undoDepth > 0)
                    && !themeInspector.busy
                onClicked: themeInspector.undo()
            }

            AppButton {
                text: qsTr("Revert")
                icon.name: "document-revert"
                enabled: !themeInspector.readOnly
                    && (themeInspector.sessionChanges > 0 || themeInspector.undoDepth > 0)
                    && !themeInspector.busy
                onClicked: revertDialog.open()
            }
        }
    }
}
