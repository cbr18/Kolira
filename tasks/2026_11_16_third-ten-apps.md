# Третья партия приложений: P0 21–29 + P1 1

Статус: DONE — 2026-11-16 (исходники и комплектные пакеты; live deploy не выполнялся).

Micro; lsd/Glow; Chromium/Chrome; GIMP; Heroic; Prism; Vim;
mpv/Cliamp; VS Code/VSCodium. LibreOffice исключён пользователем: стандартное оформление, конфиги не трогаем.
Продолжаем уже начатые интеграции; новые приложения взамен пока не добавляем.

Критерии: цвета из палитры, native selectors, общий Plan/preview/MutationGuard,
возврат к отсутствующим конфигам и сохранение late edits. Не переносить аккаунты,
игровые настройки, документы/историю, плагины или произвольный Vimscript.
Новые программы не устанавливаются автоматически.

Chromium: только штатный user theme extension, без Preferences/Secure Preferences
и системных policies. Подключение/reload/сброс темы только через UI браузера;
ограничение должно быть видно, а не выдаваться за автоматический возврат клиента.

Установленные версии: Micro 2.0.15, lsd 1.2.0, Glow 3.0.0, Chromium 157,
GIMP 3.2.0rc3, Heroic 2.22.4, Prism 11.1.1, Vim 9.2.1178,
mpv 0.41.0, Cliamp 2.4.1. VSCodium/Code не установлены.

Исследованы GIMP --dump-gimprc и native
CSS, Heroic app.asar (только код приложения, не аккаунты). Основные источники:
- https://prismlauncher.org/wiki/getting-started/change-themes/
- https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/wiki/Custom-CSS-Snippets
- https://github.com/bjarneo/cliamp/blob/main/theme/theme.go
- https://github.com/GNOME/gimp/blob/master/app/gui/themes.c

Реализация и проверка завершены в границах ниже.

Учтена пользовательская правка
[Glass / Material You wallpaper accent](2026_11_16_glass-material-wallpaper-accent.md):
maintenance generator сохраняет выбор, устраняет case aliases, синхронизирует
manifest; добавлены проверки повторной генерации и всех восьми пакетов.

## Реализовано

- 12 новых registry IDs (53 всего), palette targets и native appearance adapters;
  LibreOffice исключён, новая программа вместо него пока не добавляется.
- Ресурсы всех восьми тем берутся из base16/accent; 35 явных app_targets.
- JSONC вставляет отсутствующий leaf по позиции вложенного объекта.
- Heroic сохраняет личный CSS/игры и поздние изменения вне управляемого блока;
  normal/hover text contrast >= 4.5 проверяется для кнопок всех восьми пакетов.
- Vim получает только JSON и фиксированные loaders core; чужой Vimscript не
  копируется, конфликт с личным loader останавливает preview до записи.
- mpv получает только root OSD colours; #цвета обязательно в кавычках, иначе
  нативный parser считает их комментариями. Секции профилей сохраняются.
- Chromium/Chrome: нативный frame accent без Preferences/policies/text/icon
  overrides; ручной load/reload/reset виден в actions, включая возврат.

## Проверки

- 11 регрессий third_ten: три цикла, отсутствующие ресурсы, поздние правки,
  личный CSS, loader conflicts, небезопасные manifests/CSS, LibreOffice,
  XML не добавлялся и не изменяется; 3 maintenance regression tests.
- Нативные Micro/lsd/Glow/Vim/mpv/Cliamp для всех 8 пакетов PASS в disposable HOME.
  Cliamp: PTY, фактический ANSI accent, autoplay выключен, ALSA null и собственные IPC roots.
- GTK3 parser GIMP CSS всех 8 пакетов PASS.
- Нативная загрузка Prism и компиляция theme extension Chromium для всех
  8 пакетов PASS (копии ресурсов, offscreen/headless).
- Настоящий Heroic в Xvfb: customCSS --background/--accent и color-scheme
  всех 8 тем PASS; тестовый HOME без аккаунтов/игр, через локальный CDP.
- Rust `cargo test --all-targets`: 276 PASS; release build и clippy -D warnings PASS.
- Qt/Python 345 PASS; shell installer 260 PASS; shellcheck, qmllint, diff --check PASS.
- 922 перевода, 0 unfinished.
- Docker: настоящий KWin + plasmashell, все 8 тем + возврат, затем цикл через
  контроллер GUI — ALL PASSED. Логи `/tmp/kolira-third-e2e-breeze.log`,
  `/tmp/kolira-third-e2e-out3/`; snapshot `/tmp/kolira-third-e2e-source3/`.
  В контейнере нет большинства клиентов третьей партии; их нативные smoke
  проверки перечислены отдельно. Сценарий проверяет desktop regression/возврат,
  а не визуальные игровые/браузерные сеансы.
- Первые прогоны тестовой упаковки были неуспешны: snapshot поймал ещё не
  пересчитанные hashes ручной правки тем; прямой native Chromium probe создал
  Cached Theme.pak рядом с исходным manifest (удалён, постоянный тест копирует
  ресурс во временный HOME). Финальный снимок проверяет хеши до запуска.
  Для кэша Docker использован target symlink только в тестовом git snapshot:
  CARGO_TARGET_DIR не поддерживается deployment script, штатный путь оставлен.

Границы: игровой/авторизованный Heroic/Prism workflow и визуальная проверка
Google Chrome не объявляются выполненными; неустановленные приложения
не устанавливались. Живая установленная Kolira и локальные пакеты этой партией
пока не обновлялись, Desktop Apply на рабочем столе не запускался.

Пользователь подтвердил сохранение BreezeLight/BreezeDark: генератор больше
не меняет выбранную KDE-схему; добавлена проверка двух повторных refresh для
обоих Breeze и согласованности восьми theme.json/kdeglobals. Для VSCodium
добавлена отдельная colour-only JSONC policy: отсутствующие в пакете fonts,
icons и appearance extensions не удаляются sparse-снимком.
