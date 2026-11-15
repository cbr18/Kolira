# Ревью b6596af..aa45e73 и исправлений

2026-11-15. Запрос: code review, тесты и слияние explore/two-stage-apply в main.
Ревью выполнено по diff восьми коммитов и связанным путям Apply/preview/undo,
а не только по unit-тестам. Все перечисленные функциональные находки исправлены.

| Находка | Уверенность | Исправление / проверка |
|---|---:|---|
| Проверка живых симлинков назначения удалена | 100 | Проверка восстановлена, audit regression проходит |
| editor local_path не валидировался; пустая строка выбирала корень всей темы | 100 | Подкаталог assets/editor-themes, валидация ID/дерева до записи |
| install_from_cache копировал themes/node_modules как расширения и удалял одноимённые каталоги | 100 | Полное расширение в собственном namespace, исходные сохранены, ZIP/restore regression |
| create_native_extension копировал файл сам на себя, обнуляя содержимое | 100 | Метаданные меняются отдельным планом без self-copy, регрессия |
| Новый кэш не попадал в экспорт/импорт | 100 | Кэши включены в private ZIP, public исключает бинарники без лицензии |
| Open VSX latest возвращал метаданные вместо VSIX; удалялись все компоненты package в ZIP-пути | 100 | Чтение download URL, целая структура, предел распаковки и запрет traversal/ссылок |
| Stage 2 отсутствовал в preview, полной recovery-копии и журнале следующей темы | 100 | Общий Plan, один lock/backup/journal, проверены restore и rollback при ошибке записи |
| Ошибки адаптеров попадали в synced как успешные строки | 100 | Ошибка плана/commit завершает транзакцию с откатом |
| Любая палитра навязывала оформление всем установленным приложениям | 100 | Явные app_targets[], default [], snapshot имеет приоритет; shell regressions |
| Chromium заменял Preferences.profile целиком и сохранял личный Preferences в теме | 100 | Этот неподдерживаемый адаптер исключён; отсутствие записи проверяется |
| Firefox затирал пользовательский CSS, не включал поддержку stylesheet и доверял пути профиля | 100 | Отдельный импорт CSS, appearance-only user.js, общий план путей и recovery |
| GUI передавал without_sync в метод, который его не принимает | 100 | Опция в реальном profile preview; live Apply вернул корректный вызов |
| Новый openSystemSettings перекрывал метод открытия KCM | 100 | Один метод, две Qt-перегрузки; тест обоих способов |
| saveProfile регистрировал один аргумент, QML передавал два | 100 | Qt-перегрузка str,str проверена через metaObject |
| Save as theme сохранял прежнее оформление после импорта/редактирования палитры | 100 | Gate после KDE apply и атомарная повторная сверка live colors в Rust |

## QML Code Review Report

**Scope**: diff `git diff main` для PalettePage.qml, ThemePage.qml,
ApplyPreviewDialog.qml и связанного Python-кода.
**Files reviewed**: 3 QML + Python glue.
**Issues found**: 4 функциональные находки deep analysis, исправлены.
**qmllint**: ran, весь qml/, exit 0.

### Lint findings

Детерминированный сканер навыка выполнен до и после ревью. В добавленных блоках
он отметил порядок свойств ORD-1 (как в окружающем коде) и BND-2 для сброса
локальных text/visible полей формы. BND-2 проверены: это намеренное изменение
локального состояния, динамические bindings там не восстанавливаются. ORD-1
не влияет на выполнение; массовая перестановка существующего QML не выполнялась.
Полные диагностические логи — `/tmp/kolira-review-qml-lint-final.log`.

### Deep analysis findings

| ID | Категория | Файл/место | Уверенность | Решение |
|---|---|---|---:|---|
| D-001 | Bindings & Properties | theme_bundle.py applyChanges | 100 | Убран неверный аргумент live Apply |
| D-002 | Layout & Anchoring | ThemePage длинная галочка в Flow | 88 | Опция перенесена в preview с wrapping Label |
| D-003 | Component Loading & Lifecycle | theme_bundle.py openSystemSettings | 100 | Qt-перегрузки вместо двух Python-методов |
| D-004 | Bindings & Properties | PalettePage Save | 95 | Сверка live KDE-палитры перед сохранением |

Trace/mitigation: см. таблицу выше и регрессии test_theme_bundle.py,
test_palette.py, tests/two_stage.rs. По ListView/Delegates, States/Structure
и Performance/Quality новых подтверждённых дефектов нет. Шесть проходов
выполнены; финальные изменения повторно просмотрены по тем же категориям.

### Investigation targets (human verification needed)

Нет неподтверждённых функциональных находок, блокирующих слияние. Запуск каждого
реального редактора, сетевые установки и полная Plasma-визуальная матрица
остаются за пределами этой проверки.

### Summary

| Category | Lint | Deep | Investigate |
|---|---|---:|---:|
| Bindings/Properties | локальное состояние формы проверено | 2, исправлены | 0 |
| Layout/Anchoring | стиль существующего кода | 1, исправлена | 0 |
| Lifecycle | — | 1, исправлена | 0 |
| Delegates/States/Performance | — | 0 | 0 |

Результаты полных проверок и слияния находятся в
[tasks/2026_11_15_two-stage-review-and-merge.md](../../tasks/2026_11_15_two-stage-review-and-merge.md).
