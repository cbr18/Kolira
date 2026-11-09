#!/usr/bin/env bash
# update-translations.sh — обновить .ts из исходников и собрать .qm.
#
# Два экстрактора, ДВА .ts (ни один не сохраняет контексты другого):
#   lupdate      — строки QML (qsTr)        → i18n/qml_ru.ts
#   pylupdate6   — строки Python (tr())     → i18n/py_ru.ts
# Сборка: lrelease сливает оба .ts в один kolira_ru.qm.
#
# Новые строки появляются как "unfinished" — переведите их в соответствующем
# .ts и перезапустите этот скрипт.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="$SCRIPT_DIR/gui/qt-app"
QML_TS="$APP_DIR/i18n/qml_ru.ts"
PY_TS="$APP_DIR/i18n/py_ru.ts"
QM_FILE="$APP_DIR/i18n/kolira_ru.qm"

LUPDATE="${LUPDATE:-lupdate}"
PYLUPDATE6="${PYLUPDATE6:-pylupdate6}"
LRELEASE="${LRELEASE:-lrelease}"

for tool in "$LUPDATE" "$PYLUPDATE6" "$LRELEASE"; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "$tool not found (qt6-tools / pyside6; or set the env override)" >&2
    exit 1
  fi
done

cd "$APP_DIR"
echo "== lupdate: строки QML → qml_ru.ts =="
"$LUPDATE" qml -ts "$QML_TS"

echo "== строки из данных (поля, приложения, ядро) → src/core/i18n_strings.py =="
"${PYTHON:-python3}" tools/gen_i18n_strings.py

echo "== pylupdate6: строки Python → py_ru.ts =="
"$PYLUPDATE6" src --ts "$PY_TS"
"${PYTHON:-python3}" tools/gen_i18n_strings.py --fill

echo "== lrelease: слияние в kolira_ru.qm =="
"$LRELEASE" "$QML_TS" "$PY_TS" -qm "$QM_FILE"

echo "Done: $QM_FILE"
