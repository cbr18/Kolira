import QtQuick
import QtQuick.Layouts
import org.kde.kirigami.platform as Platform

// Let the parent layout shrink the flow below its unwrapped implicit width.
Flow {
    Layout.fillWidth: true
    Layout.minimumWidth: 0
    spacing: Platform.Units.smallSpacing
}
