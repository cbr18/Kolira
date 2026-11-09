import QtQuick
import QtQuick.Controls
import org.kde.kirigami 2.20 as Kirigami
import org.kde.kirigami.platform as Platform

// Keep native header/footer and lazy sizing; inset content exactly once.
Kirigami.Dialog {
    id: dialog
    padding: Platform.Units.largeSpacing
    // A full-height popup must stay inside the overlay while it fades in.
    // Kirigami's opacity-dependent y offset can feed back into popup fitting.
    y: parent ? Math.max(Platform.Units.largeSpacing, Math.round((parent.height - height) / 2)) : 0
    Binding {
        target: dialog.contentItem
        property: "rightPadding"
        when: dialog.contentItem instanceof ScrollView
        value: dialog.contentItem instanceof ScrollView
            ? dialog.contentItem.ScrollBar.vertical.implicitWidth + Platform.Units.smallSpacing : 0
        restoreMode: Binding.RestoreBindingOrValue
    }
}
