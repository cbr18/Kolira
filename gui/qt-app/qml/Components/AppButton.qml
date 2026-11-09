import QtQuick
import QtQuick.Controls as Controls

/*! Native button with Paper's mouse/keyboard focus distinction.
 * KDE's StyleItem normally treats activeFocus or highlighted as keyboard
 * focus. Sandstone then draws an inset focus frame on mouse-focused and
 * highlighted buttons. Keep that frame for actual keyboard focus only;
 * native hover/pressed painting, icons and labels remain owned by the style.
 */
Controls.Button {
    id: control

    readonly property bool paperMode: typeof themeController !== "undefined"
        && themeController.currentTheme === "paper"
    readonly property bool supportsNativeFocus: background !== null
        && typeof background.hasFocus === "boolean"

    Binding {
        target: control.supportsNativeFocus ? control.background : null
        property: "hasFocus"
        when: control.paperMode && control.supportsNativeFocus
        value: control.visualFocus
        restoreMode: Binding.RestoreBindingOrValue
    }
}
