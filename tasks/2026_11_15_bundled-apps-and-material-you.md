# Обновление комплектных/локальных тем и установка

Статус: DONE — 2026-11-15.

## Запрос

Установить первую партию адаптеров, обновить существующие темы для новых
приложений и добавить Material You в репозиторий. Не менять личные KDE/GTK
настройки сохранённых локальных вариантов; сохранить безопасный возврат.

## Реализация

- Восемь пакетов, включая material-you: base16 + explicit targets, собственные
  Zed/Neovim, встроенные Cursor/Code с цветами, Zen colours/day-night и Firefox native day/night без подключения CSS,
  Kitty и Konsole. Pywalfox не включён автоматически.
- Material You экспортирован публично из временной копии; wallpaper проверен
  по SHA-256 с уже опубликованным Dracula. Исключены личные пути, сторонние
  библиотеки/кэши, UI CSS браузера. Внешние ресурсы остались зависимостями.
- Исправлена позиционная замена JSONC objects/arrays: повторное обновление
  больше не добавляет дубли; JSONC scalar-листья Inspector не изменены.
  Сгенерированные staging/repo файлы пересозданы из исходников без дублей.
  Все байты восьми пакетов совпадают после повторного refresh; отдельный unit PASS.
- Rust example refresh_theme_apps обслуживает только repo/staging, не live themes;
  сохраняет terminal opacity/blur и права файлов, пишет атомарно, пересчитывает hashes экспортов.
- У пользователя Firefox содержит отключённый личный CSS: комплектные темы
  не включают stylesheets. Mapped browser snapshot имеет приоритет над генерацией.
- Исправлен share.rs: экспорт контролируемых цветов/day-night из браузерного
  профиля снимка при отличающемся имени профиля на экспортирующей машине.
- Локальные восемь тем подготовлены отдельно из копий исходных пользовательских
  пакетов. KDE/GTK/обои сохраняются; обновляются app resources. Применение к
  рабочему desktop не выполняется: установщик/импорт обновляют код и пакеты.

## Проверки и доставка

- Новая regression: все восемь пакетов import → ZIP export/import с другим browser
  profile → Apply → neutral без конфига; personal fonts/keys/terminal command,
  privacy, Obsidian font size и tdata сохраняются. PASS.
- 246 Rust (включая unit примера обслуживания), 345 Qt/Python, 260 shell;
  release build, clippy -D warnings, shellcheck, qmllint, 903 finished-перевода PASS.
- Docker: восемь тем, включая Minecraft/Material You, возврат файлов/GSettings
  и цикл через GUI — ALL PASSED. Проверялись файлы/процессы, не screenshots.
  Docker snapshot предшествовал финальному исправлению JSONC и native Firefox;
  эти уточнения закрыты повторным полным Rust/Qt/shell-прогоном и живым preview.
  Артефакты: /tmp/kolira-theme-refresh-e2e-out, /tmp/kolira-theme-refresh-docker.log.
- Сверены 277 файлов KDE/GTK, обоев и связанных ресурсов: staging локальных тем
  совпадает с исходными пользовательскими вариантами побайтно.
- Восемь export.json проверены по SHA-256; абсолютные личные пути отсутствуют.
- CLI/GUI установлены scripts/install-kolira.sh --restart, GUI перезапущен.
  CLI SHA-256 и GUI theme_fields.json совпадают с исходниками.
- Восемь локальных пакетов обновлены kolira import --force с pre-import backup.
  Дополнительно исправлены три старых GTK CSS/import: theme-profile.css → kolira.css
  в локальных Frutiger-пакетах, с тем же CSS. Helper проверяет конфликт имён и
  выполняет миграцию ресурса вместе с host-ссылкой; unit идемпотентности PASS.
- После импорта: 270 файлов побайтно прежние; 3 CSS + 3 import перенесены без
  изменения стилей; один L&F layout script исключён согласно контракту оформления.
  Работающие GTK/KDE-конфиги и текущая тема desktop не применялись/не менялись.
- Установленный CLI прошёл preview всех восьми тем на реальных конфигах.
  Активный профиль остался material-you; новый Apply не запускался.

## Резервные копии

Код: ~/.config/kolira/backups/deploy-kolira-20261115-213430/ и соседний backup CLI.
Темы: ~/.config/kolira/backups/pre-import-<name>-… .
Полные пути сохранены в /tmp/kolira-theme-refresh-import.log и
/tmp/kolira-theme-refresh-gtk-import.log; отчёты и сверка —
/tmp/kolira-theme-refresh-vcvfmwea/. Push в этой задаче не выполнялся.
