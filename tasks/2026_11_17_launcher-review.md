# Ревью слияния лаунчеров

Статус: DONE — 2026-11-17 в описанных границах. Находки ревью исправлены;
финальный native preflight и release build PASS. Безопасность Walker исправлена;
полная автоматизация вне существующего CSS остаётся в P4 основной очереди.
При начале проверки рабочее дерево было чистым; проверено слияние `78aca01`
относительно первого родителя. Во время первоначального ревью код и установленные
приложения не менялись. Следующий запрос пользователя разрешил исправления.

## Находки исходного ревью (исправления ниже)

1. **P1 — чужие настройки попадают в личные конфиги.**
   `src/launcher_apps.rs::plan` читает ресурс целиком, включая Rofi host.
   `appearance.rs` не подключает launcher capture/merge/restore: эти пути имеют
   `Policy::Whole`. В изолированном HOME применение пакета с
   `configuration { modi: "run"; kb-accept-entry: "Control+X"; }` заменило
   личный Rofi config. Fuzzel принял чужие `[main]`, `font`, `launch-prefix`.
   Наличие одного RGB в Rasi/CSS не делает остальной файл appearance-only:
   ресурс с `@import "/tmp/foreign.rasi"` тоже принимается.
   Нужны whitelist данных и единая политика capture/import/apply/export.

2. **P1 — возврат после личной правки оставляет старые цвета и битую ссылку.**
   Применить palette-тему с тремя launcher targets, добавить личный комментарий
   в каждый live host, применить нейтральную тему без ресурсов и targets.
   Rofi `config.rasi` сохраняет `@theme` на удалённый `themes/kolira.rasi`;
   Fuzzel и Walker сохраняют `BEGIN kolira colours` и предыдущие цвета.
   Причина: `appearance::restore_changed` использует `Policy::Whole` и возвращает
   весь current. Функция `launcher_apps::restore` вообще не вызывается;
   простое её подключение тоже недостаточно: её ветка Rofi удаляет все `@theme`,
   включая личные. Нужен возврат только принадлежащих Kolira изменений и тесты
   отсутствующих файлов, поздних цветов, комментариев и ссылок.

3. **P1 — комплектный Rofi ресурс синтаксически невалиден.**
   Нативный `rofi -no-config -rasi-validate` для Everforest возвращает код 1:
   `syntax error, unexpected Hexidecimal color value` на строке
   `border: 1px solid #...`. Генератор использует CSS вместо Rasi, меняет
   padding/radius и использует неверные имена ряда свойств/селекторов.
   Подключение через `@theme` сбрасывает прежнюю тему вместе с layout;
   для цветового дополнения нужен сохраняющий layout механизм.
   [Формат Rofi](https://davatorium.github.io/rofi/current/rofi-theme.5/).

4. **P1 — комплектный Fuzzel ресурс не загружается.**
   Нативный Fuzzel сообщает `[color]: invalid section name: color`.
   Нужны `[colors]`, нативные ключи selection/selection-text и формат RGBA,
   а не текущие `background-selected`, `text-selected`, `#rrggbb`.
   Проверка сделана во временном Docker-контейнере; отсутствие Wayland отдельно
   видно в stderr и не является причиной ошибки секции.

5. **P2 — Walker не получает выбранную тему автоматически.**
   Пишется `.config/walker/style.css`, без выбора темы. Текущий upstream
   загружает `themes/<name>/style.css` с `theme` в `config.toml`; CSS использует
   классы `.input`, `.item-box`, а не текущие `#input`/`#entry`/`#text`.
   Версия клиента не проверяется. Нужен адаптер подтверждённой версии с
   сохранением текущего layout либо перенос в очередь неподдержанных сценариев.
   [Upstream Walker](https://github.com/abenz1267/walker#theming).

6. **P2 — повторное применение дублирует Rofi подключение.**
   Проверяется наличие `@theme "kolira"`, записывается абсолютный путь.
   Два последовательных Apply в изолированном HOME оставили две управляемые
   строки. Существующий unit-тест повторяет другой алгоритм с проверкой полного
   пути и потому не проверяет реализацию.

7. **P2 — maintenance refresh не генерирует ресурсы новых лаунчеров.**
   В восьми пакетах добавлены файлы, но везде осталось 35 `app_targets`, без
   Rofi/Fuzzel/Walker. `examples/refresh_theme_apps.rs` не включает их targets
   и `bundle_resources`; следующий refresh не обновит их цвета из палитры.

## Проверки до исправлений

- `cargo test`: 294 passed, 0 failed; четыре существующих launcher integration
  tests тоже проходят, хотя перечисленные нарушения воспроизводятся отдельно.
- `cargo build --release`, `cargo clippy --all-targets -- -D warnings`: PASS.
- Python/Qt offscreen: 345 passed; shell harness: 260 passed; shellcheck: PASS.
- `cargo fmt -- --check`: FAIL — форматирование нового launcher модуля и тестов.
- Native Starship RGB, Foot parser и WezTerm loader: все восемь тем PASS в
  существующем изолированном Docker harness `tests/e2e/four-apps-native.py`.
- Native Rofi/Fuzzel: ошибки парсеров выше, проверен комплектный Everforest.
- Отдельный CLI repro в disposable HOME подтвердил пункты 1, 2 и 6, без
  внешних команд (`KTM_THEME_NO_APPLY=1`).

Полный сценарий настоящей Plasma, live Walker и GUI визуально не проверялись.
Установка, Apply на рабочем столе, коммит и push не выполнялись.
Зелёный общий набор тестов не означает готовность лаунчеров: нужны регрессии
на реальные нарушения и native проверки исправленных ресурсов.

## Реализованные исправления

- Hosts Rofi/Walker исключены из save/import/export; ресурсы Rasi/CSS принимают
  только известные цветовые свойства и RGB. Fuzzel фильтруется по `[colors]`.
  Снимок без палитры поддерживается; приоритет явного ресурса сохраняется.
- Rofi получает inline overrides вместо сбрасывающего layout `@theme`. Native
  состояния normal/alternate/selected покрыты; include не нужен и не дублируется.
- Fuzzel получает нативные RGBA и ключи, сохраняет личную alpha, font и команды.
  Поздняя alpha сохраняется и при возврате RGB исходной темы.
- Restore интегрирован в общий журнал. Цвета возвращаются по значениям даже
  после переформатирования; комментарии, личные свойства/цвета и неизвестные
  изменения сохраняются. Отсутствующие файлы восстанавливают отсутствие.
- Битые/дублированные маркеры, личный нецветовой ресурс и ссылки проверяются
  до записи. Все записи идут через Plan/atomic permission-preserving writer.
- Walker 2.x меняет существующий CSS выбранной темы. Встроенный default CSS,
  SCSS, внешние темы и другие версии пропускаются; **Walker остаётся P4** до
  native Apply/перезапуска/возврата. Подмена встроенного CSS изменила бы layout.
  Старый `walker/style.css`, конфиг выбора темы и XML сохраняются личными.
- Maintenance refresh включает все три ресурса и targets. Восемь пакетов
  обновлены из палитр, по 38 targets; SHA-256 обновлены. Повторный refresh
  всех восьми пакетов побайтно стабилен; авторские KDE choices сохранены.
- Удалены недостижимые/неатомарные launcher ветки `palette apply`: эти цели
  никогда не входили в его public TARGETS. Поддерживаемый путь — theme Apply.
- Исправлен GUI-тест реестра: свежий core содержит 56, а не 53 записи.
  Добавлены переводы метаданных лаунчеров; каталог: 923 finished, 0 unfinished.

## Проверки исправлений

- 15 launcher integration regressions: три цикла, исходные права/байты,
  отсутствующие hosts, поздние личные цвета/alpha/комментарии/свойства,
  переформатирование, повторный Apply, отказ записи с откатом, malicious
  resources/foreign commands/hosts, snapshot без палитры, XDG, ссылки,
  повреждённые маркеры, version guards и отсутствие probes при NO_APPLY.
- Общий Rust: 299 passed; дополнительно 3 generator tests, clippy, fmt PASS.
- Python/Qt на свежем debug core: 345 passed; shell: 260 passed;
  release build, shellcheck PASS.
- `tests/e2e/launchers-native.py`: восемь тем, настоящий Rofi layout/parser,
  Fuzzel parser/opacity и GTK4 вычисленный текстовый цвет/return после late edits.
  Настоящие версии: Rofi 2.1.1, Fuzzel 1.15.0. Preflight отдельно проверяет
  `KTM_THEME_NO_APPLY=0` на всех восьми пакетах PASS; Rofi использует `-version`,
  остальные — `--version`.
- Регрессионный `four-apps-native.py`: Starship/Foot/WezTerm всех восьми тем PASS.

Воспроизводимый launcher parser/GTK прогон (сначала собрать debug Rust CLI):

```bash
docker build --network=host -f tests/e2e/Dockerfile.launchers -t kolira-launchers-native tests/e2e
docker run --rm --network none -v "$PWD:/src:ro" kolira-launchers-native
```

GTK CSS проверяется реальным GTK4, но это не полный native Walker UI.
До отдельного запроса установки эти проверки не затрагивали установленный код
и локальные темы. Результаты последующей установки приведены ниже.

## Локальная установка (2026-11-17)

DONE: установка, локальные проверки и обязательный clean-machine Plasma прогон
перед коммитом завершены успешно по отдельному запросу пользователя. Первый installer обновил CLI, затем остановился на старых локальных
пакетах: `unknown app palette target: chromium`. GUI тогда ещё не обновился.
Добавлена совместимость retired Chromium/Chrome для load/import, файловых
политик и всех форматов undo-journal; расширена регрессия retired clients.
Генератор очищает эти цели/ресурсы в staging. Восемь локальных тем обновлены
из копий с сохранением палитр/KDE choices и импортом с `pre-import` backup.

Локальная поставка:

- Штатный `scripts/install-kolira.sh --restart` завершился успешно.
  Код/launcher/Desktop Entry сохранены в
  `~/.config/kolira/backups/deploy-kolira-20261117-182426/`.
- Исполняемый `~/.local/bin/kolira` побайтно совпадает с текущей release сборкой.
  Установленные Python entry point, i18n strings и QM совпадают с исходниками.
- Восемь локальных пакетов содержат по 38 app_targets; исходные base/accent/
  variant и ColorScheme/AccentColorFromWallpaper сохранены. Для всех восьми
  проверено наличие оригинального pre-import бэкапа.
- Installed registry: 56 entries; Rofi/Fuzzel/Walker присутствуют. Native клиенты
  этих лаунчеров локально не установлены; их parser/GTK проверки выполнены в Docker.
- Настоящий installed GUI открыл Applications page, сохранил PNG 1025×850 и
  штатно завершился. Визуальная проверка: окно загружено, карточки/текст читаемы,
  ошибок загрузки нет. Депрекация Qt `QLibraryInfo.location` — существующая.
- Восемь preview установленным CLI на реальных конфигах: PASS,
  `KTM_THEME_NO_APPLY=0`, offline, keep-wallpaper; фактический Apply не запускался.
  Хеши kdeglobals/kwinrc/plasmashellrc/appletsrc/active-profile неизменны,
  активный профиль остаётся `everforest`.
- Повторно после совместимости старых целей: 299 Rust + 3 generator,
  345 Qt на **установленном** CLI, 260 shell, clippy, fmt, shellcheck, qmllint PASS;
  переводы: 923 finished, 0 unfinished.

Временные журналы и снимки этой проверки находятся в ignored
`tmp/validation/local-install/`; они не включаются в коммит.
Push не запрашивался и не выполняется.

Clean-machine Plasma проверка завершена: **ALL PASSED**. Тест запущен на отдельном
Git-снимке текущего рабочего дерева (не на прежнем HEAD), в `ktm-friend` под
непривилегированным пользователем с настоящими KWin/plasmashell и D-Bus.
Проверены шесть стандартных тем: frutiger-aero-light, frutiger-aero, Paper,
dracula, everforest, glass. Каждый CLI Apply → возврат восстановил файлы и
GSettings; сессия и GUI живы. Финальный цикл через GUI controller тоже вернул
исходные файлы; status и проверка crash logs PASS. Визуальный вид KDE в этом
контейнерном сценарии не оценивается; optional внешние theme assets не устанавливались.
Все обязательные проверки перед коммитом завершены. Обновлённый GUI запущен
локально; активный Everforest и исходные файлы рабочего стола сохранены.
