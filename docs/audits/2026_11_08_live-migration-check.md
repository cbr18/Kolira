# Проверка установленной сборки после переноса в Rust

2026-11-08. Продолжение [миграции](../../plans/2026_11_08_single-core-completion.md),
выполнено по согласию пользователя на обновление и проверку в Plasma.

## Установка

Штатный `scripts/install-theme-profile-kit.sh --restart --no-themes --no-original-theme`
обновил Rust CLI, Python/QML GUI и launcher. Перезапуск старого процесса выполнен,
автозапуск временно отключён через `KTM_INSTALL_NO_LAUNCH=1` для проверки.
Системный PySide6 6.12.0 используется без venv. Комплектные темы не синхронизировались,
установщик не применял оформление.

Бэкапы прежнего кода в `$XDG_CONFIG_HOME/kde-theme-profile/backups/`:

- `deploy-theme-profile-kit-20261108-155611-86897` — CLI;
- `deploy-kde-theme-manager-20261108-155611` — GUI, launcher и desktop entry.

## Проверка в текущей Plasma/Wayland

Запущена именно установленная копия `~/.local/share/kde-theme-manager/` с
CLI `~/.local/bin/theme-profile`. Использован настоящий Qt window, QML и
асинхронные контроллеры; `KTM_THEME_NO_APPLY` отключён, сетевой поиск отключён.

1. Главное окно успешно показано; страница темы отрисована и сохранён screenshot.
2. Снимок активной `glass` открыт как read-only, затем выбран Live.
3. В Inspector изменён один канал `window_bg`; staging не изменяет живой файл.
4. Apply записал выбранное значение и создал Undo-запись.
5. Undo вернул исходное значение.
6. Повторные изменение/Apply и Revert восстановили `kdeglobals` **байт в байт**.
7. Сессия штатно завершилась; её постоянный бэкап сохранён. Перед записью
   дополнительно сохранена приватная копия исходного файла в локальных артефактах.

Активный профиль `glass` не переключался. Полное применение другого профиля,
перезапуск plasmashell и связанные D-Bus-хуки в этом сценарии не проверялись.
Это проверка установленного приложения и живого редактирования Inspector;
она не заменяет отдельные тесты защиты применения, AUR или сетевых установок.

Локальные артефакты (gitignored):
`gui/qt-app/.pytest_cache/core-migration/install-live.log`, `live-gui.log`,
`live-gui-result.json`, `live-gui.png`, `live-kdeglobals.before` (0600).
