## QML Code Review Report

**Scope**: files: `gui/qt-app/qml/**/*.qml` и связанные Python-контроллеры
**Files reviewed**: 19
**Issues found**: 499 (493 from lint, 6 from deep analysis)
**qmllint**: ran; текстовый режим, без замечаний. Установленная версия не поддерживает `--json`.

После аудита D-001–D-006 исправлены: [регрессии и итог](2026_11_08_audit-fixes.md).
Этот документ сохраняет исходный срез; механические замечания не исправлялись
массовой перестановкой QML. Investigation targets остаются неподтверждёнными.

Дата: 2026-11-08. Рабочее дерево, включая незакоммиченные изменения. Код не исправлялся.
Линтер навыка: `/home/cbr/.agents/skills/qt-qml-review/references/lint-scripts/qt_qml_lint.py`.
Шесть независимых миссий выполнены волнами в пределах доступных слотов.
Механические замечания сохранены полностью; их количество не означает количество воспроизведённых багов.
Более широкий аудит и приоритеты: [отчёт](2026_11_08_codebase-audit.md).

---

### Lint findings

#### [L-001] STY-1
- **File**: `gui/qt-app/qml/Components/AutomationList.qml:9`
- **Rule**: STY-1
- **Finding**: Top-level component should have id: root (enables qualified lookup, future-proofs for QML 3)
- **Mitigation**: Привести имя/структуру объявления к правилу без функциональных изменений.

#### [L-002] BND-1
- **File**: `gui/qt-app/qml/Components/AutomationList.qml:11`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-003] BND-1
- **File**: `gui/qt-app/qml/Components/AutomationList.qml:14`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-004] BND-1
- **File**: `gui/qt-app/qml/Components/AutomationList.qml:29`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-005] ORD-1
- **File**: `gui/qt-app/qml/Components/AutomationList.qml:29`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-006] ORD-1
- **File**: `gui/qt-app/qml/Components/AutomationList.qml:34`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-007] ORD-1
- **File**: `gui/qt-app/qml/Components/AutomationList.qml:37`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-008] ORD-1
- **File**: `gui/qt-app/qml/Components/AutomationList.qml:43`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-009] ORD-1
- **File**: `gui/qt-app/qml/Components/AutomationList.qml:51`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-010] JS-1
- **File**: `gui/qt-app/qml/Components/AutomationList.qml:54`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-011] IMP-4
- **File**: `gui/qt-app/qml/Components/ComponentCategoryGroup.qml:2`
- **Rule**: IMP-4
- **Finding**: Non-Qt import before Qt import -- order: Qt modules, third-party, local C++, QML folders
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-012] IMP-3
- **File**: `gui/qt-app/qml/Components/ComponentCategoryGroup.qml:3`
- **Rule**: IMP-3
- **Finding**: import QtQuick.Controls without style qualifier -- use Controls.Basic (or specific style) when customizing contentItem/background/indicator/handle
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-013] IMP-2
- **File**: `gui/qt-app/qml/Components/ComponentCategoryGroup.qml:5`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-014] STY-1
- **File**: `gui/qt-app/qml/Components/ComponentCategoryGroup.qml:7`
- **Rule**: STY-1
- **Finding**: Top-level component should have id: root (enables qualified lookup, future-proofs for QML 3)
- **Mitigation**: Привести имя/структуру объявления к правилу без функциональных изменений.

#### [L-015] ORD-1
- **File**: `gui/qt-app/qml/Components/ComponentCategoryGroup.qml:28`
- **Rule**: ORD-1
- **Finding**: property declaration appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-016] BND-1
- **File**: `gui/qt-app/qml/Components/ComponentCategoryGroup.qml:30`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-017] BND-1
- **File**: `gui/qt-app/qml/Components/ComponentCategoryGroup.qml:31`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-018] BND-1
- **File**: `gui/qt-app/qml/Components/ComponentCategoryGroup.qml:36`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-019] ORD-1
- **File**: `gui/qt-app/qml/Components/ComponentCategoryGroup.qml:56`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-020] ORD-1
- **File**: `gui/qt-app/qml/Components/ComponentCategoryGroup.qml:82`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-021] PRF-3
- **File**: `gui/qt-app/qml/Components/ComponentCategoryGroup.qml:89`
- **Rule**: PRF-3
- **Finding**: clip: true disables scene graph batching -- verify this is needed (acceptable on ListView)
- **Mitigation**: Проверить необходимость дополнительного узла/эффекта и убрать лишние затраты там, где сохраняется поведение.

#### [L-022] ORD-1
- **File**: `gui/qt-app/qml/Components/ComponentCategoryGroup.qml:89`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-023] JS-1
- **File**: `gui/qt-app/qml/Components/ComponentCategoryGroup.qml:105`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-024] JS-1
- **File**: `gui/qt-app/qml/Components/ComponentCategoryGroup.qml:122`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-025] JS-1
- **File**: `gui/qt-app/qml/Components/ComponentCategoryGroup.qml:123`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-026] IMP-4
- **File**: `gui/qt-app/qml/Components/ComponentCheckbox.qml:2`
- **Rule**: IMP-4
- **Finding**: Non-Qt import before Qt import -- order: Qt modules, third-party, local C++, QML folders
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-027] IMP-3
- **File**: `gui/qt-app/qml/Components/ComponentCheckbox.qml:3`
- **Rule**: IMP-3
- **Finding**: import QtQuick.Controls without style qualifier -- use Controls.Basic (or specific style) when customizing contentItem/background/indicator/handle
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-028] IMP-2
- **File**: `gui/qt-app/qml/Components/ComponentCheckbox.qml:5`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-029] STY-1
- **File**: `gui/qt-app/qml/Components/ComponentCheckbox.qml:7`
- **Rule**: STY-1
- **Finding**: Top-level component should have id: root (enables qualified lookup, future-proofs for QML 3)
- **Mitigation**: Привести имя/структуру объявления к правилу без функциональных изменений.

#### [L-030] ORD-1
- **File**: `gui/qt-app/qml/Components/ComponentCheckbox.qml:11`
- **Rule**: ORD-1
- **Finding**: property declaration appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-031] ORD-1
- **File**: `gui/qt-app/qml/Components/ComponentCheckbox.qml:64`
- **Rule**: ORD-1
- **Finding**: property assignment appears after child object -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-032] ORD-1
- **File**: `gui/qt-app/qml/Components/ComponentCheckbox.qml:77`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-033] ORD-1
- **File**: `gui/qt-app/qml/Components/ComponentCheckbox.qml:84`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-034] ORD-1
- **File**: `gui/qt-app/qml/Components/ComponentCheckbox.qml:114`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-035] STY-1
- **File**: `gui/qt-app/qml/Components/PalettePreview.qml:10`
- **Rule**: STY-1
- **Finding**: Top-level component should have id: root (enables qualified lookup, future-proofs for QML 3)
- **Mitigation**: Привести имя/структуру объявления к правилу без функциональных изменений.

#### [L-036] BND-1
- **File**: `gui/qt-app/qml/Components/PalettePreview.qml:12`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-037] JS-1
- **File**: `gui/qt-app/qml/Components/PalettePreview.qml:19`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-038] JS-1
- **File**: `gui/qt-app/qml/Components/PalettePreview.qml:24`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-039] ORD-1
- **File**: `gui/qt-app/qml/Components/PalettePreview.qml:33`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-040] ORD-1
- **File**: `gui/qt-app/qml/Components/PalettePreview.qml:39`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-041] PRF-3
- **File**: `gui/qt-app/qml/Components/PalettePreview.qml:43`
- **Rule**: PRF-3
- **Finding**: clip: true disables scene graph batching -- verify this is needed (acceptable on ListView)
- **Mitigation**: Проверить необходимость дополнительного узла/эффекта и убрать лишние затраты там, где сохраняется поведение.

#### [L-042] ORD-1
- **File**: `gui/qt-app/qml/Components/PalettePreview.qml:50`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-043] ORD-1
- **File**: `gui/qt-app/qml/Components/PalettePreview.qml:60`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-044] ORD-1
- **File**: `gui/qt-app/qml/Components/PalettePreview.qml:64`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-045] ORD-1
- **File**: `gui/qt-app/qml/Components/PalettePreview.qml:83`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-046] ORD-1
- **File**: `gui/qt-app/qml/Components/PalettePreview.qml:90`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-047] ORD-1
- **File**: `gui/qt-app/qml/Components/PalettePreview.qml:114`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-048] ORD-1
- **File**: `gui/qt-app/qml/Components/PalettePreview.qml:136`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-049] ORD-1
- **File**: `gui/qt-app/qml/Components/PalettePreview.qml:153`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-050] BND-1
- **File**: `gui/qt-app/qml/Components/PalettePreview.qml:162`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-051] ORD-1
- **File**: `gui/qt-app/qml/Components/PalettePreview.qml:162`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-052] IMP-4
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:2`
- **Rule**: IMP-4
- **Finding**: Non-Qt import before Qt import -- order: Qt modules, third-party, local C++, QML folders
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-053] IMP-3
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:3`
- **Rule**: IMP-3
- **Finding**: import QtQuick.Controls without style qualifier -- use Controls.Basic (or specific style) when customizing contentItem/background/indicator/handle
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-054] IMP-2
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:5`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-055] STY-1
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:7`
- **Rule**: STY-1
- **Finding**: Top-level component should have id: root (enables qualified lookup, future-proofs for QML 3)
- **Mitigation**: Привести имя/структуру объявления к правилу без функциональных изменений.

#### [L-056] ORD-1
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:13`
- **Rule**: ORD-1
- **Finding**: property declaration appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-057] BND-1
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:26`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-058] JS-1
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:36`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-059] JS-1
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:44`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-060] JS-1
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:49`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-061] JS-1
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:56`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-062] JS-1
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:65`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-063] ORD-1
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:109`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-064] ORD-1
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:113`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-065] LAY-2
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:131`
- **Rule**: LAY-2
- **Finding**: width: inside RowLayout child -- use Layout.preferredWidth or Layout.fillWidth
- **Mitigation**: Проверить владение геометрией и оставить один механизм размещения для затронутого объекта.

#### [L-066] ORD-1
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:131`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-067] LAY-2
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:132`
- **Rule**: LAY-2
- **Finding**: height: inside RowLayout child -- use Layout.preferredHeight or Layout.fillHeight
- **Mitigation**: Проверить владение геометрией и оставить один механизм размещения для затронутого объекта.

#### [L-068] ORD-1
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:148`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-069] ORD-1
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:156`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-070] ORD-1
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:171`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-071] ORD-1
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:180`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-072] ORD-1
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:185`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-073] ORD-1
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:205`
- **Rule**: ORD-1
- **Finding**: attached property appears after child object -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-074] ORD-1
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:213`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-075] ORD-1
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:217`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-076] ORD-1
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:226`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-077] ORD-1
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:238`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-078] ORD-1
- **File**: `gui/qt-app/qml/Components/ProfileCardDelegate.qml:272`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-079] IMP-2
- **File**: `gui/qt-app/qml/Components/ThemeColors.qml:3`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-080] STY-1
- **File**: `gui/qt-app/qml/Components/ThemeColors.qml:14`
- **Rule**: STY-1
- **Finding**: Top-level component should have id: root (enables qualified lookup, future-proofs for QML 3)
- **Mitigation**: Привести имя/структуру объявления к правилу без функциональных изменений.

#### [L-081] JS-1
- **File**: `gui/qt-app/qml/Components/ThemeColors.qml:16`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-082] JS-1
- **File**: `gui/qt-app/qml/Components/ThemeColors.qml:29`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-083] STY-1
- **File**: `gui/qt-app/qml/Components/TypeScale.qml:12`
- **Rule**: STY-1
- **Finding**: Top-level component should have id: root (enables qualified lookup, future-proofs for QML 3)
- **Mitigation**: Привести имя/структуру объявления к правилу без функциональных изменений.

#### [L-084] JS-1
- **File**: `gui/qt-app/qml/Components/TypeScale.qml:17`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-085] IMP-3
- **File**: `gui/qt-app/qml/Main.qml:2`
- **Rule**: IMP-3
- **Finding**: import QtQuick.Controls without style qualifier -- use Controls.Basic (or specific style) when customizing contentItem/background/indicator/handle
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-086] IMP-2
- **File**: `gui/qt-app/qml/Main.qml:6`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-087] IMP-2
- **File**: `gui/qt-app/qml/Main.qml:8`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-088] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:44`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-089] JS-1
- **File**: `gui/qt-app/qml/Main.qml:135`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-090] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:231`
- **Rule**: ORD-1
- **Finding**: property assignment appears after child object -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-091] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:304`
- **Rule**: ORD-1
- **Finding**: id appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-092] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:364`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-093] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:375`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-094] BND-2
- **File**: `gui/qt-app/qml/Main.qml:471`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'exportFileDialog.profileName' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-095] BND-2
- **File**: `gui/qt-app/qml/Main.qml:477`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'duplicateNameField.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-096] BND-2
- **File**: `gui/qt-app/qml/Main.qml:478`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'duplicateError.visible' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-097] BND-2
- **File**: `gui/qt-app/qml/Main.qml:483`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'descriptionEdit.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-098] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:491`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-099] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:532`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-100] IMG-1
- **File**: `gui/qt-app/qml/Main.qml:534`
- **Rule**: IMG-1
- **Finding**: Image without sourceSize -- decodes full resolution into GPU memory; set sourceSize to display dimensions
- **Mitigation**: Уточнить размер декодирования и режим загрузки с проверкой качества предпросмотра.

#### [L-101] PRF-1
- **File**: `gui/qt-app/qml/Main.qml:552`
- **Rule**: PRF-1
- **Finding**: Rectangle with 'transparent' color -- use Item instead, or toggle visible on the Rectangle (creates geometry node even when transparent)
- **Mitigation**: Проверить необходимость дополнительного узла/эффекта и убрать лишние затраты там, где сохраняется поведение.

#### [L-102] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:561`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-103] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:569`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-104] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:573`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-105] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:580`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-106] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:588`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-107] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:607`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-108] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:620`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-109] BND-2
- **File**: `gui/qt-app/qml/Main.qml:633`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'keepWallpaperBox.checked' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-110] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:645`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-111] BND-1
- **File**: `gui/qt-app/qml/Main.qml:646`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-112] BND-1
- **File**: `gui/qt-app/qml/Main.qml:651`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-113] JS-1
- **File**: `gui/qt-app/qml/Main.qml:656`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-114] JS-1
- **File**: `gui/qt-app/qml/Main.qml:657`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-115] JS-1
- **File**: `gui/qt-app/qml/Main.qml:658`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-116] JS-1
- **File**: `gui/qt-app/qml/Main.qml:665`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-117] JS-1
- **File**: `gui/qt-app/qml/Main.qml:666`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-118] BND-2
- **File**: `gui/qt-app/qml/Main.qml:669`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'rememberStore.checked' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-119] BND-2
- **File**: `gui/qt-app/qml/Main.qml:670`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'rememberPackages.checked' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-120] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:729`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-121] BND-1
- **File**: `gui/qt-app/qml/Main.qml:740`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-122] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:740`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-123] JS-1
- **File**: `gui/qt-app/qml/Main.qml:748`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-124] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:755`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-125] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:758`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-126] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:767`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-127] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:775`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-128] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:783`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-129] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:807`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-130] JS-1
- **File**: `gui/qt-app/qml/Main.qml:828`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-131] JS-1
- **File**: `gui/qt-app/qml/Main.qml:830`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-132] JS-1
- **File**: `gui/qt-app/qml/Main.qml:831`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-133] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:847`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-134] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:852`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-135] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:858`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-136] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:866`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-137] JS-1
- **File**: `gui/qt-app/qml/Main.qml:873`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-138] BND-2
- **File**: `gui/qt-app/qml/Main.qml:875`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'newThemeError.visible' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-139] BND-2
- **File**: `gui/qt-app/qml/Main.qml:879`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'newThemeError.visible' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-140] BND-2
- **File**: `gui/qt-app/qml/Main.qml:881`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'newThemeName.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-141] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:914`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-142] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:921`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-143] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:940`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-144] BND-1
- **File**: `gui/qt-app/qml/Main.qml:941`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-145] BND-1
- **File**: `gui/qt-app/qml/Main.qml:942`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-146] JS-1
- **File**: `gui/qt-app/qml/Main.qml:945`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-147] JS-1
- **File**: `gui/qt-app/qml/Main.qml:946`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-148] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:974`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-149] BND-1
- **File**: `gui/qt-app/qml/Main.qml:981`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-150] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:981`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-151] JS-1
- **File**: `gui/qt-app/qml/Main.qml:987`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-152] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:994`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-153] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:997`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-154] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1004`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-155] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1011`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-156] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1060`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-157] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1073`
- **Rule**: ORD-1
- **Finding**: signal handler appears after child object -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-158] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1092`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-159] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1108`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-160] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1115`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-161] JS-1
- **File**: `gui/qt-app/qml/Main.qml:1125`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-162] JS-1
- **File**: `gui/qt-app/qml/Main.qml:1126`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-163] BND-2
- **File**: `gui/qt-app/qml/Main.qml:1127`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'importReplaceBox.checked' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-164] BND-2
- **File**: `gui/qt-app/qml/Main.qml:1128`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'importNameField.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-165] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1139`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-166] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1152`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-167] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1187`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-168] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1193`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-169] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1201`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-170] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1220`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-171] JS-1
- **File**: `gui/qt-app/qml/Main.qml:1228`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-172] BND-2
- **File**: `gui/qt-app/qml/Main.qml:1234`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'saveError.visible' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-173] BND-2
- **File**: `gui/qt-app/qml/Main.qml:1240`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'nameField.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-174] BND-2
- **File**: `gui/qt-app/qml/Main.qml:1241`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'descriptionField.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-175] BND-2
- **File**: `gui/qt-app/qml/Main.qml:1242`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'includeWallpaperBox.checked' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-176] BND-2
- **File**: `gui/qt-app/qml/Main.qml:1243`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'makeLookAndFeelBox.checked' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-177] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1250`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-178] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1272`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-179] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1278`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-180] JS-1
- **File**: `gui/qt-app/qml/Main.qml:1286`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-181] BND-2
- **File**: `gui/qt-app/qml/Main.qml:1288`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'duplicateError.visible' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-182] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1299`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-183] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1322`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-184] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1340`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-185] BND-2
- **File**: `gui/qt-app/qml/Main.qml:1352`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'diffText.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-186] BND-2
- **File**: `gui/qt-app/qml/Main.qml:1355`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'diffText.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-187] BND-2
- **File**: `gui/qt-app/qml/Main.qml:1357`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'diffText.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-188] BND-2
- **File**: `gui/qt-app/qml/Main.qml:1368`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'diffText.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-189] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1383`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-190] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1398`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-191] PRF-3
- **File**: `gui/qt-app/qml/Main.qml:1399`
- **Rule**: PRF-3
- **Finding**: clip: true disables scene graph batching -- verify this is needed (acceptable on ListView)
- **Mitigation**: Проверить необходимость дополнительного узла/эффекта и убрать лишние затраты там, где сохраняется поведение.

#### [L-192] STY-3
- **File**: `gui/qt-app/qml/Main.qml:1420`
- **Rule**: STY-3
- **Finding**: 3 palette.* properties using dot notation -- use group notation: palette { ... }
- **Mitigation**: Привести имя/структуру объявления к правилу без функциональных изменений.

#### [L-193] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1434`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-194] BND-1
- **File**: `gui/qt-app/qml/Main.qml:1436`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-195] JS-1
- **File**: `gui/qt-app/qml/Main.qml:1450`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-196] JS-1
- **File**: `gui/qt-app/qml/Main.qml:1459`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-197] JS-1
- **File**: `gui/qt-app/qml/Main.qml:1460`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-198] JS-1
- **File**: `gui/qt-app/qml/Main.qml:1461`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-199] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1490`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-200] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1494`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-201] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1501`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-202] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1513`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-203] ORD-1
- **File**: `gui/qt-app/qml/Main.qml:1522`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-204] PRF-3
- **File**: `gui/qt-app/qml/Main.qml:1523`
- **Rule**: PRF-3
- **Finding**: clip: true disables scene graph batching -- verify this is needed (acceptable on ListView)
- **Mitigation**: Проверить необходимость дополнительного узла/эффекта и убрать лишние затраты там, где сохраняется поведение.

#### [L-205] STY-3
- **File**: `gui/qt-app/qml/Main.qml:1544`
- **Rule**: STY-3
- **Finding**: 3 palette.* properties using dot notation -- use group notation: palette { ... }
- **Mitigation**: Привести имя/структуру объявления к правилу без функциональных изменений.

#### [L-206] IMP-2
- **File**: `gui/qt-app/qml/pages/AppConfigTree.qml:4`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-207] IMP-2
- **File**: `gui/qt-app/qml/pages/AppConfigTree.qml:6`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-208] STY-1
- **File**: `gui/qt-app/qml/pages/AppConfigTree.qml:15`
- **Rule**: STY-1
- **Finding**: Top-level component should have id: root (enables qualified lookup, future-proofs for QML 3)
- **Mitigation**: Привести имя/структуру объявления к правилу без функциональных изменений.

#### [L-209] ORD-1
- **File**: `gui/qt-app/qml/pages/AppConfigTree.qml:22`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-210] BND-1
- **File**: `gui/qt-app/qml/pages/AppConfigTree.qml:40`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-211] ORD-1
- **File**: `gui/qt-app/qml/pages/AppConfigTree.qml:42`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-212] ORD-1
- **File**: `gui/qt-app/qml/pages/AppConfigTree.qml:46`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-213] BND-1
- **File**: `gui/qt-app/qml/pages/AppConfigTree.qml:64`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-214] BND-1
- **File**: `gui/qt-app/qml/pages/AppConfigTree.qml:67`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-215] BND-1
- **File**: `gui/qt-app/qml/pages/AppConfigTree.qml:68`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-216] ORD-1
- **File**: `gui/qt-app/qml/pages/AppConfigTree.qml:72`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-217] ORD-1
- **File**: `gui/qt-app/qml/pages/AppConfigTree.qml:76`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-218] BND-1
- **File**: `gui/qt-app/qml/pages/AppConfigTree.qml:87`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-219] ORD-1
- **File**: `gui/qt-app/qml/pages/AppConfigTree.qml:94`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-220] ORD-1
- **File**: `gui/qt-app/qml/pages/AppConfigTree.qml:102`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-221] ORD-1
- **File**: `gui/qt-app/qml/pages/AppConfigTree.qml:122`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-222] ORD-1
- **File**: `gui/qt-app/qml/pages/AppConfigTree.qml:127`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-223] ORD-1
- **File**: `gui/qt-app/qml/pages/AppConfigTree.qml:163`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-224] ORD-1
- **File**: `gui/qt-app/qml/pages/AppConfigTree.qml:172`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-225] BND-2
- **File**: `gui/qt-app/qml/pages/AppConfigTree.qml:182`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'newValueField.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-226] IMP-3
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:2`
- **Rule**: IMP-3
- **Finding**: import QtQuick.Controls without style qualifier -- use Controls.Basic (or specific style) when customizing contentItem/background/indicator/handle
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-227] IMP-2
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:4`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-228] IMP-2
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:6`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-229] STY-1
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:12`
- **Rule**: STY-1
- **Finding**: Top-level component should have id: root (enables qualified lookup, future-proofs for QML 3)
- **Mitigation**: Привести имя/структуру объявления к правилу без функциональных изменений.

#### [L-230] ORD-1
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:36`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-231] ORD-1
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:54`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-232] ORD-1
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:98`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-233] ORD-1
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:108`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-234] BND-1
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:115`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-235] BND-1
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:138`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-236] ORD-1
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:141`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-237] ORD-1
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:146`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-238] ORD-1
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:167`
- **Rule**: ORD-1
- **Finding**: signal handler appears after child object -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-239] ORD-1
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:182`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-240] ORD-1
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:215`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-241] ORD-1
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:252`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-242] BND-2
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:277`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'nameField.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-243] BND-2
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:278`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'categoryField.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-244] BND-2
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:279`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'iconField.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-245] BND-2
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:280`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'descriptionField.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-246] BND-2
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:281`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'pathsArea.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-247] BND-2
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:282`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'reloadField.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-248] BND-2
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:283`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'errorLabel.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-249] BND-2
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:289`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'nameField.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-250] BND-2
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:290`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'categoryField.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-251] BND-2
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:291`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'iconField.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-252] BND-2
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:292`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'descriptionField.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-253] BND-2
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:293`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'pathsArea.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-254] BND-2
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:294`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'reloadField.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-255] BND-2
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:295`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'errorLabel.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-256] ORD-1
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:300`
- **Rule**: ORD-1
- **Finding**: child object appears after function -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-257] BND-2
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:319`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'errorLabel.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-258] ORD-1
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:379`
- **Rule**: ORD-1
- **Finding**: property assignment appears after child object -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-259] ORD-1
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:414`
- **Rule**: ORD-1
- **Finding**: property assignment appears after child object -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-260] BND-2
- **File**: `gui/qt-app/qml/pages/ApplicationsPage.qml:420`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'errorLabel.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-261] IMP-3
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:2`
- **Rule**: IMP-3
- **Finding**: import QtQuick.Controls without style qualifier -- use Controls.Basic (or specific style) when customizing contentItem/background/indicator/handle
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-262] IMP-2
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:4`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-263] IMP-2
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:6`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-264] STY-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:8`
- **Rule**: STY-1
- **Finding**: Top-level component should have id: root (enables qualified lookup, future-proofs for QML 3)
- **Mitigation**: Привести имя/структуру объявления к правилу без функциональных изменений.

#### [L-265] BND-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:13`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-266] ORD-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:14`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-267] ORD-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:86`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-268] ORD-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:122`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-269] ORD-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:128`
- **Rule**: ORD-1
- **Finding**: attached property appears after signal handler -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-270] ORD-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:136`
- **Rule**: ORD-1
- **Finding**: attached property appears after signal handler -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-271] ORD-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:151`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-272] ORD-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:160`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-273] ORD-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:170`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-274] ORD-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:178`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-275] PRF-3
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:180`
- **Rule**: PRF-3
- **Finding**: clip: true disables scene graph batching -- verify this is needed (acceptable on ListView)
- **Mitigation**: Проверить необходимость дополнительного узла/эффекта и убрать лишние затраты там, где сохраняется поведение.

#### [L-276] ORD-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:191`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-277] ORD-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:196`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-278] ORD-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:222`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-279] PRF-3
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:223`
- **Rule**: PRF-3
- **Finding**: clip: true disables scene graph batching -- verify this is needed (acceptable on ListView)
- **Mitigation**: Проверить необходимость дополнительного узла/эффекта и убрать лишние затраты там, где сохраняется поведение.

#### [L-280] ORD-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:227`
- **Rule**: ORD-1
- **Finding**: id appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-281] BND-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:228`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-282] STY-3
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:235`
- **Rule**: STY-3
- **Finding**: 3 anchors.* properties using dot notation -- use group notation: anchors { ... }
- **Mitigation**: Привести имя/структуру объявления к правилу без функциональных изменений.

#### [L-283] ORD-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:243`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-284] ORD-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:256`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-285] ORD-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:264`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-286] ORD-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:291`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-287] BND-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:301`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-288] BND-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:326`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-289] ORD-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:333`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-290] BND-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:340`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-291] ORD-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:342`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-292] ORD-1
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:349`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-293] IMP-3
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:2`
- **Rule**: IMP-3
- **Finding**: import QtQuick.Controls without style qualifier -- use Controls.Basic (or specific style) when customizing contentItem/background/indicator/handle
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-294] IMP-2
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:5`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-295] IMP-2
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:7`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-296] STY-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:15`
- **Rule**: STY-1
- **Finding**: Top-level component should have id: root (enables qualified lookup, future-proofs for QML 3)
- **Mitigation**: Привести имя/структуру объявления к правилу без функциональных изменений.

#### [L-297] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:29`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-298] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:147`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-299] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:154`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-300] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:164`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-301] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:176`
- **Rule**: ORD-1
- **Finding**: attached property appears after signal handler -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-302] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:184`
- **Rule**: ORD-1
- **Finding**: attached property appears after signal handler -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-303] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:202`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-304] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:211`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-305] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:226`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-306] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:238`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-307] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:246`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-308] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:253`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-309] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:260`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-310] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:269`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-311] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:285`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-312] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:300`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-313] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:322`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-314] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:338`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-315] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:348`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-316] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:364`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-317] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:381`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-318] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:388`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-319] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:418`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-320] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:425`
- **Rule**: ORD-1
- **Finding**: attached property appears after signal handler -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-321] BND-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:437`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-322] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:437`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-323] ORD-1
- **File**: `gui/qt-app/qml/pages/PalettePage.qml:444`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-324] IMP-3
- **File**: `gui/qt-app/qml/pages/SectionPage.qml:2`
- **Rule**: IMP-3
- **Finding**: import QtQuick.Controls without style qualifier -- use Controls.Basic (or specific style) when customizing contentItem/background/indicator/handle
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-325] IMP-2
- **File**: `gui/qt-app/qml/pages/SectionPage.qml:4`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-326] IMP-2
- **File**: `gui/qt-app/qml/pages/SectionPage.qml:6`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-327] STY-1
- **File**: `gui/qt-app/qml/pages/SectionPage.qml:14`
- **Rule**: STY-1
- **Finding**: Top-level component should have id: root (enables qualified lookup, future-proofs for QML 3)
- **Mitigation**: Привести имя/структуру объявления к правилу без функциональных изменений.

#### [L-328] ORD-1
- **File**: `gui/qt-app/qml/pages/SectionPage.qml:18`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-329] ORD-1
- **File**: `gui/qt-app/qml/pages/SectionPage.qml:60`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-330] ORD-1
- **File**: `gui/qt-app/qml/pages/SectionPage.qml:68`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-331] ORD-1
- **File**: `gui/qt-app/qml/pages/SectionPage.qml:73`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-332] ORD-1
- **File**: `gui/qt-app/qml/pages/SectionPage.qml:78`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-333] IMP-3
- **File**: `gui/qt-app/qml/pages/SettingsPage.qml:2`
- **Rule**: IMP-3
- **Finding**: import QtQuick.Controls without style qualifier -- use Controls.Basic (or specific style) when customizing contentItem/background/indicator/handle
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-334] IMP-2
- **File**: `gui/qt-app/qml/pages/SettingsPage.qml:5`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-335] IMP-2
- **File**: `gui/qt-app/qml/pages/SettingsPage.qml:7`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-336] STY-1
- **File**: `gui/qt-app/qml/pages/SettingsPage.qml:9`
- **Rule**: STY-1
- **Finding**: Top-level component should have id: root (enables qualified lookup, future-proofs for QML 3)
- **Mitigation**: Привести имя/структуру объявления к правилу без функциональных изменений.

#### [L-337] ORD-1
- **File**: `gui/qt-app/qml/pages/SettingsPage.qml:33`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-338] ORD-1
- **File**: `gui/qt-app/qml/pages/SettingsPage.qml:57`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-339] ORD-1
- **File**: `gui/qt-app/qml/pages/SettingsPage.qml:107`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-340] ORD-1
- **File**: `gui/qt-app/qml/pages/SettingsPage.qml:113`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-341] ORD-1
- **File**: `gui/qt-app/qml/pages/SettingsPage.qml:118`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-342] ORD-1
- **File**: `gui/qt-app/qml/pages/SettingsPage.qml:167`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-343] ORD-1
- **File**: `gui/qt-app/qml/pages/SettingsPage.qml:211`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-344] DEL-1
- **File**: `gui/qt-app/qml/pages/SettingsPage.qml:216`
- **Rule**: DEL-1
- **Finding**: model.roleName without required property -- declare required properties for type safety and qmlsc compilation
- **Mitigation**: Сделать контракт ролей и жизненного цикла делегата явным.

#### [L-345] ORD-1
- **File**: `gui/qt-app/qml/pages/SettingsPage.qml:227`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-346] DEL-1
- **File**: `gui/qt-app/qml/pages/SettingsPage.qml:232`
- **Rule**: DEL-1
- **Finding**: model.roleName without required property -- declare required properties for type safety and qmlsc compilation
- **Mitigation**: Сделать контракт ролей и жизненного цикла делегата явным.

#### [L-347] ORD-1
- **File**: `gui/qt-app/qml/pages/SettingsPage.qml:239`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-348] ORD-1
- **File**: `gui/qt-app/qml/pages/SettingsPage.qml:272`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-349] ORD-1
- **File**: `gui/qt-app/qml/pages/SettingsPage.qml:278`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-350] ORD-1
- **File**: `gui/qt-app/qml/pages/SettingsPage.qml:389`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-351] ORD-1
- **File**: `gui/qt-app/qml/pages/SettingsPage.qml:397`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-352] IMP-3
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:2`
- **Rule**: IMP-3
- **Finding**: import QtQuick.Controls without style qualifier -- use Controls.Basic (or specific style) when customizing contentItem/background/indicator/handle
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-353] IMP-2
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:4`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-354] IMP-2
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:6`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-355] STY-1
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:9`
- **Rule**: STY-1
- **Finding**: Top-level component should have id: root (enables qualified lookup, future-proofs for QML 3)
- **Mitigation**: Привести имя/структуру объявления к правилу без функциональных изменений.

#### [L-356] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:30`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-357] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:34`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-358] BND-1
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:43`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-359] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:60`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-360] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:72`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-361] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:75`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-362] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:81`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-363] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:104`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-364] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:111`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-365] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:144`
- **Rule**: ORD-1
- **Finding**: attached property appears after signal handler -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-366] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:167`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-367] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:176`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-368] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:223`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-369] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:236`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-370] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:242`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-371] BND-1
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:253`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-372] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:253`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-373] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:259`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-374] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:262`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-375] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:268`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-376] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:279`
- **Rule**: ORD-1
- **Finding**: attached property appears after signal handler -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-377] IMP-3
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:2`
- **Rule**: IMP-3
- **Finding**: import QtQuick.Controls without style qualifier -- use Controls.Basic (or specific style) when customizing contentItem/background/indicator/handle
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-378] IMP-2
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:5`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-379] IMP-2
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:7`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-380] STY-1
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:10`
- **Rule**: STY-1
- **Finding**: Top-level component should have id: root (enables qualified lookup, future-proofs for QML 3)
- **Mitigation**: Привести имя/структуру объявления к правилу без функциональных изменений.

#### [L-381] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:14`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-382] BND-1
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:17`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-383] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:48`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-384] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:52`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-385] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:59`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-386] BND-1
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:69`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-387] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:85`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-388] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:96`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-389] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:100`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-390] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:103`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-391] LAY-2
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:109`
- **Rule**: LAY-2
- **Finding**: width: inside RowLayout child -- use Layout.preferredWidth or Layout.fillWidth
- **Mitigation**: Проверить владение геометрией и оставить один механизм размещения для затронутого объекта.

#### [L-392] LAY-2
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:110`
- **Rule**: LAY-2
- **Finding**: height: inside RowLayout child -- use Layout.preferredHeight or Layout.fillHeight
- **Mitigation**: Проверить владение геометрией и оставить один механизм размещения для затронутого объекта.

#### [L-393] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:116`
- **Rule**: ORD-1
- **Finding**: property assignment appears after child object -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-394] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:130`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-395] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:158`
- **Rule**: ORD-1
- **Finding**: attached property appears after signal handler -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-396] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:169`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-397] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:211`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-398] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:215`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-399] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:249`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-400] IMP-2
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:5`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-401] IMP-2
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:7`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-402] STY-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:15`
- **Rule**: STY-1
- **Finding**: Top-level component should have id: root (enables qualified lookup, future-proofs for QML 3)
- **Mitigation**: Привести имя/структуру объявления к правилу без функциональных изменений.

#### [L-403] BND-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:18`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-404] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:24`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-405] JS-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:34`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-406] JS-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:42`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-407] JS-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:43`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-408] JS-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:48`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-409] JS-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:49`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-410] JS-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:52`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-411] JS-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:57`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-412] JS-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:65`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-413] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:74`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-414] JS-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:78`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-415] LAY-2
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:92`
- **Rule**: LAY-2
- **Finding**: width: inside RowLayout child -- use Layout.preferredWidth or Layout.fillWidth
- **Mitigation**: Проверить владение геометрией и оставить один механизм размещения для затронутого объекта.

#### [L-416] LAY-2
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:93`
- **Rule**: LAY-2
- **Finding**: height: inside RowLayout child -- use Layout.preferredHeight or Layout.fillHeight
- **Mitigation**: Проверить владение геометрией и оставить один механизм размещения для затронутого объекта.

#### [L-417] LAY-2
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:108`
- **Rule**: LAY-2
- **Finding**: width: inside RowLayout child -- use Layout.preferredWidth or Layout.fillWidth
- **Mitigation**: Проверить владение геометрией и оставить один механизм размещения для затронутого объекта.

#### [L-418] LAY-2
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:109`
- **Rule**: LAY-2
- **Finding**: height: inside RowLayout child -- use Layout.preferredHeight or Layout.fillHeight
- **Mitigation**: Проверить владение геометрией и оставить один механизм размещения для затронутого объекта.

#### [L-419] PRF-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:112`
- **Rule**: PRF-1
- **Finding**: Rectangle with 'transparent' color -- use Item instead, or toggle visible on the Rectangle (creates geometry node even when transparent)
- **Mitigation**: Проверить необходимость дополнительного узла/эффекта и убрать лишние затраты там, где сохраняется поведение.

#### [L-420] JS-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:151`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-421] JS-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:156`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-422] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:180`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-423] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:185`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-424] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:218`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-425] JS-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:226`
- **Rule**: JS-1
- **Finding**: Use let/const instead of var -- var has function scope and hoisting bugs
- **Mitigation**: Использовать блочную область видимости и явные сравнения с сохранением семантики.

#### [L-426] BND-2
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:229`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'confirmDialog.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-427] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:241`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-428] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:250`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-429] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:275`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-430] LAY-2
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:279`
- **Rule**: LAY-2
- **Finding**: width: inside RowLayout child -- use Layout.preferredWidth or Layout.fillWidth
- **Mitigation**: Проверить владение геометрией и оставить один механизм размещения для затронутого объекта.

#### [L-431] LAY-2
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:280`
- **Rule**: LAY-2
- **Finding**: height: inside RowLayout child -- use Layout.preferredHeight or Layout.fillHeight
- **Mitigation**: Проверить владение геометрией и оставить один механизм размещения для затронутого объекта.

#### [L-432] PRF-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:283`
- **Rule**: PRF-1
- **Finding**: Rectangle with 'transparent' color -- use Item instead, or toggle visible on the Rectangle (creates geometry node even when transparent)
- **Mitigation**: Проверить необходимость дополнительного узла/эффекта и убрать лишние затраты там, где сохраняется поведение.

#### [L-433] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:330`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-434] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:343`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-435] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:352`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-436] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeFieldRow.qml:361`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-437] IMP-3
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:2`
- **Rule**: IMP-3
- **Finding**: import QtQuick.Controls without style qualifier -- use Controls.Basic (or specific style) when customizing contentItem/background/indicator/handle
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-438] IMP-2
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:4`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-439] IMP-2
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:6`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-440] STY-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:11`
- **Rule**: STY-1
- **Finding**: Top-level component should have id: root (enables qualified lookup, future-proofs for QML 3)
- **Mitigation**: Привести имя/структуру объявления к правилу без функциональных изменений.

#### [L-441] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:15`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-442] BND-2
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:32`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'messageBox.visible' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-443] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:97`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-444] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:100`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-445] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:107`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-446] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:112`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-447] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:120`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-448] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:125`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-449] BND-2
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:139`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'profileNameError.visible' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-450] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:155`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-451] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:158`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-452] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:165`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-453] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:185`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-454] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:193`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-455] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:197`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-456] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:240`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-457] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:265`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-458] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:296`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-459] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:305`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-460] BND-2
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:312`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'profileNameField.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-461] BND-2
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:313`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'profileDescriptionField.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-462] BND-2
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:314`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'profileNameError.visible' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-463] BND-2
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:324`
- **Rule**: BND-2
- **Finding**: Imperative '=' on 'customThemeName.text' destroys its binding -- use Qt.binding() to restore, or verify this is intentional
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-464] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:364`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-465] BND-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:372`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-466] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:374`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-467] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:415`
- **Rule**: ORD-1
- **Finding**: id appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-468] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:421`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-469] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:430`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-470] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:438`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-471] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:447`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-472] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:464`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-473] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:494`
- **Rule**: ORD-1
- **Finding**: id appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-474] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:500`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-475] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:509`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-476] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:521`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-477] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:534`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-478] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:543`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-479] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:553`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-480] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:583`
- **Rule**: ORD-1
- **Finding**: id appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-481] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:589`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-482] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:598`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-483] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:611`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-484] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemePage.qml:617`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-485] IMP-2
- **File**: `gui/qt-app/qml/pages/ThemeSectionFields.qml:4`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-486] IMP-2
- **File**: `gui/qt-app/qml/pages/ThemeSectionFields.qml:6`
- **Rule**: IMP-2
- **Finding**: Versioned import -- Qt 6 does not require version numbers on imports
- **Mitigation**: Привести импорты к правилу; после изменения проверить доступность типов и выбранный стиль Controls.

#### [L-487] BND-1
- **File**: `gui/qt-app/qml/pages/ThemeSectionFields.qml:17`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-488] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeSectionFields.qml:34`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-489] BND-1
- **File**: `gui/qt-app/qml/pages/ThemeSectionFields.qml:52`
- **Rule**: BND-1
- **Finding**: property var -- use a typed property (int, string, etc.) for qmlsc compilation and type safety
- **Mitigation**: Уточнить тип/зависимость свойства согласно правилу, сохранив поддержку моделей Python.

#### [L-490] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeSectionFields.qml:52`
- **Rule**: ORD-1
- **Finding**: property declaration appears after property assignment -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

#### [L-491] PRF-1
- **File**: `gui/qt-app/qml/pages/ThemeSectionFields.qml:58`
- **Rule**: PRF-1
- **Finding**: Rectangle with 'transparent' color -- use Item instead, or toggle visible on the Rectangle (creates geometry node even when transparent)
- **Mitigation**: Проверить необходимость дополнительного узла/эффекта и убрать лишние затраты там, где сохраняется поведение.

#### [L-492] STY-3
- **File**: `gui/qt-app/qml/pages/ThemeSectionFields.qml:64`
- **Rule**: STY-3
- **Finding**: 4 anchors.* properties using dot notation -- use group notation: anchors { ... }
- **Mitigation**: Привести имя/структуру объявления к правилу без функциональных изменений.

#### [L-493] ORD-1
- **File**: `gui/qt-app/qml/pages/ThemeSectionFields.qml:76`
- **Rule**: ORD-1
- **Finding**: property assignment appears after attached property -- expected order: id, properties, signals, assignments, attached, states, transitions, handlers, children, functions
- **Mitigation**: Переставить объявления по принятому порядку без изменения поведения.

---

### Deep analysis findings

#### [D-001] Страница компонентов удерживает устаревший массив
- **File**: `gui/qt-app/qml/pages/ThemeComponentsPage.qml:17`
- **Category**: Bindings & Properties
- **Confidence**: 98/100
- **Finding**: Переданный при открытии components остаётся прежним при замене списка групп контроллером; значения, choices, staging и предупреждения не обновляются.
- **Trace**: Main.qml:75–79 и ThemePage.qml:392–397 передают group.components обычным значением. controllers/theme_bundle.py:161–168 заменяет _groups и испускает groupsChanged; setComponent:251–258 вызывает refresh. Страница не выводит свой массив из актуального groups.
- **Mitigation**: Передавать groupId и реактивно получать актуальные компоненты либо обновлять массив при groupsChanged. Проверить изменение, Refresh и Undo без закрытия страницы.

#### [D-002] Журнал результатов вытесняет таблицу зависимостей
- **File**: `gui/qt-app/qml/pages/DependenciesPage.qml:289`
- **Category**: Layout & Anchoring
- **Confidence**: 98/100
- **Finding**: Label всего журнала не ограничен по высоте, конкурирует с таблицей за высоту фиксированной страницы и выходит за её границу.
- **Trace**: Offscreen, подставной контроллер, окно 820×680: без строк таблица539px; 40 строк — журнал680px, таблица0px, нижняя граница журнала y775. controllers/dependencies.py:115 добавляет каждый item.
- **Mitigation**: Выделить ограниченную прокручиваемую область результатов и гарантировать таблице место. Проверить массовые операции и длинные сообщения.

#### [D-003] Завершённые работы удерживают сигнальные объекты
- **File**: `gui/qt-app/src/controllers/theme_inspector.py:64`
- **Category**: Component Loading & Lifecycle
- **Confidence**: 98/100
- **Finding**: _run создаёт _Signals(owner), соединяет done и не освобождает объект/подключение после завершения; на постоянных контроллерах накапливаются children и callback closures.
- **Trace**: 30 последовательных успешно завершённых задач на одном QObject: completed30, retained_signal_objects30. Helper используется Inspector, palette и theme-bundle.
- **Mitigation**: Явно управлять временем жизни каждой работы и освобождать helper после фактического завершения worker. Сохранить защиту от преждевременного удаления; проверить завершение приложения.

#### [D-004] Ошибка картинки оставляет вечный индикатор загрузки
- **File**: `gui/qt-app/qml/Main.qml:546`
- **Category**: Component Loading & Lifecycle
- **Confidence**: 99/100
- **Finding**: Условие running: status !== Image.Ready остаётся истинным в Image.Error.
- **Trace**: Offscreen с отсутствующим файлом: imageError=True, busy=True. CliWallpaperCore.current возвращает путь Plasma, контроллер передаёт его без гарантии наличия файла.
- **Mitigation**: Запускать индикатор для Image.Loading, предусмотреть ошибку/резервное изображение и проверить повреждённый файл.

#### [D-005] Диалог каталога вызывает отсутствующий метод QML url
- **File**: `gui/qt-app/qml/pages/SettingsPage.qml:23`
- **Category**: States & Structure
- **Confidence**: 100/100
- **Finding**: FolderDialog.onAccepted вызывает selectedFolder.toLocalFile(), которого нет у QML url; настройка каталога не меняется.
- **Trace**: Offscreen с тем же выражением: TypeError: Property toLocalFile of object file:///tmp/pictures is not a function. Python setter controllers/__init__.py:1062 ожидает строку.
- **Mitigation**: Передавать URL в Python-слот и преобразовывать через QUrl.toLocalFile. Проверить пробелы, Unicode и отмену диалога.

#### [D-006] Поздний ответ поиска заменяет актуальную галерею
- **File**: `gui/qt-app/src/controllers/palette.py:109`
- **Category**: Performance & Quality
- **Confidence**: 99/100
- **Finding**: Все завершения searchGallery без проверки актуальности заменяют _gallery, даже если пользователь уже запустил новый запрос.
- **Trace**: PalettePage.qml:424,432 допускает повторные запросы. Подставной core: old250мс, new20мс; completion history=new,old; итог=old. Сетевой запрос может выполняться до60с.
- **Mitigation**: Ввести номер поколения и публиковать только последний результат; ограничить или отменять устаревшие работы. Проверить завершение в обратном порядке.

---

### Investigation targets (human verification needed)

#### [I-001] Синхронное обновление после каждого перемещения Slider
- **File**: `gui/qt-app/src/controllers/theme_bundle.py:162`
- **Category**: Performance & Quality
- **Confidence**: 75/100
- **Finding**: refresh синхронно собирает группы, приложения и профили; ThemeComponentsPage.qml:180 вызывает изменение из Slider.onMoved. Повторяются запрос обоев и перечисление ресурсов.
- **Unverified because**: нет измерения задержки кадра в реальной сессии с большим числом ресурсов.
- **How to verify**: профиль UI-потока/частоты кадров при непрерывном перемещении slider на репрезентативной библиотеке тем; затем решать вопрос coalescing/cache/worker.

#### [I-002] Live-инструменты открываются из просмотра сохранённой темы
- **File**: `gui/qt-app/qml/pages/ThemeApplicationsPage.qml:119`
- **Category**: Bindings & Properties
- **Confidence**: 65/100
- **Finding**: приложения читаются из Live; у страницы нет guard readOnly, карточки приложений/палитры доступны и из saved scope.
- **Unverified because**: переход к глобальным Live-инструментам может быть намеренной семантикой; одного отсутствия guard недостаточно для признания бага.
- **How to verify**: сверить продуктовое требование и подписи, пройти сохранённую тему; явно различить инструменты Live и редактирование снимка.

---

### Summary

| Category | Lint | Deep | Investigate | Total |
|----------|------|------|-------------|-------|
| BND | 83 | 0 | 0 | 83 |
| DEL | 2 | 0 | 0 | 2 |
| IMG | 1 | 0 | 0 | 1 |
| IMP | 43 | 0 | 0 | 43 |
| JS | 48 | 0 | 0 | 48 |
| LAY | 10 | 0 | 0 | 10 |
| ORD | 275 | 0 | 0 | 275 |
| PRF | 10 | 0 | 0 | 10 |
| STY | 21 | 0 | 0 | 21 |
| Bindings & Properties | 0 | 1 | 1 | 2 |
| Component Loading & Lifecycle | 0 | 2 | 0 | 2 |
| Layout & Anchoring | 0 | 1 | 0 | 1 |
| ListView & Delegates | 0 | 0 | 0 | 0 |
| Performance & Quality | 0 | 1 | 1 | 2 |
| States & Structure | 0 | 1 | 0 | 1 |
| **Total** | **493** | **6** | **2** | **501** |

Findings below confidence 60 are suppressed entirely.

Новых подтверждённых ошибок моделей/required-ролей и reuse делегатов не обнаружено. Формальных State/Transition в дереве нет. Это не утверждение об отсутствии всех возможных ошибок.
