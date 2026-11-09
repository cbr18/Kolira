import QtQuick.Controls
import org.kde.kirigami.platform as Platform

// Reserve transient scrollbar channels even while the bars fade out.
ScrollView {
    clip: true
    rightPadding: ScrollBar.vertical.policy === ScrollBar.AlwaysOff ? 0
        : ScrollBar.vertical.implicitWidth + Platform.Units.smallSpacing
    bottomPadding: ScrollBar.horizontal.policy === ScrollBar.AlwaysOff ? 0
        : ScrollBar.horizontal.implicitHeight + Platform.Units.smallSpacing
}
