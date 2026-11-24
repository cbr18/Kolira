# Задачи проекта

Актуализировано 2026-11-23 по файлам задач и текущему коду. Это индекс локальных
задач; `plans/` хранит планы, `docs/ROADMAP.md` — направление развития.
Статус DONE означает завершённую реализацию в пределах описанной проверки,
а не проверку всех сценариев на настоящем рабочем столе.

## Открыто

| Задача | Что осталось |
|---|---|
| [Kolira Quick Switch](2026_11_24_kolira_quick_switch.md) | Реализовано и влито в 0.9.5 (оверлей, карусель, шейдер, настройки, хоткей); остался живой прогон на Plasma и правки по нему. План — [plans](../plans/2026_11_24_kolira_quick_switch.md). |
| [Реальный цикл зависимостей из AUR](2026_11_08_aur-dependency-cycle.md) | Код поддерживает AUR (источник "aur", команда `yay -S {id}`, проверка наличия helper). Требует реального тестирования в изолированном Arch Linux контейнере. |
| [Обои: несколько изображений и живые обои](2026_11_24_wallpapers-multi-and-live.md) | Крупная доработка: несколько обоев на тему (по монитору/слайд-шоу) и подключение живых обоев/провайдеров. Затрагивает формат темы, ядро, apply/undo, зависимости, export/import, GUI. Требует декомпозиции на подэтапы. |

Блокирующие дефекты F01/F02 аудита и защита применения реализованы.
Приоритетные расписание, аудит смен и MCP реализованы и проверены.
Границы визуальной проверки объединённого UI записаны в его отдельной задаче.
Аудит смен тем использует
ID операций и бэкапы защиты применения; AUR и сценарий палитры имеют отдельные карточки.
Каталог Plasma не дублируем; установка полных тем по Git-ссылке не планируется.
Работы по дополнительным источникам/библиотеке обоев отложены пользователем.

## Закрыто

- [Полная проработка kolira lint](2026_11_24_lint-full-coverage.md) — DONE 2026-11-24: lint теперь проверяет Kvantum (тултип/текст/акцент), терминалы ghostty/wezterm/foot (bright black + foreground) и GTK colors.css (fg/bg), помимо существующих icons/colors/kitty/alacritty/konsole. Ловит светлый тултип Sandstone (Paper). 363 Rust PASS, clippy/fmt чисты, 0 ложных на 9 bundled-темах.
- [Политика хранения бэкапов и журнала](2026_11_24_backup-retention.md) — DONE 2026-11-24: `backups/` и `operations/` ограничены (12 записей / 1 GiB для пары запись+`pre-apply`, до 5 `deploy-kolira`, до 10 одиночных копий / 14 дней); атомарная эвикция пары, защита незавершённых/восстановительных операций, best-effort после apply. 359 Rust PASS, clippy/fmt чисты.
- [UX-редизайн Kolira](../plans/2026_11_22_kolira_ux_redesign.md) — галерея, детали, редактор, сводка применения, зависимости, история, восстановление и интеграция разделов проверены в `ux/redesign-plan`.
- [Более реалистичная миниатюра темы](2026_11_22_theme_thumbnail_fidelity.md) — общий шаблон учитывает прозрачность, шрифты и геометрию.
- [Текст карточек тем и список приложений после применения](2026_11_23_theme_card_and_apply_followup_layout.md) — текст остаётся внутри карточки, общая рекомендация сопровождается списком. Закрывает прежнюю [задачу группировки предупреждений](2026_11_18_appearance-reload-one-line.md).

- [Core audit 2026-11-22](2026_11_22_core-audit.md) — F1/F2 исправлены и CLI установлен; 308 Rust / 260 shell PASS, clippy PASS. UI/UX не менялся.

- [Zen legacy user.js](2026_11_22_zen-legacy-userjs.md) — исправлена очистка некорректных appearance assignments; визуальная проверка после перезапуска Zen остаётся.

- [Покрытие приложений: существующий срез](2026_11_15_app-coverage-expansion.md) — CLOSED 2026-11-17
  по уточнению пользователя: дальше покрытие в этой задаче не расширяем.
  [Матрица всех 56 registry entries](../docs/audits/2026_11_17_existing-adapters-closeout.md).
  Исправлены btop native-format return и существующий Spicetify safety/error/return.
  306 Rust + 3 generator / 345 Qt / 260 shell PASS; native Firefox/Zen,
  btop/Yazi/lazygit RGB туда/обратно, Prism/Heroic/CLI/GTK и контейнерные
  launcher/terminal проверки восьми палитр. Plasma 6 циклов + GUI return ALL PASSED.
  CLI обновлён локально; новых приложений/targets нет, ограничения P4 сохранены.

- [Исправления лаунчеров](2026_11_17_launcher-review.md) — DONE 2026-11-17:
  исправлены native Rofi/Fuzzel, ownership/возврат, повторный Apply и генератор.
  299 Rust + 3 generator / 345 Qt / 260 shell PASS; release, clippy, fmt,
  shellcheck, 923 finished переводов. Native Rofi/Fuzzel/GTK4 + preflight/return
  восьми тем PASS; Starship/Foot/WezTerm регрессии восьми тем PASS.
  Walker 2.x ограничен существующим CSS; полная автоматизация остаётся P4.
  CLI/GUI и восемь локальных тем обновлены с бэкапами; installed GUI и реальные
  preview восьми тем PASS. Clean-machine Plasma: шесть тем + GUI возврат ALL PASSED.
  Активный Everforest и файлы рабочего стола сохранены.

- [Starship, tmux, Foot, WezTerm](2026_11_16_four-terminal-apps.md) — DONE:
  код и восемь тем готовы; повторная native проверка Starship/Foot/WezTerm восьми
  пакетов 2026-11-17 PASS; границы tmux и UI описаны в карточке.

- [Аудит терминалов Alacritty/Ghostty](2026_11_16_alacritty-ghostty-audit.md) — DONE 2026-11-16:
  статический аудит файловых адаптеров: Alacritty (toml_edit split/host, palette генерация,
  оба формата импорта) и Ghostty (look-split, include в конце, replace_block для палитры).
  Найдены пробелы: ALACRITTY_IMPORT использует `~` вместо XDG_CONFIG_HOME, ghostty использует
  home_dir() вместо XDG resolver, нет тестов на возврат хост-конфига для Ghostty.
  Живая проверка не проводилась.

- Очередь автоматизации — актуализирована 2026-11-16: ручные действия и
  неподтверждённый hot reload вынесены в P4 в конец
  [приоритетов](../docs/APP_COVERAGE_PRIORITIES.md). Native Chromium: 32 цикла
  возврата PASS, GTK/Qt FAIL; интеграция отложена,
  [исследование и воспроизведение](2026_11_16_chromium-policy-sync.md).

- [Удаление ручной интеграции мессенджеров](2026_11_16_remove-telegram.md) — DONE 2026-11-16:
  Telegram/Kotatogram удалены из генерации, GUI/CLI, действий и ресурсов всех восьми тем.
  Личные конфиги и данные клиентов сохраняются. Проверка: 275 Rust / 345 Python/Qt /
  260 shell PASS; clippy, shellcheck, cargo fmt, Python syntax PASS; переводы 918 finished.

- [Chromium / Chrome: автоматический акцент](2026_11_16_chromium-policy-sync.md) — CANCELLED 2026-11-16:
  задача отменена пользователем. Chromium/Chrome убраны из очереди покрытия приложений.
  Исследование chrome.send() механизма осталось в docs/CHROMIUM_RESEARCH.md.

- [Третья партия приложений](2026_11_16_third-ten-apps.md) — DONE 2026-11-16:
  Micro, lsd/Glow, Chromium/Chrome (ручной выбор), GIMP 3, Heroic, Prism, Vim,
  mpv/Cliamp, VSCodium. 53 registry entries; 8 пакетов, 35 targets, LibreOffice
  исключён. 276 Rust / 345 Qt / 260 shell; нативные проверки всех 8 пакетов,
  Docker Plasma + GUI return ALL PASSED. BreezeLight/BreezeDark и акцент от
  обоев Glass/Material You сохранены. Live deploy этой партии не выполнялся.

- [Glass / Material You: акцент от обоев](2026_11_16_glass-material-wallpaper-accent.md) — пользовательская правка DONE;
  maintenance refresh теперь сохраняет её и выбранные BreezeLight/BreezeDark,
  регрессии повторной генерации и согласованности 8 пакетов PASS.

- [Палитра восьми тем и native browser accent](2026_11_16_palette-consistency.md) — DONE 2026-11-16:
  исправлен stale красный GTK Everforest, согласованы palette resources/выбор
  поддержанных приложений. Native Firefox/Zen: 8 тем + возврат, без chrome CSS.
  263 Rust / 345 Qt / 260 shell, clippy/линтеры; 19 GTK CSS, 76 contrast pairs PASS.
  Docker 8 тем + GUI ALL PASSED. Восемь локальных пакетов обновлены с backup,
  Everforest применён с сохранением обоев, операция `apply-hKWDh8`, ошибок нет.

- [Вторая партия P0 № 11–20](2026_11_16_second-ten-apps.md) — DONE 2026-11-16:
  fish/Pure/OMP, btop/Cava, Yazi/bat, Wofi/fzf, lazygit/Zellij, OpenCode V2,
  Claude Code, Codex. 262 Rust / 345 Qt / 260 shell, clippy/линтеры/910 переводов;
  Docker восемь тем и GUI — ALL PASSED (границы snapshot в карточке).
  Code/GUI установлены, восемь локальных тем обновлены с backup, реальные previews PASS.
  Active profile не менялся; по последующему запросу выполнен живой Apply обновлённой Material You
  с сохранением обоев, операция `apply-KnvVpV`, ошибок нет.

- [Установка, обновление тем и Material You](2026_11_15_bundled-apps-and-material-you.md) — DONE 2026-11-15:
  восемь комплектных и локальных пакетов обновлены; Material You в репозитории,
  code/GUI установлены и перезапущены, pre-import/deploy backups сохранены.
  246 Rust / 345 Qt / 260 shell, линтеры/903 перевода; Docker восемь тем PASS,
  preview всех восьми на реальных конфигах PASS. Desktop Apply не запускался.

- [Первые десять приложений P0](2026_11_15_first-ten-apps.md) — DONE 2026-11-15:
  Cursor/Zed, Neovim, выбранные Obsidian vault, безопасные Zen/Firefox/Pywalfox,
  Discord/Vesktop, Kitty, Konsole/Yakuake. 243 Rust / 345 Qt /
  258 shell, линтеры и 903 перевода; реальные браузеры, native Cursor, nvim headless;
  Docker: шесть тем + возврат через GUI — ALL PASSED. Runtime-ограничения в карточке.

- [Возврат исходного состояния при отсутствующих конфигах](2026_11_15_restore-missing-config-baseline.md) — DONE 2026-11-15:
  полный before/after-журнал; отсутствующие конфиги сохраняют baseline вместо
  defaults, личные правки остаются. Обои, GTK/GSettings, KWin, include-файлы и
  второй этап охвачены возвратом. 228 Rust / 345 Qt / 258 shell, линтеры,
  905 переводов; Docker по семи темам, включая Minecraft — ALL PASSED.

- [Ревью и слияние двухэтапного Apply](2026_11_15_two-stage-review-and-merge.md) — DONE 2026-11-15:
  исправлены безопасность кэша/путей, общие preview/backup/rollback и GUI;
  генерация только по явным app_targets[], сохранение палитры проверяет live KDE.
  218 Rust / 345 Qt/Python / 258 shell, линтеры и 905 переводов; два Docker
  friend-прогона ALL PASSED. Восемь коммитов и исправления 1e3cb43 влиты в локальный main.
  Chromium и расширение авторского GUI/CLI остаются backlog с явными границами.

- [Старые прерванные применения](2026_11_13_historical-apply-popup.md) — DONE 2026-11-13:
  убрана повторная всплывающая ошибка при запуске/refresh; история и бэкапы сохранены.
  2026-11-15 фикс включён в main после полного прогона 345 Qt/Python-тестов.

- [MCP/GUI и инструкции агентам](2026_11_10_mcp-app-audit.md) — DONE 2026-11-10:
  ошибки списка тем видимы, MCP требует previewToken, GUI refresh асинхронный.
  README/AGENTS v2 установлены в каталог тем; 204 Rust и 342 Qt/Python прошли,
  1 прежний тест переводов исключён из-за удалённого скрипта.

- [Prompt и Minecraft blur](2026_11_10_prompt-and-minecraft-blur.md) — DONE 2026-11-10:
  исправлено удаление общего prompt в Undo, Minecraft получил Better Blur DX
  и прозрачность. 202 Rust-теста, сборка и живой Apply прошли; эффект загружен,
  prompt рендерится без ошибки, файл обоев декодируется.

- [Minecraft: панели и Zen](2026_11_10_minecraft-followup.md) — DONE 2026-11-10:
  закреплён breeze-dark, сохранён Hack при Apply и Undo старых журналов; локальный
  пакет обновлён с backup, повторный Apply успешен, окно Zen подтверждено KWin.
  200 Rust-тестов и новая интеграционная регрессия прошли.

- [Glass: обои и KDE шрифт](2026_11_09_glass-font-wallpaper.md) — DONE 2026-11-09:
  добавлены `activeFont` и синхронизация GTK font GSettings, чтобы Monocraft не
  перекрывал Noto Sans. Glass повторно применён; Plasma сообщает Saturn на обоих
  экранах, GSettings содержит Noto Sans и системный Monospace. Rust-тесты прошли.

- [Блокировка после сбоя и Plasma вне systemd](2026_11_09_plasma-recovery-outside-systemd.md) — DONE 2026-11-09:
  блокировка только на время операции; старые ошибки/бэкапы остаются без запрета
  новых действий. CLI/GUI установлены, 199 Rust / 336 Qt, clippy и переводы прошли;
  живой Apply и удаление пользовательской темы не выполнялись.

- [Minecraft Overworld Dark](2026_11_09_minecraft-overworld-dark.md) — DONE 2026-11-09:
  исправлены ресурсы/форматы/метаданные, добавлены voxel-обои и перенос иконок;
  проверены export/import/apply в изоляции и все семь предпросмотров.
  Ограничения и сбои общих тестов на параллельных правках — в карточке.

- [Переименование Kolira](2026_11_09_kolira-rename.md) — DONE 2026-11-09:
  сборка/команды/иконка/миграция/старые форматы/комплектные ресурсы исправлены;
  194 Rust / 336 Qt / 257 shell, линтеры и переводы прошли. Без реального Apply.

- [Интерфейс и русский перевод](2026_11_08_ui-consistency-and-localization.md) — DONE 2026-11-09:
  все 18 находок реализованы и объединены с main; 192 Rust / 336 Qt / 256 shell
  прошли; 893 finished-переводов. Живая Plasma-матрица и финальный Docker GUI-цикл
  остановлены по просьбе пользователя (границы в карточке).
- [Расписание](2026_11_08_scheduled-theme-switching.md),
  [аудит 10 смен](2026_11_08_theme-change-audit.md) и
  [MCP/инструкции](2026_11_08_mcp-agent-workflow-core-audit.md) — DONE 2026-11-09:
  Rust/CLI/API/MCP, страницы/адаптер, offline-подготовка и безопасная доставка
  инструкций. 190 Rust / 330 Qt/Python / 256 shell; живой фоновый apply/restore
  в изолированной Plasma. Загрузка инструкций проверена локальными клиентами
  без модели, способы запуска/ограничения документированы.
  [Доказательства и пределы проверки](../docs/audits/2026_11_09_priority-validation.md).
  Проверки объединённого UI и их границы записаны в отдельной задаче.

- [Предварительный diff](2026_11_08_pre-apply-diff.md) — DONE: GUI/CLI/MCP
  используют общий Rust-план и подтверждение, требующее пересмотра при изменении
  состояния. Добавлены сохранение текущей темы и сохранение обоев; 176 Rust,
  319 Qt/Python и 256 shell-проверок прошли.
  [Контракт и пределы](../docs/APPLY_PREVIEW.md).

- [Hover кнопок Paper](2026_11_08_paper-button-hover.md) — DONE:
  убрана лишняя светлая рамка при мышином фокусе/выделении, Tab-фокус сохранён.

- [Защита применения темы](2026_11_08_apply-protection.md) — DONE:
  единая блокировка, подготовка, проверяемый откат, ID/бэкап и восстановление
  прерванных операций; 166 Rust / 314 Qt / 256 CLI-проверок.
  [Контракт и ограничения](../docs/APPLY_PROTECTION.md).

- [Исправления по аудиту кодовой базы](2026_11_08_codebase-audit-fixes.md) — DONE:
  F01–F11, A01–A02 и cleanup. Полный перенос оставшейся логики в Rust завершён;
  [реализация и проверки](../plans/2026_11_08_single-core-completion.md).

- [Создание темы из палитры](2026_11_08_palette-workflow.md) — DONE 2026-11-14:
  PalettePage.qml с галереей, из обоев, импортом, редактированием слотов,
  предпросмотром (PalettePreview) и применением к целям (KDE, kitty, Alacritty,
  Konsole, Discord, Spicetify, Ghostty, cava, Kvantum). Добавлена кнопка
  "Save as theme…" для сохранения результата как применимой темы через
  Rust palette.save_theme с повторной проверкой живых цветов (ревью 2026-11-15). 4 Rust-теста палитры, 39 Qt-тестов theme_bundle проходят.

- [Скачиваемые темы редакторов (VS Code, VSCodium, Zed и др.)](2026_11_14_10_17_editor-theme-downloads.md) — DONE 2026-11-14:
  `editor_themes[]` в `theme.json`, скачивание `.vsix`/тем Zed в `assets/editor-themes/`,
  адаптер установки при `apply`, нативное расширение с `_watch: true` для редакторов,
  поддержка в экспорте/импорте ZIP, исправлены симлинки внутри тем и corrupt package.json.
  134 Rust-теста, clippy чистый.

- [Двухэтапный apply (этап 1 + этап 2)](2026_11_14_11_05_two-stage-apply-write-if-changed.md) — DONE 2026-11-14:
  Этап 1 — восстановление снимка; этап 2 — последовательный докатчик под той же
  `MutationGuard` (уточнено ревью 2026-11-15), для редакторов, мессенджеров (Discord/Vesktop/Vencord), терминалов
  (Konsole/Ghostty/Kitty/Alacritty), оболочки (Cava). Флаг `--without-sync`
  пропускает этап 2. Докатчик записывает только изменившиеся файлы; отсутствие
  всех перезаписей этапа 1 не заявляется. Историческая проверка: 134 Rust-теста, clippy чистый.

- [Исследование двухэтапного apply](2026_11_14_explore_two-stage-apply-hypotheses.md) — DONE 2026-11-14:
  все гипотезы проверены экспериментом, решение реализовано.

- [Управление зависимостями](2026_11_07_dependency-management.md) — DONE 2026-11-08:
  реестр Rust, страница с галочками/удалением и отдельными названием/категорией,
  прогресс/отмена и сохранение политик; реальные циклы KDE Store, Zed и pacman
  проверены в изоляции, GUI — в Plasma. Границы проверки перечислены в карточке.

- [Не трогать fastfetch и меню запуска](2026_11_07_preserve-fastfetch-and-launcher.md) — реализовано: fastfetch исключён, меню и его значок сохраняются, старые темы фильтруются.

- [Отчёт друга](2026_11_07_16_00_friend-report.md) — CLOSED: логов не будет, расследование закрыто пользователем без подтверждения причины зависания.

- [Обновление кнопок](2026_11_07_10_26_app-buttons-stale-after-apply.md) — DONE: пользователь подтвердил обновление оформления в живой сессии 2026-11-07 и закрыл задачу.

Другие завершённые или закрытые карточки:

- Аудит P0–P3; интеграция обоев; конструктор (сценарий создания темы из палитры
  закрыт отдельной карточкой).
- Экспорт/импорт ZIP; публикация готовых тем; регистронезависимая уникальность имён.
- `breezerc` и политика blur; оформление и зависимости; тематизация приложений.
- Расположения вне тем; автоматизация; MCP; русский перевод.
- Сценарий друга с возвратом к своей теме.
- [Копии ресурсов по ссылкам](2026_11_04_10_00_asset-copies-by-reference.md) — CLOSED решением пользователя.
- [Проверка текущих правок на чистой машине](2026_11_07_14_30_test-current-edits.md) — CLOSED **без прогона**, решением пользователя. Не переоткрывать автоматически и не считать проверкой на Plasma.

Исторические описания и незакрытые чекбоксы внутри DONE/CLOSED-карточек сохраняют
контекст первоначального плана; актуальный итог указан в статусе и результатах.
Например, остаточные проблемы `status` и синхронного diff в аудите октября
позднее закрыты Rust CLI и асинхронным GUI diff (см. roadmap).

## Backlog и спецификации

После ревью двухэтапного Apply отдельно остаются расширения:

- Chromium: нужен поддерживаемый адаптер темы; прямая запись Preferences исключена.
- Редакторы: GUI выбора источника и флаг `save --vscode-theme` из исходного плана
  не реализованы. Работают capture VS Code/Cursor и явные локальные метаданные
  кэша для остальных поддерживаемых редакторов; границы — в задаче ревью.

Идеи без отдельной активной карточки перечислены в [roadmap](../docs/ROADMAP.md).
Подсветку diff нужно сначала сверить с текущим QML: отдельного файла
`DiffDialog.qml` в дереве нет. Наличие `qtbot`-тестов контроллеров подтверждено
в `gui/qt-app/tests/test_gui.py`; старый пункт «добавить pytest-qt» устарел.
Полнота покрытия и соответствие всему ТЗ конструктора отдельно не аудировались.
